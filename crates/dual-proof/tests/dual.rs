//! Prova dupla: saída IEEE bit-exata, enclosure contém a trajetória exata,
//! combinação correta e única, Nova ponta a ponta.

use std::collections::VecDeque;

use ff::Field;

use dual_proof::{check_z0, combine, f32_ratio, f32_to_rational, DualCircuit, Verdict};
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
use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::ToPrimitive;
use pnf_circuit::{
    gadgets::{to_biguint, CheckCS},
    step::LogisticCircuit,
};

type E1 = PallasEngine;
type E2 = VestaEngine;
type S1 = RelaxedR1CSSNARK<E1, EvaluationEngine<E1>>;
type S2 = RelaxedR1CSSNARK<E2, EvaluationEngine<E2>>;
type Fq = <E1 as Engine>::Scalar;

fn fold<const K: usize>(c: &DualCircuit<K>, z: &[Fq]) -> Vec<Fq> {
    let mut cs = CheckCS::<Fq>::default();
    let zs: Vec<AllocatedNum<Fq>> = z
        .iter()
        .map(|v| AllocatedNum::alloc(&mut cs, || Ok(*v)).unwrap())
        .collect();
    let out = StepCircuit::<Fq>::synthesize(c, &mut cs, &zs).unwrap();
    assert!(cs.is_satisfied(), "witness honesto insatisfeito");
    out.iter().map(|n| n.get_value().unwrap()).collect()
}

fn u(v: Fq) -> u128 {
    to_biguint(v).to_u128().unwrap()
}

/// Enclosure `[c-R, c+R]` lido do estado, como racionais.
fn enclosure<const K: usize>(z: &[Fq]) -> (BigRational, BigRational) {
    let base = DualCircuit::<K>::idx_cbd();
    let scale = 1_u128 << (16 * K);
    let limbs = z[base..base + K]
        .iter()
        .fold(0_u128, |acc, &l| (acc << 16) + u(l));
    let c = limbs + u(z[base + K]) * scale;
    let r = u(z[base + K + 1]);
    let q = |n: i128| BigRational::new(BigInt::from(n), BigInt::from(scale));
    (q(c as i128 - r as i128), q((c + r) as i128))
}

const CASES: [(f32, f32); 6] = [
    (3.9, 0.1),
    (3.9, 0.3),
    (3.9, 0.7),
    (3.7, 0.123_456),
    (3.9, 1.0e-12), // inexato na grade de K=2: raio inicial de 1 ULP
    (4.0, 0.5),     // x1 = 1 (is_one), x2 = 0
];

fn ieee_bit_exact<const K: usize>(n: usize) {
    for (r, x0) in CASES {
        let first = DualCircuit::<K>::new(r, x0);
        let mut z = first.z0::<Fq>();
        assert!(check_z0::<K>(r, x0, &z));
        let mut native = x0;
        for (i, c) in first.trajectory(n).iter().enumerate() {
            z = fold(c, &z);
            native = (native * (1.0 - native)) * r;
            let expect = LogisticCircuit::new(r, native).z0::<Fq>();
            assert_eq!(z[0], Fq::from(i as u64 + 1));
            assert_eq!(z[1..7], expect[1..7], "r={r} x0={x0} passo {}", i + 1);
            assert_eq!(z[7] == Fq::from(1), native >= 0.5);
            // Estado CBD e veredito batem com a simulação fora do circuito.
            let rec = c.record(i + 1);
            assert_eq!(rec.y.to_bits(), native.to_bits());
            assert_eq!(z[DualCircuit::<K>::idx_verdict()], Fq::from(rec.verdict.code()));
            assert_eq!(z[DualCircuit::<K>::idx_verdict() - 1], Fq::from(rec.cbd_code));
            let (lo, hi) = enclosure::<K>(&z);
            let s = BigRational::from_integer(BigInt::from(1_u128 << (16 * K)));
            let c = BigRational::from_integer(BigInt::from(rec.center)) / &s;
            let rr = BigRational::from_integer(BigInt::from(rec.radius)) / &s;
            assert_eq!((lo, hi), (&c - &rr, &c + &rr));
        }
    }
}

#[test]
fn ieee_output_is_bit_exact_k2() {
    ieee_bit_exact::<2>(120);
}

#[test]
fn ieee_output_is_bit_exact_k4() {
    ieee_bit_exact::<4>(120);
}

