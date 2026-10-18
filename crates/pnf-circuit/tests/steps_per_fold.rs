//! `steps_per_fold`: um fold com S passos encadeados produz exatamente o
//! estado de S folds de um passo (contador, `x` bit a bit, decisão), e o
//! custo cresce linearmente em S.

use nova_snark::{
    frontend::{
        num::AllocatedNum, test_cs::TestConstraintSystem, ConstraintSystem, Index,
        LinearCombination, SynthesisError, Variable,
    },
    nova::{PublicParams, RecursiveSNARK},
    provider::{ipa_pc::EvaluationEngine, PallasEngine, VestaEngine},
    spartan::snark::RelaxedR1CSSNARK,
    traits::{circuit::StepCircuit, snark::RelaxedR1CSSNARKTrait, Engine},
};
use pnf_circuit::{gadgets::CheckCS, step::LogisticCircuit};

type E1 = PallasEngine;
type E2 = VestaEngine;
type S1 = RelaxedR1CSSNARK<E1, EvaluationEngine<E1>>;
type S2 = RelaxedR1CSSNARK<E2, EvaluationEngine<E2>>;
type Fq = <E1 as Engine>::Scalar;

/// Sintetiza um fold sobre o estado `z` e devolve (saída, constraints).
fn fold(c: &LogisticCircuit, z: &[Fq]) -> (Vec<Fq>, usize) {
    let mut cs = CheckCS::<Fq>::default();
    let zs: Vec<AllocatedNum<Fq>> = z
        .iter()
        .map(|v| AllocatedNum::alloc(&mut cs, || Ok(*v)).unwrap())
        .collect();
    let out = StepCircuit::<Fq>::synthesize(c, &mut cs, &zs).unwrap();
    assert!(cs.is_satisfied(), "witness honesto insatisfeito");
    (
        out.iter().map(|n| n.get_value().unwrap()).collect(),
        cs.constraints,
    )
}

/// Encadeia `folds` folds de `c` (witness avançado pela referência).
fn run(c: &LogisticCircuit, folds: usize) -> Vec<Fq> {
    let mut z = c.z0::<Fq>();
    for f in c.trajectory(folds) {
        z = fold(&f, &z).0;
    }
    z
}

#[test]
fn chained_fold_equals_separate_folds() {
    // Entradas que percorrem caminhos distintos: normal, r que diverge para
    // inf/NaN, subnormal, zero, -0, inf e NaN.
    let cases: [(f32, f32); 8] = [
        (3.7, 0.1),
        (3.9, 0.3),
        (5.0, 0.4),
        (3.7, f32::from_bits(1)),
        (3.7, 0.0),
        (3.7, -0.0),
        (3.7, f32::INFINITY),
        (3.7, f32::NAN),
    ];
    for (r, x0) in cases {
        for s in [1usize, 2, 4, 8] {
            let one = LogisticCircuit::new(r, x0);
            let many = LogisticCircuit {
                steps_per_fold: s,
                ..one.clone()
            };
            assert_eq!(run(&many, 3), run(&one, 3 * s), "r={r} x0={x0} S={s}");
        }
    }
}

#[test]
fn final_state_matches_native_f32() {
    let (r, x0, s, folds) = (3.9_f32, 0.1_f32, 16, 20);
    let c = LogisticCircuit {
        steps_per_fold: s,
        ..LogisticCircuit::new(r, x0)
    };
    let z = run(&c, folds);
    let mut native = x0;
    for _ in 0..s * folds {
        native = (native * (1.0 - native)) * r;
    }
    let expect = LogisticCircuit::new(r, native).z0::<Fq>();
    assert_eq!(z[0], Fq::from((s * folds) as u64));
    assert_eq!(z[1..7], expect[1..7]);
    assert_eq!(z[7] == Fq::from(1), native >= 0.5);
}

#[test]
fn constraints_are_linear_in_steps_per_fold() {
    let base = LogisticCircuit::new(3.7, 0.1);
    let count = |s: usize| {
        fold(
            &LogisticCircuit {
                steps_per_fold: s,
                ..base.clone()
            },
            &base.z0::<Fq>(),
        )
        .1
    };
    let (c1, c2) = (count(1), count(2));
    for s in [4usize, 16, 64] {
        assert_eq!(count(s), c1 + (s - 1) * (c2 - c1), "S={s}");
    }
}

#[test]
fn test_constraint_system_agrees() {
    let c = LogisticCircuit {
        steps_per_fold: 4,
        ..LogisticCircuit::new(3.7, 0.1)
    };
    let mut t = TestConstraintSystem::<Fq>::new();
    let zs: Vec<AllocatedNum<Fq>> = c
        .z0::<Fq>()
        .into_iter()
        .enumerate()
        .map(|(i, v)| AllocatedNum::alloc(t.namespace(|| format!("z{i}")), || Ok(v)).unwrap())
        .collect();
    let out = StepCircuit::<Fq>::synthesize(&c, &mut t, &zs).unwrap();
    assert!(t.is_satisfied(), "{:?}", t.which_is_unsatisfied());
    let got: Vec<Fq> = out.iter().map(|n| n.get_value().unwrap()).collect();
    assert_eq!(got, fold(&c, &c.z0::<Fq>()).0);
}

