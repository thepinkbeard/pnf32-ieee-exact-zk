//! Perfil de um fold Nova por fase, para PNF32 / FP32 / CBD-32 com S passos
//! por fold (log 0006). Usa a cópia instrumentada de nova-snark
//! (`nova_snark::profile`).
//!
//! Por medição: setup, `new`, o `prove_step` no-op, e então `--folds` folds
//! estacionários cronometrados (censo desligado). Depois, um fold extra com o
//! censo de larguras ligado, fora dos tempos.
//!
//! ```text
//! spf-profile --repr fp32 --steps-per-fold 64 --folds 8 --output results/<dir>/x.json
//! ```

use std::{fs, path::PathBuf, time::Instant};

use anyhow::{bail, Context, Result};
use clap::{Parser, ValueEnum};
use ff::PrimeField;
use nova_snark::{
    frontend::{num::AllocatedNum, solver::SatisfyingAssignment, ConstraintSystem},
    nova::{PublicParams, RecursiveSNARK},
    profile,
    provider::{ipa_pc::EvaluationEngine, PallasEngine, VestaEngine},
    spartan::snark::RelaxedR1CSSNARK,
    traits::{circuit::StepCircuit, snark::RelaxedR1CSSNARKTrait, Engine},
};
use pnf_circuit::step::LogisticCircuit;
use serde_json::{json, Value};
use spf_baselines::*;

type E1 = PallasEngine;
type E2 = VestaEngine;
type S1 = RelaxedR1CSSNARK<E1, EvaluationEngine<E1>>;
type S2 = RelaxedR1CSSNARK<E2, EvaluationEngine<E2>>;
type Fq = <E1 as Engine>::Scalar;

#[derive(Clone, Copy, ValueEnum)]
enum ReprArg {
    Pnf32,
    Fp32,
    Cbd32,
}

#[derive(Parser)]
struct Cli {
    #[arg(long, value_enum)]
    repr: ReprArg,
    #[arg(long)]
    steps_per_fold: usize,
    /// Folds estacionários cronometrados.
    #[arg(long, default_value_t = 8)]
    folds: usize,
    #[arg(long)]
    output: PathBuf,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    if cli.output.exists() {
        bail!("{} já existe; resultados nunca são sobrescritos", cli.output.display());
    }
    if let Some(d) = cli.output.parent() {
        fs::create_dir_all(d)?;
    }
    let (s, f) = (cli.steps_per_fold, cli.folds);
    // folds: 0 (new), 1..=f cronometrados, f+1 com censo.
    let n_folds = f + 2;
    let n = s * n_folds;
    let w = Workload::default();
    let (label, mut doc) = match cli.repr {
        ReprArg::Pnf32 => {
            let base = LogisticCircuit {
                steps_per_fold: s,
                ..LogisticCircuit::new(w.r_f32(), w.x0_f32())
            };
            let circuits = base.trajectory(n_folds);
            ("pnf32", run(&circuits, &base.z0::<Fq>(), f)?)
        }
        ReprArg::Fp32 => {
            let (steps, z0) = fp32_trajectory(&w, n)?;
            let z0: Vec<Fq> = z0.into_iter().map(Fq::from).collect();
            ("fp32", run(&group(steps, s), &z0, f)?)
        }
        ReprArg::Cbd32 => {
            let (steps, z0) = cbd_trajectory(&w, n);
            let z0: Vec<Fq> = z0.into_iter().map(field_u128::<Fq>).collect();
            ("cbd32", run(&group(steps, s), &z0, f)?)
        }
    };
    doc["representation"] = json!(label);
    doc["steps_per_fold"] = json!(s);
    doc["timed_folds"] = json!(f);
    doc["peak_rss_bytes"] = json!(peak_rss());
    println!("{}", serde_json::to_string_pretty(&doc)?);
    fs::write(&cli.output, serde_json::to_vec_pretty(&doc)?)?;
    Ok(())
}

