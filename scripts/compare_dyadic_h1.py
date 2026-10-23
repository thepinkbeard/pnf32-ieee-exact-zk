#!/usr/bin/env python3
"""Confere as decisões do circuito diádico fundido contra o h1.

1. Referência independente: recalcula a trajetória diádica fundida em inteiros
   Python (sementes e r iguais ao h1) e exige igualdade com o circuito, decisão
   a decisão.
2. Divergência: compara com a coluna `reference` (órbita exata certificada) de
   `oracle-case-study/results/h1-20261022/decisions.csv`, no mesmo formato da
   tabela do h1 (1º N divergente mediano, p10–p90, fração em N fixos), e com a
   previsão `p·ln2/λ`.

uso: compare_dyadic_h1.py H1_DECISIONS.csv OUT.md dyadic_p{P}_r{R}.csv ...
Só biblioteca padrão.
"""
import csv
import math
import re
import statistics
import sys
from fractions import Fraction

S = 22
NS = [10, 20, 30, 50, 100, 200]
H1_COLS = ["binary32 (= PNF32)", "binary64", "WAD 18 casas, piso", "Q64.96, piso"]


def to_fixed(q: Fraction, bits: int) -> int:
    n = q * (1 << bits)
    assert n.denominator == 1, "conversão não exata"
    return n.numerator


def fused_traj(x0: Fraction, r: Fraction, p: int, nmax: int):
    one, sh = 1 << p, p + S
    x, rr = to_fixed(x0, p), to_fixed(r, S)
    out = [int(x >= one >> 1)]
    for _ in range(nmax):
        x = (x * (one - x) * rr) >> sh
        assert 0 <= x <= one
        out.append(int(x >= one >> 1))
    return out


def lyapunov(r: float, n=1_000_000, x=0.1234):
    for _ in range(1000):
        x = r * x * (1 - x)
    s = 0.0
    for _ in range(n):
        s += math.log(abs(r * (1 - 2 * x)))
        x = r * x * (1 - x)
    return s / n


def divergence(rows, col):
    """rows: {seed: {N: (ref, d)}} -> (primeiros N divergentes, fração por N)."""
    firsts, frac = [], {}
    for n in NS:
        tot = bad = 0
        for per in rows.values():
            ref, d = per[n][0], per[n][1][col]
            if ref is None:
                continue
            tot += 1
            bad += ref != d
        frac[n] = 100.0 * bad / tot if tot else float("nan")
    for per in rows.values():
        for n in sorted(per):
            ref, d = per[n][0], per[n][1][col]
            if ref is not None and ref != d:
                firsts.append(n)
                break
    return firsts, frac


def pct(v, q):
    v = sorted(v)
    return v[min(len(v) - 1, int(q * len(v)))]


def main():
    h1_path, out_path, files = sys.argv[1], sys.argv[2], sys.argv[3:]
    # circuito: (p, r) -> {seed: {N: d}}
    circ = {}
    for f in files:
        m = re.search(r"dyadic_p(\d+)_r([\d.]+)\.csv$", f)
        p, rs = int(m.group(1)), m.group(2)
        d = circ.setdefault((p, rs), {})
        with open(f) as fh:
            for row in csv.DictReader(fh):
                d.setdefault(int(row["seed"]), {})[int(row["N"])] = int(row["decision"])
    rvals = sorted({rs for _, rs in circ}, key=float)
    ps = sorted({p for p, _ in circ})

    # h1: r -> {seed: {N: (ref, {col: d})}} e x0 exato por semente
    h1, x0s = {}, {}
    with open(h1_path) as fh:
        for row in csv.DictReader(fh):
            rs = row["r"]
            if rs not in rvals:
                continue
            seed, n = int(row["seed"]), int(row["N"])
            ref = None if row["reference"] == "" else int(row["reference"])
            cols = {c: int(row[c]) for c in H1_COLS}
            h1.setdefault(rs, {}).setdefault(seed, {})[n] = (ref, cols)
            if n == 0:
                bits = int(row["x0_bits"], 16)
                import struct
                x0s[seed] = Fraction(struct.unpack("<f", struct.pack("<I", bits))[0])

    import struct

    def fl32(v):
        return Fraction(struct.unpack("<f", struct.pack("<f", v))[0])

    lines = ["# Ponto fixo diádico fundido × h1", ""]
    mism = 0
    total = 0
    for (p, rs), d in sorted(circ.items(), key=lambda k: (k[0][0], float(k[0][1]))):
        r = fl32(float(rs))
        for seed, per in d.items():
            nmax = max(per)
            ref = fused_traj(x0s[seed], r, p, nmax)
            for n, v in per.items():
                total += 1
                mism += v != ref[n]
    lines.append(f"Referência Python independente: {total} decisões do circuito conferidas, "
                 f"**{mism} diferenças**.")
    lines.append("")
    lines.append("Decisões contra a órbita exata certificada (coluna `reference` do h1); "
                 "N fixos = fração de sementes com decisão diferente da exata.")
    lines.append("")
    for rs in rvals:
        lam = lyapunov(float(rs))
        lines.append(f"## r = {rs} (λ = {lam:.3f})")
        lines.append("")
        lines.append("| representação | bits | previsão p·ln2/λ | 1º N divergente (mediana) | p10–p90 | "
                     + " | ".join(f"N={n}" for n in NS) + " |")
        lines.append("| --- | ---: | ---: | --- | --- | " + " | ".join("---:" for _ in NS) + " |")
        rows_all = h1[rs]
        bits_of = {"binary32 (= PNF32)": 24, "binary64": 53, "WAD 18 casas, piso": math.log2(1e18),
                   "Q64.96, piso": 96}
        entries = []
        for col in H1_COLS:
            entries.append((col, bits_of[col], rows_all, col))
        for p in ps:
            if (p, rs) not in circ:
                continue
            merged = {}
            for seed, per in circ[(p, rs)].items():
                merged[seed] = {n: (rows_all[seed][n][0], {"c": v}) for n, v in per.items() if n in rows_all[seed]}
            entries.append((f"**diádico fundido p={p} (circuito)**", p, merged, "c"))
        for name, bits, rows, col in sorted(entries, key=lambda e: e[1]):
            firsts, frac = divergence(rows, col)
            pred = bits * math.log(2) / lam if lam > 0 else float("inf")
            if firsts:
                med = f"{statistics.median(firsts):g} ({len(firsts)} sementes)"
                rng = f"{pct(firsts, 0.1)}–{pct(firsts, 0.9)}"
            else:
                med, rng = "— (0 sementes)", "—"
            pr = f"{pred:.0f}" if math.isfinite(pred) else "—"
            lines.append(f"| {name} | {bits:.1f} | {pr} | {med} | {rng} | "
                         + " | ".join(f"{frac[n]:.1f}%" for n in NS) + " |")
        lines.append("")
    with open(out_path, "w") as fh:
        fh.write("\n".join(lines) + "\n")
    print("\n".join(lines))
    if mism:
        sys.exit(1)


if __name__ == "__main__":
    main()
