//! Gadgets R1CS mínimos sobre o frontend do Nova.
//!
//! [`Expr`] é uma combinação linear com o valor concreto ao lado. Constantes
//! são dobradas: produto por constante e checagens de constantes não geram
//! constraints. Toda constraint passa por [`enforce`], que contabiliza por
//! categoria (ver [`Category`]) para o breakdown de custo.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};

use ff::PrimeField;
use nova_snark::frontend::{
    num::AllocatedNum, ConstraintSystem, Index, LinearCombination, SynthesisError, Variable,
};
use num_bigint::BigUint;

// ---------------------------------------------------------------------------
// Conversões de campo
// ---------------------------------------------------------------------------

pub fn f_u128<F: PrimeField>(v: u128) -> F {
    F::from((v >> 64) as u64) * F::from(1u64 << 32).square() + F::from(v as u64)
}

pub fn f_i128<F: PrimeField>(v: i128) -> F {
    if v >= 0 {
        f_u128(v as u128)
    } else {
        -f_u128::<F>(v.unsigned_abs())
    }
}

pub fn f_pow2<F: PrimeField>(k: u32) -> F {
    let mut x = F::ONE;
    for _ in 0..k {
        x = x.double();
    }
    x
}

/// Inteiro canônico do elemento (assume repr little-endian, como Pallas/Vesta;
/// verificado em teste).
pub fn to_biguint<F: PrimeField>(v: F) -> BigUint {
    BigUint::from_bytes_le(v.to_repr().as_ref())
}

/// Leitura com sinal: valores acima de (p-1)/2 são negativos.
pub fn to_i128<F: PrimeField>(v: F) -> i128 {
    let pos = to_biguint(v);
    if pos.bits() <= 120 {
        return i128::try_from(pos).unwrap();
    }
    let neg = to_biguint(-v);
    assert!(neg.bits() <= 120, "elemento de campo não é inteiro pequeno");
    -i128::try_from(neg).unwrap()
}

// ---------------------------------------------------------------------------
// Contabilidade por categoria
// ---------------------------------------------------------------------------

/// Categoria corrente das constraints emitidas (RAII).
pub struct Category {
    previous: &'static str,
}

thread_local! {
    static CURRENT: RefCell<&'static str> = const { RefCell::new("other") };
    static COUNTS: RefCell<BTreeMap<&'static str, usize>> = const { RefCell::new(BTreeMap::new()) };
}

impl Category {
    pub fn enter(name: &'static str) -> Self {
        let previous = CURRENT.with(|c| std::mem::replace(&mut *c.borrow_mut(), name));
        Self { previous }
    }
}

impl Drop for Category {
    fn drop(&mut self) {
        CURRENT.with(|c| *c.borrow_mut() = self.previous);
    }
}

pub fn reset_counts() {
    COUNTS.with(|c| c.borrow_mut().clear());
}

pub fn take_counts() -> BTreeMap<&'static str, usize> {
    COUNTS.with(|c| std::mem::take(&mut *c.borrow_mut()))
}

// ---------------------------------------------------------------------------
// Modo só-witness
// ---------------------------------------------------------------------------

thread_local! {
    static WITNESS_ONLY: Cell<bool> = const { Cell::new(false) };
}

/// Guarda RAII do modo só-witness. Quando o CS é um gerador de witness
/// (`ConstraintSystem::is_witness_generator`, p.ex. o `WitnessCS` do prover
/// Nova, cujo `enforce` ignora as combinações lineares), `Expr` deixa de
/// montar `lc` e só propaga valores. Alocações e constraints emitidas são as
/// mesmas; só o trabalho descartado some (log 0006). Nunca ligue isto com um
/// CS que avalia ou registra as combinações lineares.
pub struct WitnessOnly {
    previous: bool,
}

impl WitnessOnly {
    pub fn enter(on: bool) -> Self {
        Self {
            previous: WITNESS_ONLY.with(|c| c.replace(on)),
        }
    }
}

impl Drop for WitnessOnly {
    fn drop(&mut self) {
        WITNESS_ONLY.with(|c| c.set(self.previous));
    }
}

#[inline]
fn build_lc() -> bool {
    !WITNESS_ONLY.with(|c| c.get())
}

static NAME: AtomicU64 = AtomicU64::new(0);

fn fresh() -> String {
    NAME.fetch_add(1, Ordering::Relaxed).to_string()
}

pub fn one_var() -> Variable {
    Variable::new_unchecked(Index::Input(0))
}

