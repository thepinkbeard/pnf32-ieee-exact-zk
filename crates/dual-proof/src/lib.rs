//! Prova dupla L2+L3 num único passo Nova.
//!
//! Cada fold prova, sobre o mesmo par público `(r, x0)`:
//!
//! - **L2** — o passo logístico em binary32 bit-exato (PNF32,
//!   [`pnf_circuit::step::LogisticCircuit`]) e a decisão IEEE `y >= 1/2`;
//! - **L3** — um enclosure certificado `[c - R, c + R]` da trajetória em
//!   matemática exata (CBD com bound de Taylor local,
//!   [`nova_circuit::CbdLogisticStepCircuit`]) e a decisão trivalorada
//!   da CBD sobre ele;
//! - **combinação** — `RobustTrue` (1) se a decisão IEEE é verdadeira e o
//!   enclosure inteiro está em `>= 1/2`; `RobustFalse` (0) se a decisão IEEE
//!   é falsa e o enclosure inteiro está em `< 1/2`; `Indeterminate` (2) caso
//!   contrário.
//!
//! Os dois sub-circuitos são reutilizados como caixas-pretas, sem cópia: a
//! composição só compartilha o contador e acrescenta 6 constraints.
//!
//! Estado (aridade `12 + K`):
//!
//! ```text
//! [ contador, s, eb, m, z, inf, nan, dec_ieee,      // 0..8   (PNF32)
//!   limbs[K], is_one, raio, dec_cbd,                // 8..11+K (CBD)
//!   veredito ]                                      // 11+K
//! ```
//!
//! As posições de decisão de `z_i` são só saídas: nenhum fold as lê.

use ff::PrimeField;
use nova_circuit::{
    half_threshold_numerator, initial_cbd_state, CbdLogisticStepCircuit, CbdRadiusBound,
    CbdStepWitness,
};
use nova_snark::{
    frontend::{num::AllocatedNum, ConstraintSystem, SynthesisError},
    traits::circuit::StepCircuit,
};
use num_bigint::BigInt;
use num_rational::BigRational;
use pnf_circuit::{
    gadgets::CheckCS,
    step::{LogisticCircuit, STEP_ARITY},
};
use pnf_reference::{logistic_step, Format, Pnf};

/// Veredito combinado, com o mesmo código da CBD (0 falso, 1 verdadeiro, 2
/// indeterminado).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, PartialOrd, Ord)]
pub enum Verdict {
    RobustFalse = 0,
    RobustTrue = 1,
    Indeterminate = 2,
}

impl Verdict {
    /// Regra de combinação (a mesma que o circuito impõe).
    pub fn combine(ieee: bool, cbd_code: u64) -> Self {
        match (ieee, cbd_code) {
            (true, 1) => Self::RobustTrue,
            (false, 0) => Self::RobustFalse,
            _ => Self::Indeterminate,
        }
    }

    pub fn code(self) -> u64 {
        self as u64
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::RobustFalse => "robust_false",
            Self::RobustTrue => "robust_true",
            Self::Indeterminate => "indeterminate",
        }
    }
}

/// Valor exato de um f32 finito como racional diádico.
pub fn f32_to_rational(v: f32) -> BigRational {
    assert!(v.is_finite());
    let (num, exp) = f32_parts(v);
    let sign = if v.is_sign_negative() { -1 } else { 1 };
    let num = BigInt::from(sign) * BigInt::from(num);
    if exp >= 0 {
        BigRational::from_integer(num << exp as usize)
    } else {
        BigRational::new(num, BigInt::from(1) << (-exp) as usize)
    }
}

/// `|v| = num * 2^exp` com `num` ímpar (ou zero).
fn f32_parts(v: f32) -> (u64, i32) {
    let bits = v.to_bits();
    let be = ((bits >> 23) & 0xff) as i32;
    let frac = (bits & 0x7f_ffff) as u64;
    let (mut num, mut exp) = if be == 0 {
        (frac, -149)
    } else {
        (frac | 0x80_0000, be - 150)
    };
    while num != 0 && num % 2 == 0 {
        num /= 2;
        exp += 1;
    }
    (num, exp)
}

/// `r` binary32 como `r_num / r_den` exato (o `r` da CBD é o mesmo número
/// que a PNF32 usa; ver `docs/08`).
pub fn f32_ratio(r: f32) -> (u64, u64) {
    assert!(r > 0.0 && r <= 4.0, "r tem que estar em (0, 4]");
    let (num, exp) = f32_parts(r);
    if exp >= 0 {
        (num << exp, 1)
    } else {
        (num, 1_u64 << (-exp))
    }
}