/// Síntese só do circuito de passo (sem o circuito aumentado do Nova), no
/// mesmo `SatisfyingAssignment` do prover; mediana de 5.
fn step_synthesis_seconds<C: StepCircuit<Fq>>(c: &C, z0: &[Fq]) -> f64 {
    let mut ts: Vec<f64> = (0..5)
        .map(|_| {
            let mut cs = SatisfyingAssignment::<E1>::new();
            let z: Vec<AllocatedNum<Fq>> = z0
                .iter()
                .enumerate()
                .map(|(i, v)| {
                    AllocatedNum::alloc(cs.namespace(|| format!("z{i}")), || Ok(*v)).unwrap()
                })
                .collect();
            let t = Instant::now();
            c.synthesize(&mut cs, &z).expect("síntese");
            t.elapsed().as_secs_f64()
        })
        .collect();
    ts.sort_by(f64::total_cmp);
    ts[2]
}

fn run<C: StepCircuit<Fq> + Clone>(circuits: &[C], z0: &[Fq], f: usize) -> Result<Value> {
    profile::enable_census(false);
    let step_synth = step_synthesis_seconds(&circuits[0], z0);
    let t = Instant::now();
    let pp = PublicParams::<E1, E2, C>::setup(&circuits[0], &*S1::ck_floor(), &*S2::ck_floor())
        .context("setup")?;
    let setup = t.elapsed().as_secs_f64();
    let t = Instant::now();
    let mut rs = RecursiveSNARK::<E1, E2, C>::new(&pp, &circuits[0], z0)?;
    let rs_new = t.elapsed().as_secs_f64();
    rs.prove_step(&pp, &circuits[0])?; // no-op em Nova 0.73
    let _ = profile::take();
    let t = Instant::now();
    for c in &circuits[1..=f] {
        rs.prove_step(&pp, c)?;
    }
    let wall = t.elapsed().as_secs_f64();
    let (times, _) = profile::take();
    profile::enable_census(true);
    rs.prove_step(&pp, &circuits[f + 1])?;
    profile::enable_census(false);
    let (_, census) = profile::take();
    rs.verify(&pp, f + 2, z0).context("verify")?;

    let phases: serde_json::Map<String, Value> = times
        .iter()
        .map(|(k, (secs, calls))| {
            (k.clone(), json!({"seconds_per_fold": secs / f as f64, "calls": calls}))
        })
        .collect();
    let census: serde_json::Map<String, Value> = census
        .iter()
        .map(|(k, c)| {
            let total: u64 = c.iter().sum();
            let m: serde_json::Map<String, Value> =
                profile::GROUPS.iter().zip(c).map(|(g, n)| (g.to_string(), json!(n))).collect();
            (k.clone(), json!({"len": total, "groups": m}))
        })
        .collect();
    let (p, sc) = pp.num_constraints();
    let (pv, sv) = pp.num_variables();
    Ok(json!({
        "primary_constraints": p, "secondary_constraints": sc,
        "primary_variables": pv, "secondary_variables": sv,
        "setup_seconds": setup, "recursive_new_seconds": rs_new,
        "steady_wall_seconds_per_fold": wall / f as f64,
        "step_only_synthesis_seconds": step_synth,
        "phases": phases, "census": census,
        "field_bits": Fq::NUM_BITS,
        "backend": "nova-snark 0.73.0 INSTRUMENTADO (vendor/nova-snark-0.73.0-prof)",
    }))
}

fn peak_rss() -> Option<u64> {
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::uninit();
    // SAFETY: ponteiro válido e gravável para getrusage.
    let rc = unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) };
    if rc != 0 {
        return None;
    }
    // SAFETY: getrusage retornou sucesso e inicializou a estrutura.
    let max = unsafe { usage.assume_init() }.ru_maxrss as u64;
    Some(if cfg!(target_os = "macos") { max } else { max * 1024 })
}
