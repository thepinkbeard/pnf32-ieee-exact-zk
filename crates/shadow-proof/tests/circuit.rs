//! O fold aceita o certificado honesto, encadeia o estado, reproduz o f32
//! nativo e rejeita adulterações do certificado.

use nova_snark::frontend::{num::AllocatedNum, test_cs::TestConstraintSystem, ConstraintSystem};
use nova_snark::traits::circuit::StepCircuit;
use num_bigint::BigInt;
use shadow_proof::bench::{setup_circuits, synth_one, Fq};

#[test]
fn honest_folds_chain_and_match_native() {
    let (_, orbit, circuits) = setup_circuits(3.9, 0.1, 300).unwrap();
    let mut z = circuits[0].z0::<Fq>();
    let mut native = 0.1f32;
    for (n, c) in circuits.iter().enumerate() {
        assert_eq!(c.p.to_f32().to_bits(), native.to_bits());
        assert_eq!(z, {
            let mut w = c.z0::<Fq>();
            w[0] = Fq::from(n as u64);
            w[shadow_proof::circuit::SHADOW_ARITY - 1] = z[shadow_proof::circuit::SHADOW_ARITY - 1];
            w
        });
        let (_, ok, _, out) = synth_one(c, &z);
        assert!(ok, "fold {n} insatisfeito");
        native = (native * (1.0 - native)) * 3.9f32;
        // veredito robusto (lado certificado = decisão IEEE) em todo passo
        let v = out[shadow_proof::circuit::SHADOW_ARITY - 1];
        assert!(v == Fq::from(0) || v == Fq::from(1), "fold {n} indeterminado");
        z = out;
    }
    assert_eq!(orbit.len(), 301);
}

#[test]
fn tampering_is_rejected() {
    let (_, _, circuits) = setup_circuits(3.9, 0.1, 64).unwrap();
    let c = &circuits[30];
    let z = c.z0::<Fq>();
    assert!(synth_one(c, &z).1);
    // alargar o próximo intervalo quebra o aninhamento
    let mut t = c.clone();
    t.next.a -= 1;
    assert!(!synth_one(&t, &z).1, "A' alargado aceito");
    let mut t = c.clone();
    t.next.b += 1;
    assert!(!synth_one(&t, &z).1, "B' alargado aceito");
    // lado errado
    let mut t = c.clone();
    t.side = !t.side;
    assert!(!synth_one(&t, &z).1, "lado trocado aceito");
    // ε abaixo do necessário em algum fold
    let tight = BigInt::from(0);
    let rejected = circuits.iter().any(|c| {
        let mut t = c.clone();
        t.eps = tight.clone();
        !synth_one(&t, &t.z0::<Fq>()).1
    });
    assert!(rejected, "ε = 0 aceito em todos os folds");
}

#[test]
fn test_cs_agrees() {
    let (_, _, circuits) = setup_circuits(3.9, 0.1, 4).unwrap();
    let c = &circuits[1];
    let mut cs = TestConstraintSystem::<Fq>::new();
    let z: Vec<AllocatedNum<Fq>> =
        c.z0::<Fq>()
            .iter()
            .enumerate()
            .map(|(i, v)| AllocatedNum::alloc(cs.namespace(|| format!("z{i}")), || Ok(*v)).unwrap())
            .collect();
    StepCircuit::<Fq>::synthesize(c, &mut cs, &z).unwrap();
    assert!(cs.is_satisfied(), "{:?}", cs.which_is_unsatisfied());
}

// --- binary64 (PNF64 + grade 2^-96) -------------------------------------------

use pnf_reference::Format;
use shadow_proof::bench::setup_circuits_fmt;

#[test]
fn binary64_honest_folds_chain_and_match_native() {
    let (_, orbit, circuits) = setup_circuits_fmt(Format::BINARY64, 3.9, 0.1, 300).unwrap();
    let mut z = circuits[0].z0::<Fq>();
    let mut native = 0.1f64;
    for (n, c) in circuits.iter().enumerate() {
        assert_eq!(c.p.to_f64().to_bits(), native.to_bits());
        let (_, ok, _, out) = synth_one(c, &z);
        assert!(ok, "fold {n} insatisfeito");
        native = (native * (1.0 - native)) * 3.9f64;
        let v = out[shadow_proof::circuit::SHADOW_ARITY - 1];
        assert!(v == Fq::from(0) || v == Fq::from(1), "fold {n} indeterminado");
        z = out;
    }
    assert_eq!(orbit.len(), 301);
}

#[test]
fn binary64_tampering_is_rejected() {
    let (_, _, circuits) = setup_circuits_fmt(Format::BINARY64, 3.9, 0.1, 64).unwrap();
    let c = &circuits[30];
    let z = c.z0::<Fq>();
    assert!(synth_one(c, &z).1);
    let mut t = c.clone();
    t.next.a -= 1;
    assert!(!synth_one(&t, &z).1, "A' alargado aceito");
    let mut t = c.clone();
    t.next.b += 1;
    assert!(!synth_one(&t, &z).1, "B' alargado aceito");
    let mut t = c.clone();
    t.side = !t.side;
    assert!(!synth_one(&t, &z).1, "lado trocado aceito");
    let rejected = circuits.iter().any(|c| {
        let mut t = c.clone();
        t.eps = BigInt::from(0);
        !synth_one(&t, &t.z0::<Fq>()).1
    });
    assert!(rejected, "ε = 0 aceito em todos os folds");
}

#[test]
fn binary64_test_cs_agrees() {
    let (_, _, circuits) = setup_circuits_fmt(Format::BINARY64, 3.9, 0.1, 4).unwrap();
    let c = &circuits[1];
    let mut cs = TestConstraintSystem::<Fq>::new();
    let z: Vec<AllocatedNum<Fq>> = c
        .z0::<Fq>()
        .iter()
        .enumerate()
        .map(|(i, v)| AllocatedNum::alloc(cs.namespace(|| format!("z{i}")), || Ok(*v)).unwrap())
        .collect();
    StepCircuit::<Fq>::synthesize(c, &mut cs, &z).unwrap();
    assert!(cs.is_satisfied(), "{:?}", cs.which_is_unsatisfied());
}