/// Um fold da prova dupla: witness PNF32 + witness CBD de `K` limbs.
#[derive(Clone, Debug)]
pub struct DualCircuit<const K: usize> {
    pub pnf: LogisticCircuit,
    pub cbd: CbdLogisticStepCircuit<K>,
}

pub const PNF_ARITY: usize = STEP_ARITY;

impl<const K: usize> DualCircuit<K> {
    pub const fn arity_value() -> usize {
        PNF_ARITY + K + 4
    }

    pub fn idx_cbd() -> usize {
        PNF_ARITY
    }

    pub fn idx_verdict() -> usize {
        PNF_ARITY + K + 3
    }

    /// Primeiro fold a partir de `(r, x0)` binary32. A CBD parte de `x0`
    /// quantizado na grade `2^-16K` com raio de uma ULP se inexato.
    pub fn new(r: f32, x0: f32) -> Self {
        let (r_num, r_den) = f32_ratio(r);
        Self::with_model_r(r, x0, r_num, r_den)
    }

    /// Variante "modelo": a PNF32 usa `fl(r)` e a CBD encerra a trajetória
    /// exata com `r = r_num / r_den` declarado (p.ex. 39/10). O enclosure
    /// passa a cobrir também o erro de representação de `r`.
    pub fn with_model_r(r: f32, x0: f32, r_num: u64, r_den: u64) -> Self {
        assert!((0.0..=1.0).contains(&x0), "x0 em [0,1]");
        assert!(r_den > 0 && r_num <= 4 * r_den, "r do modelo em [0,4]");
        let (limbs, is_one, radius) = initial_cbd_state::<K>(&f32_to_rational(x0));
        let cbd = CbdLogisticStepCircuit {
            witness: cbd_witness::<K>(limbs, is_one, radius, r_num, r_den),
            r_num,
            r_den,
            radius_bound: CbdRadiusBound::LocalTaylor,
            threshold_numerator: half_threshold_numerator(K),
        };
        Self {
            pnf: LogisticCircuit::new(r, x0),
            cbd,
        }
    }

    /// Circuito do fold seguinte (witness avançado pelas referências).
    pub fn next(&self) -> Self {
        let pnf = LogisticCircuit {
            x: logistic_step(self.pnf.fmt, self.pnf.x, self.pnf.r),
            ..self.pnf.clone()
        };
        let w = &self.cbd.witness;
        let cbd = CbdLogisticStepCircuit {
            witness: cbd_witness::<K>(
                w.output_limbs,
                w.output_is_one,
                w.output_radius,
                self.cbd.r_num,
                self.cbd.r_den,
            ),
            ..self.cbd.clone()
        };
        Self { pnf, cbd }
    }

    pub fn trajectory(&self, n: usize) -> Vec<Self> {
        let mut out = Vec::with_capacity(n);
        let mut c = self.clone();
        for _ in 0..n {
            let next = c.next();
            out.push(c);
            c = next;
        }
        out
    }

    /// Estado inicial público. As posições de decisão valem 0 e não são lidas.
    pub fn z0<F: PrimeField>(&self) -> Vec<F> {
        let mut z = self.pnf.z0::<F>();
        let w = &self.cbd.witness;
        z.extend(w.input_limbs.iter().map(|&l| F::from(l)));
        z.push(F::from(w.input_is_one as u64));
        z.push(field_u128(w.input_radius));
        z.push(F::ZERO);
        z.push(F::ZERO);
        z
    }

    /// Resultado deste fold calculado fora do circuito.
    pub fn record(&self, step: usize) -> StepRecord {
        let y = logistic_step(self.pnf.fmt, self.pnf.x, self.pnf.r);
        let ieee = pnf_reference::ge(Format::BINARY32, y, Pnf::from_f32(0.5));
        let w = &self.cbd.witness;
        let center = if w.output_is_one {
            1_u128 << (16 * K)
        } else {
            w.output_limbs
                .iter()
                .fold(0_u128, |acc, &l| (acc << 16) + l as u128)
        };
        StepRecord {
            step,
            y: y.to_f32(),
            ieee_decision: ieee,
            center,
            radius: w.output_radius,
            cbd_code: w.decision_half,
            verdict: Verdict::combine(ieee, w.decision_half),
        }
    }

    /// Simula `n` passos fora do circuito.
    pub fn simulate(&self, n: usize) -> Vec<StepRecord> {
        self.trajectory(n)
            .iter()
            .enumerate()
            .map(|(i, c)| c.record(i + 1))
            .collect()
    }

