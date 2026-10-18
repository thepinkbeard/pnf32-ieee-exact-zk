//! Medições do protótipo de sombreamento. Saídas sempre em arquivos novos.
//!
//! ```text
//! shadow-bench scan   --format b32 --r 3.9 --seeds 200 --nexp 16 --out results/<dir>/scan_b32_r3.9.csv
//! shadow-bench attack --witness ../rounding-steering-attack/results/campaign-20261016/witness_f32_d24_r3.9.json --out ...
//! shadow-bench constraints --out results/<dir>/constraints.json
//! shadow-bench nova --steps 16 --out results/<dir>/nova.json
//! ```

use std::{fs, path::PathBuf, sync::Mutex};

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use serde_json::json;
use shadow_proof::cert::*;

#[derive(Clone, Copy, ValueEnum)]
enum Fmt {
    B32,
    B64,
}

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Horizonte e distância de sombreamento por semente.
    Scan {
        #[arg(long, value_enum)]
        format: Fmt,
        #[arg(long)]
        r: f64,
        #[arg(long, default_value_t = 200)]
        seeds: usize,
        /// horizontes 2^4 .. 2^nexp
        #[arg(long, default_value_t = 16)]
        nexp: u32,
        #[arg(long)]
        out: PathBuf,
    },
    /// Certificado para a pseudo-órbita adversarial de um witness do ataque.
    Attack {
        #[arg(long)]
        witness: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    /// Constraints por fold do circuito PNF32 + sombreamento.
    Constraints {
        #[arg(long)]
        out: PathBuf,
    },
    /// IVC Nova curto com o circuito completo.
    Nova {
        #[arg(long, default_value_t = 16)]
        steps: usize,
        #[arg(long, default_value_t = 3.9)]
        r: f64,
        #[arg(long, default_value_t = 0.1)]
        x0: f64,
        #[arg(long)]
        out: PathBuf,
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

pub fn grid_bits(b32: bool) -> u32 {
    if b32 {
        48
    } else {
        96
    }
}

/// Mesmo gerador de sementes do ataque (rounding-steering-attack/src/main.rs).
fn seed_x0(b32: bool, seed: usize) -> f64 {
    let round = |x: f64| if b32 { x as f32 as f64 } else { x };
    if seed == 0 {
        return round(0.1);
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
    round(0.01 + 0.98 * u)
}

fn main() -> Result<()> {
    match Cli::parse().cmd {
        Cmd::Scan { format, r, seeds, nexp, out } => {
            fresh(&out)?;
            let b32 = matches!(format, Fmt::B32);
            let rr = if b32 { r as f32 as f64 } else { r };
            let map = Map::from_f64(rr, grid_bits(b32));
            let rows = Mutex::new(Vec::<(usize, String)>::new());
            let next = Mutex::new(0usize);
            let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(8);
            std::thread::scope(|s| {
                for _ in 0..threads {
                    s.spawn(|| loop {
                        let seed = {
                            let mut g = next.lock().unwrap();
                            *g += 1;
                            *g - 1
                        };
                        if seed > seeds {
                            break;
                        }
                        let x0 = seed_x0(b32, seed);
                        let full = ieee_orbit(b32, rr, x0, 1 << nexp);
                        let mut local = String::new();
                        for e in 4..=nexp {
                            let n = 1usize << e;
                            let orbit = &full[..=n];
                            let (status, eps, d0, where_) = match build(&map, orbit) {
                                Ok(c) => {
                                    check(&map, orbit, &c, &c.max_dist).expect("checagem independente falhou");
                                    ("ok", map.grid_f64(&c.max_dist), map.grid_f64(&c.dist0), -1i64)
                                }
                                Err(Failure::Breakdown(m)) => ("breakdown", f64::NAN, f64::NAN, m as i64),
                                Err(Failure::NotRepresentable(m)) => ("not_representable", f64::NAN, f64::NAN, m as i64),
                            };
                            local += &format!(
                                "{},{rr},{seed},{x0:e},{n},{status},{eps:e},{d0:e},{where_}\n",
                                if b32 { "binary32" } else { "binary64" }
                            );
                        }
                        rows.lock().unwrap().push((seed, local));
                    });
                }
            });
            let mut rows = rows.into_inner().unwrap();
            rows.sort_by_key(|r| r.0);
            let mut csv = String::from("format,r,seed,x0,N,status,eps,dist0,failure_step\n");
            for (_, r) in rows {
                csv += &r;
            }
            fs::write(&out, csv)?;
        }
        Cmd::Attack { witness, out } => {
            fresh(&out)?;
            let w: serde_json::Value = serde_json::from_slice(&fs::read(&witness).context("witness")?)?;
            let fmt = w["format"].as_str().unwrap();
            let b32 = fmt == "binary32";
            let dec = |h: &str| -> f64 {
                let v = u64::from_str_radix(h.trim_start_matches("0x"), 16).unwrap();
                if b32 {
                    f32::from_bits(v as u32) as f64
                } else {
                    f64::from_bits(v)
                }
            };
            let r = dec(w["r_bits"].as_str().unwrap());
            let x0 = dec(w["x0_bits"].as_str().unwrap());
            let mut adv = vec![x0];
            for s in w["steps"].as_array().unwrap() {
                adv.push(dec(s["y"].as_str().unwrap()));
            }
            let n = adv.len() - 1;
            let honest = ieee_orbit(b32, r, x0, n);
            let map = Map::from_f64(r, grid_bits(b32));
            let mut res = serde_json::Map::new();
            for (name, orbit) in [("honest_ieee", &honest), ("adversarial", &adv)] {
                let entry = match build(&map, orbit) {
                    Ok(c) => {
                        check(&map, orbit, &c, &c.max_dist).expect("checagem");
                        let x0p = (map.grid_f64(&c.intervals[0].a), map.grid_f64(&c.intervals[0].b));
                        json!({"shadowed": true, "eps": map.grid_f64(&c.max_dist),
                               "dist0": map.grid_f64(&c.dist0),
                               "true_x0_interval": [x0p.0, x0p.1],
                               "final_decision": orbit[n] >= 0.5,
                               "true_orbit_final_side_right": c.sides[n]})
                    }
                    Err(f) => json!({"shadowed": false, "failure": format!("{f:?}")}),
                };
                res.insert(name.into(), entry);
            }
            let doc = json!({"witness": witness.display().to_string(), "format": fmt, "r": r,
                             "x0": x0, "N": n, "result": res});
            println!("{}", serde_json::to_string_pretty(&doc)?);
            fs::write(&out, serde_json::to_vec_pretty(&doc)?)?;
        }
        Cmd::Constraints { out } => shadow_proof::bench::constraints(out)?,
        Cmd::Nova { steps, r, x0, out } => shadow_proof::bench::nova(steps, r, x0, out)?,
    }
    Ok(())
}
