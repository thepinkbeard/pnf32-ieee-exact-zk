//! Medições da prova dupla L2+L3. Cada subcomando grava num arquivo novo.
//!
//! ```text
//! dual-bench constraints --output results/<dir>/constraints.json
//! dual-bench nova --k 2 --steps 10 --compress --output results/<dir>/nova_k2.json
//! dual-bench coverage --seeds 1000 --output results/<dir>/coverage.json
//! ```

use std::{fs, path::PathBuf, time::Instant};

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use dual_proof::{check_z0, f32_ratio, f32_to_rational, Breakdown, DualCircuit, Verdict};
use nova_circuit::{
    half_threshold_numerator, initial_cbd_state, CbdLogisticStepCircuit, CbdRadiusBound,
    CbdStepWitness,
};
use nova_snark::{
    frontend::num::AllocatedNum,
    nova::{CompressedSNARK, PublicParams, RecursiveSNARK},
    provider::{ipa_pc::EvaluationEngine, PallasEngine, VestaEngine},
    spartan::snark::RelaxedR1CSSNARK,
    traits::{circuit::StepCircuit, snark::RelaxedR1CSSNARKTrait, Engine},
};
use num_bigint::BigInt;
use num_rational::BigRational;
use pnf_circuit::gadgets::CheckCS;
use serde_json::{json, Value};

type E1 = PallasEngine;
type E2 = VestaEngine;
type S1 = RelaxedR1CSSNARK<E1, EvaluationEngine<E1>>;
type S2 = RelaxedR1CSSNARK<E2, EvaluationEngine<E2>>;
type Fq = <E1 as Engine>::Scalar;

#[derive(Parser)]
#[command(about = "Prova dupla PNF32 (L2) + CBD (L3) num passo Nova")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Constraints por fold com breakdown PNF/CBD/combinação.
    Constraints {
        #[arg(long, default_value_t = 3.9)]
        r: f32,
        #[arg(long)]
        output: PathBuf,
    },
    /// IVC Nova (um passo logístico por fold), opcionalmente comprimida.
    Nova {
        #[arg(long, default_value_t = 2)]
        k: usize,
        #[arg(long, default_value_t = 10)]
        steps: usize,
        #[arg(long, default_value_t = 3.9)]
        r: f32,
        #[arg(long, default_value_t = 0.3)]
        x0: f32,
        /// `r` do modelo exato como `num/den` (p.ex. 39/10); omisso = fl(r).
        #[arg(long)]
        model_r: Option<String>,
        #[arg(long)]
        compress: bool,
        #[arg(long, default_value_t = 1)]
        reps: usize,
        #[arg(long)]
        output: PathBuf,
    },
    /// Cobertura da decisão robusta sobre sementes x0 pseudoaleatórias.
    Coverage {
        #[arg(long, default_value_t = 1000)]
        seeds: usize,
        #[arg(long, default_value_t = 20261016)]
        seed: u64,
        #[arg(long, default_value_t = 3.9)]
        r: f32,
        #[arg(long, value_delimiter = ',', default_values_t = [10, 30, 100])]
        horizons: Vec<usize>,
        /// Sementes conferidas também no circuito (CheckCS) até o maior horizonte.
        #[arg(long, default_value_t = 20)]
        circuit_check: usize,
        #[arg(long)]
        output: PathBuf,
    },
}

fn main() -> Result<()> {
    match Cli::parse().cmd {
        Cmd::Constraints { r, output } => constraints(r, output),
        Cmd::Nova {
            k,
            steps,
            r,
            x0,
            model_r,
            compress,
            reps,
            output,
        } => {
            let model = parse_ratio(model_r.as_deref(), r)?;
            match k {
                2 => nova::<2>(steps, r, x0, model, compress, reps, output),
                4 => nova::<4>(steps, r, x0, model, compress, reps, output),
                _ => bail!("K suportado: 2 ou 4"),
            }
        }
        Cmd::Coverage {
            seeds,
            seed,
            r,
            horizons,
            circuit_check,
            output,
        } => coverage(seeds, seed, r, horizons, circuit_check, output),
    }
}

