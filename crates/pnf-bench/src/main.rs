//! Medições da PNF32. Cada subcomando grava JSON/CSV num arquivo novo.
//!
//! ```text
//! pnf-bench constraints --output results/<dir>/constraints.json
//! pnf-bench nova --steps 10 --compress --output results/<dir>/nova.json
//! pnf-bench nova --steps 256 --steps-per-fold 16 --compress --output results/<dir>/nova_s16.json
//! pnf-bench trajectory --r 3.9 --n 1000 --output results/<dir>/traj_r3.9.csv
//! pnf-bench sweep --output results/<dir>/sweep.json
//! ```

use std::{collections::BTreeMap, fs, path::PathBuf, time::Instant};

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use nova_snark::{
    frontend::num::AllocatedNum,
    nova::{CompressedSNARK, PublicParams, RecursiveSNARK},
    provider::{ipa_pc::EvaluationEngine, PallasEngine, VestaEngine},
    spartan::snark::RelaxedR1CSSNARK,
    traits::{circuit::StepCircuit, snark::RelaxedR1CSSNARKTrait, Engine},
};
use pnf_circuit::{
    gadgets::{reset_counts, take_counts, CheckCS},
    ops,
    pnf::PnfVar,
    step::{logistic, LogisticCircuit, STEP_ARITY},
};
use pnf_reference::{self as reference, Format, Pnf};
use serde_json::json;

type E1 = PallasEngine;
type E2 = VestaEngine;
type S1 = RelaxedR1CSSNARK<E1, EvaluationEngine<E1>>;
type S2 = RelaxedR1CSSNARK<E2, EvaluationEngine<E2>>;
type Fq = <E1 as Engine>::Scalar;
const B32: Format = Format::BINARY32;

#[derive(Parser)]
#[command(about = "Medições da PNF32 (binary32 bit-exato em R1CS/Nova)")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Contagem de constraints por operação e por passo, com breakdown.
    Constraints {
        /// `b32` (PNF32) ou `b64` (PNF64).
        #[arg(long, default_value = "b32")]
        format: String,
        #[arg(long)]
        output: PathBuf,
    },
    /// IVC Nova do mapa logístico (mesmas métricas do baseline).
    Nova {
        /// Total de passos logísticos (múltiplo de `--steps-per-fold`).
        #[arg(long, default_value_t = 10)]
        steps: usize,
        /// Passos logísticos encadeados em cada fold Nova.
        #[arg(long, default_value_t = 1)]
        steps_per_fold: usize,
        #[arg(long, default_value_t = 3.7)]
        r: f32,
        #[arg(long, default_value_t = 0.1)]
        x0: f32,
        #[arg(long)]
        compress: bool,
        #[arg(long)]
        validate_input: bool,
        #[arg(long)]
        r_variable: bool,
        #[arg(long, default_value_t = 1)]
        reps: usize,
        #[arg(long)]
        output: PathBuf,
    },
    /// Trajetória no circuito (CheckCS) contra f32 nativo, passo a passo.
    Trajectory {
        #[arg(long)]
        r: f32,
        #[arg(long, default_value_t = 0.1)]
        x0: f32,
        #[arg(long, default_value_t = 1000)]
        n: usize,
        #[arg(long)]
        output: PathBuf,
    },
    /// Varredura exaustiva 2^32 (off-circuit): 1-x, x*r e decode/encode.
    Sweep {
        #[arg(long, default_value_t = 3.9)]
        r: f32,
        #[arg(long)]
        output: PathBuf,
    },
}

fn main() -> Result<()> {
    match Cli::parse().cmd {
        Cmd::Constraints { format, output } => constraints(&format, output),
        Cmd::Nova {
            steps,
            steps_per_fold,
            r,
            x0,
            compress,
            validate_input,
            r_variable,
            reps,
            output,
        } => nova(
            steps,
            steps_per_fold,
            r,
            x0,
            compress,
            validate_input,
            r_variable,
            reps,
            output,
        ),
        Cmd::Trajectory { r, x0, n, output } => trajectory(r, x0, n, output),
        Cmd::Sweep { r, output } => sweep(r, output),
    }
}

