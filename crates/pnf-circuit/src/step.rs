//! StepCircuit Nova do mapa logístico em PNF32, bit-exato com binary32.
//!
//! Estado (aridade 8): `[contador, s, eb, m, z, inf, nan, decisão]`.
//! A entrada `x` NÃO é revalidada a cada fold (técnica T2): as saídas de cada
//! fold são válidas por construção e `z0` é checado pelo verificador fora do
//! circuito ([`pnf_reference::Pnf::is_valid`]). `validate_input = true`
//! liga a revalidação, para medir a ablação.
//!
//! `steps_per_fold = S` encadeia S passos (logística + decisão) num único
//! fold: a saída de um passo entra direto no seguinte, sem passar pelo estado
//! Nova. O estado após um fold é idêntico ao de S folds de um passo
//! (contador `+S`, `x` após S passos, decisão do último passo); as decisões
//! intermediárias são restritas mas não expostas. Entre passos encadeados,
//! [`PnfVar::compact`] materializa os campos que não são uma variável só.

use ff::PrimeField;
use nova_snark::{
    frontend::{num::AllocatedNum, ConstraintSystem, SynthesisError},
    traits::circuit::StepCircuit,
};
use pnf_reference::{logistic_step, one, Format, Pnf};

use crate::gadgets::*;
use crate::ops::{ge_pow2, mul, sub};
use crate::pnf::{PnfVar, ARITY};

pub const STEP_ARITY: usize = ARITY + 2;

/// `y = (x*(1-x))*r` na ordem do baseline FP32 (três arredondamentos).
pub fn logistic<F: PrimeField, CS: ConstraintSystem<F>>(
    cs: &mut CS,
    fmt: Format,
    x: &PnfVar<F>,
    r: &PnfVar<F>,
) -> PnfVar<F> {
    let one = PnfVar::constant(fmt, one(fmt));
    let c = sub(cs, fmt, &one, x, None);
    let p = mul(cs, fmt, x, &c, None);
    mul(cs, fmt, &p, r, None)
}

#[derive(Clone, Debug)]
pub struct LogisticCircuit {
    pub fmt: Format,
    pub r: Pnf,
    /// Witness: valor de entrada deste fold.
    pub x: Pnf,
    /// Decisão `y >= 2^threshold_exp` (0,5 => -1).
    pub threshold_exp: i32,
    pub validate_input: bool,
    /// `r` como variável alocada em vez de constante (ablação de dobra de constantes).
    pub r_as_variable: bool,
    /// Passos logísticos encadeados por fold (S >= 1).
    pub steps_per_fold: usize,
}

impl LogisticCircuit {
    pub fn new(r: f32, x: f32) -> Self {
        Self {
            fmt: Format::BINARY32,
            r: Pnf::from_f32(r),
            x: Pnf::from_f32(x),
            threshold_exp: -1,
            validate_input: false,
            r_as_variable: false,
            steps_per_fold: 1,
        }
    }

    /// Circuitos de `n` folds a partir de `x0` (witness pela referência);
    /// cada fold avança `steps_per_fold` passos.
    pub fn trajectory(&self, n: usize) -> Vec<Self> {
        let mut x = self.x;
        (0..n)
            .map(|_| {
                let c = Self { x, ..self.clone() };
                for _ in 0..self.steps_per_fold {
                    x = logistic_step(self.fmt, x, self.r);
                }
                c
            })
            .collect()
    }

    pub fn z0<F: PrimeField>(&self) -> Vec<F> {
        let v = PnfVar::<F>::constant(self.fmt, self.x);
        let mut z = vec![F::ZERO];
        z.extend([v.s, v.eb, v.m, v.z, v.inf, v.nan].map(|e| e.val));
        z.push(F::ZERO);
        z
    }
}

impl<F: PrimeField> StepCircuit<F> for LogisticCircuit {
    fn arity(&self) -> usize {
        STEP_ARITY
    }

    fn synthesize<CS: ConstraintSystem<F>>(
        &self,
        cs: &mut CS,
        z: &[AllocatedNum<F>],
    ) -> Result<Vec<AllocatedNum<F>>, SynthesisError> {
        assert!(self.steps_per_fold >= 1, "steps_per_fold >= 1");
        let _w = WitnessOnly::enter(cs.is_witness_generator());
        let fmt = self.fmt;
        let mut x = PnfVar::from_nums(&z[1..1 + ARITY], fmt, self.x);
        if self.validate_input {
            x.validate(cs, fmt);
        }
        let r = if self.r_as_variable {
            let r = PnfVar::alloc_unchecked(cs, fmt, self.r);
            r.validate(cs, fmt);
            r
        } else {
            PnfVar::constant(fmt, self.r)
        };
        let mut dec = Expr::zero();
        for i in 0..self.steps_per_fold {
            x = logistic(cs, fmt, &x, &r);
            dec = ge_pow2(cs, fmt, &x, self.threshold_exp, None);
            // Sem isto, a combinação linear de `nan` cresce 6 termos por passo
            // encadeado (síntese quadrática em S, linhas densas; log 0006).
            if i + 1 < self.steps_per_fold {
                x = x.compact(cs);
            }
        }
        let counter_val = z[0].get_value().unwrap_or(F::ZERO);
        let counter = Expr::from_num(&z[0], counter_val).add_int(self.steps_per_fold as i128);
        let mut out = vec![to_num(cs, &counter)];
        out.extend(x.to_nums(cs));
        out.push(to_num(cs, &dec));
        Ok(out)
    }
}
