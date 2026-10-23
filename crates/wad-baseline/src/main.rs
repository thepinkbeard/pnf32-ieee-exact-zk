//! wad-bench constraints --output results/<dir>/wad_constraints.json
//! wad-bench decisions   --r 3.9 --seeds 1000 --nmax 200 --output results/<dir>/wad_r3.9.csv
//!
//! `decisions` roda cada fold no circuito (CheckCS) encadeando o estado e grava
//! a decisão por passo, nas sementes do ataque, para comparar com
//! `oracle-case-study/scripts/h1_divergence.py` (coluna "WAD 18 casas, piso").

use std::{collections::BTreeMap, fs, path::PathBuf};

use anyhow::{bail, ensure, Result};
use clap::{Parser, Subcommand};
use nova_snark::{
    frontend::num::AllocatedNum,
    provider::PallasEngine,
    traits::{circuit::StepCircuit, Engine},
};
use pnf_circuit::gadgets::{reset_counts, take_counts, to_biguint, CheckCS};
use serde_json::json;
use wad_baseline::{step, WadCircuit, ARITY, W};

type Fq = <PallasEngine as Engine>::Scalar;

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    Constraints {
        #[arg(long)]
        output: PathBuf,
    },
    Decisions {
        #[arg(long)]
        r: f32,
        #[arg(long, default_value_t = 1000)]
        seeds: usize,
        #[arg(long, default_value_t = 200)]
        nmax: usize,
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
fn synth(c: &WadCircuit, z: &[Fq]) -> (usize, bool, BTreeMap<&'static str, usize>, Vec<Fq>) {
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
        Cmd::Constraints { output } => {
            fresh(&output)?;
            let c = WadCircuit::new(3.9f32 as f64, 0.1f32 as f64);
            let (n, ok, br, _) = synth(&c, &c.z0::<Fq>());
            ensure!(ok, "witness honesto insatisfeito");
            println!("WAD step + decision: {n} constraints {br:?}");
            fs::write(
                &output,
                serde_json::to_vec_pretty(&json!({
                    "workload": "logistic-map", "representation": "WAD (10^18, mulWad piso)",
                    "field": "Pallas scalar", "arity": ARITY,
                    "constraints_per_step": n, "breakdown": br,
                }))?,
            )?;
        }
        Cmd::Decisions { r, seeds, nmax, output } => {
            fresh(&output)?;
            let mut csv = String::from("r,seed,N,decision\n");
            for seed in 0..=seeds {
                let c = WadCircuit::new(r as f64, seed_x0(seed) as f64);
                ensure!(c.z0_is_valid());
                let mut z = c.z0::<Fq>();
                let mut x = c.x;
                csv += &format!("{r},{seed},0,{}\n", (x >= W / 2) as u8);
                for (n, f) in c.trajectory(nmax).iter().enumerate() {
                    let (_, ok, _, out) = synth(f, &z);
                    ensure!(ok, "semente {seed}, fold {n}: insatisfeito");
                    x = step(x, c.r);
                    ensure!(to_biguint(out[1]) == x.into(), "semente {seed}, fold {n}: estado difere da referência");
                    csv += &format!("{r},{seed},{},{}\n", n + 1, to_biguint(out[2]));
                    z = out;
                }
            }
            fs::write(&output, csv)?;
        }
    }
    Ok(())
}
