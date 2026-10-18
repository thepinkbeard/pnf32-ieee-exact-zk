//! Valor PNF no circuito: `(s, eb, m, z, inf, nan)`.
//!
//! - `eb = e - emin >= 0` (expoente deslocado, para o estado IVC caber em
//!   elementos não negativos);
//! - `m` é o significando com bit implícito explícito (`m < 2^p`);
//! - `z`, `inf`, `nan` são flags de classe mutuamente exclusivas; finito não
//!   nulo é `fnz = 1 - z - inf - nan` (linear, sem custo).
//!
//! A semântica é exatamente a de [`pnf_reference::Pnf`].

use ff::PrimeField;
use nova_snark::frontend::{num::AllocatedNum, ConstraintSystem};
use pnf_reference::{Class, Format, Pnf};

use crate::gadgets::*;

#[derive(Clone)]
pub struct PnfVar<F: PrimeField> {
    pub s: Expr<F>,
    pub eb: Expr<F>,
    pub m: Expr<F>,
    pub z: Expr<F>,
    pub inf: Expr<F>,
    pub nan: Expr<F>,
}

pub const ARITY: usize = 6;

impl<F: PrimeField> PnfVar<F> {
    /// Expoente sem deslocamento, `e = eb + emin` (linear).
    pub fn e(&self, fmt: Format) -> Expr<F> {
        self.eb.add_int(fmt.emin() as i128)
    }

    pub fn fnz(&self) -> Expr<F> {
        Expr::one().sub(&self.z).sub(&self.inf).sub(&self.nan)
    }

    fn fields(fmt: Format, v: Pnf) -> [F; ARITY] {
        let b = |x: bool| if x { F::ONE } else { F::ZERO };
        [
            b(v.sign),
            f_i128((v.exp - fmt.emin()) as i128),
            F::from(v.sig),
            b(v.is_zero()),
            b(v.is_inf()),
            b(v.is_nan()),
        ]
    }

    fn from_exprs(e: [Expr<F>; ARITY]) -> Self {
        let [s, eb, m, z, inf, nan] = e;
        Self {
            s,
            eb,
            m,
            z,
            inf,
            nan,
        }
    }

    pub fn constant(fmt: Format, v: Pnf) -> Self {
        Self::from_exprs(Self::fields(fmt, v).map(Expr::constant))
    }

    /// Aloca sem nenhuma checagem (entrada herdada pelo invariante IVC).
    pub fn alloc_unchecked<CS: ConstraintSystem<F>>(cs: &mut CS, fmt: Format, v: Pnf) -> Self {
        Self::from_exprs(Self::fields(fmt, v).map(|x| alloc(cs, x)))
    }

    /// Envolve as variáveis do estado Nova; os valores vêm do witness.
    pub fn from_nums(nums: &[AllocatedNum<F>], fmt: Format, v: Pnf) -> Self {
        let vals = Self::fields(fmt, v);
        let mut it = nums.iter().zip(vals).map(|(n, x)| Expr::from_num(n, x));
        Self::from_exprs(std::array::from_fn(|_| it.next().unwrap()))
    }

    pub fn to_nums<CS: ConstraintSystem<F>>(&self, cs: &mut CS) -> Vec<AllocatedNum<F>> {
        [&self.s, &self.eb, &self.m, &self.z, &self.inf, &self.nan]
            .into_iter()
            .map(|x| to_num(cs, x))
            .collect()
    }

    /// Materializa (1 constraint cada) os campos que não são uma variável
    /// alocada nem constante, para que a combinação linear não cresça ao
    /// encadear passos sem passar pelo estado Nova. A variável nova é
    /// determinada (`v = expr`), então unicidade e semântica não mudam. O
    /// critério não olha `lc`, que fica vazia no modo só-witness.
    pub fn compact<CS: ConstraintSystem<F>>(&self, cs: &mut CS) -> Self {
        let mut c = |e: &Expr<F>| {
            if e.konst || e.var.is_some() {
                e.clone()
            } else {
                let v = alloc(cs, e.val);
                assert_zero(cs, &v.sub(e));
                v
            }
        };
        Self {
            s: c(&self.s),
            eb: c(&self.eb),
            m: c(&self.m),
            z: c(&self.z),
            inf: c(&self.inf),
            nan: c(&self.nan),
        }
    }

    /// Lê o valor semântico (sem checar validade).
    pub fn value(&self, fmt: Format) -> Pnf {
        let class = if self.nan.b() {
            Class::Nan
        } else if self.inf.b() {
            Class::Inf
        } else {
            Class::Finite
        };
        Pnf {
            sign: self.s.b(),
            exp: (self.eb.i() + fmt.emin() as i128) as i32,
            sig: u64::try_from(self.m.u()).unwrap_or(u64::MAX),
            class,
        }
    }

    pub fn neg(&self) -> Self {
        // NaN canônico tem sinal 0; as operações recanonicalizam o sinal de NaN.
        Self {
            s: self.s.not(),
            ..self.clone()
        }
    }

    /// Checagem completa do invariante de validade (custo da ablação T2 e da
    /// decodificação de entradas externas).
    pub fn validate<CS: ConstraintSystem<F>>(&self, cs: &mut CS, fmt: Format) {
        let _g = Category::enter("validate");
        let erange = bitlen((fmt.emax() - fmt.emin()) as u128);
        for flag in [&self.s, &self.z, &self.inf, &self.nan] {
            enforce(cs, flag, &flag.not(), &Expr::zero());
        }
        let special = self.z.add(&self.inf).add(&self.nan);
        enforce(cs, &special, &special.not(), &Expr::zero());
        let mbits = range(cs, &self.m, fmt.p);
        range(cs, &self.eb, erange);
        range(
            cs,
            &Expr::int((fmt.emax() - fmt.emin()) as i128).sub(&self.eb),
            erange,
        );
        // subnormal (bit implícito 0) só no menor expoente
        let top = &mbits[fmt.p as usize - 1];
        enforce(cs, &top.not(), &self.eb, &Expr::zero());
        // finito não nulo <=> m != 0; classes especiais canônicas
        let inv = alloc(cs, self.m.val.invert().unwrap_or(F::ZERO));
        enforce(cs, &self.m, &inv, &self.fnz());
        enforce(cs, &special, &self.m, &Expr::zero());
        enforce(cs, &special, &self.eb, &Expr::zero());
        enforce(cs, &self.nan, &self.s, &Expr::zero());
    }
}