#[test]
fn nova_chained_matches_unchained() {
    let prove = |s: usize, folds: usize| {
        let c = LogisticCircuit {
            steps_per_fold: s,
            ..LogisticCircuit::new(3.7, 0.1)
        };
        let circuits = c.trajectory(folds);
        let z0 = c.z0::<Fq>();
        let pp = PublicParams::<E1, E2, LogisticCircuit>::setup(
            &circuits[0],
            &*S1::ck_floor(),
            &*S2::ck_floor(),
        )
        .unwrap();
        let mut rs = RecursiveSNARK::new(&pp, &circuits[0], &z0).unwrap();
        for f in &circuits {
            rs.prove_step(&pp, f).unwrap();
        }
        rs.verify(&pp, folds, &z0).unwrap()
    };
    assert_eq!(prove(4, 2), prove(1, 8));
}

/// Conta termos não nulos das linhas A, B, C (densidade das matrizes).
#[derive(Default)]
struct TermCountCS {
    aux: usize,
    terms: usize,
    constraints: usize,
}

impl ConstraintSystem<Fq> for TermCountCS {
    type Root = Self;
    fn alloc<V, A, AR>(&mut self, _: A, _: V) -> Result<Variable, SynthesisError>
    where
        V: FnOnce() -> Result<Fq, SynthesisError>,
        A: FnOnce() -> AR,
        AR: Into<String>,
    {
        self.aux += 1;
        Ok(Variable::new_unchecked(Index::Aux(self.aux - 1)))
    }
    fn alloc_input<V, A, AR>(&mut self, a: A, v: V) -> Result<Variable, SynthesisError>
    where
        V: FnOnce() -> Result<Fq, SynthesisError>,
        A: FnOnce() -> AR,
        AR: Into<String>,
    {
        self.alloc(a, v)
    }
    fn enforce<A, AR, LA, LB, LC>(&mut self, _: A, a: LA, b: LB, c: LC)
    where
        A: FnOnce() -> AR,
        AR: Into<String>,
        LA: FnOnce(LinearCombination<Fq>) -> LinearCombination<Fq>,
        LB: FnOnce(LinearCombination<Fq>) -> LinearCombination<Fq>,
        LC: FnOnce(LinearCombination<Fq>) -> LinearCombination<Fq>,
    {
        self.constraints += 1;
        let n = |lc: LinearCombination<Fq>| lc.iter().count();
        self.terms += n(a(LinearCombination::zero()))
            + n(b(LinearCombination::zero()))
            + n(c(LinearCombination::zero()));
    }
    fn push_namespace<NR: Into<String>, N: FnOnce() -> NR>(&mut self, _: N) {}
    fn pop_namespace(&mut self) {}
    fn get_root(&mut self) -> &mut Self {
        self
    }
}

#[test]
fn matrix_density_is_linear_in_steps_per_fold() {
    // Regressão do 0006: sem `compact`, a combinação de `nan` crescia 6 termos
    // por passo encadeado e as matrizes ficavam O(S²) termos.
    let base = LogisticCircuit::new(3.7, 0.1);
    let terms = |s: usize| {
        let c = LogisticCircuit {
            steps_per_fold: s,
            ..base.clone()
        };
        let mut cs = TermCountCS::default();
        let z: Vec<AllocatedNum<Fq>> = c
            .z0::<Fq>()
            .into_iter()
            .map(|v| AllocatedNum::alloc(&mut cs, || Ok(v)).unwrap())
            .collect();
        StepCircuit::<Fq>::synthesize(&c, &mut cs, &z).unwrap();
        cs.terms
    };
    let (t1, t2) = (terms(1), terms(2));
    for s in [16usize, 64, 256] {
        assert_eq!(terms(s), t1 + (s - 1) * (t2 - t1), "S={s}");
    }
}

#[test]
fn witness_only_mode_produces_identical_witness() {
    // O prover Nova sintetiza num gerador de witness (modo só-witness: `Expr`
    // não monta `lc`). A atribuição tem que ser idêntica, entrada a entrada,
    // à do modo completo (CheckCS, que monta e checa cada constraint).
    use nova_snark::frontend::{solver::SatisfyingAssignment, ConstraintSystem as _};
    for (r, x0) in [(3.7_f32, 0.1_f32), (5.0, 0.4), (3.7, f32::from_bits(1)), (3.7, f32::NAN)] {
        for s in [1usize, 4, 64] {
            let c = LogisticCircuit {
                steps_per_fold: s,
                ..LogisticCircuit::new(r, x0)
            };
            let mut full = CheckCS::<Fq>::default();
            let mut fast = SatisfyingAssignment::<E1>::new();
            assert!(fast.is_witness_generator() && !full.is_witness_generator());
            let zf: Vec<AllocatedNum<Fq>> = c
                .z0::<Fq>()
                .into_iter()
                .map(|v| AllocatedNum::alloc(&mut full, || Ok(v)).unwrap())
                .collect();
            let zw: Vec<AllocatedNum<Fq>> = c
                .z0::<Fq>()
                .into_iter()
                .map(|v| AllocatedNum::alloc(&mut fast, || Ok(v)).unwrap())
                .collect();
            let of = StepCircuit::<Fq>::synthesize(&c, &mut full, &zf).unwrap();
            let ow = StepCircuit::<Fq>::synthesize(&c, &mut fast, &zw).unwrap();
            assert!(full.is_satisfied());
            assert_eq!(full.aux(), fast.aux_assignment(), "r={r} x0={x0} S={s}");
            let v = |o: &[AllocatedNum<Fq>]| o.iter().map(|n| n.get_value()).collect::<Vec<_>>();
            assert_eq!(v(&of), v(&ow));
        }
    }
}
