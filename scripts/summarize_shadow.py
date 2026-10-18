#!/usr/bin/env python3
"""Resumo da campanha de sombreamento: fração certificada e ε por horizonte."""
import csv, glob, json, os, sys, statistics as st
out = sys.argv[1]
print("| formato | r | N | sementes | certificadas | ε mediano | ε p90 | dist(x0,x0') mediana | falhas (breakdown / fora da grade) |")
print("|---|---|---:|---:|---:|---:|---:|---:|---|")
for f in sorted(glob.glob(os.path.join(out, "scan_*.csv"))):
    rows = list(csv.DictReader(open(f)))
    by = {}
    for r in rows: by.setdefault(int(r["N"]), []).append(r)
    for n in sorted(by):
        rs = by[n]; ok = [r for r in rs if r["status"] == "ok"]
        eps = sorted(float(r["eps"]) for r in ok); d0 = sorted(float(r["dist0"]) for r in ok)
        q = lambda v, p: f"{v[min(len(v)-1, int(p*len(v)))]:.1e}" if v else "—"
        bd = sum(r["status"] == "breakdown" for r in rs); nr = sum(r["status"] == "not_representable" for r in rs)
        if n in (16, 64, 256, 1024, 4096, 16384, 65536):
            print(f"| {rs[0]['format']} | {float(rs[0]['r']):.4g} | {n} | {len(rs)} | {len(ok)} ({100*len(ok)/len(rs):.0f}%) | {q(eps,.5)} | {q(eps,.9)} | {q(d0,.5)} | {bd} / {nr} |")
print()
print("## Witnesses do ataque")
print("| witness | N | honesto sombreado (ε) | adversarial sombreado (ε) | x0' do adversário (intervalo) |")
print("|---|---:|---|---|---|")
for f in sorted(glob.glob(os.path.join(out, "attack_*.json"))):
    d = json.load(open(f)); h = d["result"]["honest_ieee"]; a = d["result"]["adversarial"]
    fmt = lambda e: f"sim ({e['eps']:.1e})" if e["shadowed"] else f"não ({e.get('failure')})"
    iv = a.get("true_x0_interval", ["—", "—"])
    print(f"| {os.path.basename(f)} | {d['N']} | {fmt(h)} | {fmt(a)} | [{iv[0]!r}, {iv[1]!r}] |")
