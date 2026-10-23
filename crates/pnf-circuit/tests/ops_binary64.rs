//! Bit-exatidão do circuito PNF64 contra o oráculo exato e contra o f64
//! nativo. Mesmos casos de `ops_binary32.rs`, com o formato binary64.

use nova_snark::{
    frontend::{num::AllocatedNum, test_cs::TestConstraintSystem, ConstraintSystem},
    provider::PallasEngine,
    traits::{circuit::StepCircuit, Engine},
};
use pnf_circuit::{gadgets::*, ops, pnf::PnfVar, step::*};
use pnf_reference::{self as reference, Format, Pnf, Rng};

type Fq = <PallasEngine as Engine>::Scalar;
const B64: Format = Format::BINARY64;

#[derive(Clone, Copy, Debug)]
enum Op {
    Add,
    Sub,
    Mul,
}

fn run_op<CS: ConstraintSystem<Fq>>(cs: &mut CS, op: Op, a: Pnf, b: Pnf) -> Pnf {
    let va = PnfVar::alloc_unchecked(cs, B64, a);
    let vb = PnfVar::alloc_unchecked(cs, B64, b);
    let out = match op {
        Op::Add => ops::add(cs, B64, &va, &vb, None),
        Op::Sub => ops::sub(cs, B64, &va, &vb, None),
        Op::Mul => ops::mul(cs, B64, &va, &vb, None),
    };
    out.validate(cs, B64);
    out.value(B64)
}

fn oracle(op: Op, a: Pnf, b: Pnf) -> Pnf {
    match op {
        Op::Add => reference::add(B64, a, b),
        Op::Sub => reference::sub(B64, a, b),
        Op::Mul => reference::mul(B64, a, b),
    }
}

fn native(op: Op, a: f64, b: f64) -> f64 {
    match op {
        Op::Add => a + b,
        Op::Sub => a - b,
        Op::Mul => a * b,
    }
}

fn check(op: Op, a: Pnf, b: Pnf) {
    let mut cs = CheckCS::<Fq>::default();
    let got = run_op(&mut cs, op, a, b);
    let (fa, fb) = (a.to_f64(), b.to_f64());
    assert!(cs.is_satisfied(), "{op:?} {fa:e} {fb:e}: witness honesto insatisfeito");
    assert_eq!(got, oracle(op, a, b), "{op:?} {fa:e} {fb:e}");
    let n = native(op, fa, fb);
    if !n.is_nan() {
        assert_eq!(got.to_f64().to_bits(), n.to_bits(), "{op:?} {fa:e} {fb:e} vs nativo");
    } else {
        assert!(got.is_nan());
    }
}

#[test]
fn random_binary64_pairs() {
    let mut rng = Rng(0xC0FFEE64);
    for _ in 0..50_000 {
        let a = Pnf::from_bits(B64, rng.next_u64());
        let b = Pnf::from_bits(B64, rng.next_u64());
        for op in [Op::Add, Op::Sub, Op::Mul] {
            check(op, a, b);
        }
    }
}

#[test]
fn unit_interval_pairs() {
    let mut rng = Rng(0x0001);
    let unit = |rng: &mut Rng| (rng.next_u64() >> 11) as f64 / (1u64 << 53) as f64;
    for _ in 0..50_000 {
        let (a, b) = (unit(&mut rng), unit(&mut rng));
        for op in [Op::Add, Op::Sub, Op::Mul] {
            check(op, Pnf::from_f64(a), Pnf::from_f64(b));
        }
        check(Op::Sub, Pnf::from_f64(1.0), Pnf::from_f64(a));
        check(Op::Mul, Pnf::from_f64(a), Pnf::from_f64(3.9));
    }
}

#[test]
fn near_path_cancellation() {
    let mut rng = Rng(0xBEEF64);
    for _ in 0..50_000 {
        let a = rng.next_u64();
        let b = (a & 0xFFF0_0000_0000_0000) ^ (rng.next_u64() & 0x800F_FFFF_FFFF_FFFF);
        let (a, b) = (Pnf::from_bits(B64, a), Pnf::from_bits(B64, b));
        check(Op::Add, a, b);
        check(Op::Sub, a, b);
    }
}

#[test]
fn hard_cases() {
    let hard = [
        0.0_f64,
        -0.0,
        1.0,
        -1.0,
        0.5,
        f64::MIN_POSITIVE,
        f64::MAX,
        -f64::MAX,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NAN,
        f64::from_bits(1),
        f64::from_bits(0x8000_0000_0000_0001),
        f64::from_bits(0x000F_FFFF_FFFF_FFFF),
        f64::from_bits(0x0010_0000_0000_0001),
        1.0 - f64::EPSILON / 2.0,
        1.0 + f64::EPSILON,
        1.0 / 3.0,
        3.9,
        0.1,
        2.0f64.powi(512),
        2.0f64.powi(-512),
        f64::from_bits(0x3FEF_FFFF_FFFF_FFFF),
        f64::from_bits(0x3CA0_0000_0000_0000),
        f64::from_bits(0x3C90_0000_0000_0000),
    ];
    for a in hard {
        for b in hard {
            for op in [Op::Add, Op::Sub, Op::Mul] {
                check(op, Pnf::from_f64(a), Pnf::from_f64(b));
            }
        }
    }
}

