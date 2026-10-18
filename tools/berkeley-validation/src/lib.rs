//! Validação diferencial da PNF32 contra Berkeley SoftFloat 3 (C), o árbitro
//! de fato do IEEE 754. Compara o oráculo `pnf-reference` E a saída do
//! circuito (CheckCS) com o resultado de Berkeley, bit a bit (NaN: só a
//! classe, pois o payload depende da especialização).

#[cfg(all(test, target_arch = "x86_64", target_os = "linux"))]
mod tests {
    use nova_snark::{provider::PallasEngine, traits::Engine};
    use pnf_circuit::{gadgets::CheckCS, ops, pnf::PnfVar};
    use pnf_reference::{self as reference, Format, Pnf, Rng};
    use softfloat_wrapper::{Float, RoundingMode, F32};

    type Fq = <PallasEngine as Engine>::Scalar;
    const B32: Format = Format::BINARY32;
    const RNE: RoundingMode = RoundingMode::TiesToEven;

    fn same(a: u32, b: u32) -> bool {
        (f32::from_bits(a).is_nan() && f32::from_bits(b).is_nan()) || a == b
    }

    fn berkeley(op: u8, a: u32, b: u32) -> u32 {
        let (x, y) = (F32::from_bits(a), F32::from_bits(b));
        match op {
            0 => x.add(y, RNE),
            1 => x.sub(y, RNE),
            _ => x.mul(y, RNE),
        }
        .to_bits()
    }

    fn ours(op: u8, a: u32, b: u32, circuit: bool) -> u32 {
        let (pa, pb) = (Pnf::from_bits(B32, a as u64), Pnf::from_bits(B32, b as u64));
        let v = if circuit {
            let mut cs = CheckCS::<Fq>::default();
            let (x, y) = (
                PnfVar::alloc_unchecked(&mut cs, B32, pa),
                PnfVar::alloc_unchecked(&mut cs, B32, pb),
            );
            let out = match op {
                0 => ops::add(&mut cs, B32, &x, &y, None),
                1 => ops::sub(&mut cs, B32, &x, &y, None),
                _ => ops::mul(&mut cs, B32, &x, &y, None),
            };
            assert!(cs.is_satisfied());
            out.value(B32)
        } else {
            match op {
                0 => reference::add(B32, pa, pb),
                1 => reference::sub(B32, pa, pb),
                _ => reference::mul(B32, pa, pb),
            }
        };
        v.to_bits(B32) as u32
    }

    #[test]
    fn reference_matches_berkeley() {
        let mut rng = Rng(0xBE4C);
        for _ in 0..2_000_000 {
            let (a, b) = (rng.f32_bits(), rng.f32_bits());
            for op in 0..3 {
                assert!(
                    same(ours(op, a, b, false), berkeley(op, a, b)),
                    "op {op} {a:#x} {b:#x}"
                );
            }
        }
    }

    #[test]
    fn circuit_matches_berkeley() {
        let mut rng = Rng(0xC1C);
        for _ in 0..100_000 {
            let (a, b) = (rng.f32_bits(), rng.f32_bits());
            for op in 0..3 {
                assert!(
                    same(ours(op, a, b, true), berkeley(op, a, b)),
                    "op {op} {a:#x} {b:#x}"
                );
            }
        }
    }
}
