//! Certificado de sombreamento de Hammel–Yorke–Grebogi (J. Complexity 1987)
//! para o mapa logístico, em aritmética inteira **exata**.
//!
//! Grade: todo ponto é `X / 2^K` com `X` inteiro. O mapa é
//! `f(x) = R·x·(1−x)` com `R = Rn / 2^Rs` (o valor diádico exato do `r` em float),
//! logo `f(X/2^K) = Rn·X·(2^K − X) / 2^(Rs+2K)`.
//!
//! O certificado é uma sequência de intervalos de grade `I_n = [A_n, B_n]`
//! com:
//!
//! 1. `A_n ≤ B_n`, `I_n` inteiro de um lado de `1/2` (lado `s_n`);
//! 2. aninhamento `f(I_n) ⊇ I_{n+1}` para `n < N`;
//! 3. `|p_n − x| ≤ ε` para todo `x ∈ I_n` (distância à pseudo-órbita).
//!
//! Por (2), existe uma órbita verdadeira `x_n ∈ I_n` para `n = 0..N`
//! (escolhe-se `x_N ∈ I_N` e anda-se para trás pelas pré-imagens). Por (3),
//! ela `ε`-sombreia a pseudo-órbita `{p_n}`. O provador constrói os
//! intervalos **para trás** (caro); o verificador só checa (1)–(3) localmente.

use num_bigint::BigInt;
use num_traits::{One, Signed, ToPrimitive, Zero};

#[derive(Clone, Debug)]
pub struct Map {
    /// `R = rn / 2^rs`
    pub rn: BigInt,
    pub rs: u32,
    /// bits fracionários da grade
    pub k: u32,
}

impl Map {
    /// Mapa com `r` dado por um float (o valor diádico exato do float).
    pub fn from_f64(r: f64, k: u32) -> Self {
        assert!(r > 0.0 && r.is_finite());
        let bits = r.to_bits();
        let e = ((bits >> 52) & 0x7ff) as i32 - 1075;
        let mut m = (bits & ((1 << 52) - 1)) | (1 << 52);
        let mut e = e;
        while m & 1 == 0 {
            m >>= 1;
            e += 1;
        }
        if e > 0 {
            return Self { rn: BigInt::from(m) << e as usize, rs: 0, k };
        }
        Self { rn: BigInt::from(m), rs: (-e) as u32, k }
    }

    pub fn r(&self) -> f64 {
        self.rn.to_f64().unwrap() / 2f64.powi(self.rs as i32)
    }

    pub fn one(&self) -> BigInt {
        BigInt::one() << self.k as usize
    }

    pub fn half(&self) -> BigInt {
        BigInt::one() << (self.k - 1) as usize
    }

    /// `f(X/2^K) · 2^(Rs+2K)` (exato).
    pub fn image_num(&self, x: &BigInt) -> BigInt {
        &self.rn * x * (self.one() - x)
    }

    /// `Y/2^K` levado à mesma escala de `image_num`.
    pub fn target(&self, y: &BigInt) -> BigInt {
        y << (self.rs + self.k) as usize
    }

    /// Ponto do formato (f64 exato) na grade, se representável.
    pub fn to_grid(&self, p: f64) -> Option<BigInt> {
        if p == 0.0 {
            return Some(BigInt::zero());
        }
        let bits = p.to_bits();
        if bits >> 63 == 1 {
            return None;
        }
        let ef = ((bits >> 52) & 0x7ff) as i32;
        let (m, e) = if ef == 0 { (bits & ((1 << 52) - 1), -1074) } else { ((bits & ((1 << 52) - 1)) | (1 << 52), ef - 1075) };
        let shift = e + self.k as i32;
        if shift >= 0 {
            Some(BigInt::from(m) << shift as usize)
        } else if m.trailing_zeros() as i32 >= -shift {
            Some(BigInt::from(m >> (-shift)))
        } else {
            None
        }
    }

