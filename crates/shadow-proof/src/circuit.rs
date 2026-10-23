//! Fold Nova: passo PNF bit-exato + checagem local do certificado de
//! sombreamento de HYG + veredito da decisão. Formato binary32 (grade 2^-48)
//! ou binary64 (grade 2^-96).
//!
//! Estado (aridade 10): `[contador, s, eb, m, z, inf, nan, A, B, veredito]`,
//! onde `(s..nan)` é `p_n` em PNF, e `[A, B]` é o intervalo de
//! grade `I_n = [A/2^K, B/2^K]` que contém a órbita verdadeira.
//!
//! O fold `n` impõe:
//!
//! 1. `p_{n+1} = fl(fl(p_n·fl(1−p_n))·r)` (PNF32, bit-exato);
//! 2. `p_n` finito positivo e na grade (`p_n ≥ 2^-(K-p+1)`: 2^-25 em binary32, 2^-44 em binary64);
//! 3. `I_n` inteiro de um lado de 1/2 (bit testemunhado `σ_n`);
//! 4. `I_{n+1}` testemunhado, com `0 ≤ A' ≤ B' ≤ 2^K`;
//! 5. aninhamento `f(I_n) ⊇ I_{n+1}` (2 comparações inteiras exatas, escolhidas por `σ_n`);
//! 6. `|p_n − x| ≤ ε` para todo `x ∈ I_n` (ε público, em unidades da grade);
//! 7. veredito: `robusto` se o lado certificado `σ_n` coincide com a decisão IEEE `p_n ≥ 1/2`.
//!
//! O verificador checa fora do circuito `z0` (o `x0` público e `I_0` com
//! distância ≤ ε) e, no estado final, a distância de `p_N` a `I_N`.

use ff::PrimeField;
use nova_snark::{
    frontend::{num::AllocatedNum, ConstraintSystem, SynthesisError},
    traits::circuit::StepCircuit,
};
use num_bigint::BigInt;
use num_traits::ToPrimitive;
use pnf_circuit::{
    gadgets::*,
    ops::{ge_pow2, FIELD_SAFE_BITS},
    pnf::{PnfVar, ARITY},
    step::logistic,
};
use pnf_reference::{Format, Pnf};

use crate::cert::{Certificate, Interval, Map};

pub const SHADOW_ARITY: usize = 1 + ARITY + 3;
const B32: Format = Format::BINARY32;
/// bits fracionários da grade para binary32
pub const K: u32 = 48;
/// bits fracionários da grade para binary64
pub const K64: u32 = 96;

pub fn grid_bits_for(fmt: Format) -> u32 {
    if fmt == B32 {
        K
    } else {
        assert_eq!(fmt, Format::BINARY64);
        K64
    }
}

fn pnf_of(fmt: Format, x: f64) -> Pnf {
    if fmt == B32 {
        Pnf::from_f32(x as f32)
    } else {
        Pnf::from_f64(x)
    }
}

fn fb<F: PrimeField>(x: &BigInt) -> F {
    f_u128(x.to_u128().expect("valor de grade cabe em u128"))
}

#[derive(Clone, Debug)]
pub struct ShadowCircuit {
    pub fmt: Format,
    pub map: Map,
    pub r: Pnf,
    pub p: Pnf,
    pub cur: Interval,
    pub next: Interval,
    pub side: bool,
    /// ε em unidades da grade (público)
    pub eps: BigInt,
}

impl ShadowCircuit {
    /// Circuitos dos folds `0..N` a partir de uma pseudo-órbita binary32 e
    /// do seu certificado.
    pub fn trajectory(map: &Map, r: f32, orbit: &[f64], cert: &Certificate, eps: &BigInt) -> Vec<Self> {
        Self::trajectory_fmt(B32, map, r as f64, orbit, cert, eps)
    }

    /// Como [`Self::trajectory`], no formato dado (binary32 ou binary64).
    pub fn trajectory_fmt(
        fmt: Format,
        map: &Map,
        r: f64,
        orbit: &[f64],
        cert: &Certificate,
        eps: &BigInt,
    ) -> Vec<Self> {
        (0..orbit.len() - 1)
            .map(|n| Self {
                fmt,
                map: map.clone(),
                r: pnf_of(fmt, r),
                p: pnf_of(fmt, orbit[n]),
                cur: cert.intervals[n].clone(),
                next: cert.intervals[n + 1].clone(),
                side: cert.sides[n],
                eps: eps.clone(),
            })
            .collect()
    }

    pub fn z0<F: PrimeField>(&self) -> Vec<F> {
        let v = PnfVar::<F>::constant(self.fmt, self.p);
        let mut z = vec![F::ZERO];
        z.extend([v.s, v.eb, v.m, v.z, v.inf, v.nan].map(|e| e.val));
        z.push(fb(&self.cur.a));
        z.push(fb(&self.cur.b));
        z.push(F::ZERO);
        z
    }

    /// Estado esperado depois do fold, para conferência fora do circuito.
    pub fn expected_next<F: PrimeField>(&self, counter: u64, p_next: Pnf, verdict: u64) -> Vec<F> {
        let v = PnfVar::<F>::constant(self.fmt, p_next);
        let mut z = vec![F::from(counter)];
        z.extend([v.s, v.eb, v.m, v.z, v.inf, v.nan].map(|e| e.val));
        z.push(fb(&self.next.a));
        z.push(fb(&self.next.b));
        z.push(F::from(verdict));
        z
    }
}

