//! Operações PNF bit-exatas (IEEE 754, RNE) em R1CS.
//!
//! Cada operação recebe *hints* opcionais: são exatamente as variáveis de
//! escolha do provador (bits que não são decomposição de um valor já fixado).
//! Todas as demais variáveis são funções determinísticas das entradas e dos
//! hints. Por isso, enumerar todos os hints enumera todos os witnesses
//! possíveis — é o que o teste de unicidade exaustiva faz (ver
//! `docs/02-especificacao-pnf32.md`, seção Unicidade).

use ff::PrimeField;
use nova_snark::frontend::ConstraintSystem;
use pnf_reference::Format;

use crate::gadgets::*;
use crate::pnf::PnfVar;

/// Larguras e constantes derivadas do formato.
#[derive(Clone, Copy, Debug)]
pub struct Widths {
    pub p: i128,
    pub emin: i128,
    pub emax: i128,
    /// mul: corte fixo `K = 2p+1`; bits de `k = K - dexp`
    pub mul_k: i128,
    pub mul_kb: u32,
    pub mul_c: i128,
    pub mul_yb: u32,
    pub mul_ob: u32,
    /// add: corte fixo `K2 = p+3`; bits de `k2`
    pub add_k: i128,
    pub add_kb: u32,
    pub add_db: u32,
    pub add_bigb: u32,
    pub add_powb: u32,
    pub add_ub: u32,
    pub add_ob: u32,
}

impl Widths {
    pub fn of(fmt: Format) -> Self {
        let p = fmt.p as i128;
        let (emin, emax) = (fmt.emin() as i128, fmt.emax() as i128);
        let mul_k = 2 * p + 1;
        let mul_c = mul_k - (emin + p - 1 - 2 * emax);
        let add_k = p + 3;
        let bl = |v: i128| bitlen(v.max(1) as u128);
        Self {
            p,
            emin,
            emax,
            mul_k,
            mul_kb: bl(mul_k + p - 1),
            mul_c,
            mul_yb: bl((2 * emax - emin + 1).max(2 * (emax - emin))),
            mul_ob: bl((emax - 2 * emin - p - 2).max(emax + 1)),
            add_k,
            add_kb: bl(add_k + p - 1),
            add_db: bl(emax - emin),
            add_bigb: bl((emax - emin - (p + 3)).max(p + 2)),
            add_powb: bl(p + 2),
            add_ub: bl(add_k - (emin + p + 2) + emax),
            add_ob: bl(emax - emin + 2 * p + 1),
        }
    }
}

// ---------------------------------------------------------------------------
// Arredondamento comum: guard/sticky/RNE/carry/zero
// ---------------------------------------------------------------------------

struct Rounded<F: PrimeField> {
    m: Expr<F>,
    carry: Expr<F>,
    zf: Expr<F>,
}

/// `Q` (p bits) e `R` (cut bits) já decompostos; RNE sobre o bit guard
/// `R[cut-1]` e o sticky `R mod 2^(cut-1) != 0`.
fn round_rne<F: PrimeField, CS: ConstraintSystem<F>>(
    cs: &mut CS,
    p: u32,
    qbits: &[Expr<F>],
    rbits: &[Expr<F>],
) -> Rounded<F> {
    let _g = Category::enter("rounding");
    let cut = rbits.len();
    let q = Expr::from_bits(qbits, 0);
    let guard = &rbits[cut - 1];
    let low = Expr::from_bits(&rbits[..cut - 1], 0);
    let sticky = is_zero(cs, &low).not();
    let lsb = &qbits[0];
    let sl = product(cs, &sticky, lsb);
    let up = product(cs, guard, &sticky.add(lsb).sub(&sl));
    let qr = q.add(&up);
    let all_ones = is_zero(cs, &q.sub(&Expr::constant(f_pow2::<F>(p) - F::ONE)));
    let carry = product(cs, &up, &all_ones);
    let m = qr.sub(&carry.scale(f_pow2(p - 1)));
    let zf = is_zero(cs, &qr);
    Rounded { m, carry, zf }
}