    pub fn grid_f64(&self, x: &BigInt) -> f64 {
        x.to_f64().unwrap() / 2f64.powi(self.k as i32)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Interval {
    pub a: BigInt,
    pub b: BigInt,
}

#[derive(Clone, Debug)]
pub struct Certificate {
    pub intervals: Vec<Interval>,
    /// lado de `1/2` de cada intervalo (`true` = direita, `x ≥ 1/2`)
    pub sides: Vec<bool>,
    /// `max_n max_{x∈I_n} |p_n − x|`, em unidades da grade
    pub max_dist: BigInt,
    /// distância no passo 0: |x0 − x0'| ≤ dist0 (erro retroativo da entrada)
    pub dist0: BigInt,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Failure {
    /// pseudo-órbita fora da grade/domínio
    NotRepresentable(usize),
    /// pré-imagem inexistente (intervalo acima do máximo de f): separação
    Breakdown(usize),
}

/// Maior `X` em `[lo, hi]` com `pred(X)` verdadeiro, para `pred` monótono
/// decrescente (verdadeiro..verdadeiro, falso..falso). `None` se nenhum.
fn last_true(lo: &BigInt, hi: &BigInt, guess: &BigInt, pred: &dyn Fn(&BigInt) -> bool) -> Option<BigInt> {
    if !pred(lo) {
        return None;
    }
    if pred(hi) {
        return Some(hi.clone());
    }
    // colchete a partir do palpite (busca exponencial), depois bissecção
    let (mut t, mut f): (BigInt, BigInt);
    let g = guess.clone().max(lo.clone()).min(hi.clone());
    let mut step = BigInt::one() << 8usize;
    if pred(&g) {
        t = g.clone();
        loop {
            let c = (&t + &step).min(hi.clone());
            if pred(&c) {
                t = c;
                step <<= 1;
            } else {
                f = c;
                break;
            }
        }
    } else {
        f = g.clone();
        loop {
            let c = (&f - &step).max(lo.clone());
            if pred(&c) {
                t = c;
                break;
            } else {
                f = c;
                step <<= 1;
            }
        }
    }
    while &f - &t > BigInt::one() {
        let m: BigInt = (&t + &f) >> 1usize;
        if pred(&m) {
            t = m;
        } else {
            f = m;
        }
    }
    Some(t)
}

fn preimage_guess(map: &Map, y: &BigInt, right: bool) -> BigInt {
    let yf = map.grid_f64(y);
    let d = (1.0 - 4.0 * yf / map.r()).max(0.0).sqrt();
    let x = if right { 0.5 + d / 2.0 } else { 0.5 - d / 2.0 };
    let scaled = x * 2f64.powi(map.k as i32);
    BigInt::from(scaled as u128)
}

/// Constrói o certificado para trás a partir de `I_N = {p_N}` (HYG §3).
/// `orbit[n]` é a pseudo-órbita (valores exatos do formato).
pub fn build(map: &Map, orbit: &[f64]) -> Result<Certificate, Failure> {
    let n = orbit.len() - 1;
    let grid: Vec<BigInt> = orbit
        .iter()
        .enumerate()
        .map(|(i, &p)| map.to_grid(p).ok_or(Failure::NotRepresentable(i)))
        .collect::<Result<_, _>>()?;
    let half = map.half();
    let one = map.one();
    let mut intervals = vec![Interval { a: grid[n].clone(), b: grid[n].clone() }; n + 1];
    let mut sides = vec![false; n + 1];
    sides[n] = grid[n] >= half;
    for m in (0..n).rev() {
        let next = intervals[m + 1].clone();
        let right = grid[m] >= half;
        sides[m] = right;
        let (tl, th) = (map.target(&next.a), map.target(&next.b));
        let interval = if !right {
            // ramo crescente em [0, 1/2]
            if map.image_num(&half) < th {
                return Err(Failure::Breakdown(m));
            }
            let zero = BigInt::zero();
            let a = last_true(&zero, &half, &preimage_guess(map, &next.a, false), &|x| map.image_num(x) <= tl)
                .ok_or(Failure::Breakdown(m))?;
            // menor X com f(X) ≥ H  ==  (maior X com f(X) < H) + 1
            let b = match last_true(&zero, &half, &preimage_guess(map, &next.b, false), &|x| map.image_num(x) < th) {
                Some(x) => x + 1,
                None => zero.clone(),
            };
            Interval { a, b }
        } else {
            // ramo decrescente em [1/2, 1]
            if map.image_num(&half) < th {
                return Err(Failure::Breakdown(m));
            }
            let a = last_true(&half, &one, &preimage_guess(map, &next.b, true), &|x| map.image_num(x) >= th)
                .ok_or(Failure::Breakdown(m))?;
            let b = match last_true(&half, &one, &preimage_guess(map, &next.a, true), &|x| map.image_num(x) > tl) {
                Some(x) => x + 1,
                None => half.clone(),
            };
            Interval { a, b }
        };
        intervals[m] = interval;
    }
    let dist = |m: usize| {
        let i = &intervals[m];
        (&grid[m] - &i.a).abs().max((&i.b - &grid[m]).abs())
    };
    let max_dist = (0..=n).map(dist).max().unwrap();
    let dist0 = dist(0);
    Ok(Certificate { intervals, sides, max_dist, dist0 })
}

/// Checagem independente das três condições (o que o circuito impõe).
pub fn check(map: &Map, orbit: &[f64], cert: &Certificate, eps: &BigInt) -> Result<(), String> {
    let half = map.half();
    let one = map.one();
    let n = orbit.len() - 1;
    if cert.intervals.len() != n + 1 {
        return Err("tamanho".into());
    }
    for (m, i) in cert.intervals.iter().enumerate() {
        let p = map.to_grid(orbit[m]).ok_or("p fora da grade")?;
        if i.a > i.b || i.a.is_negative() || i.b > one {
            return Err(format!("intervalo inválido em {m}"));
        }
        let right = cert.sides[m];
        if (right && i.a < half) || (!right && i.b > half) {
            return Err(format!("intervalo cruza 1/2 em {m}"));
        }
        if &p - &i.a > *eps || &i.b - &p > *eps || &i.a - &p > *eps || &p - &i.b > *eps {
            return Err(format!("distância > ε em {m}"));
        }
        if m < n {
            let nx = &cert.intervals[m + 1];
            let (fa, fb) = (map.image_num(&i.a), map.image_num(&i.b));
            let (tl, th) = (map.target(&nx.a), map.target(&nx.b));
            let ok = if right { fa >= th && fb <= tl } else { fa <= tl && fb >= th };
            if !ok {
                return Err(format!("aninhamento falha em {m}"));
            }
        }
    }
    Ok(())
}

/// Pseudo-órbita IEEE honesta (`c = 1−x; p = x·c; y = p·r`) em binary32 ou binary64.
pub fn ieee_orbit(binary32: bool, r: f64, x0: f64, n: usize) -> Vec<f64> {
    let mut out = Vec::with_capacity(n + 1);
    if binary32 {
        let (r, mut x) = (r as f32, x0 as f32);
        out.push(x as f64);
        for _ in 0..n {
            x = (x * (1.0 - x)) * r;
            out.push(x as f64);
        }
    } else {
        let mut x = x0;
        out.push(x);
        for _ in 0..n {
            x = (x * (1.0 - x)) * r;
            out.push(x);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_and_map_exact() {
        let m = Map::from_f64(3.9f32 as f64, 48);
        assert_eq!(m.r(), 3.9f32 as f64);
        let x = m.to_grid(0.25).unwrap();
        // f(1/4) = R·3/16
        assert_eq!(m.image_num(&x), &m.rn * BigInt::from(3) << (2 * 48 - 4) as usize);
    }

    #[test]
    fn certificate_checks_and_rejects_tampering() {
        let map = Map::from_f64(3.9f32 as f64, 48);
        let orbit = ieee_orbit(true, 3.9f32 as f64, 0.1f32 as f64, 400);
        let cert = build(&map, &orbit).expect("certificado");
        check(&map, &orbit, &cert, &cert.max_dist).unwrap();
        // ε menor que o necessário falha
        assert!(check(&map, &orbit, &cert, &(&cert.max_dist - 1)).is_err());
        // estreitar um intervalo quebra o aninhamento
        let mut bad = cert.clone();
        let mid = 200;
        bad.intervals[mid].a += 1;
        bad.intervals[mid].b = bad.intervals[mid].a.clone();
        let w = check(&map, &orbit, &bad, &cert.max_dist);
        assert!(w.is_err(), "adulteração aceita");
    }

    #[test]
    fn hyg_case_binary64_is_shadowed() {
        // Caso do paper de 1987: a = 3,8, p0 = 0,4 (aqui em binary64, 20 mil passos).
        let map = Map::from_f64(3.8, 72);
        let orbit = ieee_orbit(false, 3.8, 0.4, 20_000);
        let cert = build(&map, &orbit).expect("sombreado");
        let eps = map.grid_f64(&cert.max_dist);
        assert!(eps < 1e-6, "ε = {eps}");
    }
}
