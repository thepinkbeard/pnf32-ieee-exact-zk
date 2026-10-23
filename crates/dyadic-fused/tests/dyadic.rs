//! O circuito diádico fundido reproduz a referência, aceita só o witness
//! honesto (enumeração exaustiva em perfis mini) e concorda com o
//! TestConstraintSystem.

use dyadic_fused::{decision, f_big, is_exact, step, step_full, to_fixed, DyadicCircuit, Profile};
use nova_snark::{
    frontend::{num::AllocatedNum, test_cs::TestConstraintSystem, ConstraintSystem},
    provider::PallasEngine,
    traits::{circuit::StepCircuit, Engine},
};
use num_bigint::BigUint;
use pnf_circuit::gadgets::{to_biguint, CheckCS};

type Fq = <PallasEngine as Engine>::Scalar;

const PS: [u32; 4] = [32, 64, 96, 110];

fn run(c: &DyadicCircuit, z: &[Fq]) -> (bool, Vec<Fq>) {
    let mut cs = CheckCS::<Fq>::default();
    let zs: Vec<AllocatedNum<Fq>> = z.iter().map(|v| AllocatedNum::alloc(&mut cs, || Ok(*v)).unwrap()).collect();
    let out = StepCircuit::<Fq>::synthesize(c, &mut cs, &zs).unwrap();
    (cs.is_satisfied(), out.iter().map(|o| o.get_value().unwrap()).collect())
}

#[test]
fn conversions_are_exact() {
    let p = Profile::new(32);
    assert_eq!(to_fixed(0.5, 32), p.half());
    assert_eq!(to_fixed(1.0, 32), p.one());
    // fl32(3.9) = 0x4079999a: significando 0xF9999A = 16357786, valor 16357786·2^-22
    assert_eq!(to_fixed(3.9f32 as f64, 22), BigUint::from(16_357_786u64));
    for r in [3.5f32, 3.7, 3.9, 4.0] {
        assert!(is_exact(r as f64, 22), "r={r}");
    }
    // 0,01 em binary32 tem ulp 2^-30: exato a partir de p = 30
    assert!(is_exact(0.01f32 as f64, 30));
    assert!(!is_exact(0.1, 32));
    assert_eq!(f_big::<Fq>(&BigUint::from(u64::MAX)), Fq::from(u64::MAX));
}

#[test]
fn chained_folds_match_reference() {
    for p in PS {
        let prof = Profile::new(p);
        for r in [3.5f32, 3.7, 3.9, 4.0] {
            let c = DyadicCircuit::new(prof, r as f64, 0.1f32 as f64);
            let mut z = c.z0::<Fq>();
            let mut x = c.x.clone();
            for (n, f) in c.trajectory(500).iter().enumerate() {
                let (ok, out) = run(f, &z);
                assert!(ok, "p={p} r={r} fold {n}");
                x = step(prof, &x, &c.r);
                assert_eq!(to_biguint(out[1]), x, "p={p} r={r} fold {n}");
                assert_eq!(out[2], Fq::from(decision(prof, &x) as u64));
                assert!(x <= prof.one());
                z = out;
            }
        }
    }
}

#[test]
fn extremes() {
    for p in PS {
        let prof = Profile::new(p);
        let one = prof.one();
        let half = prof.half();
        let r4 = BigUint::from(4u32) << prof.s;
        for x in [
            BigUint::from(0u32),
            BigUint::from(1u32),
            &half - 1u32,
            half.clone(),
            &half + 1u32,
            &one - 1u32,
            one.clone(),
        ] {
            let c = DyadicCircuit { prof, r: r4.clone(), x: x.clone(), witness_override: None };
            let (ok, out) = run(&c, &c.z0());
            assert!(ok, "p={p} x={x}");
            let y = step(prof, &x, &r4);
            assert_eq!(to_biguint(out[1]), y);
            assert!(y <= one);
            // x = 1/2, r = 4 dá y = 1 exatamente: bit_p = 1, bit_{p-1} = 0
            assert_eq!(out[2], Fq::from(decision(prof, &y) as u64), "p={p} x={x}");
        }
    }
}

/// Enumeração de **todos** os witnesses `(Y, rem)` alocáveis nos perfis mini:
/// só o honesto é aceito, para todo `X` e todo `R` em [0, 4·2^s].
#[test]
fn mini_profiles_exhaustive_uniqueness() {
    for (p, s) in [(2u32, 1u32), (3, 2), (4, 2)] {
        let prof = Profile::with_s(p, s);
        let rmax = 4u32 << s;
        let mut checked = 0u64;
        for xv in 0..=(1u32 << p) {
            for rv in 0..=rmax {
                let x = BigUint::from(xv);
                let r = BigUint::from(rv);
                let honest = step_full(prof, &x, &r);
                for yv in 0..(1u32 << (p + 1)) {
                    for remv in 0..(1u32 << (p + s)) {
                        let w = (BigUint::from(yv), BigUint::from(remv));
                        let c = DyadicCircuit { prof, r: r.clone(), x: x.clone(), witness_override: Some(w.clone()) };
                        let (ok, out) = run(&c, &c.z0());
                        assert_eq!(ok, w == honest, "p={p} s={s} X={xv} R={rv} Y={yv} rem={remv}");
                        if ok {
                            assert_eq!(out[2], Fq::from(decision(prof, &honest.0) as u64));
                        }
                        checked += 1;
                    }
                }
            }
        }
        eprintln!("p={p} s={s}: {checked} witnesses");
    }
}

#[test]
fn tampered_witnesses_are_rejected() {
    for p in PS {
        let prof = Profile::new(p);
        let sh = (p + prof.s) as u64;
        for x0 in [0.1f32, 0.3, 0.5, 0.77, 0.999] {
            let c = DyadicCircuit::new(prof, 3.9f32 as f64, x0 as f64);
            assert!(run(&c, &c.z0()).0);
            let (y, rem) = step_full(prof, &c.x, &c.r);
            let t = (&y << sh) + &rem;
            for d in [1u32, 2, 1 << 20] {
                for yb in [&y + d, if y >= BigUint::from(d) { &y - d } else { &y + 3u32 }] {
                    // mantém Y·2^(p+s) + rem = t quando possível (rem sai da faixa)
                    let remb = if (&yb << sh) <= t { &t - (&yb << sh) } else { rem.clone() };
                    let bad = DyadicCircuit { witness_override: Some((yb.clone(), remb)), ..c.clone() };
                    assert!(!run(&bad, &bad.z0()).0, "p={p} x0={x0} Y'={yb} aceito");
                }
            }
        }
    }
}

#[test]
fn test_cs_agrees() {
    for p in PS {
        let c = DyadicCircuit::new(Profile::new(p), 3.9f32 as f64, 0.3f32 as f64);
        let mut cs = TestConstraintSystem::<Fq>::new();
        let z: Vec<AllocatedNum<Fq>> = c
            .z0::<Fq>()
            .iter()
            .enumerate()
            .map(|(i, v)| AllocatedNum::alloc(cs.namespace(|| format!("z{i}")), || Ok(*v)).unwrap())
            .collect();
        StepCircuit::<Fq>::synthesize(&c, &mut cs, &z).unwrap();
        assert!(cs.is_satisfied(), "p={p}: {:?}", cs.which_is_unsatisfied());
    }
}
