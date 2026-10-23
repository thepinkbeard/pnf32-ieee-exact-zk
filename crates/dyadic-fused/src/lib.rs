//! Ponto fixo diádico fundido: o mapa logístico com escala `2^-p` e **um**
//! arredondamento (piso) por passo, provado em R1CS.
//!
//! Perfil `(p, s)`: estado `X = x·2^p` com `0 <= X <= 2^p`; parâmetro
//! constante `R = r·2^s` (com `s = 22`, `fl32(r)` em [2, 4] é exato). O passo é
//!
//! ```text
//! t = X · (2^p − X) · R;   Y = floor(t / 2^(p+s));   decisão = [Y >= 2^(p−1)]
//! ```
//!
//! No circuito, o passo inteiro é **uma** constraint de produto,
//! `X · (R·(2^p − X)) = Y·2^(p+s) + rem`, cujo lado direito é a recomposição
//! dos bits de `Y` (p+1 bits) e de `rem` (p+s bits). O produto por `R` é
//! linear (constante). A decisão é `bit_{p−1}(Y) + bit_p(Y)`: linear, sem
//! constraint. Custo por passo: `(p+1) + (p+s)` booleanidades, 1 produto e
//! as saídas.
//!
//! **Soundness.** Por indução, `0 <= X <= 2^p` (o verificador checa `z0`).
//! Então `t <= 2^(2p)/4 · 4·2^s = 2^(2p+s)` e o lado direito é
//! `< 2^(p+1)·2^(p+s) + 2^(p+s) < 2^(2p+s+2)`. Com `2p+s+2 <= 253`, os dois
//! lados ficam abaixo do módulo de Pallas (> 2^253), a igualdade vale nos
//! inteiros, e a divisão euclidiana com `0 <= rem < 2^(p+s)` é única: `Y` é o
//! piso honesto, logo `Y <= 2^p` (o invariante passa ao fold seguinte) e
//! `bit_{p−1}` e `bit_p` não podem ser 1 ao mesmo tempo (decisão booleana).
//! O provador não tem nenhum grau de liberdade.

use ff::PrimeField;
use nova_snark::{
    frontend::{num::AllocatedNum, ConstraintSystem, SynthesisError},
    traits::circuit::StepCircuit,
};
use num_bigint::BigUint;
use num_traits::{One, Zero};
use pnf_circuit::gadgets::*;

pub const ARITY: usize = 3;
/// Bits fracionários de `R`: `fl32(r)` com r em [2, 4) tem ulp 2^-22.
pub const S_DEFAULT: u32 = 22;
/// Largura máxima que cabe no corpo de Pallas sem wrap (ver soundness).
pub const FIELD_SAFE_BITS: u32 = 253;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Profile {
    pub p: u32,
    pub s: u32,
}

impl Profile {
    pub fn new(p: u32) -> Self {
        Self::with_s(p, S_DEFAULT)
    }
    pub fn with_s(p: u32, s: u32) -> Self {
        assert!(p >= 2, "p >= 2");
        assert!(2 * p + s + 2 <= FIELD_SAFE_BITS, "perfil p={p}, s={s} não cabe no corpo");
        Self { p, s }
    }
    pub fn one(&self) -> BigUint {
        BigUint::one() << self.p
    }
    pub fn half(&self) -> BigUint {
        BigUint::one() << (self.p - 1)
    }
}

/// `floor(v · 2^bits)` exato para um f64 finito e não negativo.
pub fn to_fixed(v: f64, bits: u32) -> BigUint {
    assert!(v.is_finite() && v >= 0.0);
    if v == 0.0 {
        return BigUint::zero();
    }
    let raw = v.to_bits();
    let e = ((raw >> 52) & 0x7ff) as i64;
    let (m, shift) = if e == 0 {
        (raw & ((1 << 52) - 1), -1074i64)
    } else {
        ((raw & ((1 << 52) - 1)) | (1 << 52), e - 1075)
    };
    let total = shift + bits as i64;
    let m = BigUint::from(m);
    if total >= 0 {
        m << total as u64
    } else {
        m >> (-total) as u64
    }
}

/// `true` se `v·2^bits` é inteiro (a conversão é exata).
pub fn is_exact(v: f64, bits: u32) -> bool {
    let f = to_fixed(v, bits);
    // reconverte comparando com a metade superior: exato sse floor == ceil
    let up = to_fixed_ceil(v, bits);
    f == up
}

fn to_fixed_ceil(v: f64, bits: u32) -> BigUint {
    let raw = v.to_bits();
    let e = ((raw >> 52) & 0x7ff) as i64;
    if v == 0.0 {
        return BigUint::zero();
    }
    let (m, shift) = if e == 0 {
        (raw & ((1 << 52) - 1), -1074i64)
    } else {
        ((raw & ((1 << 52) - 1)) | (1 << 52), e - 1075)
    };
    let total = shift + bits as i64;
    let m = BigUint::from(m);
    if total >= 0 {
        m << total as u64
    } else {
        let k = (-total) as u64;
        let q = &m >> k;
        if (&q << k) == m {
            q
        } else {
            q + 1u32
        }
    }
}