// ---------------------------------------------------------------------------
// Expr
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct Expr<F: PrimeField> {
    pub lc: LinearCombination<F>,
    pub val: F,
    pub konst: bool,
    /// `Some` quando a expressão é exatamente uma variável alocada.
    pub var: Option<Variable>,
}

impl<F: PrimeField> Expr<F> {
    pub fn constant(c: F) -> Self {
        Self {
            lc: if build_lc() {
                LinearCombination::zero() + (c, one_var())
            } else {
                LinearCombination::zero()
            },
            val: c,
            konst: true,
            var: None,
        }
    }
    pub fn int(v: i128) -> Self {
        Self::constant(f_i128(v))
    }
    pub fn zero() -> Self {
        Self::constant(F::ZERO)
    }
    pub fn one() -> Self {
        Self::constant(F::ONE)
    }
    pub fn from_var(var: Variable, val: F) -> Self {
        Self {
            lc: if build_lc() {
                LinearCombination::zero() + var
            } else {
                LinearCombination::zero()
            },
            val,
            konst: false,
            var: Some(var),
        }
    }
    pub fn from_num(num: &AllocatedNum<F>, val: F) -> Self {
        Self::from_var(num.get_variable(), val)
    }
    pub fn add(&self, o: &Self) -> Self {
        Self {
            lc: if build_lc() {
                self.lc.clone() + &o.lc
            } else {
                LinearCombination::zero()
            },
            val: self.val + o.val,
            konst: self.konst && o.konst,
            var: None,
        }
    }
    pub fn sub(&self, o: &Self) -> Self {
        Self {
            lc: if build_lc() {
                self.lc.clone() - &o.lc
            } else {
                LinearCombination::zero()
            },
            val: self.val - o.val,
            konst: self.konst && o.konst,
            var: None,
        }
    }
    pub fn scale(&self, c: F) -> Self {
        Self {
            lc: if build_lc() {
                LinearCombination::zero() + (c, &self.lc)
            } else {
                LinearCombination::zero()
            },
            val: self.val * c,
            konst: self.konst,
            var: None,
        }
    }
    pub fn add_int(&self, c: i128) -> Self {
        self.add(&Self::int(c))
    }
    pub fn scale_int(&self, c: i128) -> Self {
        self.scale(f_i128(c))
    }
    /// `1 - self`
    pub fn not(&self) -> Self {
        Self::one().sub(self)
    }
    pub fn i(&self) -> i128 {
        to_i128(self.val)
    }
    pub fn u(&self) -> BigUint {
        to_biguint(self.val)
    }
    pub fn b(&self) -> bool {
        self.val == F::ONE
    }
    /// Soma `sum_i 2^(shift+i) * bits[i]`.
    pub fn from_bits(bits: &[Self], shift: u32) -> Self {
        let mut acc = Self::zero();
        let mut c = f_pow2::<F>(shift);
        for bit in bits {
            acc = acc.add(&bit.scale(c));
            c = c.double();
        }
        acc
    }
}

// ---------------------------------------------------------------------------
// Primitivas
// ---------------------------------------------------------------------------

pub fn enforce<F: PrimeField, CS: ConstraintSystem<F>>(
    cs: &mut CS,
    a: &Expr<F>,
    b: &Expr<F>,
    c: &Expr<F>,
) {
    if build_lc() {
        let cat = CURRENT.with(|x| *x.borrow());
        COUNTS.with(|m| *m.borrow_mut().entry(cat).or_insert(0) += 1);
    }
    cs.enforce(
        || fresh(),
        |_| a.lc.clone(),
        |_| b.lc.clone(),
        |_| c.lc.clone(),
    );
}

pub fn alloc<F: PrimeField, CS: ConstraintSystem<F>>(cs: &mut CS, val: F) -> Expr<F> {
    let var = cs.alloc(fresh, || Ok(val)).expect("alloc");
    Expr::from_var(var, val)
}

/// `a == 0`; constantes são checadas fora do circuito (falha vira constraint
/// insatisfazível, nunca panic, para a enumeração exaustiva).
pub fn assert_zero<F: PrimeField, CS: ConstraintSystem<F>>(cs: &mut CS, a: &Expr<F>) {
    if a.konst {
        if a.val != F::ZERO {
            enforce(cs, &Expr::one(), &Expr::one(), &Expr::zero());
        }
        return;
    }
    enforce(cs, a, &Expr::one(), &Expr::zero());
}

