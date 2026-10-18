//! Referência off-circuit da PNF (Proof-Native Float).
//!
//! A PNF é a mesma semântica IEEE 754 binária (round-to-nearest-ties-to-even),
//! genérica no formato `(ebits, p)`. `Format::BINARY32` é o binary32; formatos
//! mini (por exemplo `ebits=3, p=3`) servem para a enumeração exaustiva.
//!
//! Este crate contém duas coisas independentes do circuito:
//!
//! 1. a codificação canônica desempacotada [`Pnf`] e a bijeção com os padrões
//!    de bits do formato (NaN é colapsado no NaN canônico);
//! 2. um **oráculo** de arredondamento exato ([`add`], [`mul`], [`sub`]): o
//!    resultado matemático é calculado exatamente em inteiros grandes e
//!    arredondado uma única vez. Ele não compartilha nenhum código com o
//!    gerador de witness do circuito, de propósito.

use num_bigint::{BigInt, BigUint, Sign};
use num_traits::{One, Signed, Zero};

/// Formato binário IEEE: `ebits` bits de expoente, `p` bits de precisão
/// (incluindo o bit implícito).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct Format {
    pub ebits: u32,
    pub p: u32,
}

impl Format {
    pub const BINARY32: Format = Format { ebits: 8, p: 24 };
    pub const BINARY16: Format = Format { ebits: 5, p: 11 };

    pub const fn bias(self) -> i32 {
        (1 << (self.ebits - 1)) - 1
    }
    pub const fn emax(self) -> i32 {
        self.bias()
    }
    pub const fn emin(self) -> i32 {
        1 - self.bias()
    }
    pub const fn fbits(self) -> u32 {
        self.p - 1
    }
    pub const fn width(self) -> u32 {
        1 + self.ebits + self.p - 1
    }
    pub const fn hidden(self) -> u64 {
        1 << (self.p - 1)
    }
    /// Expoente do quantum subnormal: todo finito é múltiplo de `2^qmin`.
    pub const fn qmin(self) -> i32 {
        self.emin() - (self.p as i32 - 1)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Class {
    Finite,
    Inf,
    Nan,
}

/// Valor PNF canônico.
///
/// - Finito: `valor = (-1)^sign * sig * 2^(exp-(p-1))`, com `sig < 2^p`,
///   `emin <= exp <= emax`. `sig = 0` é zero (com `exp = emin`); `sig < 2^(p-1)`
///   só é permitido com `exp = emin` (subnormal). Esta é exatamente a leitura
///   dos campos IEEE com o bit implícito explícito e o expoente desenviesado.
/// - Inf: `exp = emin`, `sig = 0`, sinal livre.
/// - NaN: `exp = emin`, `sig = 0`, `sign = false` (NaN canônico).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct Pnf {
    pub sign: bool,
    pub exp: i32,
    pub sig: u64,
    pub class: Class,
}

impl Pnf {
    pub fn zero(fmt: Format, sign: bool) -> Self {
        Self {
            sign,
            exp: fmt.emin(),
            sig: 0,
            class: Class::Finite,
        }
    }
    pub fn inf(fmt: Format, sign: bool) -> Self {
        Self {
            sign,
            exp: fmt.emin(),
            sig: 0,
            class: Class::Inf,
        }
    }
    pub fn nan(fmt: Format) -> Self {
        Self {
            sign: false,
            exp: fmt.emin(),
            sig: 0,
            class: Class::Nan,
        }
    }
    pub fn is_zero(self) -> bool {
        self.class == Class::Finite && self.sig == 0
    }
    pub fn is_nan(self) -> bool {
        self.class == Class::Nan
    }
    pub fn is_inf(self) -> bool {
        self.class == Class::Inf
    }
    pub fn neg(self) -> Self {
        if self.is_nan() {
            self
        } else {
            Self {
                sign: !self.sign,
                ..self
            }
        }
    }

    /// Invariante de validade (o que o verificador checa em `z0`).
    pub fn is_valid(self, fmt: Format) -> bool {
        match self.class {
            Class::Finite => {
                self.exp >= fmt.emin()
                    && self.exp <= fmt.emax()
                    && self.sig < (1 << fmt.p)
                    && (self.sig >= fmt.hidden() || self.exp == fmt.emin())
            }
            Class::Inf => self.exp == fmt.emin() && self.sig == 0,
            Class::Nan => self.exp == fmt.emin() && self.sig == 0 && !self.sign,
        }
    }

