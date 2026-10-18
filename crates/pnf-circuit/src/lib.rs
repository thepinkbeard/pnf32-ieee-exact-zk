//! PNF32: representação binary32 bit-exata com aritmetização barata em R1CS.
//!
//! Ver `docs/02-especificacao-pnf32.md` para as relações e os lemas de
//! unicidade, e `docs/03-modelo-de-custo.md` para a contagem analítica.

pub mod gadgets;
pub mod ops;
pub mod pnf;
pub mod step;

pub use pnf_reference::{Format, Pnf};
