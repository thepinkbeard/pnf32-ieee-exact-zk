#!/usr/bin/env python3
"""Análise da campanha steps-per-fold (log 0005).

Lê results/<dir>/raw/{repr}_s{S}_rep{k}.json e grava summary.json e
summary.md no mesmo diretório (recusa sobrescrever). IC de 95% por t de
Student; razões pareadas (mesma repetição) como média geométrica das razões
por repetição, com IC t sobre o log.
"""

import glob
import json
import math
import os
import re
import statistics as st
import sys

T975 = {1: 12.706, 2: 4.303, 3: 3.182, 4: 2.776, 5: 2.571, 6: 2.447, 7: 2.365, 8: 2.306,
        9: 2.262, 10: 2.228, 11: 2.201, 12: 2.179, 13: 2.160, 14: 2.145, 15: 2.131,
        16: 2.120, 17: 2.110, 18: 2.101, 19: 2.093, 20: 2.086, 21: 2.080, 22: 2.074,
        23: 2.069, 24: 2.064, 25: 2.060, 26: 2.056, 27: 2.052, 28: 2.048, 29: 2.045,
        30: 2.042, 40: 2.021, 60: 2.000, 120: 1.980}
REPRS = ["pnf32", "fp32", "cbd32"]


def t975(df):
    if df in T975:
        return T975[df]
    keys = sorted(k for k in T975 if k <= df)
    return T975[keys[-1]] if keys else 1.96


def ci(xs):
    n = len(xs)
    m = st.fmean(xs)
    if n < 2:
        return {"n": n, "mean": m, "sd": None, "lo": None, "hi": None}
    sd = st.stdev(xs)
    h = t975(n - 1) * sd / math.sqrt(n)
    return {"n": n, "mean": m, "sd": sd, "lo": m - h, "hi": m + h, "median": st.median(xs)}


def ratio_ci(a, b):
    """Razão pareada a/b: média geométrica e IC 95% (t sobre log)."""
    logs = [math.log(x / y) for x, y in zip(a, b)]
    c = ci(logs)
    return {"n": c["n"], "geomean": math.exp(c["mean"]),
            "lo": math.exp(c["lo"]), "hi": math.exp(c["hi"])}


METRICS = {
    "prover_s_per_step": lambda r: r["prover_seconds_per_logistic_step"],
    "steady_s_per_step": lambda r: r["steady_seconds_per_logistic_step"],
    "prover_ivc_s": lambda r: r["prover_ivc_seconds"],
    "steady_fold_s": lambda r: r["steady_fold_seconds"],
    "setup_s": lambda r: r["setup_seconds"],
    "recursive_verify_s": lambda r: r["recursive_verify_seconds"],
    "compression_s": lambda r: r["compressed"]["compression_seconds"],
    "compressed_verify_s": lambda r: r["compressed"]["compressed_verify_seconds"],
}


def main(d):
    out_json, out_md = os.path.join(d, "summary.json"), os.path.join(d, "summary.md")
    for p in (out_json, out_md):
        if os.path.exists(p):
            sys.exit(f"{p} já existe; resultados nunca são sobrescritos")
    data = {}  # (repr, S) -> {rep: (doc, run)}
    for f in glob.glob(os.path.join(d, "raw", "*.json")):
        m = re.match(r"(\w+?)_s(\d+)_rep(\d+)\.json", os.path.basename(f))
        doc = json.load(open(f))
        assert len(doc["runs"]) == 1
        data.setdefault((m[1], int(m[2])), {})[int(m[3])] = (doc, doc["runs"][0])

    # Sensibilidade (critério fixado antes de olhar os tempos por configuração):
    # descarta repetições inteiras (preserva o pareamento) cuja soma de
    # setup + prover + compressão das 12 medições passa de 1,5x a mediana.
    reps = sorted({k for runs in data.values() for k in runs})
    tot = {k: sum(v[k][1]["setup_seconds"] + v[k][1]["prover_ivc_seconds"]
                  + v[k][1]["compressed"]["compression_seconds"]
                  for v in data.values() if k in v) for k in reps}
    med = st.median(tot.values())
    excluded = [k for k in reps if tot[k] > 1.5 * med]
    full, lines = report(data, "# Steps-per-fold — resumo")
    doc = {"all": full, "rep_total_seconds": tot, "excluded_reps": excluded,
           "exclusion_criterion": "soma por repetição > 1,5 x mediana"}
    lines += ["", f"Soma por repetição (s): mediana {med:.1f}; descartadas na análise de "
              f"sensibilidade: {excluded or 'nenhuma'}."]
    if excluded:
        clean = {k: {r: v for r, v in runs.items() if r not in excluded} for k, runs in data.items()}
        doc["clean"], cl = report(clean, "# Sensibilidade: sem as repetições descartadas")
        lines += [""] + cl
    json.dump(doc, open(out_json, "w"), indent=2)
    open(out_md, "w").write("\n".join(lines) + "\n")
    print("\n".join(lines))


