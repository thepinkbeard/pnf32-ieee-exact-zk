#!/usr/bin/env python3
"""Resumo da campanha G2 diádica: constraints por fold, certificação e ε
contra o ruído do preço do estudo de caso do oráculo.

Com P = 1500 + 1000·x (USD), um ruído relativo ρ no preço vira δx ≥ 1,5·ρ
(pior caso, P = 1500), como em oracle-case-study/results/phase2-20261022.
O erro retroativo total em x0 é ε (certificado) mais, para p < 30, o
arredondamento da entrada binary32 para a grade 2^-p (≤ 2^-p).

uso: summarize_dyadic_g2.py DIR   (lê g2_constraints.json, g2_scan_*.csv, g2_*nova*.json)
Só biblioteca padrão.
"""
import csv
import glob
import json
import os
import statistics
import sys
from collections import defaultdict

RHOS = [0.0005, 0.001, 0.005]
PNF32_G2 = 1035  # results/pnf64-wad-20261022/shadow_b32_constraints.json
PNF64_G2 = 1885


def q(v, f):
    v = sorted(v)
    return v[min(len(v) - 1, int(f * len(v)))]


def main():
    d = sys.argv[1]
    out = ["# G2 sobre o perfil diádico fundido", ""]

    cons = json.load(open(os.path.join(d, "g2_constraints.json")))
    out += ["## Constraints por fold (passo + certificado HYG + veredito)", "",
            "| p | K | larguras | constraints/fold | bits de ε (grade) | contra PNF32+G2 (1.035) |",
            "| ---: | ---: | --- | ---: | ---: | ---: |"]
    for r in cons["profiles"]:
        n = r["constraints_per_fold"]
        out.append(f"| {r['p']} | {r['K']} | {r['widths']} | {n} | {r['eps_grid_bits']} | {n / PNF32_G2:.2f}× |")
    out += ["", f"Referência: PNF32 + G2 = {PNF32_G2} (K=48), PNF64 + G2 = {PNF64_G2} (K=96), larguras completas.", ""]

    rows = defaultdict(list)
    for f in sorted(glob.glob(os.path.join(d, "g2_scan_*.csv"))):
        for r in csv.DictReader(open(f)):
            rows[(int(r["p"]), int(r["K"]), r["r"], int(r["N"]))].append(r)
    out += ["## Certificação e ε (sementes do ataque)", "",
            "| p | K | r | N | certificadas | ε mediano | ε p90 | ε máx | ε máx + entrada |"
            + "".join(f" ε máx < δx (ρ={rho * 100:g}%) |" for rho in RHOS),
            "| ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |" + " ---: |" * len(RHOS)]
    worst = defaultdict(float)  # (p, K) -> pior erro retroativo total em qualquer r, N
    fails = defaultdict(int)
    for (p, k, r, n), rs in sorted(rows.items(), key=lambda kv: (kv[0][0], kv[0][1], float(kv[0][2]), kv[0][3])):
        eps = [float(x["eps"]) for x in rs if x["certified"] == "1"]
        fails[(p, k)] += len(rs) - len(eps)
        if not eps:
            out.append(f"| {p} | {k} | {r} | {n} | 0/{len(rs)} | | | | |" + " |" * len(RHOS))
            continue
        inp = 2.0 ** -p if p < 30 else 0.0
        tot = max(eps) + inp
        worst[(p, k)] = max(worst[(p, k)], tot)
        cells = "".join(f" {sum(1 for e in eps if e + inp < 1.5 * rho)}/{len(rs)} |" for rho in RHOS)
        out.append(f"| {p} | {k} | {r} | {n} | {len(eps)}/{len(rs)} | {statistics.median(eps):.2e} | "
                   f"{q(eps, 0.9):.2e} | {max(eps):.2e} | {tot:.2e} |{cells}")
    out += ["", "## Menor perfil por nível de ruído", "",
            "Pior erro retroativo total (ε máx + entrada) em todos os r e N medidos, contra δx = 1,5·ρ.", "",
            "| p | K | falhas de certificação | pior ε total |" + "".join(f" ρ={rho * 100:g}% |" for rho in RHOS),
            "| ---: | ---: | ---: | ---: |" + " --- |" * len(RHOS)]
    for (p, k), w in sorted(worst.items()):
        cells = "".join(f" {'ok' if w < 1.5 * rho and fails[(p, k)] == 0 else 'não'} ({1.5 * rho / w:.0f}×) |"
                        for rho in RHOS)
        out.append(f"| {p} | {k} | {fails[(p, k)]} | {w:.2e} |{cells}")

    novas = sorted(glob.glob(os.path.join(d, "g2_*nova*.json")))
    if novas:
        out += ["", "## Nova (16 folds, uma rodada, smoke)", "",
                "| p | K | larguras | constraints primárias | prover (s) | compressão (s) | verificação (ms) | prova (B) |",
                "| ---: | ---: | --- | ---: | ---: | ---: | ---: | ---: |"]
        for f in novas:
            j = json.load(open(f))
            out.append(f"| {j['p']} | {j['K']} | {j['widths']} | {j['primary_constraints']} | {j['prover_seconds']:.2f} | "
                       f"{j['compression_seconds']:.2f} | {j['compressed_verify_seconds'] * 1000:.0f} | {j['compressed_proof_bytes']} |")
    text = "\n".join(out) + "\n"
    open(os.path.join(d, "summary.md"), "w").write(text)
    print(text)


if __name__ == "__main__":
    main()
