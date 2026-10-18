//! [pnf32-ieee-exact-zk] Instrumentação de perfil adicionada a esta cópia de
//! nova-snark 0.73.0 (só para `profiling/`). Não altera nenhuma computação:
//! apenas cronometra trechos de `RecursiveSNARK::prove_step` e, quando
//! `enable_census(true)`, conta as larguras dos escalares que entram nas MSMs
//! com a mesma classificação de `provider::msm::msm`.
use ff::PrimeField;
use std::{
  collections::BTreeMap,
  sync::{
    atomic::{AtomicBool, AtomicU8, Ordering},
    Mutex,
  },
  time::Instant,
};

static TIMES: Mutex<BTreeMap<String, (f64, u64)>> = Mutex::new(BTreeMap::new());
static CENSUS: Mutex<BTreeMap<String, [u64; 7]>> = Mutex::new(BTreeMap::new());
static CENSUS_ON: AtomicBool = AtomicBool::new(false);
/// 0 = fora de prove_step, 1 = NIFS do primário, 2 = NIFS do secundário.
static SIDE: AtomicU8 = AtomicU8::new(0);

/// Grupos de largura, como em `msm`: zero, ±1, ≤8, ≤16, ≤32, ≤64 bits, largo
/// (largura = min(bits(s), bits(-s))).
pub const GROUPS: [&str; 7] = ["zero", "unit", "le8", "le16", "le32", "le64", "large"];

/// Liga/desliga o censo de larguras (fora dos trechos cronometrados).
pub fn enable_census(on: bool) {
  CENSUS_ON.store(on, Ordering::SeqCst);
}

pub(crate) fn set_side(side: u8) {
  SIDE.store(side, Ordering::SeqCst);
}

pub(crate) fn side_prefix() -> &'static str {
  match SIDE.load(Ordering::SeqCst) {
    1 => "primary.",
    2 => "secondary.",
    _ => "other.",
  }
}

pub(crate) fn record(name: &str, t: Instant) {
  let dt = t.elapsed().as_secs_f64();
  let mut m = TIMES.lock().unwrap();
  let e = m.entry(name.to_string()).or_insert((0.0, 0));
  e.0 += dt;
  e.1 += 1;
}

fn bits<F: PrimeField>(s: &F) -> u32 {
  let r = s.to_repr();
  let b = r.as_ref();
  for (i, byte) in b.iter().enumerate().rev() {
    if *byte != 0 {
      return (i as u32) * 8 + (8 - byte.leading_zeros());
    }
  }
  0
}

pub(crate) fn census<F: PrimeField>(name: &str, v: &[F]) {
  if !CENSUS_ON.load(Ordering::SeqCst) {
    return;
  }
  let mut c = [0u64; 7];
  for s in v {
    let w = bits(s).min(bits(&-*s));
    let g = if bool::from(s.is_zero()) {
      0
    } else if w <= 1 {
      1
    } else if w <= 8 {
      2
    } else if w <= 16 {
      3
    } else if w <= 32 {
      4
    } else if w <= 64 {
      5
    } else {
      6
    };
    c[g] += 1;
  }
  let mut m = CENSUS.lock().unwrap();
  let e = m.entry(name.to_string()).or_insert([0; 7]);
  for i in 0..7 {
    e[i] += c[i];
  }
}

/// Devolve e zera (tempos: nome -> (segundos acumulados, chamadas); censo).
#[allow(clippy::type_complexity)]
pub fn take() -> (BTreeMap<String, (f64, u64)>, BTreeMap<String, [u64; 7]>) {
  (
    std::mem::take(&mut *TIMES.lock().unwrap()),
    std::mem::take(&mut *CENSUS.lock().unwrap()),
  )
}
