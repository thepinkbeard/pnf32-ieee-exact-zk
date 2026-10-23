//! dyadic-bench constraints --ps 32,64,96,110 --output results/<dir>/dyadic_constraints.json
//! dyadic-bench decisions   --p 64 --r 3.9 --seeds 1000 --nmax 200 --output results/<dir>/dyadic_p64_r3.9.csv
//! dyadic-bench nova        --p 64 --steps 16 --output results/<dir>/dyadic_p64_nova_n16.json
//!
//! `decisions` roda cada fold no circuito (CheckCS) encadeando o estado e grava
//! a decisão por passo, nas sementes do ataque, para comparar com a coluna
//! `reference` de `oracle-case-study/results/h1-20261022/decisions.csv`.

use std::{collections::BTreeMap, fs, path::PathBuf, time::Instant};

use anyhow::{bail, ensure, Result};
use clap::{Parser, Subcommand};
use dyadic_fused::{decision, f_big, is_exact, step, DyadicCircuit, Profile, ARITY};
use nova_snark::{
    frontend::num::AllocatedNum,
    nova::{CompressedSNARK, PublicParams, RecursiveSNARK},
    provider::{ipa_pc::EvaluationEngine, PallasEngine, VestaEngine},
    spartan::snark::RelaxedR1CSSNARK,
    traits::{circuit::StepCircuit, snark::RelaxedR1CSSNARKTrait, Engine},
};
use pnf_circuit::gadgets::{reset_counts, take_counts, to_biguint, CheckCS};
use serde_json::json;

type E1 = PallasEngine;
type E2 = VestaEngine;
type S1 = RelaxedR1CSSNARK<E1, EvaluationEngine<E1>>;
type S2 = RelaxedR1CSSNARK<E2, EvaluationEngine<E2>>;
type Fq = <E1 as Engine>::Scalar;

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    Constraints {
        #[arg(long, value_delimiter = ',', default_value = "32,64,96,110")]
        ps: Vec<u32>,
        #[arg(long)]
        output: PathBuf,
    },
    Decisions {
        #[arg(long)]
        p: u32,
        #[arg(long)]
        r: f32,
        #[arg(long, default_value_t = 1000)]
        seeds: usize,
        #[arg(long, default_value_t = 200)]
        nmax: usize,
        #[arg(long)]
        output: PathBuf,
    },
    Nova {
        #[arg(long)]
        p: u32,
        #[arg(long, default_value_t = 16)]
        steps: usize,
        #[arg(long, default_value_t = 3.9)]
        r: f32,
        #[arg(long, default_value_t = 0.1)]
        x0: f32,
        #[arg(long)]
        output: PathBuf,
    },
}

fn fresh(p: &PathBuf) -> Result<()> {
    if p.exists() {
        bail!("{} já existe; resultados não são sobrescritos", p.display());
    }
    if let Some(d) = p.parent() {
        fs::create_dir_all(d)?;
    }
    Ok(())
}

/// Um fold num CheckCS: (constraints, satisfeito, breakdown, saídas).
fn synth(c: &DyadicCircuit, z: &[Fq]) -> (usize, bool, BTreeMap<&'static str, usize>, Vec<Fq>) {
    let mut cs = CheckCS::<Fq>::default();
    reset_counts();
    let zs: Vec<AllocatedNum<Fq>> = z.iter().map(|v| AllocatedNum::alloc(&mut cs, || Ok(*v)).unwrap()).collect();
    let out = StepCircuit::<Fq>::synthesize(c, &mut cs, &zs).unwrap();
    let br = take_counts();
    (cs.constraints, cs.is_satisfied(), br, out.iter().map(|o| o.get_value().unwrap()).collect())
}

