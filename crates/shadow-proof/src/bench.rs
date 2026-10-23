//! Medições do circuito: constraints por fold e IVC Nova curto.

use std::{fs, path::PathBuf, time::Instant};

use anyhow::{bail, Result};
use nova_snark::{
    frontend::num::AllocatedNum,
    nova::{CompressedSNARK, PublicParams, RecursiveSNARK},
    provider::{ipa_pc::EvaluationEngine, PallasEngine, VestaEngine},
    spartan::snark::RelaxedR1CSSNARK,
    traits::{circuit::StepCircuit, snark::RelaxedR1CSSNARKTrait, Engine},
};
use pnf_circuit::gadgets::{reset_counts, take_counts, CheckCS};
use pnf_reference::Pnf;
use serde_json::json;

use crate::{
    cert::{build, check, ieee_orbit, Map},
    circuit::{grid_bits_for, ShadowCircuit, SHADOW_ARITY},
};
use pnf_reference::Format;

type E1 = PallasEngine;
type E2 = VestaEngine;
type S1 = RelaxedR1CSSNARK<E1, EvaluationEngine<E1>>;
type S2 = RelaxedR1CSSNARK<E2, EvaluationEngine<E2>>;
pub type Fq = <E1 as Engine>::Scalar;

fn fresh(p: &PathBuf) -> Result<()> {
    if p.exists() {
        bail!("{} já existe; resultados não são sobrescritos", p.display());
    }
    if let Some(d) = p.parent() {
        fs::create_dir_all(d)?;
    }
    Ok(())
}

/// Circuitos e ε para `steps` folds a partir de `(r, x0)` em binary32.
pub fn setup_circuits(r: f64, x0: f64, steps: usize) -> Result<(Map, Vec<f64>, Vec<ShadowCircuit>)> {
    setup_circuits_fmt(Format::BINARY32, r, x0, steps)
}

/// Como [`setup_circuits`], em binary32 ou binary64.
pub fn setup_circuits_fmt(
    fmt: Format,
    r: f64,
    x0: f64,
    steps: usize,
) -> Result<(Map, Vec<f64>, Vec<ShadowCircuit>)> {
    let b32 = fmt == Format::BINARY32;
    let (rr, xx) = if b32 { (r as f32 as f64, x0 as f32 as f64) } else { (r, x0) };
    let map = Map::from_f64(rr, grid_bits_for(fmt));
    let orbit = ieee_orbit(b32, rr, xx, steps);
    let cert = build(&map, &orbit).map_err(|f| anyhow::anyhow!("sem certificado: {f:?}"))?;
    check(&map, &orbit, &cert, &cert.max_dist).map_err(anyhow::Error::msg)?;
    let circuits = ShadowCircuit::trajectory_fmt(fmt, &map, rr, &orbit, &cert, &cert.max_dist);
    Ok((map, orbit, circuits))
}

fn pnf_of(fmt: Format, x: f64) -> Pnf {
    if fmt == Format::BINARY32 {
        Pnf::from_f32(x as f32)
    } else {
        Pnf::from_f64(x)
    }
}

/// Sintetiza um fold num CheckCS; devolve (constraints, satisfeito, breakdown, saídas).
pub fn synth_one(c: &ShadowCircuit, z: &[Fq]) -> (usize, bool, std::collections::BTreeMap<&'static str, usize>, Vec<Fq>) {
    let mut cs = CheckCS::<Fq>::default();
    reset_counts();
    let zs: Vec<AllocatedNum<Fq>> =
        z.iter().map(|v| AllocatedNum::alloc(&mut cs, || Ok(*v)).unwrap()).collect();
    let out = StepCircuit::<Fq>::synthesize(c, &mut cs, &zs).unwrap();
    let br = take_counts();
    let vals = out.iter().map(|o| o.get_value().unwrap()).collect();
    (cs.constraints, cs.is_satisfied(), br, vals)
}

