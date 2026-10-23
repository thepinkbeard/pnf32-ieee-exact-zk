//! G2 sobre o perfil diádico: passo diádico fundido + checagem local do
//! certificado de sombreamento de HYG (o mesmo de `shadow-proof`) + veredito.
//!
//! Estado (aridade 6): `[contador, X, d, A, B, veredito]`, onde `X = p_n·2^p`,
//! `d = [p_n ≥ 1/2]` e `[A, B]` é o intervalo de grade `I_n = [A/2^K, B/2^K]`.
//!
//! O fold `n` impõe:
//!
//! 1. o passo diádico (`crate::DyadicCircuit`, uma constraint de produto);
//! 2. `p_n` na grade: `X·2^(K−p)` (linear, K ≥ p; nada a checar);
//! 3. `I_n` inteiro de um lado de 1/2 (bit testemunhado `σ_n`);
//! 4. `I_{n+1}` testemunhado com `0 ≤ A' ≤ B' ≤ 2^K`;
//! 5. aninhamento `f(I_n) ⊇ I_{n+1}` (2 comparações inteiras exatas);
//! 6. `|p_n − x| ≤ ε` para todo `x ∈ I_n`;
//! 7. veredito: robusto se `σ_n` coincide com `d`.
//!
//! O verificador checa fora do circuito `z0` (`X_0`, `d_0`, `I_0` com
//! distância ≤ ε) e, no estado final, a distância de `p_N` a `I_N`.
//!
//! **Larguras.** [`Widths::Full`] usa as mesmas larguras de range check do G2
//! da PNF (`shadow-proof`), para comparação justa. [`Widths::Tight`] limita cada
//! range check pela folga **honesta**. Cada checagem continua impondo a mesma
//! desigualdade (valor ≥ 0), só com um teto menor que o witness honesto
//! respeita, então a soundness não muda:
//! - aninhamento: o construtor de HYG escolhe o extremo mais justo da grade,
//!   então a folga é menor que um degrau de `f` entre pontos vizinhos,
//!   `rn·(2^K − 2a − 1) < rn·2^K`: largura `bits(rn) + K + 1`;
//! - distância: `a − p + ε` e `p + ε − b` ficam em `[0, 2ε]`: largura `bits(2ε) + 1`;
//! - largura do intervalo `B' − A' ≤ 2ε` (pela checagem 6 do fold seguinte):
//!   largura `bits(2ε) + 1`, e `B'` não precisa de range próprio
//!   (`A' ≥ 0` e `B' − A' ≥ 0` já dão `B' ≥ 0`).

use ff::PrimeField;
use nova_snark::{
    frontend::{num::AllocatedNum, ConstraintSystem, SynthesisError},
    traits::circuit::StepCircuit,
};
use num_bigint::{BigInt, BigUint, Sign};
use num_traits::{One, Signed};
use pnf_circuit::gadgets::*;
use shadow_proof::cert::{build_grid, Certificate, Failure, Interval, Map};

use crate::{decision, f_big, step, step_full, Profile, FIELD_SAFE_BITS};

pub const SHADOW_ARITY: usize = 6;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Widths {
    Full,
    Tight,
}

fn fbi<F: PrimeField>(x: &BigInt) -> F {
    assert!(!x.is_negative());
    f_big(&x.to_biguint().unwrap())
}

fn to_int(x: &BigUint) -> BigInt {
    BigInt::from_biguint(Sign::Plus, x.clone())
}

/// Mapa exato do certificado com o mesmo `r` do perfil (`R = r·2^s`).
pub fn map_for(prof: Profile, r: &BigUint, k: u32) -> Map {
    assert!(k >= prof.p, "grade do certificado mais grossa que o estado");
    // R/2^s reduzido (o certificado usa o valor diádico exato de r)
    let mut rn = to_int(r);
    let mut rs = prof.s;
    while rs > 0 && (&rn & BigInt::one()) == BigInt::from(0) {
        rn >>= 1usize;
        rs -= 1;
    }
    Map { rn, rs, k }
}

/// Pseudo-órbita diádica `X_0..X_N` (inteiros, escala 2^p).
pub fn orbit(prof: Profile, r: &BigUint, x0: &BigUint, n: usize) -> Vec<BigUint> {
    let mut out = Vec::with_capacity(n + 1);
    let mut x = x0.clone();
    out.push(x.clone());
    for _ in 0..n {
        x = step(prof, &x, r);
        out.push(x.clone());
    }
    out
}

pub fn to_grid(prof: Profile, k: u32, x: &BigUint) -> BigInt {
    to_int(x) << (k - prof.p) as usize
}

/// Certificado HYG para a pseudo-órbita diádica.
pub fn certify(prof: Profile, map: &Map, orb: &[BigUint]) -> Result<Certificate, Failure> {
    let grid: Vec<BigInt> = orb.iter().map(|x| to_grid(prof, map.k, x)).collect();
    build_grid(map, &grid)
}

#[derive(Clone, Debug)]
pub struct DyadicShadowCircuit {
    pub prof: Profile,
    /// `R = r·2^s`
    pub r: BigUint,
    pub x: BigUint,
    pub map: Map,
    pub cur: Interval,
    pub next: Interval,
    pub side: bool,
    /// ε em unidades da grade (público)
    pub eps: BigInt,
    pub widths: Widths,
}

impl DyadicShadowCircuit {
    pub fn trajectory(
        prof: Profile,
        r: &BigUint,
        map: &Map,
        orb: &[BigUint],
        cert: &Certificate,
        eps: &BigInt,
        widths: Widths,
    ) -> Vec<Self> {
        (0..orb.len() - 1)
            .map(|n| Self {
                prof,
                r: r.clone(),
                x: orb[n].clone(),
                map: map.clone(),
                cur: cert.intervals[n].clone(),
                next: cert.intervals[n + 1].clone(),
                side: cert.sides[n],
                eps: eps.clone(),
                widths,
            })
            .collect()
    }