    pub fn from_bits(fmt: Format, bits: u64) -> Self {
        let fbits = fmt.fbits();
        let emask = (1_u64 << fmt.ebits) - 1;
        let sign = (bits >> (fmt.width() - 1)) & 1 == 1;
        let efield = (bits >> fbits) & emask;
        let frac = bits & ((1_u64 << fbits) - 1);
        if efield == emask {
            if frac == 0 {
                Self::inf(fmt, sign)
            } else {
                Self::nan(fmt)
            }
        } else if efield == 0 {
            Self {
                sign,
                exp: fmt.emin(),
                sig: frac,
                class: Class::Finite,
            }
        } else {
            Self {
                sign,
                exp: efield as i32 - fmt.bias(),
                sig: frac | fmt.hidden(),
                class: Class::Finite,
            }
        }
    }

    /// Codificação IEEE. NaN vira o NaN quieto canônico (`0x7FC00000` em
    /// binary32), como na especialização RISC-V/ARM default-NaN do SoftFloat.
    pub fn to_bits(self, fmt: Format) -> u64 {
        let fbits = fmt.fbits();
        let emask = (1_u64 << fmt.ebits) - 1;
        let sign = (self.sign as u64) << (fmt.width() - 1);
        match self.class {
            Class::Nan => (emask << fbits) | (1 << (fbits - 1)),
            Class::Inf => sign | (emask << fbits),
            Class::Finite => {
                if self.sig >= fmt.hidden() {
                    let efield = (self.exp + fmt.bias()) as u64;
                    sign | (efield << fbits) | (self.sig - fmt.hidden())
                } else {
                    sign | self.sig
                }
            }
        }
    }

    pub fn from_f32(value: f32) -> Self {
        Self::from_bits(Format::BINARY32, value.to_bits() as u64)
    }
    pub fn to_f32(self) -> f32 {
        f32::from_bits(self.to_bits(Format::BINARY32) as u32)
    }