/// Contra a trajetória racional exata (curta: o denominador dobra de tamanho
/// a cada passo).
fn contains_exact<const K: usize>(n: usize) {
    for (r, x0) in CASES {
        let rq = f32_to_rational(r);
        let exact =
            numeric_reference::logistic_trajectory(&f32_to_rational(x0), &rq, n).unwrap();
        let first = DualCircuit::<K>::new(r, x0);
        let mut z = first.z0::<Fq>();
        let (lo, hi) = enclosure::<K>(&z);
        assert!(lo <= exact[0] && exact[0] <= hi);
        for (i, c) in first.trajectory(n).iter().enumerate() {
            z = fold(c, &z);
            let (lo, hi) = enclosure::<K>(&z);
            let x = &exact[i + 1];
            assert!(&lo <= x && x <= &hi, "r={r} x0={x0} K={K} passo {}", i + 1);
            let half = BigRational::new(1.into(), 2.into());
            match Verdict::combine(z[7] == Fq::from(1), u(z[DualCircuit::<K>::idx_verdict() - 1]) as u64) {
                Verdict::RobustTrue => assert!(x >= &half),
                Verdict::RobustFalse => assert!(x < &half),
                Verdict::Indeterminate => {}
            }
        }
    }
}

#[test]
fn enclosure_contains_exact_rational_trajectory() {
    contains_exact::<2>(12);
    contains_exact::<4>(12);
}

/// Horizonte longo: o intervalo dirigido de 4096 bits (que contém a
/// trajetória exata) tem que caber no enclosure CBD.
fn contains_directed<const K: usize>(n: usize) -> [usize; 3] {
    let mut verdicts = [0; 3];
    for (r, x0) in CASES {
        let refr = numeric_reference::directed_dynamic_trajectory(
            &f32_to_rational(x0),
            &f32_to_rational(r),
            None,
            n,
            4096,
        )
        .unwrap();
        let first = DualCircuit::<K>::new(r, x0);
        let mut z = first.z0::<Fq>();
        let half = BigRational::new(1.into(), 2.into());
        for (i, c) in first.trajectory(n).iter().enumerate() {
            z = fold(c, &z);
            let (lo, hi) = enclosure::<K>(&z);
            let (rlo, rhi) = &refr[i + 1];
            assert!(&lo <= rlo && rhi <= &hi, "r={r} x0={x0} K={K} passo {}", i + 1);
            let v = u(z[DualCircuit::<K>::idx_verdict()]) as usize;
            verdicts[v] += 1;
            match v {
                1 => assert!(rlo >= &half),
                0 => assert!(rhi < &half),
                _ => {}
            }
        }
    }
    verdicts
}

#[test]
fn enclosure_contains_directed_reference_long_horizon() {
    let v2 = contains_directed::<2>(100);
    let v4 = contains_directed::<4>(100);
    // As trajetórias exercitam os três vereditos.
    assert!(v2.iter().all(|&c| c > 0), "{v2:?}");
    assert!(v4[0] > 0 && v4[1] > 0, "{v4:?}");
}

#[test]
fn r_and_x0_are_the_same_numbers_in_both_parts() {
    assert_eq!(f32_ratio(3.9), (8_178_893, 1 << 21));
    assert_eq!(
        f32_to_rational(3.9),
        BigRational::new(8_178_893.into(), (1 << 21).into())
    );
    assert_eq!(f32_ratio(4.0), (4, 1));
    let c = DualCircuit::<2>::new(3.9, 0.1);
    assert_eq!((c.cbd.r_num, c.cbd.r_den), f32_ratio(c.pnf.r.to_f32()));
}

#[test]
fn z0_check_rejects_inconsistent_initial_state() {
    let c = DualCircuit::<2>::new(3.9, 0.1);
    let mut z = c.z0::<Fq>();
    assert!(check_z0::<2>(3.9, 0.1, &z));
    // CBD começando noutro ponto que a PNF32.
    let idx = DualCircuit::<2>::idx_cbd() + 1;
    z[idx] += Fq::from(1);
    assert!(!check_z0::<2>(3.9, 0.1, &z));
    // Raio inicial inflado também é rejeitado (z0 é canônico).
    let mut z = c.z0::<Fq>();
    z[DualCircuit::<2>::idx_cbd() + 3] = Fq::from(5);
    assert!(!check_z0::<2>(3.9, 0.1, &z));
}