// ---------------------------------------------------------------------------
// Multiplicação
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MulHints {
    /// `k = K - dexp`, onde `2^dexp` é o quantum relativo do quociente.
    pub k: u64,
    pub ovf: bool,
}

pub fn honest_mul_hints<F: PrimeField>(fmt: Format, a: &PnfVar<F>, b: &PnfVar<F>) -> MulHints {
    let w = Widths::of(fmt);
    let (ma, mb) = (a.m.i(), b.m.i());
    let (ea, eb) = (a.e(fmt).i(), b.e(fmt).i());
    let prod = (ma * mb) as u128;
    let fab = a.fnz().b() && b.fnz().b();
    let dexp = if !fab || prod == 0 {
        w.mul_k
    } else {
        let lead = bitlen(prod) as i128 - 1;
        let normal = lead - (w.p - 1);
        let sub = w.emin + w.p - 1 - ea - eb;
        if normal >= sub {
            normal
        } else {
            sub.min(w.mul_k)
        }
    };
    let k = (w.mul_k - dexp) as u32;
    let (q, r) = split(prod << k, w.mul_k as u32);
    let carry = rounds_to_carry(q, r, w.mul_k as u32, fmt.p);
    let e_out = ea + eb - w.p + 1 + dexp + carry as i128;
    MulHints {
        k: k as u64,
        ovf: e_out > w.emax,
    }
}

fn split(n: u128, cut: u32) -> (u128, u128) {
    (n >> cut, n & ((1 << cut) - 1))
}

fn rounds_to_carry(q: u128, r: u128, cut: u32, p: u32) -> bool {
    let guard = (r >> (cut - 1)) & 1 == 1;
    let sticky = r & ((1 << (cut - 1)) - 1) != 0;
    let up = guard && (sticky || q & 1 == 1);
    up && q == (1 << p) - 1
}

pub fn mul<F: PrimeField, CS: ConstraintSystem<F>>(
    cs: &mut CS,
    fmt: Format,
    a: &PnfVar<F>,
    b: &PnfVar<F>,
    hints: Option<MulHints>,
) -> PnfVar<F> {
    let w = Widths::of(fmt);
    let h = hints.unwrap_or_else(|| honest_mul_hints(fmt, a, b));
    let (ea, eb) = (a.e(fmt), b.e(fmt));

    let (fab, sx, prod) = {
        let _g = Category::enter("arith");
        let fab = product(cs, &a.fnz(), &b.fnz());
        let sasb = product(cs, &a.s, &b.s);
        let sx = a.s.add(&b.s).sub(&sasb.scale_int(2));
        (fab, sx, product(cs, &a.m, &b.m))
    };

    // Quociente com corte fixo: P * 2^k = Q * 2^K + R.
    let kbits = {
        let _g = Category::enter("range");
        (0..w.mul_kb)
            .map(|i| bit(cs, (h.k >> i) & 1 == 1))
            .collect::<Vec<_>>()
    };
    let k = Expr::from_bits(&kbits, 0);
    let dexp = Expr::int(w.mul_k).sub(&k);
    let scale = pow2(cs, &kbits);
    let scaled = {
        let _g = Category::enter("arith");
        product(cs, &prod, &scale)
    };
    let (qbits, rbits) = split_bits(cs, &scaled, w.mul_k as u32, fmt.p);
    let t = qbits[fmt.p as usize - 1].clone();

    // Caminho normal (t=1): dexp >= w. Subnormal (t=0): dexp = w ou dexp = K.
    {
        let _g = Category::enter("exponent");
        let wv = ea
            .add(&eb)
            .scale_int(-1)
            .add_int(w.emin + w.p - 1)
            .add(&fab.not().scale_int(w.mul_c));
        let delta = dexp.sub(&wv);
        let sign = t.scale_int(2).add_int(-1);
        range_product(cs, &sign, &delta, &Expr::zero(), w.mul_yb);
        let m1 = product(cs, &t.not(), &delta);
        enforce(cs, &m1, &k, &Expr::zero());
    }

    let rd = round_rne(cs, fmt.p, &qbits, &rbits);
    let e_out = ea.add(&eb).add_int(1 - w.p).add(&dexp).add(&rd.carry);
    let ovf = overflow(cs, &w, &e_out, h.ovf, w.mul_ob, &rd.zf);

    let _g = Category::enter("class");
    let nor = or(cs, &a.nan, &b.nan);
    let iazb = product(cs, &a.inf, &b.z);
    let zaib = product(cs, &a.z, &b.inf);
    let nan = nor.add(&iazb).add(&zaib);
    let ior = or(cs, &a.inf, &b.inf);
    let iany = or(cs, &ior, &ovf);
    finish(cs, &w, nan, iany, sx, rd, e_out)
}