    /// Constraints de um fold por parte, medidas com o CS de checagem.
    pub fn breakdown(&self) -> Breakdown {
        let z0 = self.z0::<Fq>();
        let count = |f: &dyn Fn(&mut CheckCS<Fq>, &[AllocatedNum<Fq>])| {
            let mut cs = CheckCS::<Fq>::default();
            let z: Vec<_> = z0
                .iter()
                .map(|v| AllocatedNum::alloc(&mut cs, || Ok(*v)).unwrap())
                .collect();
            let before = cs.constraints;
            f(&mut cs, &z);
            assert!(cs.is_satisfied(), "witness honesto insatisfeito");
            cs.constraints - before
        };
        let pnf = count(&|cs, z| {
            StepCircuit::<Fq>::synthesize(&self.pnf, cs, &z[..PNF_ARITY]).unwrap();
        });
        let cbd = count(&|cs, z| {
            StepCircuit::<Fq>::synthesize(&self.cbd, cs, &cbd_view::<K, Fq>(z)).unwrap();
        });
        let combination = count(&|cs, z| {
            combine(cs, &z[PNF_ARITY - 1], &z[Self::idx_verdict() - 1]).unwrap();
        });
        let total = count(&|cs, z| {
            StepCircuit::<Fq>::synthesize(self, cs, z).unwrap();
        });
        Breakdown {
            pnf,
            cbd,
            combination,
            total,
        }
    }
}

type Fq = <nova_snark::provider::PallasEngine as nova_snark::traits::Engine>::Scalar;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Breakdown {
    pub pnf: usize,
    pub cbd: usize,
    pub combination: usize,
    pub total: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StepRecord {
    /// Índice do passo (1 = após o primeiro passo).
    pub step: usize,
    /// Saída binary32 do passo.
    pub y: f32,
    pub ieee_decision: bool,
    /// Centro do enclosure, numerador na escala `2^(16K)`.
    pub center: u128,
    /// Raio do enclosure, mesma escala.
    pub radius: u128,
    pub cbd_code: u64,
    pub verdict: Verdict,
}

fn cbd_witness<const K: usize>(
    limbs: [u64; K],
    is_one: bool,
    radius: u128,
    r_num: u64,
    r_den: u64,
) -> CbdStepWitness<K> {
    CbdStepWitness::compute_with_threshold(
        limbs,
        is_one,
        radius,
        r_num,
        r_den,
        CbdRadiusBound::LocalTaylor,
        half_threshold_numerator(K),
    )
}

fn field_u128<F: PrimeField>(v: u128) -> F {
    F::from((v >> 64) as u64) * F::from(1_u64 << 32) * F::from(1_u64 << 32) + F::from(v as u64)
}

/// Visão do estado na disposição do circuito CBD: o contador é compartilhado.
fn cbd_view<const K: usize, F: PrimeField>(z: &[AllocatedNum<F>]) -> Vec<AllocatedNum<F>> {
    let mut v = vec![z[0].clone()];
    v.extend_from_slice(&z[PNF_ARITY..PNF_ARITY + K + 3]);
    v
}

/// Combinação: `ieee` é a decisão binária PNF32 e `code` o código CBD em
/// `{0,1,2}` (ambos já restritos pelos sub-circuitos). Devolve o veredito.
///
/// `code = a + 2b` com `a, b` booleanos tem decomposição única porque
/// `code != 3`; `RT = a*ieee`, `RF = (1-a-b)*(1-ieee)`,
/// `veredito = 2 - RT - 2*RF`. Seis constraints.
pub fn combine<F: PrimeField, CS: ConstraintSystem<F>>(
    cs: &mut CS,
    ieee: &AllocatedNum<F>,
    code: &AllocatedNum<F>,
) -> Result<AllocatedNum<F>, SynthesisError> {
    let code_u = || -> Result<u64, SynthesisError> {
        let v = code.get_value().ok_or(SynthesisError::AssignmentMissing)?;
        (0..3)
            .find(|&c| F::from(c) == v)
            .ok_or(SynthesisError::Unsatisfiable("código CBD fora de {0,1,2}".into()))
    };
    let flag = |cs: &mut CS, name: &'static str, f: fn(u64) -> bool| {
        let n = AllocatedNum::alloc(cs.namespace(|| name), || {
            Ok(if f(code_u()?) { F::ONE } else { F::ZERO })
        })?;
        cs.enforce(
            || format!("{name} booleano"),
            |lc| lc + n.get_variable(),
            |lc| lc + CS::one() - n.get_variable(),
            |lc| lc,
        );
        Ok::<_, SynthesisError>(n)
    };
    let a = flag(cs, "cbd_true", |c| c == 1)?;
    let b = flag(cs, "cbd_indet", |c| c == 2)?;
    cs.enforce(
        || "code = a + 2b",
        |lc| lc,
        |lc| lc,
        |lc| lc + a.get_variable() + (F::from(2), b.get_variable()) - code.get_variable(),
    );
    let rt = a.mul(cs.namespace(|| "robust_true"), ieee)?;
    let rf = AllocatedNum::alloc(cs.namespace(|| "robust_false"), || {
        let a = a.get_value().ok_or(SynthesisError::AssignmentMissing)?;
        let b = b.get_value().ok_or(SynthesisError::AssignmentMissing)?;
        let d = ieee.get_value().ok_or(SynthesisError::AssignmentMissing)?;
        Ok((F::ONE - a - b) * (F::ONE - d))
    })?;
    cs.enforce(
        || "RF = (1-a-b)(1-ieee)",
        |lc| lc + CS::one() - a.get_variable() - b.get_variable(),
        |lc| lc + CS::one() - ieee.get_variable(),
        |lc| lc + rf.get_variable(),
    );
    let verdict = AllocatedNum::alloc(cs.namespace(|| "verdict"), || {
        let rt = rt.get_value().ok_or(SynthesisError::AssignmentMissing)?;
        let rf = rf.get_value().ok_or(SynthesisError::AssignmentMissing)?;
        Ok(F::from(2) - rt - rf - rf)
    })?;
    cs.enforce(
        || "veredito = 2 - RT - 2RF",
        |lc| lc,
        |lc| lc,
        |lc| {
            lc + verdict.get_variable() + rt.get_variable() + (F::from(2), rf.get_variable())
                - (F::from(2), CS::one())
        },
    );
    Ok(verdict)
}