#[test]
fn test_constraint_system_agrees_with_check_cs() {
    let mut rng = Rng(4264);
    for _ in 0..100 {
        let a = Pnf::from_bits(B64, rng.next_u64());
        let b = Pnf::from_bits(B64, rng.next_u64());
        for op in [Op::Add, Op::Mul] {
            let mut t = TestConstraintSystem::<Fq>::new();
            let got = run_op(&mut t, op, a, b);
            assert!(t.is_satisfied(), "{:?}", t.which_is_unsatisfied());
            assert_eq!(got, oracle(op, a, b));
        }
    }
}

#[test]
fn tampered_witnesses_are_rejected() {
    // Todo hint que satisfaz o circuito tem de dar a saída honesta.
    let mut rng = Rng(964);
    let unit = |rng: &mut Rng| (rng.next_u64() >> 11) as f64 / (1u64 << 53) as f64;
    for i in 0..120 {
        let (a, b) = if i % 2 == 0 {
            (Pnf::from_bits(B64, rng.next_u64()), Pnf::from_bits(B64, rng.next_u64()))
        } else {
            (Pnf::from_f64(unit(&mut rng)), Pnf::from_f64(unit(&mut rng)))
        };
        let want = reference::mul(B64, a, b);
        let kmax = 1u64 << ops::Widths::of(B64).mul_kb;
        for k in 0..kmax {
            for ovf in [false, true] {
                let mut cs = CheckCS::<Fq>::default();
                let va = PnfVar::alloc_unchecked(&mut cs, B64, a);
                let vb = PnfVar::alloc_unchecked(&mut cs, B64, b);
                let out = ops::mul(&mut cs, B64, &va, &vb, Some(ops::MulHints { k, ovf }));
                if cs.is_satisfied() {
                    assert_eq!(out.value(B64), want, "mul k={k} ovf={ovf}");
                }
            }
        }
        let want = reference::add(B64, a, b);
        let kmax = 1u64 << ops::Widths::of(B64).add_kb;
        for bits in 0..16u8 {
            for k in 0..kmax {
                let h = ops::AddHints {
                    swap: bits & 1 == 1,
                    big: bits & 2 == 2,
                    neg: bits & 4 == 4,
                    ovf: bits & 8 == 8,
                    k,
                };
                let mut cs = CheckCS::<Fq>::default();
                let va = PnfVar::alloc_unchecked(&mut cs, B64, a);
                let vb = PnfVar::alloc_unchecked(&mut cs, B64, b);
                let out = ops::add(&mut cs, B64, &va, &vb, Some(h));
                if cs.is_satisfied() {
                    assert_eq!(out.value(B64), want, "add {h:?}");
                }
            }
        }
    }
}

#[test]
fn logistic_steps_match_native_f64() {
    for r in [3.5_f64, 3.7, 3.9, 4.0] {
        let base = LogisticCircuit::new_f64(r, 0.1);
        let mut native = 0.1_f64;
        for c in base.trajectory(1000) {
            let mut cs = CheckCS::<Fq>::default();
            let x = PnfVar::alloc_unchecked(&mut cs, B64, c.x);
            let rr = PnfVar::constant(B64, c.r);
            let y = logistic(&mut cs, B64, &x, &rr);
            assert!(cs.is_satisfied());
            assert_eq!(c.x.to_f64().to_bits(), native.to_bits());
            native = (native * (1.0 - native)) * r;
            assert_eq!(y.value(B64).to_f64().to_bits(), native.to_bits());
        }
    }
}

#[test]
fn step_circuit_binary64_is_satisfied() {
    let c = LogisticCircuit::new_f64(3.9, 0.1);
    for step in c.trajectory(5) {
        let mut cs = TestConstraintSystem::<Fq>::new();
        let z: Vec<AllocatedNum<Fq>> = step
            .z0::<Fq>()
            .iter()
            .enumerate()
            .map(|(i, v)| AllocatedNum::alloc(cs.namespace(|| format!("z{i}")), || Ok(*v)).unwrap())
            .collect();
        let out = StepCircuit::<Fq>::synthesize(&step, &mut cs, &z).unwrap();
        assert!(cs.is_satisfied(), "{:?}", cs.which_is_unsatisfied());
        assert_eq!(out.len(), STEP_ARITY);
    }
}