/// Decompõe `scaled = Q*2^cut + R`, `Q < 2^p`, `R < 2^cut`.
fn split_bits<F: PrimeField, CS: ConstraintSystem<F>>(
    cs: &mut CS,
    scaled: &Expr<F>,
    cut: u32,
    p: u32,
) -> (Vec<Expr<F>>, Vec<Expr<F>>) {
    let _g = Category::enter("range");
    let n = scaled.u();
    let rbits: Vec<Expr<F>> = (0..cut).map(|i| bit(cs, n.bit(i as u64))).collect();
    let qbits: Vec<Expr<F>> = (0..p).map(|i| bit(cs, n.bit((cut + i) as u64))).collect();
    let recomposed = Expr::from_bits(&rbits, 0).add(&Expr::from_bits(&qbits, cut));
    assert_zero(cs, &recomposed.sub(scaled));
    (qbits, rbits)
}

/// Bit de overflow honesto `e_out > emax`, mascarado quando o resultado é zero.
fn overflow<F: PrimeField, CS: ConstraintSystem<F>>(
    cs: &mut CS,
    w: &Widths,
    e_out: &Expr<F>,
    hint: bool,
    nbits: u32,
    zf: &Expr<F>,
) -> Expr<F> {
    let _g = Category::enter("exponent");
    let ovf = bit(cs, hint);
    // ovf=1: e_out-(emax+1) >= 0;  ovf=0: emax-e_out >= 0.
    let d = e_out.add_int(-(w.emax + 1));
    let sign = ovf.scale_int(2).add_int(-1);
    range_product(cs, &sign, &d, &ovf.not(), nbits);
    product(cs, &ovf, &zf.not())
}

/// Seleção final de classe, sinal, significando e expoente.
fn finish<F: PrimeField, CS: ConstraintSystem<F>>(
    cs: &mut CS,
    w: &Widths,
    nan: Expr<F>,
    inf_any: Expr<F>,
    sign: Expr<F>,
    rd: Rounded<F>,
    e_out: Expr<F>,
) -> PnfVar<F> {
    let _g = Category::enter("class");
    let inf = product(cs, &nan.not(), &inf_any);
    let z = product(cs, &nan.not().sub(&inf), &rd.zf);
    let fnz = nan.not().sub(&inf).sub(&z);
    let s = product(cs, &nan.not(), &sign);
    let m = product(cs, &fnz, &rd.m);
    let eb = product(cs, &fnz, &e_out.add_int(-w.emin));
    PnfVar {
        s,
        eb,
        m,
        z,
        inf,
        nan,
    }
}

// ---------------------------------------------------------------------------
// Adição
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AddHints {
    /// troca para que `eA >= eB`
    pub swap: bool,
    /// `d >= p+3`: B contribui só como sticky
    pub big: bool,
    /// soma exata negativa
    pub neg: bool,
    /// `k = K2 - dexp`
    pub k: u64,
    pub ovf: bool,
}