impl<F: PrimeField, const K: usize> StepCircuit<F> for DualCircuit<K> {
    fn arity(&self) -> usize {
        Self::arity_value()
    }

    fn synthesize<CS: ConstraintSystem<F>>(
        &self,
        cs: &mut CS,
        z: &[AllocatedNum<F>],
    ) -> Result<Vec<AllocatedNum<F>>, SynthesisError> {
        assert_eq!(z.len(), Self::arity_value());
        assert_eq!(self.pnf.steps_per_fold, 1, "prova dupla: um passo por fold");
        assert_eq!(self.pnf.fmt, Format::BINARY32);
        // L2: binary32 bit-exato; saída [contador+1, x(6), dec_ieee].
        let pnf = self
            .pnf
            .synthesize(&mut cs.namespace(|| "pnf32"), &z[..PNF_ARITY])?;
        // L3: enclosure CBD sobre o mesmo contador; o contador de saída da
        // CBD (também contador+1) é descartado.
        let cbd = self
            .cbd
            .synthesize(&mut cs.namespace(|| "cbd"), &cbd_view::<K, F>(z))?;
        let verdict = combine(
            &mut cs.namespace(|| "combinacao"),
            &pnf[PNF_ARITY - 1],
            &cbd[K + 3],
        )?;
        let mut out = pnf;
        out.extend_from_slice(&cbd[1..]);
        out.push(verdict);
        debug_assert_eq!(out.len(), Self::arity_value());
        Ok(out)
    }
}

/// Checagem do verificador sobre `z0` (fora do circuito, como na técnica T2
/// da PNF32): contador zero, `x0` PNF32 canônico, CBD canônico e o enclosure
/// inicial contém o valor exato de `x0` binary32.
pub fn check_z0<const K: usize>(r: f32, x0: f32, z0: &[Fq]) -> bool {
    let honest = DualCircuit::<K>::new(r, x0);
    if !Pnf::from_f32(x0).is_valid(Format::BINARY32) {
        return false;
    }
    let expected = honest.z0::<Fq>();
    if z0.len() != expected.len() || z0 != expected.as_slice() {
        return false;
    }
    let w = &honest.cbd.witness;
    let scale = BigRational::from_integer(BigInt::from(1_u128 << (16 * K)));
    let center = if w.input_is_one {
        BigRational::from_integer(1.into())
    } else {
        BigRational::new(
            BigInt::from(
                w.input_limbs
                    .iter()
                    .fold(0_u128, |acc, &l| (acc << 16) + l as u128),
            ),
            BigInt::from(1_u128 << (16 * K)),
        )
    };
    let radius = BigRational::from_integer(BigInt::from(w.input_radius)) / scale;
    let x = f32_to_rational(x0);
    &center - &radius <= x && x <= &center + &radius
}
