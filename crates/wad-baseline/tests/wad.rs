//! O circuito WAD reproduz o `mulWad` de referência e rejeita witnesses
//! adulterados.

use nova_snark::{
    frontend::{num::AllocatedNum, test_cs::TestConstraintSystem, ConstraintSystem},
    provider::PallasEngine,
    traits::{circuit::StepCircuit, Engine},
};
use pnf_circuit::gadgets::{f_u128, to_biguint, CheckCS};
use wad_baseline::{mul_wad, step, to_wad, WadCircuit, W};

type Fq = <PallasEngine as Engine>::Scalar;

fn run(c: &WadCircuit, z: &[Fq]) -> (bool, Vec<Fq>) {
    let mut cs = CheckCS::<Fq>::default();
    let zs: Vec<AllocatedNum<Fq>> = z.iter().map(|v| AllocatedNum::alloc(&mut cs, || Ok(*v)).unwrap()).collect();
    let out = StepCircuit::<Fq>::synthesize(c, &mut cs, &zs).unwrap();
    (cs.is_satisfied(), out.iter().map(|o| o.get_value().unwrap()).collect())
}

#[test]
fn to_wad_is_floor() {
    assert_eq!(to_wad(0.5), W / 2);
    assert_eq!(to_wad(1.0), W);
    assert_eq!(to_wad(4.0), 4 * W);
    // 0.1 em f64 = 0.1000000000000000055511151231257827...
    assert_eq!(to_wad(0.1), 100_000_000_000_000_005);
    // fl32(3.9) = 3.900000095367431640625
    assert_eq!(to_wad(3.9f32 as f64), 3_900_000_095_367_431_640);
}

#[test]
fn chained_folds_match_reference() {
    for r in [3.5f32, 3.7, 3.9, 4.0] {
        let c = WadCircuit::new(r as f64, 0.1f32 as f64);
        let mut z = c.z0::<Fq>();
        let mut x = c.x;
        for (n, f) in c.trajectory(1000).iter().enumerate() {
            let (ok, out) = run(f, &z);
            assert!(ok, "r={r} fold {n}");
            x = step(x, c.r);
            assert_eq!(to_biguint(out[1]), x.into());
            assert_eq!(out[2], Fq::from((x >= W / 2) as u64));
            assert!(x <= W);
            z = out;
        }
    }
}

#[test]
fn extremes() {
    for x in [0, 1, W / 2 - 1, W / 2, W / 2 + 1, W - 1, W] {
        let c = WadCircuit { r: 4 * W, x, tamper: Default::default() };
        let (ok, out) = run(&c, &c.z0());
        assert!(ok, "x={x}");
        let y = mul_wad(mul_wad(x, W - x), 4 * W);
        assert_eq!(to_biguint(out[1]), y.into());
        assert!(y <= W);
    }
}

#[test]
fn tampered_witnesses_are_rejected() {
    use wad_baseline::Tamper;
    for x0 in [0.1, 0.3, 0.5, 0.77, 0.999] {
        let c = WadCircuit::new(3.9, x0);
        assert!(run(&c, &c.z0()).0);
        for d in [-2i128, -1, 1, 2, 1 << 40] {
            for t in [Tamper { q1: d, ..Default::default() }, Tamper { y: d, ..Default::default() }] {
                let bad = WadCircuit { tamper: t, ..c.clone() };
                assert!(!run(&bad, &bad.z0()).0, "x0={x0} {t:?} aceito");
            }
        }
        let bad = WadCircuit { tamper: Tamper { flip_decision: true, ..Default::default() }, ..c.clone() };
        assert!(!run(&bad, &bad.z0()).0, "x0={x0} decisão invertida aceita");
    }
}

#[test]
fn test_cs_agrees() {
    let c = WadCircuit::new(3.9, 0.3);
    let mut cs = TestConstraintSystem::<Fq>::new();
    let z: Vec<AllocatedNum<Fq>> = c
        .z0::<Fq>()
        .iter()
        .enumerate()
        .map(|(i, v)| AllocatedNum::alloc(cs.namespace(|| format!("z{i}")), || Ok(*v)).unwrap())
        .collect();
    StepCircuit::<Fq>::synthesize(&c, &mut cs, &z).unwrap();
    assert!(cs.is_satisfied(), "{:?}", cs.which_is_unsatisfied());
}