fn new_file(path: &PathBuf) -> Result<()> {
    if path.exists() {
        bail!(
            "{} já existe; resultados nunca são sobrescritos",
            path.display()
        );
    }
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------

fn measure(f: impl FnOnce(&mut CheckCS<Fq>)) -> serde_json::Value {
    let mut cs = CheckCS::<Fq>::default();
    reset_counts();
    f(&mut cs);
    let breakdown = take_counts();
    assert!(cs.is_satisfied(), "witness honesto insatisfeito na medição");
    json!({"constraints": cs.constraints, "breakdown": breakdown})
}

fn step_constraints(c: &LogisticCircuit) -> serde_json::Value {
    measure(|cs| {
        let z: Vec<AllocatedNum<Fq>> = c
            .z0::<Fq>()
            .into_iter()
            .map(|v| AllocatedNum::alloc(&mut *cs, || Ok(v)).unwrap())
            .collect();
        assert_eq!(z.len(), STEP_ARITY);
        StepCircuit::<Fq>::synthesize(c, cs, &z).unwrap();
    })
}

fn constraints(format: &str, output: PathBuf) -> Result<()> {
    let b64 = match format {
        "b32" => false,
        "b64" => true,
        _ => anyhow::bail!("formato: b32 ou b64"),
    };
    new_file(&output)?;
    let fmt = if b64 { Format::BINARY64 } else { B32 };
    let pf = |v: f32| if b64 { Pnf::from_f64(v as f64) } else { Pnf::from_f32(v) };
    let (a, b) = (pf(0.3), pf(0.7));
    let r = pf(3.7);
    let mut table = BTreeMap::new();
    let var2 = |cs: &mut CheckCS<Fq>| {
        (
            PnfVar::alloc_unchecked(cs, fmt, a),
            PnfVar::alloc_unchecked(cs, fmt, b),
        )
    };
    table.insert(
        "add (var,var)",
        measure(|cs| {
            let (x, y) = var2(cs);
            ops::add(cs, fmt, &x, &y, None);
        }),
    );
    table.insert(
        "sub (var,var)",
        measure(|cs| {
            let (x, y) = var2(cs);
            ops::sub(cs, fmt, &x, &y, None);
        }),
    );
    table.insert(
        "mul (var,var)",
        measure(|cs| {
            let (x, y) = var2(cs);
            ops::mul(cs, fmt, &x, &y, None);
        }),
    );
    table.insert(
        "1 - x (const,var)",
        measure(|cs| {
            let x = PnfVar::alloc_unchecked(cs, fmt, a);
            let one = PnfVar::constant(fmt, reference::one(fmt));
            ops::sub(cs, fmt, &one, &x, None);
        }),
    );
    table.insert(
        "x * r (var,const)",
        measure(|cs| {
            let x = PnfVar::alloc_unchecked(cs, fmt, a);
            ops::mul(cs, fmt, &x, &PnfVar::constant(fmt, r), None);
        }),
    );
    table.insert(
        "decision x >= 0.5",
        measure(|cs| {
            let x = PnfVar::alloc_unchecked(cs, fmt, a);
            ops::ge_pow2(cs, fmt, &x, -1, None);
        }),
    );
    table.insert(
        "validate (T2 ablation)",
        measure(|cs| {
            let x = PnfVar::alloc_unchecked(cs, fmt, a);
            x.validate(cs, fmt);
        }),
    );
    table.insert(
        "logistic only (1-x, x*c, p*r)",
        measure(|cs| {
            let x = PnfVar::alloc_unchecked(cs, fmt, pf(0.1));
            logistic(cs, fmt, &x, &PnfVar::constant(fmt, r));
        }),
    );
    let base = if b64 {
        LogisticCircuit::new_f64(3.7, 0.1)
    } else {
        LogisticCircuit::new(3.7, 0.1)
    };
    table.insert(
        "STEP logistic + decision (T2 on, r const)",
        step_constraints(&base),
    );
    table.insert(
        "STEP ablation: validate input every fold",
        step_constraints(&LogisticCircuit {
            validate_input: true,
            ..base.clone()
        }),
    );
    table.insert(
        "STEP ablation: r as validated variable",
        step_constraints(&LogisticCircuit {
            r_as_variable: true,
            ..base.clone()
        }),
    );
    for (k, v) in &table {
        println!("{:<45} {:>6}   {}", k, v["constraints"], v["breakdown"]);
    }
    let w = ops::Widths::of(fmt);
    fs::write(
        &output,
        serde_json::to_vec_pretty(&json!({
            "format": if b64 { "binary64" } else { "binary32" }, "rounding": "RNE",
            "nan": if b64 { "canonical 0x7FF8000000000000" } else { "canonical 0x7FC00000" },
            "field": "Pallas scalar", "widths": format!("{w:?}"), "measurements": table,
        }))?,
    )?;
    Ok(())
}

// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn nova(
    steps: usize,
    steps_per_fold: usize,
    r: f32,
    x0: f32,
    compress: bool,
    validate_input: bool,
    r_variable: bool,
    reps: usize,
    output: PathBuf,
) -> Result<()> {
    if steps_per_fold == 0 || steps == 0 || steps % steps_per_fold != 0 {
        bail!("--steps ({steps}) tem que ser múltiplo positivo de --steps-per-fold ({steps_per_fold})");
    }
    new_file(&output)?;
    let folds = steps / steps_per_fold;
    let base = LogisticCircuit {
        validate_input,
        r_as_variable: r_variable,
        steps_per_fold,
        ..LogisticCircuit::new(r, x0)
    };
    let raw = step_constraints(&LogisticCircuit {
        steps_per_fold: 1,
        ..base.clone()
    })["constraints"]
        .clone();
    let raw_fold = step_constraints(&base)["constraints"].clone();
    let circuits = base.trajectory(folds);
    let z0 = base.z0::<Fq>();
    if !Pnf::from_f32(x0).is_valid(B32) {
        bail!("z0 inválido");
    }
    let mut runs = Vec::new();
    for rep in 0..reps {
        let t = Instant::now();
        let pp = PublicParams::<E1, E2, LogisticCircuit>::setup(
            &circuits[0],
            &*S1::ck_floor(),
            &*S2::ck_floor(),
        )
        .context("setup")?;
        let setup = t.elapsed().as_secs_f64();
        // `new` já sintetiza e prova o primeiro fold (o primeiro `prove_step`
        // é no-op em Nova 0.73), então o custo do prover é `new + prove_step*`.
        let t = Instant::now();
        let mut rs = RecursiveSNARK::<E1, E2, LogisticCircuit>::new(&pp, &circuits[0], &z0)?;
        let rs_new = t.elapsed().as_secs_f64();
        let t = Instant::now();
        for c in &circuits {
            rs.prove_step(&pp, c)?;
        }
        let prove = t.elapsed().as_secs_f64();
        let prover_ivc = rs_new + prove;
        let t = Instant::now();
        let zn = rs.verify(&pp, folds, &z0)?;
        let verify = t.elapsed().as_secs_f64();

        // O estado final tem que ser o binary32 nativo após `steps` passos.
        let mut native = x0;
        for _ in 0..steps {
            native = (native * (1.0 - native)) * r;
        }
        let expect = LogisticCircuit::new(r, native).z0::<Fq>();
        if zn[0] != Fq::from(steps as u64) || zn[1..7] != expect[1..7] {
            bail!("estado final difere do f32 nativo");
        }
        let decision = zn[7] == Fq::from(1);
        if decision != (native >= 0.5) {
            bail!("decisão final difere do f32 nativo");
        }

        let mut comp = json!(null);
        if compress {
            let t = Instant::now();
            let (pk, vk) = CompressedSNARK::<E1, E2, LogisticCircuit, S1, S2>::setup(&pp)?;
            let csetup = t.elapsed().as_secs_f64();
            let t = Instant::now();
            let proof = CompressedSNARK::<E1, E2, LogisticCircuit, S1, S2>::prove(&pp, &pk, &rs)?;
            let cprove = t.elapsed().as_secs_f64();
            let bytes = bincode::serde::encode_to_vec(&proof, bincode::config::legacy())?.len();
            let t = Instant::now();
            proof.verify(&vk, folds, &z0)?;
            let cverify = t.elapsed().as_secs_f64();
            comp = json!({"compression_setup_seconds": csetup, "compression_seconds": cprove,
                          "compressed_verify_seconds": cverify, "compressed_proof_bytes_bincode": bytes});
        }
        let (p, s) = pp.num_constraints();
        let (pv, sv) = pp.num_variables();
        eprintln!(
            "rep {rep}: S={steps_per_fold} prover {prover_ivc:.4}s ({:.6}s/passo) verify {verify:.4}s {comp}",
            prover_ivc / steps as f64
        );
        runs.push(json!({
            "rep": rep, "primary_constraints": p, "secondary_constraints": s,
            "primary_variables": pv, "secondary_variables": sv,
            "setup_seconds": setup,
            "recursive_new_seconds": rs_new, "prove_step_loop_seconds": prove,
            "prover_ivc_seconds": prover_ivc,
            "prover_seconds_per_logistic_step": prover_ivc / steps as f64,
            "prover_seconds_per_fold": prover_ivc / folds as f64,
            // Regime estacionário: o primeiro `prove_step` é no-op, então o
            // laço faz `folds - 1` folds reais.
            "steady_fold_seconds": (folds > 1).then(|| prove / (folds - 1) as f64),
            "steady_seconds_per_logistic_step":
                (folds > 1).then(|| prove / ((folds - 1) * steps_per_fold) as f64),
            // Compatível com 0004: só o laço de prove_step (sem `new`).
            "prover_total_seconds": prove,
            "prover_mean_step_seconds": prove / steps as f64,
            "recursive_verify_seconds": verify, "compressed": comp,
            "final_x_bits": format!("{:#010x}", native.to_bits()), "final_decision": decision,
        }));
    }
    let doc = json!({
        "representation": "pnf32", "workload": "logistic-map", "r": r, "x0": x0, "steps": steps,
        "steps_per_fold": steps_per_fold, "folds": folds, "raw_fold_constraints": raw_fold,
        "threshold": 0.5, "validate_input": validate_input, "r_variable": r_variable,
        "raw_step_constraints": raw, "state_field_elements": STEP_ARITY,
        "backend": "nova-snark 0.73.0, Pallas/Vesta, IPA", "peak_rss_bytes": peak_rss(),
        "runs": runs,
    });
    println!("{}", serde_json::to_string_pretty(&doc)?);
    fs::write(&output, serde_json::to_vec_pretty(&doc)?)?;
    Ok(())
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

// ---------------------------------------------------------------------------

fn trajectory(r: f32, x0: f32, n: usize, output: PathBuf) -> Result<()> {
    new_file(&output)?;
    let mut csv = String::from(
        "n,x_native_bits,x_pnf_circuit_bits,x_native,equal,decision_native,decision_circuit\n",
    );
    let (mut native, mut x) = (x0, Pnf::from_f32(x0));
    let rr = Pnf::from_f32(r);
    let (mut mismatches, mut flips, mut eps_max) = (0usize, 0usize, 0.0f64);
    for i in 1..=n {
        let mut cs = CheckCS::<Fq>::default();
        let xv = PnfVar::alloc_unchecked(&mut cs, B32, x);
        let y = logistic(&mut cs, B32, &xv, &PnfVar::constant(B32, rr));
        let d = ops::ge_pow2(&mut cs, B32, &y, -1, None);
        if !cs.is_satisfied() {
            bail!("passo {i}: witness insatisfeito");
        }
        x = y.value(B32);
        native = (native * (1.0 - native)) * r;
        let eq = x.to_f32().to_bits() == native.to_bits();
        mismatches += !eq as usize;
        flips += (d.b() != (native >= 0.5)) as usize;
        eps_max = eps_max.max((x.to_f32() as f64 - native as f64).abs());
        csv += &format!(
            "{i},{:#010x},{:#010x},{native:e},{eq},{},{}\n",
            native.to_bits(),
            x.to_f32().to_bits(),
            native >= 0.5,
            d.b()
        );
    }
    fs::write(&output, csv)?;
    println!(
        "{}",
        json!({"r": r, "x0": x0, "n": n, "bit_mismatches": mismatches, "predicate_flips": flips, "eps_max": eps_max})
    );
    Ok(())
}

// ---------------------------------------------------------------------------

fn sweep(r: f32, output: PathBuf) -> Result<()> {
    new_file(&output)?;
    let threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(8) as u64;
    let rr = Pnf::from_f32(r);
    let one = reference::one(B32);
    let t = Instant::now();
    let per = (1u64 << 32).div_ceil(threads);
    let results: Vec<[u64; 3]> = std::thread::scope(|s| {
        (0..threads)
            .map(|i| {
                s.spawn(move || {
                    let mut bad = [0u64; 3];
                    for bits in (i * per)..((i + 1) * per).min(1 << 32) {
                        let x = f32::from_bits(bits as u32);
                        let p = Pnf::from_f32(x);
                        let same = |a: f32, b: f32| {
                            (a.is_nan() && b.is_nan()) || a.to_bits() == b.to_bits()
                        };
                        if !p.is_valid(B32) || (!x.is_nan() && p.to_bits(B32) != bits) {
                            bad[0] += 1;
                        }
                        if !same(reference::sub(B32, one, p).to_f32(), 1.0 - x) {
                            bad[1] += 1;
                        }
                        if !same(reference::mul(B32, p, rr).to_f32(), x * r) {
                            bad[2] += 1;
                        }
                    }
                    bad
                })
            })
            .collect::<Vec<_>>()
            .into_iter()
            .map(|h| h.join().unwrap())
            .collect()
    });
    let total = results.iter().fold([0u64; 3], |acc, b| {
        [acc[0] + b[0], acc[1] + b[1], acc[2] + b[2]]
    });
    let doc = json!({
        "inputs": 1u64 << 32, "r": r, "seconds": t.elapsed().as_secs_f64(),
        "decode_encode_mismatches": total[0], "one_minus_x_mismatches": total[1],
        "x_times_r_mismatches": total[2],
        "oracle": "pnf-reference (arredondamento exato) vs f32 nativo (aarch64)",
    });
    println!("{doc}");
    fs::write(&output, serde_json::to_vec_pretty(&doc)?)?;
    Ok(())
}