pub fn honest_add_hints<F: PrimeField>(fmt: Format, a: &PnfVar<F>, b: &PnfVar<F>) -> AddHints {
    let w = Widths::of(fmt);
    let (ea, eb) = (a.e(fmt).i(), b.e(fmt).i());
    let swap = eb > ea;
    let (big_side, small) = if swap { (b, a) } else { (a, b) };
    let (e_a, e_b) = if swap { (eb, ea) } else { (ea, eb) };
    let d = e_a - e_b;
    let big = d >= w.p + 3;
    let y: i128 = if big {
        small.fnz().b() as i128
    } else {
        small.m.i() << (w.p + 2 - d)
    };
    let sgn = |v: &PnfVar<F>| if v.s.b() { -1 } else { 1 };
    let x = sgn(big_side) * (big_side.m.i() << (w.p + 2)) + sgn(small) * y;
    let mag = x.unsigned_abs();
    let dexp = if mag == 0 {
        w.add_k
    } else {
        let normal = bitlen(mag) as i128 - 1 - (w.p - 1);
        normal.max(w.emin + w.p + 2 - e_a)
    };
    let k = (w.add_k - dexp) as u32;
    let (q, r) = split(mag << k, w.add_k as u32);
    let carry = rounds_to_carry(q, r, w.add_k as u32, fmt.p);
    let e_out = e_a - w.p - 2 + dexp + carry as i128;
    AddHints {
        swap,
        big,
        neg: x < 0,
        k: k as u64,
        ovf: e_out > w.emax,
    }
}

pub fn add<F: PrimeField, CS: ConstraintSystem<F>>(
    cs: &mut CS,
    fmt: Format,
    a: &PnfVar<F>,
    b: &PnfVar<F>,
    hints: Option<AddHints>,
) -> PnfVar<F> {
    let w = Widths::of(fmt);
    let p = fmt.p;
    let h = hints.unwrap_or_else(|| honest_add_hints(fmt, a, b));

    // Ordenação por expoente (eA >= eB); a troca só move 4 campos.
    let (s_a, e_a, m_a, s_b, e_b, m_b, f_b) = {
        let _g = Category::enter("align");
        let sw = bit(cs, h.swap);
        let s_a = select(cs, &sw, &b.s, &a.s);
        let e_a = select(cs, &sw, &b.eb, &a.eb);
        let m_a = select(cs, &sw, &b.m, &a.m);
        let f_b = select(cs, &sw, &a.fnz(), &b.fnz());
        let s_b = a.s.add(&b.s).sub(&s_a);
        let e_b = a.eb.add(&b.eb).sub(&e_a);
        let m_b = a.m.add(&b.m).sub(&m_a);
        (s_a, e_a, m_a, s_b, e_b, m_b, f_b)
    };
    let d = e_a.sub(&e_b);
    range(cs, &d, w.add_db);

    // Alinhamento com shift limitado: d <= p+2 exato; d >= p+3 vira sticky.
    let y = {
        let _g = Category::enter("align");
        let big = bit(cs, h.big);
        let sign = big.scale_int(2).add_int(-1);
        let ybits = range_product(cs, &sign, &d.add_int(-(w.p + 3)), &big.not(), w.add_bigb);
        let shift = pow2(cs, &ybits[..w.add_powb as usize]);
        let _g2 = Category::enter("align");
        let shifted = product(cs, &m_b, &shift);
        let exact = product(cs, &big.not(), &shifted);
        let sticky = product(cs, &big, &f_b);
        exact.add(&sticky)
    };

    // Soma exata com sinal e magnitude.
    let (mag, neg) = {
        let _g = Category::enter("arith");
        let ta = product(cs, &s_a.scale_int(-2).add_int(1), &m_a).scale(f_pow2(p + 2));
        let tb = product(cs, &s_b.scale_int(-2).add_int(1), &y);
        let x = ta.add(&tb);
        let neg = bit(cs, h.neg);
        (product(cs, &neg.scale_int(-2).add_int(1), &x), neg)
    };

    // Normalização por shift testemunhado: |X| * 2^k = Q * 2^K2 + R.
    let kbits = {
        let _g = Category::enter("range");
        (0..w.add_kb)
            .map(|i| bit(cs, (h.k >> i) & 1 == 1))
            .collect::<Vec<_>>()
    };
    let dexp = Expr::int(w.add_k).sub(&Expr::from_bits(&kbits, 0));
    let scale = pow2(cs, &kbits);
    let scaled = {
        let _g = Category::enter("arith");
        product(cs, &mag, &scale)
    };
    let (qbits, rbits) = split_bits(cs, &scaled, w.add_k as u32, p);
    let t = qbits[p as usize - 1].clone();
    let e_a_abs = e_a.add_int(w.emin);
    let rd = round_rne(cs, p, &qbits, &rbits);

    // Quantum >= 2^(emin-p+1); subnormal (t=0, não zero) fixa o quantum.
    {
        let _g = Category::enter("exponent");
        let u = dexp.add(&e_a_abs).add_int(-(w.emin + w.p + 2));
        range(cs, &u, w.add_ub);
        let v = product(cs, &t.not(), &u);
        enforce(cs, &v, &rd.zf.not(), &Expr::zero());
        // A soma nunca arredonda para zero (Hauser): zero só se |X| = 0.
        // Sem esta linha um dexp grande zerava Q e liberava a exceção acima
        // (contraexemplo achado pela enumeração exaustiva; registro 0003).
        enforce(cs, &rd.zf, &mag, &Expr::zero());
    }
    let e_out = e_a_abs.add_int(-w.p - 2).add(&dexp).add(&rd.carry);
    let ovf = overflow(cs, &w, &e_out, h.ovf, w.add_ob, &rd.zf);

    let _g = Category::enter("class");
    let nor = or(cs, &a.nan, &b.nan);
    let iab = product(cs, &a.inf, &b.inf);
    let sasb = product(cs, &a.s, &b.s);
    let sx = a.s.add(&b.s).sub(&sasb.scale_int(2));
    let nan = nor.add(&product(cs, &iab, &sx));
    let ior = a.inf.add(&b.inf).sub(&iab);
    let iany = or(cs, &ior, &ovf);
    // sinal: zero exato -> sa AND sb (RNE); senão o sinal da soma; inf herda.
    let s_fin = select(cs, &rd.zf, &sasb, &neg);
    let s1 = select(cs, &b.inf, &b.s, &s_fin);
    let s_inf = select(cs, &a.inf, &a.s, &s1);
    finish(cs, &w, nan, iany, s_inf, rd, e_out)
}