fn parse_ratio(s: Option<&str>, r: f32) -> Result<(u64, u64)> {
    let Some(s) = s else {
        return Ok(f32_ratio(r));
    };
    let (n, d) = s.split_once('/').context("--model-r no formato num/den")?;
    Ok((n.trim().parse()?, d.trim().parse()?))
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

fn write_json(path: &PathBuf, doc: &Value) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(doc)?);
    fs::write(path, serde_json::to_vec_pretty(doc)?)?;
    Ok(())
}

fn bd_json(b: Breakdown) -> Value {
    json!({"pnf32": b.pnf, "cbd": b.cbd, "combination": b.combination, "total": b.total})
}

/// CBD sozinha (o circuito do projeto irmão), para conferir o 709 de lá.
fn cbd_standalone<const K: usize>(r_num: u64, r_den: u64) -> usize {
    let x0 = BigRational::new(BigInt::from(3), BigInt::from(10));
    let (limbs, is_one, radius) = initial_cbd_state::<K>(&x0);
    let c = CbdLogisticStepCircuit::<K> {
        witness: CbdStepWitness::compute_with_threshold(
            limbs,
            is_one,
            radius,
            r_num,
            r_den,
            CbdRadiusBound::LocalTaylor,
            half_threshold_numerator(K),
        ),
        r_num,
        r_den,
        radius_bound: CbdRadiusBound::LocalTaylor,
        threshold_numerator: half_threshold_numerator(K),
    };
    let mut cs = CheckCS::<Fq>::default();
    let mut z: Vec<Fq> = vec![Fq::from(0)];
    z.extend(limbs.iter().map(|&l| Fq::from(l)));
    z.push(Fq::from(is_one as u64));
    z.push(Fq::from(radius as u64));
    z.push(Fq::from(0));
    let zs: Vec<_> = z
        .iter()
        .map(|v| AllocatedNum::alloc(&mut cs, || Ok(*v)).unwrap())
        .collect();
    StepCircuit::<Fq>::synthesize(&c, &mut cs, &zs).unwrap();
    assert!(cs.is_satisfied());
    cs.constraints
}

fn constraints(r: f32, output: PathBuf) -> Result<()> {
    new_file(&output)?;
    let (rn, rd) = f32_ratio(r);
    let mut rows = Vec::new();
    for (mode, num, den) in [("same_input_fl_r", rn, rd), ("model_r_39_10", 39, 10)] {
        for k in [2usize, 4] {
            let (b, cbd_alone, arity) = match k {
                2 => (
                    DualCircuit::<2>::with_model_r(r, 0.3, num, den).breakdown(),
                    cbd_standalone::<2>(num, den),
                    DualCircuit::<2>::arity_value(),
                ),
                _ => (
                    DualCircuit::<4>::with_model_r(r, 0.3, num, den).breakdown(),
                    cbd_standalone::<4>(num, den),
                    DualCircuit::<4>::arity_value(),
                ),
            };
            assert_eq!(b.cbd, cbd_alone, "CBD embutida difere da CBD sozinha");
            rows.push(json!({
                "cbd_r_mode": mode, "cbd_r_num": num, "cbd_r_den": den, "k": k,
                "fractional_bits": 16 * k, "state_field_elements": arity,
                "per_fold": bd_json(b),
                "pnf32_standalone_step": 415, "cbd_standalone_step": cbd_alone,
                "overhead_vs_sum_of_parts": b.total as i64 - 415 - cbd_alone as i64,
            }));
        }
    }
    let doc = json!({
        "workload": "logistic-map", "r_f32": r, "r_f32_exact": format!("{rn}/{rd}"),
        "threshold": 0.5, "radius_bound": "LocalTaylor",
        "note": "breakdown medido com CheckCS; a forma depende de r (larguras de range check), não de x0",
        "rows": rows,
    });
    write_json(&output, &doc)
}

