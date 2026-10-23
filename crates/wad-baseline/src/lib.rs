//! Baseline WAD: o mapa logístico em ponto fixo decimal de 18 casas, como no
//! Solidity (`mulWad(a, b) = a·b / 10^18`, divisão inteira por piso, como em
//! Solady e PRBMath), provado em R1CS.
//!
//! Estado (aridade 3): `[contador, x, decisão]`, com `x = X / 10^18`,
//! `0 <= X <= 10^18`. O passo é
//!
//! ```text
//! c = W - x;  q1 = floor(x·c / W);  y = floor(q1·R / W);  decisão = [y >= W/2]
//! ```
//!
//! com `W = 10^18` e `R = floor(r·W)` constante. Cada divisão é testemunhada:
//! `t = q·W + rem`, `0 <= rem < W`, `q` com largura limitada. Como todos os
//! inteiros ficam abaixo de 2^122, a igualdade vale nos inteiros, e `q` e
//! `rem` são únicos (divisão euclidiana). O invariante `0 <= x <= W` passa de
//! um fold ao seguinte porque `q1 <= W/4` e `R <= 4W` dão `y <= W`; o
//! verificador checa só `z0`.

use ff::PrimeField;
use nova_snark::{
    frontend::{num::AllocatedNum, ConstraintSystem, SynthesisError},
    traits::circuit::StepCircuit,
};
use pnf_circuit::gadgets::*;

pub const W: u128 = 1_000_000_000_000_000_000;
pub const ARITY: usize = 3;
/// `rem < W < 2^60`
const REM_BITS: u32 = 60;
/// `q1 <= W/4 < 2^58`
const Q1_BITS: u32 = 58;
/// `y <= W < 2^60`
const Y_BITS: u32 = 60;

/// `mulWad` de referência (piso).
pub fn mul_wad(a: u128, b: u128) -> u128 {
    a * b / W
}

/// Passo de referência fora do circuito.
pub fn step(x: u128, r: u128) -> u128 {
    mul_wad(mul_wad(x, W - x), r)
}

/// `floor(v·W)` exato para um f64 em [0, 4] (valor diádico do float).
pub fn to_wad(v: f64) -> u128 {
    assert!((0.0..=4.0).contains(&v));
    let bits = v.to_bits();
    let e = ((bits >> 52) & 0x7ff) as i32;
    if e == 0 {
        return 0;
    }
    let m = ((bits & ((1 << 52) - 1)) | (1 << 52)) as u128;
    let shift = e - 1075; // v = m · 2^shift
    if shift >= 0 {
        m * W << shift
    } else {
        // m·W < 2^113: cabe em u128
        (m * W) >> (-shift)
    }
}

/// `t = q·W + rem` com `rem in [0, W)` e `q in [0, 2^qbits)`; devolve `q`.
/// `shift` desloca o quociente testemunhado (só para testes de adulteração;
/// o resto é ajustado para manter `t = q·W + rem`).
fn div_wad<F: PrimeField, CS: ConstraintSystem<F>>(
    cs: &mut CS,
    t: &Expr<F>,
    tv: u128,
    qbits: u32,
    shift: i128,
) -> Expr<F> {
    let (q, rem) = {
        let _g = Category::enter("arith");
        let qv = (tv / W) as i128 + shift;
        let rv = tv as i128 - qv * W as i128;
        (alloc(cs, f_i128::<F>(qv)), alloc(cs, f_i128::<F>(rv)))
    };
    range(cs, &q, qbits);
    range(cs, &rem, REM_BITS);
    range(cs, &Expr::constant(f_u128::<F>(W - 1)).sub(&rem), REM_BITS);
    let _g = Category::enter("arith");
    assert_zero(cs, &q.scale(f_u128(W)).add(&rem).sub(t));
    q
}

#[derive(Clone, Debug)]
pub struct WadCircuit {
    /// `R = floor(r·W)`, constante do circuito
    pub r: u128,
    /// witness: `X` de entrada deste fold
    pub x: u128,
    /// Adulteração para testes: desloca os quocientes e inverte a decisão.
    pub tamper: Tamper,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Tamper {
    pub q1: i128,
    pub y: i128,
    pub flip_decision: bool,
}

impl WadCircuit {
    pub fn new(r: f64, x0: f64) -> Self {
        Self { r: to_wad(r), x: to_wad(x0), tamper: Tamper::default() }
    }

    pub fn trajectory(&self, n: usize) -> Vec<Self> {
        let mut x = self.x;
        (0..n)
            .map(|_| {
                let c = Self { x, ..self.clone() };
                x = step(x, self.r);
                c
            })
            .collect()
    }

    pub fn z0<F: PrimeField>(&self) -> Vec<F> {
        vec![F::ZERO, f_u128(self.x), F::ZERO]
    }

    /// O que o verificador checa fora do circuito.
    pub fn z0_is_valid(&self) -> bool {
        self.x <= W && self.r <= 4 * W
    }
}

impl<F: PrimeField> StepCircuit<F> for WadCircuit {
    fn arity(&self) -> usize {
        ARITY
    }

    fn synthesize<CS: ConstraintSystem<F>>(
        &self,
        cs: &mut CS,
        z: &[AllocatedNum<F>],
    ) -> Result<Vec<AllocatedNum<F>>, SynthesisError> {
        assert!(self.r <= 4 * W, "r em [0, 4]");
        let _w = WitnessOnly::enter(cs.is_witness_generator());
        let x = Expr::from_num(&z[1], f_u128(self.x));
        let c = Expr::constant(f_u128::<F>(W)).sub(&x);
        let t1 = {
            let _g = Category::enter("arith");
            product(cs, &x, &c)
        };
        let q1v = (self.x * (W - self.x) / W) as i128 + self.tamper.q1;
        let q1v = q1v.max(0) as u128;
        let q1 = div_wad(cs, &t1, self.x * (W - self.x), Q1_BITS, self.tamper.q1);
        // q1·R é linear (R constante)
        let t2 = q1.scale(f_u128(self.r));
        let y = div_wad(cs, &t2, q1v * self.r, Y_BITS, self.tamper.y);
        let yv = q1v * self.r / W;

        // decisão y >= W/2: d=1 => y - W/2 >= 0; d=0 => W/2 - 1 - y >= 0
        let dec = {
            let _g = Category::enter("decision");
            let d = bit(cs, (yv >= W / 2) != self.tamper.flip_decision);
            let delta = y.sub(&Expr::constant(f_u128::<F>(W / 2)));
            let sign = d.scale_int(2).add_int(-1);
            range_product(cs, &sign, &delta, &d.not(), Y_BITS + 1);
            d
        };

        let counter_val = z[0].get_value().unwrap_or(F::ZERO);
        let counter = Expr::from_num(&z[0], counter_val).add_int(1);
        Ok(vec![to_num(cs, &counter), to_num(cs, &y), to_num(cs, &dec)])
    }
}