    /// Inteiro com sinal `N` tal que `valor = N * 2^qmin` (só finitos).
    fn scaled(self, fmt: Format) -> BigInt {
        let magnitude = BigInt::from(self.sig) << ((self.exp - fmt.emin()) as usize);
        if self.sign {
            -magnitude
        } else {
            magnitude
        }
    }
}

/// Arredonda `(-1)^sign * mag * 2^scale` (`mag > 0`) para o formato, RNE.
fn round_finite(fmt: Format, sign: bool, mag: &BigUint, scale: i32) -> Pnf {
    debug_assert!(!mag.is_zero());
    let p = fmt.p as i32;
    let lead = mag.bits() as i32 - 1 + scale;
    let mut exp = lead.max(fmt.emin());
    let quantum = exp - (p - 1);
    let shift = quantum - scale;
    let mut sig: BigUint = if shift <= 0 {
        mag << ((-shift) as usize)
    } else {
        let shift = shift as usize;
        let q = mag >> shift;
        let r = mag - (&q << shift);
        let half = BigUint::one() << (shift - 1);
        let up = r > half || (r == half && q.bit(0));
        if up {
            q + 1_u32
        } else {
            q
        }
    };
    if sig == BigUint::one() << fmt.p {
        sig >>= 1;
        exp += 1;
    }
    if exp > fmt.emax() {
        return Pnf::inf(fmt, sign);
    }
    let sig = u64::try_from(sig).expect("significando cabe em u64");
    if sig == 0 {
        return Pnf::zero(fmt, sign);
    }
    debug_assert!(sig >= fmt.hidden() || exp == fmt.emin());
    Pnf {
        sign,
        exp,
        sig,
        class: Class::Finite,
    }
}

/// Soma IEEE (RNE) por arredondamento único do valor exato.
pub fn add(fmt: Format, a: Pnf, b: Pnf) -> Pnf {
    if a.is_nan() || b.is_nan() {
        return Pnf::nan(fmt);
    }
    match (a.is_inf(), b.is_inf()) {
        (true, true) => {
            return if a.sign == b.sign { a } else { Pnf::nan(fmt) };
        }
        (true, false) => return a,
        (false, true) => return b,
        _ => {}
    }
    let sum = a.scaled(fmt) + b.scaled(fmt);
    if sum.is_zero() {
        // RNE: x + (-x) = +0; (-0) + (-0) = -0.
        return Pnf::zero(fmt, a.sign && b.sign);
    }
    let sign = sum.sign() == Sign::Minus;
    round_finite(fmt, sign, &sum.abs().to_biguint().unwrap(), fmt.qmin())
}

pub fn sub(fmt: Format, a: Pnf, b: Pnf) -> Pnf {
    add(fmt, a, b.neg())
}

/// Produto IEEE (RNE) por arredondamento único do valor exato.
pub fn mul(fmt: Format, a: Pnf, b: Pnf) -> Pnf {
    if a.is_nan() || b.is_nan() {
        return Pnf::nan(fmt);
    }
    let sign = a.sign ^ b.sign;
    if (a.is_inf() && b.is_zero()) || (a.is_zero() && b.is_inf()) {
        return Pnf::nan(fmt);
    }
    if a.is_inf() || b.is_inf() {
        return Pnf::inf(fmt, sign);
    }
    if a.is_zero() || b.is_zero() {
        return Pnf::zero(fmt, sign);
    }
    let mag = BigUint::from(a.sig) * BigUint::from(b.sig);
    let scale = a.exp + b.exp - 2 * (fmt.p as i32 - 1);
    round_finite(fmt, sign, &mag, scale)
}

/// Comparação `x >= t` (falsa para NaN), exata.
pub fn ge(fmt: Format, x: Pnf, t: Pnf) -> bool {
    if x.is_nan() || t.is_nan() {
        return false;
    }
    let key = |v: Pnf| -> (i32, BigInt) {
        match v.class {
            Class::Inf => (if v.sign { -1 } else { 1 }, BigInt::zero()),
            _ => (0, v.scaled(fmt)),
        }
    };
    key(x) >= key(t)
}

/// Passo logístico na ordem do baseline FP32: `c=1-x; p=x*c; y=p*r`.
pub fn logistic_step(fmt: Format, x: Pnf, r: Pnf) -> Pnf {
    let one = one(fmt);
    let c = sub(fmt, one, x);
    let p = mul(fmt, x, c);
    mul(fmt, p, r)
}

pub fn one(fmt: Format) -> Pnf {
    Pnf {
        sign: false,
        exp: 0,
        sig: fmt.hidden(),
        class: Class::Finite,
    }
}

/// Todos os valores de um formato pequeno (NaN aparece uma única vez).
pub fn all_values(fmt: Format) -> Vec<Pnf> {
    assert!(fmt.width() <= 16, "enumeração só para formatos pequenos");
    let mut out: Vec<Pnf> = Vec::new();
    for bits in 0..(1_u64 << fmt.width()) {
        let v = Pnf::from_bits(fmt, bits);
        if !(v.is_nan() && out.iter().any(|o| o.is_nan())) {
            out.push(v);
        }
    }
    out
}

/// Gerador determinístico simples (xorshift64*), para amostras reprodutíveis.
pub struct Rng(pub u64);

impl Rng {
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Padrão binary32 com viés para casos difíceis: expoentes extremos,
    /// subnormais, especiais, vizinhos de potências de dois.
    pub fn f32_bits(&mut self) -> u32 {
        let r = self.next_u64();
        let raw = (r >> 32) as u32;
        match r % 8 {
            0 => raw & 0x807F_FFFF,                                // subnormais e zeros
            1 => raw | 0x7F80_0000,                                // inf/NaN
            2 => (raw & 0x8000_0000) | 0x7F7F_FFFF - (raw & 0xFF), // perto do máximo
            3 => (raw & 0x8000_0000) | ((raw & 0x3) << 23) | 0x0080_0000, // menores normais
            4 => (raw & 0xFF80_0000) | (raw & 0x7) | 0x007F_FFF8 * ((raw >> 3) & 1), // bordas de mantissa
            _ => raw,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn same(a: f32, b: f32) -> bool {
        (a.is_nan() && b.is_nan()) || a.to_bits() == b.to_bits()
    }

    #[test]
    fn binary32_constants() {
        let f = Format::BINARY32;
        assert_eq!(
            (f.emin(), f.emax(), f.qmin(), f.width()),
            (-126, 127, -149, 32)
        );
        assert_eq!(Pnf::nan(f).to_bits(f), 0x7FC0_0000);
    }

    #[test]
    fn bits_roundtrip_sampled() {
        let f = Format::BINARY32;
        let mut rng = Rng(7);
        for _ in 0..2_000_000 {
            let bits = rng.f32_bits() as u64;
            let v = Pnf::from_bits(f, bits);
            assert!(v.is_valid(f), "{bits:#x}");
            if !v.is_nan() {
                assert_eq!(v.to_bits(f), bits);
            }
        }
    }

    #[test]
    fn mini_roundtrip_exhaustive() {
        for f in [
            Format { ebits: 3, p: 3 },
            Format { ebits: 4, p: 4 },
            Format::BINARY16,
        ] {
            for bits in 0..(1_u64 << f.width()) {
                let v = Pnf::from_bits(f, bits);
                assert!(v.is_valid(f));
                if !v.is_nan() {
                    assert_eq!(v.to_bits(f), bits);
                }
            }
        }
    }

    /// Compara com o f32 nativo (árbitro) e conta divergências do crate
    /// `softfloat` 1.0 (port Rust puro), que tem bugs em subnormais; ver o
    /// registro 0002. Berkeley SoftFloat em C é validado à parte (Docker).
    fn check_pair(a: f32, b: f32, softfloat_mismatches: &mut usize) {
        let f = Format::BINARY32;
        let (pa, pb) = (Pnf::from_f32(a), Pnf::from_f32(b));
        let (sa, sb) = (
            softfloat::F32::from_native_f32(a),
            softfloat::F32::from_native_f32(b),
        );
        let cases = [
            (add(f, pa, pb).to_f32(), a + b, sa.add(sb).to_native_f32()),
            (sub(f, pa, pb).to_f32(), a - b, sa.sub(sb).to_native_f32()),
            (mul(f, pa, pb).to_f32(), a * b, sa.mul(sb).to_native_f32()),
        ];
        for (op, (ours, native, soft)) in ["add", "sub", "mul"].iter().zip(cases) {
            assert!(
                same(ours, native),
                "{op} {a:e} {b:e}: pnf {ours:e} native {native:e}"
            );
            if !same(ours, soft) {
                *softfloat_mismatches += 1;
            }
        }
    }

    #[test]
    fn oracle_matches_native() {
        let mut rng = Rng(0x5EED);
        let mut soft = 0;
        for _ in 0..1_000_000 {
            let a = f32::from_bits(rng.f32_bits());
            let b = f32::from_bits(rng.f32_bits());
            check_pair(a, b, &mut soft);
        }
        // Mesma binade e vizinhança: cancelamento e near path.
        for _ in 0..1_000_000 {
            let a = rng.f32_bits();
            let b = (a & 0xFF80_0000) ^ ((rng.next_u64() as u32) & 0x807F_FFFF);
            check_pair(f32::from_bits(a), f32::from_bits(b), &mut soft);
        }
        eprintln!("softfloat-1.0 (Rust) divergências do nativo: {soft} de 6.000.000 operações");
    }

    /// Contraexemplo documentado: o crate `softfloat` 1.0 arredonda este
    /// produto subnormal para 3 quanta; o correto (nativo e oráculo exato) é 2.
    #[test]
    fn softfloat_crate_subnormal_counterexample() {
        let (a, b) = (-1.825465e-39_f32, 1.8195308e-6_f32);
        let exact = mul(Format::BINARY32, Pnf::from_f32(a), Pnf::from_f32(b)).to_f32();
        assert_eq!(exact.to_bits(), (a * b).to_bits());
        assert_eq!(exact.to_bits(), 0x8000_0002);
        let soft = softfloat::F32::from_native_f32(a).mul(softfloat::F32::from_native_f32(b));
        assert_eq!(soft.to_bits(), 0x8000_0003);
    }

    #[test]
    fn hard_cases() {
        let hard = [
            0.0_f32,
            -0.0,
            1.0,
            0.5,
            f32::MIN_POSITIVE,
            f32::MAX,
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::NAN,
            f32::from_bits(1),
            f32::from_bits(0x007F_FFFF),
            1.0 - f32::EPSILON / 2.0,
            1.0 + f32::EPSILON,
            1.0 / 3.0,
            2.0 / 3.0,
            3.9,
            3.5,
            0.1,
            f32::from_bits(0x0080_0001),
            -f32::MIN_POSITIVE,
            1e-45,
            2e-45,
        ];
        let mut soft = 0;
        for a in hard {
            for b in hard {
                check_pair(a, b, &mut soft);
            }
        }
    }

    #[test]
    fn logistic_trajectory_matches_native() {
        let f = Format::BINARY32;
        for r in [3.5_f32, 3.7, 3.9, 4.0] {
            let mut native = 0.1_f32;
            let mut ours = Pnf::from_f32(0.1);
            let rp = Pnf::from_f32(r);
            for _ in 0..1000 {
                native = (native * (1.0 - native)) * r;
                ours = logistic_step(f, ours, rp);
                assert_eq!(ours.to_f32().to_bits(), native.to_bits());
            }
        }
    }
}