#[allow(clippy::too_many_arguments)]
fn nova<const K: usize>(
    steps: usize,
    r: f32,
    x0: f32,
    (rn, rd): (u64, u64),
    compress: bool,
    reps: usize,
    output: PathBuf,
) -> Result<()> {
    new_file(&output)?;
    let first = DualCircuit::<K>::with_model_r(r, x0, rn, rd);
    let circuits = first.trajectory(steps);
    let z0 = first.z0::<Fq>();
    if (rn, rd) == f32_ratio(r) && !check_z0::<K>(r, x0, &z0) {
        bail!("z0 inválido");
    }
    let expect = first.simulate(steps).pop().context("steps >= 1")?;
    let bd = first.breakdown();
    let mut runs = Vec::new();
    for rep in 0..reps {
        let t = Instant::now();
        let pp = PublicParams::<E1, E2, DualCircuit<K>>::setup(
            &circuits[0],
            &*S1::ck_floor(),
            &*S2::ck_floor(),
        )
        .context("setup")?;
        let setup = t.elapsed().as_secs_f64();
        let t = Instant::now();
        let mut rs = RecursiveSNARK::<E1, E2, DualCircuit<K>>::new(&pp, &circuits[0], &z0)?;
        let rs_new = t.elapsed().as_secs_f64();
        let t = Instant::now();
        for c in &circuits {
            rs.prove_step(&pp, c)?;
        }
        let prove = t.elapsed().as_secs_f64();
        let t = Instant::now();
        let zn = rs.verify(&pp, steps, &z0)?;
        let verify = t.elapsed().as_secs_f64();
        let mut native = x0;
        for _ in 0..steps {
            native = (native * (1.0 - native)) * r;
        }
        let pnf_expect = pnf_circuit::step::LogisticCircuit::new(r, native).z0::<Fq>();
        if zn[1..7] != pnf_expect[1..7] {
            bail!("saída IEEE difere do f32 nativo");
        }
        if zn[DualCircuit::<K>::idx_verdict()] != Fq::from(expect.verdict.code()) {
            bail!("veredito difere da simulação");
        }
        let mut comp = json!(null);
        if compress {
            type C<const K: usize> = CompressedSNARK<E1, E2, DualCircuit<K>, S1, S2>;
            let t = Instant::now();
            let (pk, vk) = C::<K>::setup(&pp)?;
            let csetup = t.elapsed().as_secs_f64();
            let t = Instant::now();
            let proof = C::<K>::prove(&pp, &pk, &rs)?;
            let cprove = t.elapsed().as_secs_f64();
            let bytes = bincode::serde::encode_to_vec(&proof, bincode::config::legacy())?.len();
            let t = Instant::now();
            proof.verify(&vk, steps, &z0)?;
            let cverify = t.elapsed().as_secs_f64();
            comp = json!({"compression_setup_seconds": csetup, "compression_seconds": cprove,
                          "compressed_verify_seconds": cverify, "compressed_proof_bytes_bincode": bytes});
        }
        let (p, s) = pp.num_constraints();
        let (pv, sv) = pp.num_variables();
        eprintln!("rep {rep}: K={K} prover {:.4}s verify {verify:.4}s {comp}", rs_new + prove);
        runs.push(json!({
            "rep": rep, "primary_constraints": p, "secondary_constraints": s,
            "primary_variables": pv, "secondary_variables": sv, "setup_seconds": setup,
            "recursive_new_seconds": rs_new, "prove_step_loop_seconds": prove,
            "prover_ivc_seconds": rs_new + prove,
            "steady_fold_seconds": (steps > 1).then(|| prove / (steps - 1) as f64),
            "recursive_verify_seconds": verify, "compressed": comp,
            "final_x_bits": format!("{:#010x}", native.to_bits()),
            "final_ieee_decision": native >= 0.5,
            "final_cbd_code": expect.cbd_code,
            "final_center_numerator": expect.center.to_string(),
            "final_radius_numerator": expect.radius.to_string(),
            "final_verdict": expect.verdict.label(),
        }));
    }
    let doc = json!({
        "representation": "dual pnf32+cbd", "workload": "logistic-map", "r_f32": r,
        "cbd_r": format!("{rn}/{rd}"), "x0": x0, "k": K, "fractional_bits": 16 * K,
        "steps": steps, "folds": steps, "threshold": 0.5,
        "raw_fold_constraints": bd_json(bd),
        "state_field_elements": DualCircuit::<K>::arity_value(),
        "backend": "nova-snark 0.73.0, Pallas/Vesta, IPA", "peak_rss_bytes": peak_rss(),
        "runs": runs,
    });
    write_json(&output, &doc)
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

struct SplitMix(u64);

impl SplitMix {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
}

/// Sementes `x0 = u / 2^24`, `u` uniforme em `[1, 2^24)`: binary32 exatos e
/// exatos também na grade `2^-32` da CBD (raio inicial zero).
fn seeds(n: usize, seed: u64) -> Vec<f32> {
    let mut g = SplitMix(seed);
    let mut out = Vec::with_capacity(n);
    while out.len() < n {
        let u = g.next() >> 40;
        if u != 0 {
            out.push(u as f32 / (1u32 << 24) as f32);
        }
    }
    out
}

/// Decisão exata `x_n >= 1/2` pelo intervalo dirigido; `None` se ele cruza.
fn exact_decisions(x0: f32, rq: &BigRational, n: usize) -> Vec<Option<bool>> {
    let half = BigRational::new(1.into(), 2.into());
    numeric_reference::directed_dynamic_trajectory(&f32_to_rational(x0), rq, None, n, 4096)
        .expect("referência dirigida")
        .into_iter()
        .map(|(lo, hi)| {
            if lo >= half {
                Some(true)
            } else if hi < half {
                Some(false)
            } else {
                None
            }
        })
        .collect()
}

#[derive(Default, Clone)]
struct Tally {
    robust_true: usize,
    robust_false: usize,
    indet_straddle: usize,
    indet_conflict: usize,
    ieee_wrong: usize,
    ieee_wrong_flagged: usize,
    robust_wrong: usize,
    exact_unknown: usize,
    radii: Vec<f64>,
}

impl Tally {
    fn json(&self, n: usize) -> Value {
        let mut r = self.radii.clone();
        r.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let q = |p: f64| r[((r.len() - 1) as f64 * p).round() as usize];
        let robust = self.robust_true + self.robust_false;
        json!({
            "seeds": n,
            "robust_true": self.robust_true, "robust_false": self.robust_false,
            "robust_total": robust, "robust_fraction": robust as f64 / n as f64,
            "indeterminate_straddle": self.indet_straddle,
            "indeterminate_ieee_conflict": self.indet_conflict,
            "ieee_decision_wrong_vs_exact": self.ieee_wrong,
            "ieee_wrong_flagged_indeterminate": self.ieee_wrong_flagged,
            "robust_verdict_wrong_vs_exact": self.robust_wrong,
            "exact_reference_undecided": self.exact_unknown,
            "radius_median": q(0.5), "radius_p90": q(0.9), "radius_max": q(1.0),
        })
    }
}

fn coverage_k<const K: usize>(
    xs: &[f32],
    r: f32,
    (rn, rd): (u64, u64),
    horizons: &[usize],
    exact: &[Vec<Option<bool>>],
    circuit_check: usize,
    csv: &mut String,
    mode: &str,
) -> Vec<Value> {
    let hmax = *horizons.iter().max().unwrap();
    let mut tallies = vec![Tally::default(); horizons.len()];
    let scale = (1u128 << (16 * K)) as f64;
    for (i, &x0) in xs.iter().enumerate() {
        let first = DualCircuit::<K>::with_model_r(r, x0, rn, rd);
        let recs = first.simulate(hmax);
        if i < circuit_check {
            // O circuito produz exatamente o veredito simulado.
            let mut z = first.z0::<Fq>();
            for (c, rec) in first.trajectory(hmax).iter().zip(&recs) {
                let mut cs = CheckCS::<Fq>::default();
                let zs: Vec<_> = z
                    .iter()
                    .map(|v| AllocatedNum::alloc(&mut cs, || Ok(*v)).unwrap())
                    .collect();
                let out = StepCircuit::<Fq>::synthesize(c, &mut cs, &zs).unwrap();
                assert!(cs.is_satisfied());
                z = out.iter().map(|n| n.get_value().unwrap()).collect();
                assert_eq!(
                    z[DualCircuit::<K>::idx_verdict()],
                    Fq::from(rec.verdict.code())
                );
            }
        }
        for (h, t) in horizons.iter().zip(tallies.iter_mut()) {
            let rec = &recs[h - 1];
            let ex = exact[i][*h];
            match rec.verdict {
                Verdict::RobustTrue => t.robust_true += 1,
                Verdict::RobustFalse => t.robust_false += 1,
                Verdict::Indeterminate if rec.cbd_code == 2 => t.indet_straddle += 1,
                Verdict::Indeterminate => t.indet_conflict += 1,
            }
            match ex {
                None => t.exact_unknown += 1,
                Some(e) => {
                    if rec.ieee_decision != e {
                        t.ieee_wrong += 1;
                        if rec.verdict == Verdict::Indeterminate {
                            t.ieee_wrong_flagged += 1;
                        }
                    }
                    let wrong = match rec.verdict {
                        Verdict::RobustTrue => !e,
                        Verdict::RobustFalse => e,
                        Verdict::Indeterminate => false,
                    };
                    t.robust_wrong += wrong as usize;
                }
            }
            t.radii.push(rec.radius as f64 / scale);
            csv.push_str(&format!(
                "{mode},{K},{i},{:#010x},{h},{:#010x},{},{},{},{},{},{}\n",
                x0.to_bits(),
                rec.y.to_bits(),
                rec.ieee_decision as u8,
                rec.center as f64 / scale,
                rec.radius as f64 / scale,
                rec.cbd_code,
                rec.verdict.code(),
                ex.map_or("?".into(), |e| (e as u8).to_string()),
            ));
        }
    }
    horizons
        .iter()
        .zip(tallies)
        .map(|(h, t)| {
            let mut v = t.json(xs.len());
            v["mode"] = json!(mode);
            v["k"] = json!(K);
            v["horizon"] = json!(h);
            v
        })
        .collect()
}

fn coverage(
    n: usize,
    seed: u64,
    r: f32,
    horizons: Vec<usize>,
    circuit_check: usize,
    output: PathBuf,
) -> Result<()> {
    if horizons.is_empty() || horizons.contains(&0) {
        bail!("horizontes >= 1");
    }
    new_file(&output)?;
    let csv_path = output.with_extension("csv");
    new_file(&csv_path)?;
    let t = Instant::now();
    let xs = seeds(n, seed);
    let hmax = *horizons.iter().max().unwrap();
    let mut rows = Vec::new();
    let mut csv = String::from(
        "cbd_r_mode,k,seed_index,x0_bits,horizon,y_bits,ieee_decision,center,radius,cbd_code,verdict,exact_decision\n",
    );
    let fl = f32_ratio(r);
    for (mode, ratio) in [("same_input_fl_r", fl), ("model_r_39_10", (39, 10))] {
        let rq = BigRational::new(BigInt::from(ratio.0), BigInt::from(ratio.1));
        let exact: Vec<_> = xs.iter().map(|&x| exact_decisions(x, &rq, hmax)).collect();
        rows.extend(coverage_k::<2>(&xs, r, ratio, &horizons, &exact, circuit_check, &mut csv, mode));
        rows.extend(coverage_k::<4>(&xs, r, ratio, &horizons, &exact, circuit_check, &mut csv, mode));
    }
    for row in &rows {
        if row["robust_verdict_wrong_vs_exact"] != json!(0) {
            bail!("veredito robusto contradiz a referência exata: {row}");
        }
    }
    let doc = json!({
        "workload": "logistic-map", "r_f32": r, "r_f32_exact": format!("{}/{}", fl.0, fl.1),
        "threshold": 0.5, "seeds": n, "seed": seed,
        "seed_rule": "x0 = u/2^24, u = (splitmix64 >> 40), u != 0",
        "horizons": horizons, "radius_bound": "LocalTaylor",
        "exact_reference": "numeric_reference::directed_dynamic_trajectory, 4096 bits",
        "circuit_checked_seeds_per_k_and_mode": circuit_check.min(n),
        "elapsed_seconds": t.elapsed().as_secs_f64(),
        "rows": rows,
    });
    fs::write(&csv_path, csv)?;
    write_json(&output, &doc)
}