    pub fn z0<F: PrimeField>(&self) -> Vec<F> {
        vec![
            F::ZERO,
            f_big(&self.x),
            F::from(decision(self.prof, &self.x) as u64),
            fbi(&self.cur.a),
            fbi(&self.cur.b),
            F::ZERO,
        ]
    }

    /// Larguras (bits) usadas: (aninhamento, distância, largura do intervalo).
    pub fn widths_bits(&self) -> (u32, u32, u32) {
        let k = self.map.k;
        let rnb = self.map.rn.bits() as u32;
        match self.widths {
            Widths::Full => (2 * k + rnb.max(self.map.rs) + 2, k + 2, k + 1),
            Widths::Tight => {
                let e2 = (&self.eps * 2u32).bits() as u32 + 1;
                (rnb + k + 1, e2, e2)
            }
        }
    }
}

impl<F: PrimeField> StepCircuit<F> for DyadicShadowCircuit {
    fn arity(&self) -> usize {
        SHADOW_ARITY
    }

    fn synthesize<CS: ConstraintSystem<F>>(
        &self,
        cs: &mut CS,
        z: &[AllocatedNum<F>],
    ) -> Result<Vec<AllocatedNum<F>>, SynthesisError> {
        let _w = WitnessOnly::enter(cs.is_witness_generator());
        let Profile { p, s } = self.prof;
        let k = self.map.k;
        assert!(k >= p);
        assert!(2 * p + s + 2 <= FIELD_SAFE_BITS);
        assert!(self.r <= (BigUint::from(4u32) << s), "r em [0, 4]");

        let x = Expr::from_num(&z[1], f_big(&self.x));
        let d = Expr::from_num(&z[2], F::from(decision(self.prof, &self.x) as u64));
        let a = Expr::from_num(&z[3], fbi(&self.cur.a));
        let b = Expr::from_num(&z[4], fbi(&self.cur.b));

        // (1) passo diádico fundido (mesma relação de crate::DyadicCircuit)
        let (ybits, y) = {
            let _g = Category::enter("dyadic");
            let one = Expr::constant(f_big::<F>(&self.prof.one()));
            let rc = one.sub(&x).scale(f_big(&self.r));
            let (yv, remv) = step_full(self.prof, &self.x, &self.r);
            let ybits: Vec<Expr<F>> = (0..p + 1).map(|i| bit(cs, yv.bit(i as u64))).collect();
            let rbits: Vec<Expr<F>> = (0..p + s).map(|i| bit(cs, remv.bit(i as u64))).collect();
            let rhs = Expr::from_bits(&ybits, p + s).add(&Expr::from_bits(&rbits, 0));
            enforce(cs, &x, &rc, &rhs);
            let y = Expr::from_bits(&ybits, 0);
            (ybits, y)
        };
        let d_next = ybits[(p - 1) as usize].add(&ybits[p as usize]);

        let _g = Category::enter("shadow");
        let (w_nest, w_dist, w_iv) = self.widths_bits();
        assert!(w_nest <= FIELD_SAFE_BITS, "aninhamento estoura o corpo");

        // (2) p_n na grade: linear
        let p_grid = x.scale(f_pow2::<F>(k - p));

        // (3) lado de 1/2
        let half = Expr::constant(f_pow2::<F>(k - 1));
        let sigma = bit(cs, self.side);
        let span = a.add(&b).sub(&half.scale_int(2));
        let side_val = half.sub(&b).add(&product(cs, &sigma, &span));
        range(cs, &side_val, k);

        // (4) próximo intervalo
        let a2 = alloc(cs, fbi(&self.next.a));
        let b2 = alloc(cs, fbi(&self.next.b));
        range(cs, &a2, k + 1);
        if self.widths == Widths::Full {
            range(cs, &b2, k + 1);
        }
        range(cs, &b2.sub(&a2), w_iv);
        range(cs, &Expr::constant(f_pow2::<F>(k)).sub(&b2), k + 1);

        // (5) aninhamento f(I_n) ⊇ I_{n+1}
        let onek = Expr::constant(f_pow2::<F>(k));
        let fa = product(cs, &a, &onek.sub(&a));
        let fbv = product(cs, &b, &onek.sub(&b));
        let lo = select(cs, &sigma, &fbv, &fa);
        let hi = select(cs, &sigma, &fa, &fbv);
        let rn: F = fbi(&self.map.rn);
        let tscale = f_pow2::<F>(self.map.rs + k);
        range(cs, &a2.scale(tscale).sub(&lo.scale(rn)), w_nest);
        range(cs, &hi.scale(rn).sub(&b2.scale(tscale)), w_nest);

        // (6) distância à pseudo-órbita
        let eps = Expr::constant(fbi(&self.eps));
        range(cs, &a.sub(&p_grid).add(&eps), w_dist);
        range(cs, &p_grid.add(&eps).sub(&b), w_dist);

        // (7) veredito: 1 robusto-verdadeiro, 0 robusto-falso, 2 indeterminado
        let both = product(cs, &d, &sigma);
        let eq = Expr::one().sub(&d).sub(&sigma).add(&both.scale_int(2));
        let verdict = both.add(&eq.not().scale_int(2));
        drop(_g);

        let counter_val = z[0].get_value().unwrap_or(F::ZERO);
        let counter = Expr::from_num(&z[0], counter_val).add_int(1);
        Ok(vec![
            to_num(cs, &counter),
            to_num(cs, &y),
            to_num(cs, &d_next),
            to_num(cs, &a2),
            to_num(cs, &b2),
            to_num(cs, &verdict),
        ])
    }
}
