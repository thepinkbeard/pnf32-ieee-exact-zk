//! H4 — unicidade exaustiva em formatos mini.
//!
//! Para TODO par de entradas válidas e TODA atribuição das variáveis de
//! escolha (hints), sintetiza o circuito e coleta as saídas das atribuições
//! que satisfazem as constraints. Como todas as demais variáveis são funções
//! determinísticas das entradas e dos hints (spec, seção Unicidade), isto
//! enumera o espaço inteiro de witnesses. A propriedade checada é:
//!
//!   exatamente uma saída é aceita, ela é válida e é igual ao oráculo exato.
//!
//! Não é amostragem: é a afirmação universal, no formato reduzido.

use std::collections::BTreeSet;

use nova_snark::{provider::PallasEngine, traits::Engine};
use pnf_circuit::{gadgets::CheckCS, ops, pnf::PnfVar};
use pnf_reference::{self as reference, all_values, Format, Pnf};

type Fq = <PallasEngine as Engine>::Scalar;

#[derive(Clone, Copy, Debug)]
enum Op {
    Add,
    Mul,
}

fn accepted(fmt: Format, op: Op, a: Pnf, b: Pnf) -> (BTreeSet<u64>, usize) {
    let w = ops::Widths::of(fmt);
    let mut outs = BTreeSet::new();
    let mut tried = 0;
    let mut try_one = |synth: &dyn Fn(&mut CheckCS<Fq>, &PnfVar<Fq>, &PnfVar<Fq>) -> PnfVar<Fq>| {
        let mut cs = CheckCS::<Fq>::default();
        let va = PnfVar::alloc_unchecked(&mut cs, fmt, a);
        let vb = PnfVar::alloc_unchecked(&mut cs, fmt, b);
        let out = synth(&mut cs, &va, &vb);
        tried += 1;
        if cs.is_satisfied() {
            let v = out.value(fmt);
            assert!(v.is_valid(fmt), "saída aceita inválida: {v:?}");
            outs.insert(v.to_bits(fmt));
        }
    };
    match op {
        Op::Mul => {
            for k in 0..(1u64 << w.mul_kb) {
                for ovf in [false, true] {
                    let h = ops::MulHints { k, ovf };
                    try_one(&|cs, x, y| ops::mul(cs, fmt, x, y, Some(h)));
                }
            }
        }
        Op::Add => {
            for k in 0..(1u64 << w.add_kb) {
                for bits in 0..16u8 {
                    let h = ops::AddHints {
                        swap: bits & 1 == 1,
                        big: bits & 2 == 2,
                        neg: bits & 4 == 4,
                        ovf: bits & 8 == 8,
                        k,
                    };
                    try_one(&|cs, x, y| ops::add(cs, fmt, x, y, Some(h)));
                }
            }
        }
    }
    (outs, tried)
}

fn exhaustive(fmt: Format, op: Op) {
    let values = all_values(fmt);
    let threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    let chunks: Vec<Vec<Pnf>> = values
        .chunks(values.len().div_ceil(threads))
        .map(|c| c.to_vec())
        .collect();
    let total: usize = std::thread::scope(|s| {
        let handles: Vec<_> = chunks
            .iter()
            .map(|chunk| {
                let values = &values;
                s.spawn(move || {
                    let mut tried = 0;
                    for &a in chunk {
                        for &b in values {
                            let want = match op {
                                Op::Add => reference::add(fmt, a, b),
                                Op::Mul => reference::mul(fmt, a, b),
                            };
                            let (outs, n) = accepted(fmt, op, a, b);
                            tried += n;
                            assert_eq!(
                                outs.into_iter().collect::<Vec<_>>(),
                                vec![want.to_bits(fmt)],
                                "{op:?} {fmt:?} a={a:?} b={b:?}"
                            );
                        }
                    }
                    tried
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).sum()
    });
    eprintln!(
        "{op:?} {fmt:?}: {} valores, {} pares, {total} witnesses sintetizados, unicidade OK",
        values.len(),
        values.len() * values.len()
    );
}

const MINI6: Format = Format { ebits: 3, p: 3 };
const MINI8: Format = Format { ebits: 4, p: 4 };

#[test]
fn mul_mini6() {
    exhaustive(MINI6, Op::Mul);
}

#[test]
fn add_mini6() {
    exhaustive(MINI6, Op::Add);
}

#[test]
fn mul_mini8() {
    exhaustive(MINI8, Op::Mul);
}

/// ~15 M sínteses; rodar com `--ignored` (cerca de 1–2 min em 10 núcleos).
#[test]
#[ignore]
fn add_mini8() {
    exhaustive(MINI8, Op::Add);
}

/// Decisão `x >= 2^k`: para todo valor e todo k admissível, só a resposta
/// exata é aceita.
#[test]
fn decision_mini8() {
    let fmt = MINI8;
    for x in all_values(fmt) {
        for kexp in (fmt.emin() + 1)..=fmt.emax() {
            let tau = Pnf {
                sign: false,
                exp: kexp,
                sig: fmt.hidden(),
                class: pnf_reference::Class::Finite,
            };
            let want = reference::ge(fmt, x, tau);
            let mut accepted = BTreeSet::new();
            for hint in [false, true] {
                let mut cs = CheckCS::<Fq>::default();
                let v = PnfVar::alloc_unchecked(&mut cs, fmt, x);
                let d = ops::ge_pow2(&mut cs, fmt, &v, kexp, Some(hint));
                if cs.is_satisfied() {
                    accepted.insert(d.b());
                }
            }
            assert_eq!(
                accepted.into_iter().collect::<Vec<_>>(),
                vec![want],
                "{x:?} >= 2^{kexp}"
            );
        }
    }
}
