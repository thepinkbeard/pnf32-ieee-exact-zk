//! IVC Nova dos baselines FP32 restrito / CBD-32 com S passos por fold,
//! com as mesmas métricas de `pnf-bench nova --steps-per-fold`.
//!
//! ```text
//! spf-baselines --repr fp32 --steps 512 --steps-per-fold 16 --compress --output results/<dir>/x.json
//! ```

use std::{fs, path::PathBuf, time::Instant};

use anyhow::{bail, Context, Result};
use clap::{Parser, ValueEnum};
use nova_snark::{
    frontend::{num::AllocatedNum, test_cs::TestConstraintSystem, ConstraintSystem},
    nova::{CompressedSNARK, PublicParams, RecursiveSNARK},
    provider::{ipa_pc::EvaluationEngine, PallasEngine, VestaEngine},
    spartan::snark::RelaxedR1CSSNARK,
    traits::{circuit::StepCircuit, snark::RelaxedR1CSSNARKTrait, Engine},
};
use serde_json::{json, Value};
use spf_baselines::*;

type E1 = PallasEngine;
type E2 = VestaEngine;
type S1 = RelaxedR1CSSNARK<E1, EvaluationEngine<E1>>;
type S2 = RelaxedR1CSSNARK<E2, EvaluationEngine<E2>>;
type Fq = <E1 as Engine>::Scalar;

#[derive(Clone, Copy, ValueEnum)]
enum ReprArg {
    Fp32,
    Cbd32,
}

#[derive(Parser)]
#[command(about = "Baselines FP32/CBD-32 com vários passos logísticos por fold Nova")]
struct Cli {
    #[arg(long, value_enum)]
    repr: ReprArg,
    /// Total de passos logísticos (múltiplo de `--steps-per-fold`).
    #[arg(long, default_value_t = 10)]
    steps: usize,
    #[arg(long, default_value_t = 1)]
    steps_per_fold: usize,
    #[arg(long)]
    compress: bool,
    #[arg(long, default_value_t = 1)]
    reps: usize,
    #[arg(long)]
    output: PathBuf,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let (s, n) = (cli.steps_per_fold, cli.steps);
    if s == 0 || n == 0 || n % s != 0 {
        bail!("--steps ({n}) tem que ser múltiplo positivo de --steps-per-fold ({s})");
    }
    if cli.output.exists() {
        bail!(
            "{} já existe; resultados nunca são sobrescritos",
            cli.output.display()
        );
    }
    if let Some(dir) = cli.output.parent() {
        fs::create_dir_all(dir)?;
    }
    let w = Workload::default();
    let (repr, doc) = match cli.repr {
        ReprArg::Fp32 => {
            let (steps, z0) = fp32_trajectory(&w, n)?;
            let expect = fp32_expected(steps.last().unwrap(), n as u64);
            let mut native = w.x0_f32();
            for _ in 0..n {
                native = (native * (1.0 - native)) * w.r_f32();
            }
            if steps.last().unwrap().witness.output.bits() != native.to_bits() {
                bail!("referência FP32 difere do f32 nativo");
            }
            let f = |v: &[u64]| v.iter().map(|&x| Fq::from(x)).collect::<Vec<_>>();
            (Repr::Fp32, run(&cli, steps, &f(&z0), &f(&expect))?)
        }
        ReprArg::Cbd32 => {
            let (steps, z0) = cbd_trajectory(&w, n);
            let expect = cbd_expected(steps.last().unwrap(), n as u64);
            let f = |v: &[u128]| v.iter().map(|&x| field_u128::<Fq>(x)).collect::<Vec<_>>();
            (Repr::Cbd32, run(&cli, steps, &f(&z0), &f(&expect))?)
        }
    };
    let mut doc = doc;
    doc["representation"] = json!(repr.label());
    doc["r"] = json!(format!("{}/{}", w.r_num, w.r_den));
    doc["x0"] = json!(format!("{}/{}", w.x0_num, w.x0_den));
    println!("{}", serde_json::to_string_pretty(&doc)?);
    fs::write(&cli.output, serde_json::to_vec_pretty(&doc)?)?;
    Ok(())
}

fn raw_constraints<C: StepCircuit<Fq>>(c: &C, z0: &[Fq]) -> usize {
    let mut cs = TestConstraintSystem::<Fq>::new();
    let z: Vec<AllocatedNum<Fq>> = z0
        .iter()
        .enumerate()
        .map(|(i, v)| AllocatedNum::alloc(cs.namespace(|| format!("z{i}")), || Ok(*v)).unwrap())
        .collect();
    c.synthesize(&mut cs, &z).expect("síntese");
    assert!(cs.is_satisfied(), "{:?}", cs.which_is_unsatisfied());
    cs.num_constraints()
}

