# dual-20261016 — prova dupla L2+L3 (log 0009, análise em docs/08)

Gerado por `target/release/dual-bench` (crate `crates/dual-proof`) e, para o
baseline contemporâneo, `target/release/pnf-bench`. Toolchain 1.85.0,
nova-snark 0.73.0 (Pallas/Vesta, IPA), macOS. Load average ≈ 53 durante as
medições Nova (tempos não conclusivos; constraints e bytes são firmes).

| arquivo | comando |
|---|---|
| constraints.json | `dual-bench constraints` |
| nova_k2_n10.json | `dual-bench nova --k 2 --steps 10 --compress --reps 3` |
| nova_k2_model_n10.json | `dual-bench nova --k 2 --steps 10 --model-r 39/10 --compress --reps 3` |
| nova_k4_n10.json | `dual-bench nova --k 4 --steps 10 --compress --reps 3` |
| nova.log | stderr das três acima |
| nova_pnf32_only_n10.json, nova_pnf32_only.log | `pnf-bench nova --steps 10 --r 3.9 --x0 0.3 --compress --reps 3` |
| coverage.json, coverage.csv, coverage.log | `dual-bench coverage --seeds 1000` (horizontes 10,30,100; K 2 e 4; modos fl(r) e 39/10) |
| tests.log | `cargo test --workspace --release` |