def report(data, title):
    spfs = sorted({s for _, s in data})
    summary = {"configs": {}, "paired_vs_baselines": {}, "vs_s1": {}}
    for (rep_name, s), runs in sorted(data.items()):
        docs = [v[0] for v in runs.values()]
        rs = [v[1] for v in runs.values()]
        summary["configs"][f"{rep_name}_s{s}"] = {
            "repr": rep_name, "steps_per_fold": s, "steps": docs[0]["steps"],
            "folds": docs[0]["folds"], "raw_step_constraints": docs[0]["raw_step_constraints"],
            "raw_fold_constraints": docs[0]["raw_fold_constraints"],
            "primary_constraints": rs[0]["primary_constraints"],
            "secondary_constraints": rs[0]["secondary_constraints"],
            "proof_bytes": sorted({r["compressed"]["compressed_proof_bytes_bincode"] for r in rs}),
            "peak_rss_bytes_max": max(x["peak_rss_bytes"] or 0 for x in docs),
            **{k: ci([f(r) for r in rs]) for k, f in METRICS.items()
               if all(f(r) is not None for r in rs)},
        }

    def paired(a, b, metric):
        ra, rb = data[a], data[b]
        common = sorted(set(ra) & set(rb))
        f = METRICS[metric]
        return ratio_ci([f(ra[k][1]) for k in common], [f(rb[k][1]) for k in common])

    for s in spfs:
        for base in ("fp32", "cbd32"):
            for metric in ("prover_s_per_step", "steady_s_per_step", "compression_s",
                           "compressed_verify_s"):
                if ("pnf32", s) in data and (base, s) in data:
                    try:
                        summary["paired_vs_baselines"][f"pnf32/{base}_s{s}_{metric}"] = \
                            paired(("pnf32", s), (base, s), metric)
                    except TypeError:
                        pass
    for r in REPRS:
        for s in spfs:
            if s != 1 and (r, s) in data and (r, 1) in data:
                summary["vs_s1"][f"{r}_s{s}/s1_prover_s_per_step"] = \
                    paired((r, s), (r, 1), "prover_s_per_step")

    # Markdown
    C = summary["configs"]
    ms = lambda c, k: (f"{c[k]['mean']*1e3:.3f} [{c[k]['lo']*1e3:.3f}, {c[k]['hi']*1e3:.3f}]"
                       if k in c else "—")
    L = [title, "",
         f"n = {C[next(iter(C))]['prover_s_per_step']['n']} repetições pareadas; IC 95% (t).", "",
         "## Tempo do prover por passo logístico (ms) — `new + prove_step*` / passos", "",
         "| S | PNF32 | FP32 | CBD-32 | PNF/FP32 (pareado) | PNF/CBD (pareado) |",
         "|---:|---|---|---|---|---|"]
    P = summary["paired_vs_baselines"]
    fr = lambda x: f"{x['geomean']:.3f} [{x['lo']:.3f}, {x['hi']:.3f}]" if x else "—"
    for s in spfs:
        row = [str(s)] + [ms(C[f"{r}_s{s}"], "prover_s_per_step") for r in REPRS]
        row += [fr(P[f"pnf32/{b}_s{s}_prover_s_per_step"]) for b in ("fp32", "cbd32")]
        L.append("| " + " | ".join(row) + " |")
    L += ["", "## Regime estacionário: `prove_step` por passo (ms), excluindo `new`", "",
          "| S | PNF32 | FP32 | CBD-32 | PNF/FP32 | PNF/CBD |", "|---:|---|---|---|---|---|"]
    for s in spfs:
        row = [str(s)] + [ms(C[f"{r}_s{s}"], "steady_s_per_step") for r in REPRS]
        row += [fr(P.get(f"pnf32/{b}_s{s}_steady_s_per_step")) for b in ("fp32", "cbd32")]
        L.append("| " + " | ".join(row) + " |")
    L += ["", "## Ganho de S sobre S = 1 (razão pareada do tempo por passo)", "",
          "| S | PNF32 | FP32 | CBD-32 |", "|---:|---|---|---|"]
    for s in spfs[1:]:
        L.append(f"| {s} | " + " | ".join(fr(summary["vs_s1"][f"{r}_s{s}/s1_prover_s_per_step"])
                                         for r in REPRS) + " |")
    L += ["", "## Constraints, compressão e verificação", "",
          "| repr | S | raw/fold | primárias | setup (s) | compressão (s) | verify comp. (ms) | verify rec. (ms) | prova (B) | RSS máx (MiB) |",
          "|---|---:|---:|---:|---|---|---|---|---|---:|"]
    sec = lambda c, k: f"{c[k]['mean']:.3f} ± {c[k]['hi']-c[k]['mean']:.3f}"
    msec = lambda c, k: f"{c[k]['mean']*1e3:.1f} ± {(c[k]['hi']-c[k]['mean'])*1e3:.1f}"
    for r in REPRS:
        for s in spfs:
            c = C[f"{r}_s{s}"]
            L.append(f"| {r} | {s} | {c['raw_fold_constraints']} | {c['primary_constraints']} | "
                     f"{sec(c, 'setup_s')} | {sec(c, 'compression_s')} | "
                     f"{msec(c, 'compressed_verify_s')} | {msec(c, 'recursive_verify_s')} | "
                     f"{'/'.join(map(str, c['proof_bytes']))} | {c['peak_rss_bytes_max']/2**20:.0f} |")
    L += ["", "## Compressão pareada (PNF / baseline)", "", "| S | PNF/FP32 | PNF/CBD |", "|---:|---|---|"]
    for s in spfs:
        L.append(f"| {s} | " + " | ".join(fr(P[f"pnf32/{b}_s{s}_compression_s"])
                                          for b in ("fp32", "cbd32")) + " |")
    return summary, L


if __name__ == "__main__":
    main(sys.argv[1])