/// Passo de referência fora do circuito: `(Y, rem)`.
pub fn step_full(prof: Profile, x: &BigUint, r: &BigUint) -> (BigUint, BigUint) {
    let t = x * (prof.one() - x) * r;
    let sh = (prof.p + prof.s) as u64;
    let y = &t >> sh;
    let rem = &t - (&y << sh);
    (y, rem)
}

pub fn step(prof: Profile, x: &BigUint, r: &BigUint) -> BigUint {
    step_full(prof, x, r).0
}

pub fn decision(prof: Profile, x: &BigUint) -> bool {
    *x >= prof.half()
}

pub fn f_big<F: PrimeField>(v: &BigUint) -> F {
    let base = F::from(1u64 << 32).square();
    v.to_u64_digits().iter().rev().fold(F::ZERO, |acc, d| acc * base + F::from(*d))
}

#[derive(Clone, Debug)]
pub struct DyadicCircuit {
    pub prof: Profile,
    /// `R = r·2^s`, constante do circuito
    pub r: BigUint,
    /// witness: `X` de entrada deste fold
    pub x: BigUint,
    /// Testes: `(Y, rem)` arbitrários no lugar do witness honesto. Os bits
    /// alocados são os `p+1` e `p+s` bits baixos desses valores.
    pub witness_override: Option<(BigUint, BigUint)>,
}

impl DyadicCircuit {
    pub fn new(prof: Profile, r: f64, x0: f64) -> Self {
        Self { prof, r: to_fixed(r, prof.s), x: to_fixed(x0, prof.p), witness_override: None }
    }

    pub fn trajectory(&self, n: usize) -> Vec<Self> {
        let mut x = self.x.clone();
        (0..n)
            .map(|_| {
                let c = Self { x: x.clone(), ..self.clone() };
                x = step(self.prof, &x, &self.r);
                c
            })
            .collect()
    }

    pub fn z0<F: PrimeField>(&self) -> Vec<F> {
        vec![F::ZERO, f_big(&self.x), F::ZERO]
    }

    /// O que o verificador checa fora do circuito.
    pub fn z0_is_valid(&self) -> bool {
        self.x <= self.prof.one() && self.r <= (BigUint::from(4u32) << self.prof.s)
    }
}

fn bits_of<F: PrimeField, CS: ConstraintSystem<F>>(cs: &mut CS, v: &BigUint, n: u32) -> Vec<Expr<F>> {
    let _g = Category::enter("range");
    (0..n).map(|i| bit(cs, v.bit(i as u64))).collect()
}

impl<F: PrimeField> StepCircuit<F> for DyadicCircuit {
    fn arity(&self) -> usize {
        ARITY
    }

    fn synthesize<CS: ConstraintSystem<F>>(
        &self,
        cs: &mut CS,
        z: &[AllocatedNum<F>],
    ) -> Result<Vec<AllocatedNum<F>>, SynthesisError> {
        let Profile { p, s } = self.prof;
        assert!(2 * p + s + 2 <= FIELD_SAFE_BITS);
        assert!(self.r <= (BigUint::from(4u32) << s), "r em [0, 4]");
        let _w = WitnessOnly::enter(cs.is_witness_generator());

        let x = Expr::from_num(&z[1], f_big(&self.x));
        let one = Expr::constant(f_big::<F>(&self.prof.one()));
        // R·(2^p − X): linear (R constante)
        let rc = one.sub(&x).scale(f_big(&self.r));

        let (yv, remv) = match &self.witness_override {
            Some(w) => w.clone(),
            None => step_full(self.prof, &self.x, &self.r),
        };
        let ybits = bits_of(cs, &yv, p + 1);
        let rbits = bits_of(cs, &remv, p + s);
        let y = Expr::from_bits(&ybits, 0);
        let rhs = Expr::from_bits(&ybits, p + s).add(&Expr::from_bits(&rbits, 0));
        {
            let _g = Category::enter("arith");
            // X · R(2^p − X) = Y·2^(p+s) + rem
            enforce(cs, &x, &rc, &rhs);
        }
        // decisão: Y >= 2^(p−1)  <=>  bit_{p−1} + bit_p (exclusivos por Y <= 2^p)
        let dec = ybits[(p - 1) as usize].add(&ybits[p as usize]);

        let counter_val = z[0].get_value().unwrap_or(F::ZERO);
        let counter = Expr::from_num(&z[0], counter_val).add_int(1);
        Ok(vec![to_num(cs, &counter), to_num(cs, &y), to_num(cs, &dec)])
    }
}
