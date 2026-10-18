//! `MultiStep` sobre os baselines: um fold de S passos dá o mesmo estado que
//! S folds de um passo, e custa exatamente `S × raw`.

use nova_snark::{
    frontend::{num::AllocatedNum, test_cs::TestConstraintSystem, ConstraintSystem},
    nova::{PublicParams, RecursiveSNARK},
    provider::{ipa_pc::EvaluationEngine, PallasEngine, VestaEngine},
    spartan::snark::RelaxedR1CSSNARK,
    traits::{circuit::StepCircuit, snark::RelaxedR1CSSNARKTrait, Engine},
};
use spf_baselines::*;

type E1 = PallasEngine;
type E2 = VestaEngine;
type S1 = RelaxedR1CSSNARK<E1, EvaluationEngine<E1>>;
type S2 = RelaxedR1CSSNARK<E2, EvaluationEngine<E2>>;
type Fq = <E1 as Engine>::Scalar;

fn fold<C: StepCircuit<Fq>>(c: &C, z: &[Fq]) -> (Vec<Fq>, usize) {
    let mut cs = TestConstraintSystem::<Fq>::new();
    let zs: Vec<AllocatedNum<Fq>> = z
        .iter()
        .enumerate()
        .map(|(i, v)| AllocatedNum::alloc(cs.namespace(|| format!("z{i}")), || Ok(*v)).unwrap())
        .collect();
    let out = c.synthesize(&mut cs, &zs).unwrap();
    assert!(cs.is_satisfied(), "{:?}", cs.which_is_unsatisfied());
    (
        out.iter().map(|n| n.get_value().unwrap()).collect(),
        cs.num_constraints(),
    )
}

fn chain<C: StepCircuit<Fq>>(cs: &[C], z0: &[Fq]) -> Vec<Fq> {
    cs.iter().fold(z0.to_vec(), |z, c| fold(c, &z).0)
}

fn check<C: StepCircuit<Fq> + Clone>(per_step: Vec<C>, z0: Vec<Fq>, expect: Vec<Fq>) {
    let n = per_step.len();
    let raw = fold(&per_step[0], &z0).1;
    assert_eq!(chain(&per_step, &z0), expect);
    for s in [2usize, 4, 8] {
        let g = group(per_step.clone(), s);
        assert_eq!(g.len(), n / s);
        assert_eq!(chain(&g, &z0), expect, "S={s}");
        assert_eq!(fold(&g[0], &z0).1, s * raw, "S={s}");
    }
}

#[test]
fn fp32_chained_equals_separate() {
    let w = Workload::default();
    let (steps, z0) = fp32_trajectory(&w, 16).unwrap();
    let expect = fp32_expected(steps.last().unwrap(), 16);
    let mut native = w.x0_f32();
    for _ in 0..16 {
        native = (native * (1.0 - native)) * w.r_f32();
    }
    assert_eq!(
        steps.last().unwrap().witness.output.bits(),
        native.to_bits()
    );
    let f = |v: Vec<u64>| v.into_iter().map(Fq::from).collect::<Vec<_>>();
    check(steps, f(z0), f(expect));
}

#[test]
fn cbd32_chained_equals_separate() {
    let (steps, z0) = cbd_trajectory(&Workload::default(), 16);
    let expect = cbd_expected(steps.last().unwrap(), 16);
    let f = |v: Vec<u128>| v.into_iter().map(field_u128::<Fq>).collect::<Vec<_>>();
    check(steps, f(z0), f(expect));
}

#[test]
fn nova_chained_matches_unchained() {
    let (steps, z0) = cbd_trajectory(&Workload::default(), 8);
    let z0: Vec<Fq> = z0.into_iter().map(field_u128::<Fq>).collect();
    let prove = |s: usize| {
        let circuits = group(steps.clone(), s);
        let pp = PublicParams::<E1, E2, _>::setup(&circuits[0], &*S1::ck_floor(), &*S2::ck_floor())
            .unwrap();
        let mut rs = RecursiveSNARK::new(&pp, &circuits[0], &z0).unwrap();
        for c in &circuits {
            rs.prove_step(&pp, c).unwrap();
        }
        rs.verify(&pp, circuits.len(), &z0).unwrap()
    };
    assert_eq!(prove(4), prove(1));
}