pub fn sub<F: PrimeField, CS: ConstraintSystem<F>>(
    cs: &mut CS,
    fmt: Format,
    a: &PnfVar<F>,
    b: &PnfVar<F>,
    hints: Option<AddHints>,
) -> PnfVar<F> {
    add(cs, fmt, a, &b.neg(), hints)
}

// ---------------------------------------------------------------------------
// Decisão x >= 2^kexp
// ---------------------------------------------------------------------------

/// `x >= 2^kexp` para `emin < kexp <= emax`: falso para NaN e negativos,
/// verdadeiro para +inf. Para normais positivos equivale a `e >= kexp`.
pub fn ge_pow2<F: PrimeField, CS: ConstraintSystem<F>>(
    cs: &mut CS,
    fmt: Format,
    x: &PnfVar<F>,
    kexp: i32,
    hint: Option<bool>,
) -> Expr<F> {
    assert!(kexp > fmt.emin() && kexp <= fmt.emax());
    let _g = Category::enter("decision");
    let e = x.e(fmt);
    let honest = !x.z.b() && !x.inf.b() && !x.nan.b() && e.i() >= kexp as i128;
    let b = bit(cs, hint.unwrap_or(honest));
    let n = bitlen((fmt.emax() - kexp).max(kexp - fmt.emin()) as u128);
    let sign = b.scale_int(2).add_int(-1);
    range_product(cs, &sign, &e.add_int(-(kexp as i128)), &b.not(), n);
    let pos = Expr::one().sub(&x.s).sub(&x.nan);
    product(cs, &pos, &b.add(&x.inf))
}