impl<F: PrimeField> StepCircuit<F> for ShadowCircuit {
    fn arity(&self) -> usize {
        SHADOW_ARITY
    }

    fn synthesize<CS: ConstraintSystem<F>>(
        &self,
        cs: &mut CS,
        z: &[AllocatedNum<F>],
    ) -> Result<Vec<AllocatedNum<F>>, SynthesisError> {
        let _w = WitnessOnly::enter(cs.is_witness_generator());
        let fmt = self.fmt;
        let k = self.map.k;
        assert_eq!(k, grid_bits_for(fmt));
        let label = if fmt == B32 { "pnf32" } else { "pnf64" };
        let x = PnfVar::from_nums(&z[1..1 + ARITY], fmt, self.p);
        let a = Expr::from_num(&z[1 + ARITY], fb(&self.cur.a));
        let b = Expr::from_num(&z[2 + ARITY], fb(&self.cur.b));

        // (1) passo IEEE bit-exato
        let y = {
            let _g = Category::enter(label);
            let r = PnfVar::constant(fmt, self.r);
            logistic(cs, fmt, &x, &r)
        };
        let dec = {
            let _g = Category::enter(label);
            ge_pow2(cs, fmt, &x, -1, None)
        };

        let _g = Category::enter("shadow");
        // (2) p_n positivo, normal, na grade: P = m · 2^(eb + K − off), eb ≥ off − K,
        // com off = (p−1) − emin (149 em binary32, 1074 em binary64). Para
        // p_n < 1, t = eb − (off − K) ≤ K − p.
        for flag in [&x.s, &x.z, &x.inf, &x.nan] {
            assert_zero(cs, flag);
        }
        let off = (fmt.p as i128 - 1) - fmt.emin() as i128;
        let t = x.eb.add_int(-(off - k as i128));
        let tbits = range(cs, &t, bitlen((k - fmt.p) as u128));
        let pow = pow2(cs, &tbits);
        let p_grid = product(cs, &x.m, &pow);

        // (3) lado de 1/2
        let half = Expr::constant(f_pow2::<F>(k - 1));
        let sigma = bit(cs, self.side);
        let span = a.add(&b).sub(&half.scale_int(2));
        let side_val = half.sub(&b).add(&product(cs, &sigma, &span));
        range(cs, &side_val, k);

        // (4) próximo intervalo
        let a2 = alloc(cs, fb(&self.next.a));
        let b2 = alloc(cs, fb(&self.next.b));
        range(cs, &a2, k + 1);
        range(cs, &b2, k + 1);
        range(cs, &b2.sub(&a2), k + 1);
        range(cs, &Expr::constant(f_pow2::<F>(k)).sub(&b2), k + 1);

        // (5) aninhamento f(I_n) ⊇ I_{n+1}
        let one = Expr::constant(f_pow2::<F>(k));
        let fa = product(cs, &a, &one.sub(&a));
        let fbv = product(cs, &b, &one.sub(&b));
        let lo = select(cs, &sigma, &fbv, &fa); // imagem que deve ficar ≤ A'
        let hi = select(cs, &sigma, &fa, &fbv); // imagem que deve ficar ≥ B'
        let rn: F = fb(&self.map.rn);
        let tscale = f_pow2::<F>(self.map.rs + k);
        // Os dois lados ficam abaixo de 2^(2K+p+1) se rn < 2^p e rs ≤ p
        // (r em [1, 4) no formato); em binary32 isto é o 2K+26 original.
        assert!(self.map.rn.bits() <= fmt.p as u64 && self.map.rs <= fmt.p, "r fora de [1, 4)");
        let wide = 2 * k + fmt.p + 2;
        assert!(wide as i128 <= FIELD_SAFE_BITS, "aninhamento estoura o corpo");
        range(cs, &a2.scale(tscale).sub(&lo.scale(rn)), wide);
        range(cs, &hi.scale(rn).sub(&b2.scale(tscale)), wide);

        // (6) distância à pseudo-órbita
        let eps = Expr::constant(fb(&self.eps));
        range(cs, &a.sub(&p_grid).add(&eps), k + 2);
        range(cs, &p_grid.add(&eps).sub(&b), k + 2);

        // (7) veredito: 1 robusto-verdadeiro, 0 robusto-falso, 2 indeterminado
        let both = product(cs, &dec, &sigma);
        let eq = Expr::one().sub(&dec).sub(&sigma).add(&both.scale_int(2));
        let verdict = both.add(&eq.not().scale_int(2));

        drop(_g);
        let counter_val = z[0].get_value().unwrap_or(F::ZERO);
        let counter = Expr::from_num(&z[0], counter_val).add_int(1);
        let mut out = vec![to_num(cs, &counter)];
        out.extend(y.to_nums(cs));
        out.push(to_num(cs, &a2));
        out.push(to_num(cs, &b2));
        out.push(to_num(cs, &verdict));
        Ok(out)
    }
}