pub fn bit<F: PrimeField, CS: ConstraintSystem<F>>(cs: &mut CS, v: bool) -> Expr<F> {
    let b = alloc(cs, if v { F::ONE } else { F::ZERO });
    enforce(cs, &b, &b.not(), &Expr::zero());
    b
}

pub fn product<F: PrimeField, CS: ConstraintSystem<F>>(
    cs: &mut CS,
    a: &Expr<F>,
    b: &Expr<F>,
) -> Expr<F> {
    if a.konst {
        return b.scale(a.val);
    }
    if b.konst {
        return a.scale(b.val);
    }
    let c = alloc(cs, a.val * b.val);
    enforce(cs, a, b, &c);
    c
}

/// Produto que sempre resulta numa variável alocada (saídas do StepCircuit).
pub fn product_num<F: PrimeField, CS: ConstraintSystem<F>>(
    cs: &mut CS,
    a: &Expr<F>,
    b: &Expr<F>,
) -> AllocatedNum<F> {
    let c = alloc(cs, a.val * b.val);
    enforce(cs, a, b, &c);
    AllocatedNum::from_parts(c.var.unwrap(), Some(c.val))
}

/// Converte para variável alocada; custa 1 constraint se não for uma.
pub fn to_num<F: PrimeField, CS: ConstraintSystem<F>>(cs: &mut CS, a: &Expr<F>) -> AllocatedNum<F> {
    if let Some(var) = a.var {
        return AllocatedNum::from_parts(var, Some(a.val));
    }
    let _g = Category::enter("io");
    product_num(cs, a, &Expr::one())
}

pub fn or<F: PrimeField, CS: ConstraintSystem<F>>(
    cs: &mut CS,
    a: &Expr<F>,
    b: &Expr<F>,
) -> Expr<F> {
    let ab = product(cs, a, b);
    a.add(b).sub(&ab)
}

/// `cond ? x : y` com `cond` booleano.
pub fn select<F: PrimeField, CS: ConstraintSystem<F>>(
    cs: &mut CS,
    cond: &Expr<F>,
    x: &Expr<F>,
    y: &Expr<F>,
) -> Expr<F> {
    let d = product(cs, cond, &x.sub(y));
    y.add(&d)
}

/// Bits little-endian dos `n` bits baixos do inteiro canônico de `value`.
fn alloc_bits<F: PrimeField, CS: ConstraintSystem<F>>(
    cs: &mut CS,
    value: &BigUint,
    n: u32,
) -> Vec<Expr<F>> {
    (0..n).map(|i| bit(cs, value.bit(i as u64))).collect()
}

/// Range check `x in [0, 2^n)`: `n` booleanidades + 1 recomposição.
pub fn range<F: PrimeField, CS: ConstraintSystem<F>>(
    cs: &mut CS,
    x: &Expr<F>,
    n: u32,
) -> Vec<Expr<F>> {
    let _g = Category::enter("range");
    let value = x.u();
    if x.konst {
        if value.bits() > n as u64 {
            enforce(cs, &Expr::one(), &Expr::one(), &Expr::zero());
        }
        return (0..n)
            .map(|i| Expr::constant(if value.bit(i as u64) { F::ONE } else { F::ZERO }))
            .collect();
    }
    let bits = alloc_bits(cs, &value, n);
    assert_zero(cs, &Expr::from_bits(&bits, 0).sub(x));
    bits
}

/// `a*b - offset in [0, 2^n)` numa única constraint de recomposição:
/// `a * b = sum(bits) + offset`.
pub fn range_product<F: PrimeField, CS: ConstraintSystem<F>>(
    cs: &mut CS,
    a: &Expr<F>,
    b: &Expr<F>,
    offset: &Expr<F>,
    n: u32,
) -> Vec<Expr<F>> {
    if a.konst || b.konst {
        let prod = product(cs, a, b);
        return range(cs, &prod.sub(offset), n);
    }
    let _g = Category::enter("range");
    let value = to_biguint(a.val * b.val - offset.val);
    let bits = alloc_bits(cs, &value, n);
    enforce(cs, a, b, &Expr::from_bits(&bits, 0).add(offset));
    bits
}

