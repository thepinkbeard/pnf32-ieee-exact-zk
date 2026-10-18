#!/usr/bin/env python3
"""Resumo do perfil por fase (log 0006): results/<dir>/raw/*.json ->
summary.json / summary.md (recusa sobrescrever). Tempos por fold em ms,
média e IC 95% t; censo de larguras (determinístico, da última repetição)."""

import glob
import json
import math
import os
import re
import statistics as st
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from analyze_spf import ci  # noqa: E402

REPRS = ["pnf32", "fp32", "cbd32"]
PHASES = [
    ("primary.synthesize", "síntese primária"),
    ("primary.commit_W", "commit W primário"),
    ("primary.nifs.T_matvec", "T: Az∘Bz−uCz−E"),
    ("primary.nifs.commit_T", "commit T primário"),
    ("primary.nifs.fold_UW", "fold U/W primário"),
    ("secondary.nifs_total", "NIFS secundário"),
    ("secondary.synthesize", "síntese secundária"),
    ("secondary.commit_W", "commit W secundário"),
]


def main(d):
    oj, om = os.path.join(d, "summary.json"), os.path.join(d, "summary.md")
    for p in (oj, om):
        if os.path.exists(p):
            sys.exit(f"{p} já existe; resultados nunca são sobrescritos")
    data = {}
    for f in glob.glob(os.path.join(d, "raw", "*.json")):
        m = re.match(r"(\w+?)_s(\d+)_rep(\d+)\.json", os.path.basename(f))
        data.setdefault((m[1], int(m[2])), []).append(json.load(open(f)))
    spfs = sorted({s for _, s in data})
    out = {}
    for (r, s), docs in sorted(data.items()):
        ms = lambda xs: ci([x * 1e3 for x in xs])
        out[f"{r}_s{s}"] = {
            "repr": r, "steps_per_fold": s, "n": len(docs),
            "primary_constraints": docs[0]["primary_constraints"],
            "fold_ms": ms([x["steady_wall_seconds_per_fold"] for x in docs]),
            "step_only_synthesis_ms": ms([x["step_only_synthesis_seconds"] for x in docs]),
            "phases_ms": {k: ms([x["phases"][k]["seconds_per_fold"] for x in docs])
                          for k in docs[0]["phases"]},
            "census": docs[-1]["census"],
        }
    json.dump(out, open(oj, "w"), indent=2)

    f = lambda c: f"{c['mean']:.1f} ± {c['hi'] - c['mean']:.1f}" if c["hi"] else f"{c['mean']:.1f}"
    L = ["# Perfil por fase de um fold (ms, média ± meia-largura do IC 95%)", "",
         f"n = {out[next(iter(out))]['n']} repetições; backend nova-snark 0.73.0 instrumentado.", ""]
    for s in spfs:
        L += [f"## S = {s}", "", "| fase | " + " | ".join(REPRS) + " |", "|---|" + "---:|" * 3]
        row = lambda name, get: L.append(f"| {name} | " + " | ".join(get(out[f'{r}_s{s}']) for r in REPRS) + " |")
        row("constraints primárias", lambda c: str(c["primary_constraints"]))
        row("**fold (parede)**", lambda c: f"**{f(c['fold_ms'])}**")
        row("síntese só do passo", lambda c: f(c["step_only_synthesis_ms"]))
        for k, name in PHASES:
            row(name, lambda c, k=k: f(c["phases_ms"][k]))
        L.append("")
    L += ["## Censo de larguras dos escalares (último fold, primário)", "",
          "| repr | S | vetor | n | zero | ±1 | ≤64 bits | largo |", "|---|---:|---|---:|---:|---:|---:|---:|"]
    for r in REPRS:
        for s in spfs:
            for vec in ("primary.W", "primary.T"):
                c = out[f"{r}_s{s}"]["census"][vec]
                g = c["groups"]
                small = sum(g[k] for k in ("le8", "le16", "le32", "le64"))
                L.append(f"| {r} | {s} | {vec.split('.')[1]} | {c['len']} | {g['zero']} | {g['unit']} | {small} | {g['large']} |")
    open(om, "w").write("\n".join(L) + "\n")
    print("\n".join(L))


if __name__ == "__main__":
    main(sys.argv[1])
