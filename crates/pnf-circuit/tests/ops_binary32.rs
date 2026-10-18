//! Bit-exatidão do circuito PNF32 contra o oráculo exato (que por sua vez é
//! validado contra o f32 nativo em `pnf-reference`).

use nova_snark::{
    frontend::{test_cs::TestConstraintSystem, ConstraintSystem},
    provider::PallasEngine,
    traits::Engine,
};
use pnf_circuit::{gadgets::*, ops, pnf::PnfVar, step::*};
use pnf_reference::{self as reference, Format, Pnf, Rng};

type Fq = <PallasEngine as Engine>::Scalar;
const B32: Format = Format::BINARY32;

#[derive(Clone, Copy, Debug)]
enum Op {
    Add,
    Sub,
    Mul,
}

fn run_op<CS: ConstraintSystem<Fq>>(cs: &mut CS, op: Op, a: Pnf, b: Pnf) -> Pnf {
    let va = PnfVar::alloc_unchecked(cs, B32, a);
    let vb = PnfVar::alloc_unchecked(cs, B32, b);
    let out = match op {
        Op::Add => ops::add(cs, B32, &va, &vb, None),
        Op::Sub => ops::sub(cs, B32, &va, &vb, None),
        Op::Mul => ops::mul(cs, B32, &va, &vb, None),
    };
    // A saída tem que ser válida (é a hipótese indutiva de T2).
    out.validate(cs, B32);
    out.value(B32)
}

fn oracle(op: Op, a: Pnf, b: Pnf) -> Pnf {
    match op {
        Op::Add => reference::add(B32, a, b),
        Op::Sub => reference::sub(B32, a, b),
        Op::Mul => reference::mul(B32, a, b),
    }
}

fn check(op: Op, a: Pnf, b: Pnf) {
    let mut cs = CheckCS::<Fq>::default();
    let got = run_op(&mut cs, op, a, b);
    let want = oracle(op, a, b);
    assert!(
        cs.is_satisfied(),
        "{op:?} {:e} {:e}: witness honesto insatisfeito",
        a.to_f32(),
        b.to_f32()
    );
    assert_eq!(
        got.to_bits(B32),
        want.to_bits(B32),
        "{op:?} {:e} {:e}",
        a.to_f32(),
        b.to_f32()
    );
    assert_eq!(got, want);
}

#[test]
fn field_repr_is_little_endian() {
    assert_eq!(to_biguint(Fq::from(258)), 258u32.into());
    assert_eq!(to_i128(-Fq::from(5)), -5);
}

#[test]
fn random_binary32_pairs() {
    let mut rng = Rng(0xC0FFEE);
    for _ in 0..200_000 {
        let a = Pnf::from_bits(B32, rng.f32_bits() as u64);
        let b = Pnf::from_bits(B32, rng.f32_bits() as u64);
        for op in [Op::Add, Op::Sub, Op::Mul] {
            check(op, a, b);
        }
    }
}

#[test]
fn near_path_cancellation() {
    let mut rng = Rng(0xBEEF);
    for _ in 0..200_000 {
        let a = rng.f32_bits();
        let b = (a & 0xFF80_0000) ^ ((rng.next_u64() as u32) & 0x807F_FFFF);
        let (a, b) = (Pnf::from_bits(B32, a as u64), Pnf::from_bits(B32, b as u64));
        check(Op::Add, a, b);
        check(Op::Sub, a, b);
    }
}

#[test]
fn hard_cases() {
    let hard = [
        0.0_f32,
        -0.0,
        1.0,
        -1.0,
        0.5,
        f32::MIN_POSITIVE,
        f32::MAX,
        -f32::MAX,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NAN,
        f32::from_bits(1),
        f32::from_bits(0x8000_0001),
        f32::from_bits(0x007F_FFFF),
        1.0 - f32::EPSILON / 2.0,
        1.0 + f32::EPSILON,
        1.0 / 3.0,
        3.9,
        3.5,
        0.1,
        f32::from_bits(0x0080_0001),
        2.0f32.powi(64),
        2.0f32.powi(-64),
        1e-45,
        f32::from_bits(0x3F7F_FFFF),
        f32::from_bits(0x3400_0000),
        f32::from_bits(0x3380_0000),
    ];
    for a in hard {
        for b in hard {
            for op in [Op::Add, Op::Sub, Op::Mul] {
                check(op, Pnf::from_f32(a), Pnf::from_f32(b));
            }
        }
    }
}

#[test]
fn test_constraint_system_agrees_with_check_cs() {
    let mut rng = Rng(42);
    for _ in 0..200 {
        let a = Pnf::from_bits(B32, rng.f32_bits() as u64);
        let b = Pnf::from_bits(B32, rng.f32_bits() as u64);
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
    // Para cada par, qualquer hint diferente do honesto que ainda satisfaça
    // o circuito tem que produzir a mesma saída.
    let mut rng = Rng(9);
    for _ in 0..300 {
        let a = Pnf::from_bits(B32, rng.f32_bits() as u64);
        let b = Pnf::from_bits(B32, rng.f32_bits() as u64);
        let want = reference::mul(B32, a, b);
        for k in 0..128 {
            for ovf in [false, true] {
                let mut cs = CheckCS::<Fq>::default();
                let va = PnfVar::alloc_unchecked(&mut cs, B32, a);
                let vb = PnfVar::alloc_unchecked(&mut cs, B32, b);
                let out = ops::mul(&mut cs, B32, &va, &vb, Some(ops::MulHints { k, ovf }));
                if cs.is_satisfied() {
                    assert_eq!(out.value(B32), want, "mul k={k} ovf={ovf}");
                }
            }
        }
        let want = reference::add(B32, a, b);
        for bits in 0..16u8 {
            for k in 0..64 {
                let h = ops::AddHints {
                    swap: bits & 1 == 1,
                    big: bits & 2 == 2,
                    neg: bits & 4 == 4,
                    ovf: bits & 8 == 8,
                    k,
                };
                let mut cs = CheckCS::<Fq>::default();
                let va = PnfVar::alloc_unchecked(&mut cs, B32, a);
                let vb = PnfVar::alloc_unchecked(&mut cs, B32, b);
                let out = ops::add(&mut cs, B32, &va, &vb, Some(h));
                if cs.is_satisfied() {
                    assert_eq!(out.value(B32), want, "add {h:?}");
                }
            }
        }
    }
}

#[test]
fn logistic_steps_match_native_f32() {
    for r in [3.5_f32, 3.9] {
        let base = LogisticCircuit::new(r, 0.1);
        let mut native = 0.1_f32;
        for c in base.trajectory(300) {
            let mut cs = CheckCS::<Fq>::default();
            let x = PnfVar::alloc_unchecked(&mut cs, B32, c.x);
            let rr = PnfVar::constant(B32, c.r);
            let y = logistic(&mut cs, B32, &x, &rr);
            assert!(cs.is_satisfied());
            assert_eq!(c.x.to_f32().to_bits(), native.to_bits());
            native = (native * (1.0 - native)) * r;
            assert_eq!(y.value(B32).to_f32().to_bits(), native.to_bits());
        }
    }
}