/// Mesmo gerador de sementes do ataque (rounding-steering-attack/src/main.rs).
fn seed_x0(seed: usize) -> f32 {
    if seed == 0 {
        return 0.1;
    }
    let mut s = 0xA5A5_5A5A_1234_5678u64 ^ (seed as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    let mut next = || {
        s ^= s >> 12;
        s ^= s << 25;
        s ^= s >> 27;
        s.wrapping_mul(0x2545_F491_4F6C_DD1D)
    };
    next();
    let u = (next() >> 11) as f64 / (1u64 << 53) as f64;
    (0.01 + 0.98 * u) as f32
}

fn main() -> Result<()> {
    match Cli::parse().cmd {
        Cmd::Constraints { ps, output } => {
            fresh(&output)?;
            let mut rows = Vec::new();
            for p in ps {
                let prof = Profile::new(p);
                let c = DyadicCircuit::new(prof, 3.9f32 as f64, 0.1f32 as f64);
                let (n, ok, br, _) = synth(&c, &c.z0::<Fq>());
                ensure!(ok, "p={p}: witness honesto insatisfeito");
                println!("p={p} s={}: {n} constraints {br:?}", prof.s);
                rows.push(json!({
                    "p": p, "s": prof.s, "max_bits": 2 * p + prof.s + 2,
                    "constraints_per_step": n, "breakdown": br,
                }));
            }
            fs::write(
                &output,
                serde_json::to_vec_pretty(&json!({
                    "workload": "logistic-map + decision x>=1/2",
                    "representation": "ponto fixo diádico fundido (escala 2^-p, piso, 1 arredondamento/passo)",
                    "field": "Pallas scalar", "arity": ARITY, "profiles": rows,
                }))?,
            )?;
        }
        Cmd::Decisions { p, r, seeds, nmax, output } => {
            fresh(&output)?;
            let prof = Profile::new(p);
            ensure!(is_exact(r as f64, prof.s), "fl32(r) não é exato com s={}", prof.s);
            let mut csv = String::from("r,seed,N,decision\n");
            for seed in 0..=seeds {
                let x0 = seed_x0(seed) as f64;
                ensure!(is_exact(x0, p), "semente {seed}: x0 não é exato com p={p}");
                let c = DyadicCircuit::new(prof, r as f64, x0);
                ensure!(c.z0_is_valid());
                let mut z = c.z0::<Fq>();
                let mut x = c.x.clone();
                csv += &format!("{r},{seed},0,{}\n", decision(prof, &x) as u8);
                for (n, f) in c.trajectory(nmax).iter().enumerate() {
                    let (_, ok, _, out) = synth(f, &z);
                    ensure!(ok, "semente {seed}, fold {n}: insatisfeito");
                    x = step(prof, &x, &c.r);
                    ensure!(to_biguint(out[1]) == x, "semente {seed}, fold {n}: estado difere da referência");
                    let d = to_biguint(out[2]);
                    ensure!(d == (decision(prof, &x) as u32).into(), "semente {seed}, fold {n}: decisão difere");
                    csv += &format!("{r},{seed},{},{d}\n", n + 1);
                    z = out;
                }
            }
            fs::write(&output, csv)?;
        }
        Cmd::Nova { p, steps, r, x0, output } => {
            fresh(&output)?;
            let prof = Profile::new(p);
            let c0 = DyadicCircuit::new(prof, r as f64, x0 as f64);
            let circuits = c0.trajectory(steps);
            let z0 = c0.z0::<Fq>();
            let t = Instant::now();
            let pp = PublicParams::<E1, E2, DyadicCircuit>::setup(&circuits[0], &*S1::ck_floor(), &*S2::ck_floor())?;
            let setup = t.elapsed().as_secs_f64();
            let t = Instant::now();
            let mut rs = RecursiveSNARK::<E1, E2, DyadicCircuit>::new(&pp, &circuits[0], &z0)?;
            for c in &circuits {
                rs.prove_step(&pp, c)?;
            }
            let prove = t.elapsed().as_secs_f64();
            let zn = rs.verify(&pp, steps, &z0)?;
            let mut x = c0.x.clone();
            for _ in 0..steps {
                x = step(prof, &x, &c0.r);
            }
            ensure!(zn[1] == f_big::<Fq>(&x), "estado final difere da referência");
            let t = Instant::now();
            let (pk, vk) = CompressedSNARK::<E1, E2, DyadicCircuit, S1, S2>::setup(&pp)?;
            let csetup = t.elapsed().as_secs_f64();
            let t = Instant::now();
            let proof = CompressedSNARK::<E1, E2, DyadicCircuit, S1, S2>::prove(&pp, &pk, &rs)?;
            let cprove = t.elapsed().as_secs_f64();
            let bytes = bincode::serde::encode_to_vec(&proof, bincode::config::legacy())?.len();
            let t = Instant::now();
            proof.verify(&vk, steps, &z0)?;
            let cverify = t.elapsed().as_secs_f64();
            let (pc, sc) = pp.num_constraints();
            let doc = json!({
                "p": p, "s": prof.s, "steps": steps, "r": r, "x0": x0,
                "primary_constraints": pc, "secondary_constraints": sc,
                "setup_seconds": setup, "prover_seconds": prove,
                "compression_setup_seconds": csetup, "compression_seconds": cprove,
                "compressed_verify_seconds": cverify, "compressed_proof_bytes": bytes,
                "note": "tempos de uma rodada só (smoke); 1 passo por fold",
            });
            println!("{}", serde_json::to_string_pretty(&doc)?);
            fs::write(&output, serde_json::to_vec_pretty(&doc)?)?;
        }
    }
    Ok(())
}