/// Flag `x == 0` (2 constraints): `x*inv = 1-z`, `x*z = 0`.
pub fn is_zero<F: PrimeField, CS: ConstraintSystem<F>>(cs: &mut CS, x: &Expr<F>) -> Expr<F> {
    if x.konst {
        return Expr::constant(if x.val == F::ZERO { F::ONE } else { F::ZERO });
    }
    let _g = Category::enter("zero-test");
    let zero = x.val == F::ZERO;
    let inv = alloc(cs, x.val.invert().unwrap_or(F::ZERO));
    let z = alloc(cs, if zero { F::ONE } else { F::ZERO });
    enforce(cs, x, &inv, &z.not());
    enforce(cs, x, &z, &Expr::zero());
    z
}

/// `2^(sum bits_i 2^i)` por produtos condicionais; o primeiro fator é linear.
pub fn pow2<F: PrimeField, CS: ConstraintSystem<F>>(cs: &mut CS, bits: &[Expr<F>]) -> Expr<F> {
    let _g = Category::enter("shift");
    let mut acc = Expr::one();
    for (i, b) in bits.iter().enumerate() {
        let factor = Expr::one().add(&b.scale(f_pow2::<F>(1 << i) - F::ONE));
        acc = product(cs, &acc, &factor);
    }
    acc
}

pub fn bitlen(v: u128) -> u32 {
    128 - v.leading_zeros()
}

// ---------------------------------------------------------------------------
// CS de checagem rápida
// ---------------------------------------------------------------------------

/// Constraint system que avalia cada constraint ao ser emitida. Usado na
/// enumeração exaustiva e nas trajetórias longas, onde `TestConstraintSystem`
/// (com nomes e hashmaps) seria lento demais. Testes cruzam os dois.
pub struct CheckCS<F: PrimeField> {
    inputs: Vec<F>,
    aux: Vec<F>,
    pub constraints: usize,
    pub failures: usize,
}

impl<F: PrimeField> Default for CheckCS<F> {
    fn default() -> Self {
        Self {
            inputs: vec![F::ONE],
            aux: Vec::new(),
            constraints: 0,
            failures: 0,
        }
    }
}

impl<F: PrimeField> CheckCS<F> {
    fn eval(&self, lc: &LinearCombination<F>) -> F {
        let mut acc = F::ZERO;
        for (var, coeff) in lc.iter() {
            let v = match var.get_unchecked() {
                Index::Input(i) => self.inputs[i],
                Index::Aux(i) => self.aux[i],
            };
            acc += v * coeff;
        }
        acc
    }
    pub fn is_satisfied(&self) -> bool {
        self.failures == 0
    }
    /// Atribuição auxiliar, na ordem de alocação.
    pub fn aux(&self) -> &[F] {
        &self.aux
    }
}

impl<F: PrimeField> ConstraintSystem<F> for CheckCS<F> {
    type Root = Self;

    fn alloc<Fv, A, AR>(&mut self, _: A, f: Fv) -> Result<Variable, SynthesisError>
    where
        Fv: FnOnce() -> Result<F, SynthesisError>,
        A: FnOnce() -> AR,
        AR: Into<String>,
    {
        self.aux.push(f()?);
        Ok(Variable::new_unchecked(Index::Aux(self.aux.len() - 1)))
    }

    fn alloc_input<Fv, A, AR>(&mut self, _: A, f: Fv) -> Result<Variable, SynthesisError>
    where
        Fv: FnOnce() -> Result<F, SynthesisError>,
        A: FnOnce() -> AR,
        AR: Into<String>,
    {
        self.inputs.push(f()?);
        Ok(Variable::new_unchecked(Index::Input(self.inputs.len() - 1)))
    }

    fn enforce<A, AR, LA, LB, LC>(&mut self, _: A, a: LA, b: LB, c: LC)
    where
        A: FnOnce() -> AR,
        AR: Into<String>,
        LA: FnOnce(LinearCombination<F>) -> LinearCombination<F>,
        LB: FnOnce(LinearCombination<F>) -> LinearCombination<F>,
        LC: FnOnce(LinearCombination<F>) -> LinearCombination<F>,
    {
        let (a, b, c) = (
            a(LinearCombination::zero()),
            b(LinearCombination::zero()),
            c(LinearCombination::zero()),
        );
        self.constraints += 1;
        if self.eval(&a) * self.eval(&b) != self.eval(&c) {
            self.failures += 1;
        }
    }

    fn push_namespace<NR, N>(&mut self, _: N)
    where
        NR: Into<String>,
        N: FnOnce() -> NR,
    {
    }

    fn pop_namespace(&mut self) {}

    fn get_root(&mut self) -> &mut Self::Root {
        self
    }
}