#[test]
fn breakdown_adds_up() {
    for (r, x0) in CASES {
        let b2 = DualCircuit::<2>::new(r, x0).breakdown();
        assert_eq!(b2.total, b2.pnf + b2.cbd + b2.combination, "{b2:?}");
        assert_eq!(b2.pnf, 415);
        assert_eq!(b2.combination, 6);
        let b4 = DualCircuit::<4>::new(r, x0).breakdown();
        assert_eq!(b4.total, b4.pnf + b4.cbd + b4.combination, "{b4:?}");
        // A forma não depende do witness.
        // A forma depende de `r` (larguras dos range checks), não de `x0`.
        assert_eq!(b2, DualCircuit::<2>::new(r, 0.1).breakdown());
        assert_eq!(b4, DualCircuit::<4>::new(r, 0.1).breakdown());
    }
}

#[test]
fn test_constraint_system_agrees() {
    let c = DualCircuit::<2>::new(3.9, 0.3).trajectory(7).pop().unwrap();
    let z0 = {
        let mut z = DualCircuit::<2>::new(3.9, 0.3).z0::<Fq>();
        for f in DualCircuit::<2>::new(3.9, 0.3).trajectory(6) {
            z = fold(&f, &z);
        }
        z
    };
    let mut t = TestConstraintSystem::<Fq>::new();
    let zs: Vec<AllocatedNum<Fq>> = z0
        .iter()
        .enumerate()
        .map(|(i, v)| AllocatedNum::alloc(t.namespace(|| format!("z{i}")), || Ok(*v)).unwrap())
        .collect();
    let out = StepCircuit::<Fq>::synthesize(&c, &mut t, &zs).unwrap();
    assert!(t.is_satisfied(), "{:?}", t.which_is_unsatisfied());
    let got: Vec<Fq> = out.iter().map(|n| n.get_value().unwrap()).collect();
    assert_eq!(got, fold(&c, &z0));
}

// ---------------------------------------------------------------------------
// Unicidade da combinação: um CS que substitui os valores alocados.
// ---------------------------------------------------------------------------

#[derive(Default)]
struct TamperCS {
    aux: Vec<Fq>,
    overrides: VecDeque<Fq>,
    failures: usize,
}

impl TamperCS {
    fn eval(&self, lc: &LinearCombination<Fq>) -> Fq {
        lc.iter()
            .map(|(v, c)| {
                *c * match v.get_unchecked() {
                    Index::Input(_) => Fq::from(1),
                    Index::Aux(i) => self.aux[i],
                }
            })
            .sum()
    }
}

impl ConstraintSystem<Fq> for TamperCS {
    type Root = Self;
    fn alloc<F, A, AR>(&mut self, _: A, f: F) -> Result<Variable, SynthesisError>
    where
        F: FnOnce() -> Result<Fq, SynthesisError>,
        A: FnOnce() -> AR,
        AR: Into<String>,
    {
        let honest = f();
        let v = match self.overrides.pop_front() {
            Some(v) => v,
            None => honest?,
        };
        self.aux.push(v);
        Ok(Variable::new_unchecked(Index::Aux(self.aux.len() - 1)))
    }
    fn alloc_input<F, A, AR>(&mut self, _: A, _: F) -> Result<Variable, SynthesisError>
    where
        F: FnOnce() -> Result<Fq, SynthesisError>,
        A: FnOnce() -> AR,
        AR: Into<String>,
    {
        unreachable!()
    }
    fn enforce<A, AR, LA, LB, LC>(&mut self, _: A, a: LA, b: LB, c: LC)
    where
        A: FnOnce() -> AR,
        AR: Into<String>,
        LA: FnOnce(LinearCombination<Fq>) -> LinearCombination<Fq>,
        LB: FnOnce(LinearCombination<Fq>) -> LinearCombination<Fq>,
        LC: FnOnce(LinearCombination<Fq>) -> LinearCombination<Fq>,
    {
        let z = LinearCombination::zero;
        if self.eval(&a(z())) * self.eval(&b(z())) != self.eval(&c(z())) {
            self.failures += 1;
        }
    }
    fn push_namespace<NR: Into<String>, N: FnOnce() -> NR>(&mut self, _: N) {}
    fn pop_namespace(&mut self) {}
    fn get_root(&mut self) -> &mut Self {
        self
    }
}