fn run<C>(cli: &Cli, per_step: Vec<C>, z0: &[Fq], expect: &[Fq]) -> Result<Value>
where
    C: StepCircuit<Fq> + Clone,
{
    let (steps, s) = (cli.steps, cli.steps_per_fold);
    let raw = raw_constraints(&per_step[0], z0);
    let circuits = group(per_step, s);
    let folds = circuits.len();
    let raw_fold = raw_constraints(&circuits[0], z0);
    let mut runs = Vec::new();
    for rep in 0..cli.reps {
        let t = Instant::now();
        let pp = PublicParams::<E1, E2, MultiStep<C>>::setup(
            &circuits[0],
            &*S1::ck_floor(),
            &*S2::ck_floor(),
        )
        .context("setup")?;
        let setup = t.elapsed().as_secs_f64();
        // `new` prova o primeiro fold; o primeiro `prove_step` é no-op.
        let t = Instant::now();
        let mut rs = RecursiveSNARK::<E1, E2, MultiStep<C>>::new(&pp, &circuits[0], z0)?;
        let rs_new = t.elapsed().as_secs_f64();
        let t = Instant::now();
        for c in &circuits {
            rs.prove_step(&pp, c)?;
        }
        let prove = t.elapsed().as_secs_f64();
        let prover_ivc = rs_new + prove;
        let t = Instant::now();
        let zn = rs.verify(&pp, folds, z0)?;
        let verify = t.elapsed().as_secs_f64();
        if zn != expect {
            bail!("estado final difere da referência de {steps} passos");
        }

        let mut comp = json!(null);
        if cli.compress {
            let t = Instant::now();
            let (pk, vk) = CompressedSNARK::<E1, E2, MultiStep<C>, S1, S2>::setup(&pp)?;
            let csetup = t.elapsed().as_secs_f64();
            let t = Instant::now();
            let proof = CompressedSNARK::<E1, E2, MultiStep<C>, S1, S2>::prove(&pp, &pk, &rs)?;
            let cprove = t.elapsed().as_secs_f64();
            let bytes = bincode::serde::encode_to_vec(&proof, bincode::config::legacy())?.len();
            let t = Instant::now();
            proof.verify(&vk, folds, z0)?;
            let cverify = t.elapsed().as_secs_f64();
            comp = json!({"compression_setup_seconds": csetup, "compression_seconds": cprove,
                          "compressed_verify_seconds": cverify, "compressed_proof_bytes_bincode": bytes});
        }
        let (p, sc) = pp.num_constraints();
        let (pv, sv) = pp.num_variables();
        eprintln!(
            "rep {rep}: S={s} prover {prover_ivc:.4}s ({:.6}s/passo) verify {verify:.4}s {comp}",
            prover_ivc / steps as f64
        );
        runs.push(json!({
            "rep": rep, "primary_constraints": p, "secondary_constraints": sc,
            "primary_variables": pv, "secondary_variables": sv,
            "setup_seconds": setup,
            "recursive_new_seconds": rs_new, "prove_step_loop_seconds": prove,
            "prover_ivc_seconds": prover_ivc,
            "prover_seconds_per_logistic_step": prover_ivc / steps as f64,
            "prover_seconds_per_fold": prover_ivc / folds as f64,
            "steady_fold_seconds": (folds > 1).then(|| prove / (folds - 1) as f64),
            "steady_seconds_per_logistic_step":
                (folds > 1).then(|| prove / ((folds - 1) * s) as f64),
            "recursive_verify_seconds": verify, "compressed": comp,
        }));
    }
    Ok(json!({
        "workload": "logistic-map", "steps": steps, "steps_per_fold": s, "folds": folds,
        "threshold": "1/2", "raw_step_constraints": raw, "raw_fold_constraints": raw_fold,
        "state_field_elements": z0.len(),
        "source": "proof-numeric-zk/crates/nova-circuit (path dependency, sem modificação)",
        "backend": "nova-snark 0.73.0, Pallas/Vesta, IPA", "peak_rss_bytes": peak_rss(),
        "runs": runs,
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
    Some(if cfg!(target_os = "macos") {
        max
    } else {
        max * 1024
    })
}