pub fn constraints(fmt: Format, out: PathBuf) -> Result<()> {
    fresh(&out)?;
    let (_, _, circuits) = setup_circuits_fmt(fmt, 3.9, 0.1, 8)?;
    let (n, ok, br, _) = synth_one(&circuits[0], &circuits[0].z0::<Fq>());
    if !ok {
        bail!("witness honesto insatisfeito");
    }
    let total_shadow: usize = br.iter().filter(|(k, _)| !k.starts_with("pnf")).map(|(_, v)| *v).sum();
    println!("constraints por fold: {n}  breakdown: {br:?}");
    fs::write(
        &out,
        serde_json::to_vec_pretty(&json!({
            "circuit": "PNF logistic step + HYG nested-interval shadowing check + verdict",
            "format": if fmt == Format::BINARY32 { "binary32" } else { "binary64" },
            "grid_bits_K": grid_bits_for(fmt), "arity": SHADOW_ARITY, "constraints_per_fold": n,
            "breakdown_by_category": br, "non_pnf_part_approx": total_shadow,
        }))?,
    )?;
    Ok(())
}

pub fn nova(fmt: Format, steps: usize, r: f64, x0: f64, out: PathBuf) -> Result<()> {
    fresh(&out)?;
    let (map, orbit, circuits) = setup_circuits_fmt(fmt, r, x0, steps)?;
    let z0 = circuits[0].z0::<Fq>();
    let t = Instant::now();
    let pp = PublicParams::<E1, E2, ShadowCircuit>::setup(&circuits[0], &*S1::ck_floor(), &*S2::ck_floor())?;
    let setup = t.elapsed().as_secs_f64();
    let t = Instant::now();
    let mut rs = RecursiveSNARK::<E1, E2, ShadowCircuit>::new(&pp, &circuits[0], &z0)?;
    for c in &circuits {
        rs.prove_step(&pp, c)?;
    }
    let prove = t.elapsed().as_secs_f64();
    let t = Instant::now();
    let zn = rs.verify(&pp, steps, &z0)?;
    let verify = t.elapsed().as_secs_f64();
    // Estado final: p_N bit-exato ao f32 nativo e I_N do certificado.
    let last = circuits.last().unwrap();
    let expect = last.expected_next::<Fq>(steps as u64, pnf_of(fmt, orbit[steps]), 0);
    if zn[..SHADOW_ARITY - 1] != expect[..SHADOW_ARITY - 1] {
        bail!("estado final difere do esperado");
    }
    let t = Instant::now();
    let (pk, vk) = CompressedSNARK::<E1, E2, ShadowCircuit, S1, S2>::setup(&pp)?;
    let csetup = t.elapsed().as_secs_f64();
    let t = Instant::now();
    let proof = CompressedSNARK::<E1, E2, ShadowCircuit, S1, S2>::prove(&pp, &pk, &rs)?;
    let cprove = t.elapsed().as_secs_f64();
    let bytes = bincode::serde::encode_to_vec(&proof, bincode::config::legacy())?.len();
    let t = Instant::now();
    proof.verify(&vk, steps, &z0)?;
    let cverify = t.elapsed().as_secs_f64();
    let (pc, sc) = pp.num_constraints();
    let doc = json!({
        "steps": steps, "r": r, "x0": x0, "grid_bits_K": grid_bits_for(fmt),
        "format": if fmt == Format::BINARY32 { "binary32" } else { "binary64" },
        "eps": map.grid_f64(&circuits[0].eps),
        "primary_constraints": pc, "secondary_constraints": sc,
        "setup_seconds": setup, "prover_seconds": prove, "recursive_verify_seconds": verify,
        "compression_setup_seconds": csetup, "compression_seconds": cprove,
        "compressed_verify_seconds": cverify, "compressed_proof_bytes": bytes,
        "final_x_bits": format!("{:#x}", pnf_of(fmt, orbit[steps]).to_bits(fmt)),
        "note": "tempos de uma rodada só (smoke); máquina compartilhada",
    });
    println!("{}", serde_json::to_string_pretty(&doc)?);
    fs::write(&out, serde_json::to_vec_pretty(&doc)?)?;
    Ok(())
}