#[test]
fn combination_is_unique_and_correct() {
    // Domínio das substituições: inclui -1 e 1/2 do campo.
    let dom = [
        Fq::from(0),
        Fq::from(1),
        Fq::from(2),
        Fq::from(3),
        -Fq::from(1),
        Fq::from(2).invert().unwrap(),
    ];
    let mut checked = 0;
    for ieee in [false, true] {
        for code in 0..3_u64 {
            let want = Verdict::combine(ieee, code);
            // Witness honesto.
            let mut cs = TamperCS::default();
            let d = AllocatedNum::alloc(&mut cs, || Ok(Fq::from(ieee as u64))).unwrap();
            let k = AllocatedNum::alloc(&mut cs, || Ok(Fq::from(code))).unwrap();
            let v = combine(&mut cs, &d, &k).unwrap();
            assert_eq!(cs.failures, 0);
            assert_eq!(v.get_value().unwrap(), Fq::from(want.code()));
            // Toda escolha de (a, b, RT, RF, veredito) que satisfaz as
            // constraints dá o veredito da regra.
            for i in 0..dom.len().pow(5) {
                let mut o = VecDeque::new();
                let mut j = i;
                for _ in 0..5 {
                    o.push_back(dom[j % dom.len()]);
                    j /= dom.len();
                }
                let mut cs = TamperCS::default();
                let d = AllocatedNum::alloc(&mut cs, || Ok(Fq::from(ieee as u64))).unwrap();
                let k = AllocatedNum::alloc(&mut cs, || Ok(Fq::from(code))).unwrap();
                cs.overrides = o;
                let v = combine(&mut cs, &d, &k).unwrap();
                if cs.failures == 0 {
                    assert_eq!(cs.aux[v.get_variable().get_unchecked().index()], Fq::from(want.code()));
                    checked += 1;
                }
            }
        }
    }
    assert_eq!(checked, 6, "exatamente uma atribuição aceita por entrada");
}

trait AuxIndex {
    fn index(&self) -> usize;
}
impl AuxIndex for Index {
    fn index(&self) -> usize {
        match self {
            Index::Aux(i) => *i,
            Index::Input(_) => unreachable!(),
        }
    }
}

#[test]
fn nova_proves_dual_trajectory() {
    let (r, x0, folds) = (3.9_f32, 0.3_f32, 4);
    let first = DualCircuit::<2>::new(r, x0);
    let circuits = first.trajectory(folds);
    let z0 = first.z0::<Fq>();
    assert!(check_z0::<2>(r, x0, &z0));
    let pp = PublicParams::<E1, E2, DualCircuit<2>>::setup(
        &circuits[0],
        &*S1::ck_floor(),
        &*S2::ck_floor(),
    )
    .unwrap();
    let mut rs = RecursiveSNARK::new(&pp, &circuits[0], &z0).unwrap();
    for c in &circuits {
        rs.prove_step(&pp, c).unwrap();
    }
    let zn = rs.verify(&pp, folds, &z0).unwrap();
    let mut z = z0.clone();
    for c in &circuits {
        z = fold(c, &z);
    }
    assert_eq!(zn, z);
    let last = first.simulate(folds).pop().unwrap();
    assert_eq!(zn[DualCircuit::<2>::idx_verdict()], Fq::from(last.verdict.code()));
}

#[test]
fn model_variant_encloses_decimal_r_trajectory() {
    // PNF32 com fl(3.9); CBD com r = 39/10 exato.
    let rq = BigRational::new(39.into(), 10.into());
    for x0 in [0.1_f32, 0.3, 0.7] {
        let first = DualCircuit::<2>::with_model_r(3.9, x0, 39, 10);
        let exact = numeric_reference::logistic_trajectory(&f32_to_rational(x0), &rq, 12).unwrap();
        let refr = numeric_reference::directed_dynamic_trajectory(
            &f32_to_rational(x0),
            &rq,
            None,
            60,
            4096,
        )
        .unwrap();
        let mut z = first.z0::<Fq>();
        let mut native = x0;
        for (i, c) in first.trajectory(60).iter().enumerate() {
            z = fold(c, &z);
            native = (native * (1.0 - native)) * 3.9;
            assert_eq!(z[1..7], LogisticCircuit::new(3.9, native).z0::<Fq>()[1..7]);
            let (lo, hi) = enclosure::<2>(&z);
            if i < 12 {
                assert!(lo <= exact[i + 1] && exact[i + 1] <= hi);
            }
            assert!(lo <= refr[i + 1].0 && refr[i + 1].1 <= hi, "x0={x0} passo {}", i + 1);
        }
    }
    let b = DualCircuit::<2>::with_model_r(3.9, 0.1, 39, 10).breakdown();
    assert_eq!(b.total, b.pnf + b.cbd + b.combination);
}
