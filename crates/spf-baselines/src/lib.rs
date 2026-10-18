//! Baselines de `proof-numeric-zk` (FP32 restrito e CBD-32) com S passos
//! logísticos por fold Nova, para comparar com `LogisticCircuit` da PNF32
//! sob o mesmo `steps_per_fold`.
//!
//! Os circuitos de passo são usados como caixa-preta via path dependency
//! ([`nova_circuit`]); nada é copiado nem modificado. [`MultiStep`] encadeia
//! S instâncias do mesmo `StepCircuit`: a saída `z` de um passo é a entrada
//! do seguinte, então o estado após um fold é idêntico ao de S folds de um
//! passo (o contador interno avança S). Cada passo mantém as checagens de
//! entrada do baseline original, logo o custo é exatamente `S × raw`.

use ff::PrimeField;
use nova_circuit::{
    half_threshold_numerator, initial_cbd_state, CbdLogisticStepCircuit, CbdRadiusBound,
    CbdStepWitness, CircuitFp32, RestrictedFp32StepCircuit, RestrictedFp32StepWitness,
    HALF_BIASED_EXPONENT,
};
use nova_snark::{
    frontend::{num::AllocatedNum, ConstraintSystem, SynthesisError},
    traits::circuit::StepCircuit,
};
use num_rational::BigRational;

/// S passos de um `StepCircuit` encadeados num único fold.
#[derive(Clone, Debug)]
pub struct MultiStep<C> {
    pub steps: Vec<C>,
}

impl<F: PrimeField, C: StepCircuit<F>> StepCircuit<F> for MultiStep<C> {
    fn arity(&self) -> usize {
        self.steps[0].arity()
    }

    fn synthesize<CS: ConstraintSystem<F>>(
        &self,
        cs: &mut CS,
        z: &[AllocatedNum<F>],
    ) -> Result<Vec<AllocatedNum<F>>, SynthesisError> {
        assert!(!self.steps.is_empty(), "steps_per_fold >= 1");
        let mut z = z.to_vec();
        for (i, c) in self.steps.iter().enumerate() {
            z = c.synthesize(&mut cs.namespace(|| format!("chained step {i}")), &z)?;
        }
        Ok(z)
    }
}

/// Agrupa uma trajetória passo a passo em folds de `s` passos.
pub fn group<C: Clone>(per_step: Vec<C>, s: usize) -> Vec<MultiStep<C>> {
    assert!(s >= 1 && per_step.len() % s == 0);
    per_step
        .chunks(s)
        .map(|c| MultiStep { steps: c.to_vec() })
        .collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Repr {
    /// `--representation fp32 --k 2 --rounding nearest-even` (raw 1.721).
    Fp32,
    /// `--representation cbd --k 2 --rounding nearest-even` (CBD-32 fused,
    /// local-Taylor, raw 709).
    Cbd32,
}

impl Repr {
    pub fn label(self) -> &'static str {
        match self {
            Repr::Fp32 => "fp32",
            Repr::Cbd32 => "cbd32",
        }
    }
}

/// Parâmetros da carga, iguais a `scripts/run_baselines.sh` (r = 37/10,
/// x0 = 1/10, limiar 1/2).
#[derive(Clone, Debug)]
pub struct Workload {
    pub r_num: u64,
    pub r_den: u64,
    pub x0_num: u64,
    pub x0_den: u64,
}

impl Default for Workload {
    fn default() -> Self {
        Self {
            r_num: 37,
            r_den: 10,
            x0_num: 1,
            x0_den: 10,
        }
    }
}

impl Workload {
    pub fn r_f32(&self) -> f32 {
        self.r_num as f32 / self.r_den as f32
    }
    pub fn x0_f32(&self) -> f32 {
        self.x0_num as f32 / self.x0_den as f32
    }
    fn x0_rational(&self) -> BigRational {
        BigRational::new(self.x0_num.into(), self.x0_den.into())
    }
}

pub fn fp32_state(v: CircuitFp32, counter: u64, decision: bool) -> Vec<u64> {
    vec![
        counter,
        v.exponent as u64,
        v.significand as u64,
        v.is_zero as u64,
        decision as u64,
    ]
}

/// `n` passos FP32 (um circuito por passo) e o estado inicial.
pub fn fp32_trajectory(
    w: &Workload,
    n: usize,
) -> anyhow::Result<(Vec<RestrictedFp32StepCircuit>, Vec<u64>)> {
    let parameter_bits = w.r_f32().to_bits();
    let mut input_bits = w.x0_f32().to_bits();
    let z0 = fp32_state(CircuitFp32::from_bits(input_bits)?, 0, false);
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        let witness = RestrictedFp32StepWitness::compute_with_threshold(
            input_bits,
            parameter_bits,
            HALF_BIASED_EXPONENT,
        )?;
        input_bits = witness.output.bits();
        out.push(RestrictedFp32StepCircuit {
            witness,
            parameter_bits,
            threshold_biased_exponent: HALF_BIASED_EXPONENT,
        });
    }
    Ok((out, z0))
}

pub const CBD_K: usize = 2;

pub fn cbd_state(
    limbs: [u64; CBD_K],
    is_one: bool,
    radius: u128,
    counter: u64,
    dec: u64,
) -> Vec<u128> {
    let mut v = vec![counter as u128];
    v.extend(limbs.map(u128::from));
    v.extend([is_one as u128, radius, dec as u128]);
    v
}

/// `n` passos CBD-32 (K = 2, local-Taylor, fused) e o estado inicial.
pub fn cbd_trajectory(w: &Workload, n: usize) -> (Vec<CbdLogisticStepCircuit<CBD_K>>, Vec<u128>) {
    let (mut limbs, mut is_one, mut radius) = initial_cbd_state::<CBD_K>(&w.x0_rational());
    let z0 = cbd_state(limbs, is_one, radius, 0, 0);
    let threshold_numerator = half_threshold_numerator(CBD_K);
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        let witness = CbdStepWitness::compute_with_threshold(
            limbs,
            is_one,
            radius,
            w.r_num,
            w.r_den,
            CbdRadiusBound::LocalTaylor,
            threshold_numerator,
        );
        limbs = witness.output_limbs;
        is_one = witness.output_is_one;
        radius = witness.output_radius;
        out.push(CbdLogisticStepCircuit {
            witness,
            r_num: w.r_num,
            r_den: w.r_den,
            radius_bound: CbdRadiusBound::LocalTaylor,
            threshold_numerator,
        });
    }
    (out, z0)
}

/// Estado esperado após o último circuito da trajetória (referência fora do
/// circuito, independente da síntese).
pub fn cbd_expected(last: &CbdLogisticStepCircuit<CBD_K>, steps: u64) -> Vec<u128> {
    let w = &last.witness;
    cbd_state(
        w.output_limbs,
        w.output_is_one,
        w.output_radius,
        steps,
        w.decision_half,
    )
}

pub fn fp32_expected(last: &RestrictedFp32StepCircuit, steps: u64) -> Vec<u64> {
    fp32_state(last.witness.output, steps, last.witness.decision_half)
}

pub fn field_u128<F: PrimeField>(v: u128) -> F {
    F::from((v >> 64) as u64) * F::from(2u64).pow_vartime([64]) + F::from(v as u64)
}
