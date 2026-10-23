//! G2 diádico: o fold aceita o certificado honesto, encadeia o estado e o
//! veredito, reproduz o passo diádico e rejeita adulterações do certificado,
//! nas duas larguras de range check.

use dyadic_fused::{
    decision,
    shadow::{certify, map_for, orbit, to_grid, DyadicShadowCircuit, Widths, SHADOW_ARITY},
    step, to_fixed, Profile,
};
use nova_snark::{
    frontend::{num::AllocatedNum, test_cs::TestConstraintSystem, ConstraintSystem},
    provider::PallasEngine,
    traits::{circuit::StepCircuit, Engine},
};
use num_bigint::BigInt;
use pnf_circuit::gadgets::{to_biguint, CheckCS};
use shadow_proof::cert::check_grid;

type Fq = <PallasEngine as Engine>::Scalar;

const PROFILES: [(u32, u32); 4] = [(24, 32), (32, 40), (64, 72), (110, 113)];

fn run(c: &DyadicShadowCircuit, z: &[Fq]) -> (bool, Vec<Fq>) {
    let mut cs = CheckCS::<Fq>::default();
    let zs: Vec<AllocatedNum<Fq>> = z.iter().map(|v| AllocatedNum::alloc(&mut cs, || Ok(*v)).unwrap()).collect();
    let out = c.synthesize(&mut cs, &zs).unwrap();
    (cs.is_satisfied(), out.iter().map(|o| o.get_value().unwrap()).collect())
}

fn setup(p: u32, k: u32, r: f32, x0: f32, n: usize, widths: Widths) -> Vec<DyadicShadowCircuit> {
    let prof = Profile::new(p);
    let rr = to_fixed(r as f64, prof.s);
    let map = map_for(prof, &rr, k);
    let orb = orbit(prof, &rr, &to_fixed(x0 as f64, p), n);
    let cert = certify(prof, &map, &orb).expect("certificado");
    let grid: Vec<BigInt> = orb.iter().map(|x| to_grid(prof, k, x)).collect();
    check_grid(&map, &grid, &cert, &cert.max_dist).unwrap();
    DyadicShadowCircuit::trajectory(prof, &rr, &map, &orb, &cert, &cert.max_dist, widths)
}

#[test]
fn map_matches_profile_r() {
    for r in [3.7f32, 3.9, 4.0] {
        let prof = Profile::new(32);
        let rr = to_fixed(r as f64, prof.s);
        let map = map_for(prof, &rr, 40);
        assert_eq!(map.r(), r as f64, "r={r}");
    }
}

#[test]
fn honest_folds_chain_in_both_widths() {
    for (p, k) in PROFILES {
        for widths in [Widths::Full, Widths::Tight] {
            if widths == Widths::Full && 2 * k + 25 > 253 {
                continue;
            }
            let cs = setup(p, k, 3.9, 0.1, 300, widths);
            let mut z = cs[0].z0::<Fq>();
            let mut x = cs[0].x.clone();
            for (n, c) in cs.iter().enumerate() {
                let (ok, out) = run(c, &z);
                assert!(ok, "p={p} K={k} {widths:?} fold {n}");
                x = step(c.prof, &x, &c.r);
                assert_eq!(to_biguint(out[1]), x);
                assert_eq!(out[2], Fq::from(decision(c.prof, &x) as u64));
                // limiar 1/2 = ponto crítico: lado certificado = decisão, sempre robusto
                let v = out[SHADOW_ARITY - 1];
                assert!(v == Fq::from(0) || v == Fq::from(1), "p={p} fold {n} indeterminado");
                z = out;
            }
        }
    }
}

#[test]
fn tampering_is_rejected() {
    for (p, k) in PROFILES {
        for widths in [Widths::Full, Widths::Tight] {
            if widths == Widths::Full && 2 * k + 25 > 253 {
                continue;
            }
            let cs = setup(p, k, 3.9, 0.1, 64, widths);
            let c = &cs[30];
            let z = c.z0::<Fq>();
            assert!(run(c, &z).0);
            // I_{n+1} alargado além da imagem exata f(I_n). (Alargar só até a
            // borda da imagem continua sendo um certificado válido: com grade
            // mais grossa a folga honesta pode passar de um degrau.)
            let (lo_end, hi_end) = if c.side { (&c.cur.b, &c.cur.a) } else { (&c.cur.a, &c.cur.b) };
            let unit = BigInt::from(1) << (c.map.rs + k) as usize;
            let lo_img = c.map.image_num(lo_end);
            let hi_img = c.map.image_num(hi_end);
            let min_a = (&lo_img + &unit - 1) / &unit; // menor A' com A'·unit ≥ f(lo)
            let max_b = &hi_img / &unit; // maior B' com B'·unit ≤ f(hi)
            assert!(c.next.a >= min_a && c.next.b <= max_b);
            let mut t = c.clone();
            t.next.a = &min_a - 1;
            assert!(!run(&t, &z).0, "p={p} {widths:?}: A' fora de f(I_n) aceito");
            let mut t = c.clone();
            t.next.b = &max_b + 1;
            assert!(!run(&t, &z).0, "p={p} {widths:?}: B' fora de f(I_n) aceito");
            // na borda exata da imagem: válido e aceito (se a largura do intervalo couber)
            let mut t = c.clone();
            t.next.a = min_a.clone();
            t.next.b = max_b.clone();
            if widths == Widths::Full {
                assert!(run(&t, &z).0, "p={p}: borda exata de f(I_n) rejeitada");
            }
            let mut t = c.clone();
            t.side = !t.side;
            assert!(!run(&t, &z).0, "p={p} {widths:?}: lado trocado aceito");
            // intervalo atual deslocado para longe da pseudo-órbita
            let mut t = c.clone();
            t.cur.a += &t.eps * 3;
            t.cur.b += &t.eps * 3;
            assert!(!run(&t, &t.z0::<Fq>()).0, "p={p} {widths:?}: I_n longe de p_n aceito");
            // decisão de entrada errada (z adulterado)
            let mut zb = z.clone();
            zb[2] = Fq::from(1) - zb[2];
            let (ok, out) = run(c, &zb);
            assert!(!ok || out[SHADOW_ARITY - 1] == Fq::from(2), "p={p}: decisão trocada passou como robusta");
            // ε = 0 falha em algum fold
            let rejected = cs.iter().any(|c| {
                let mut t = c.clone();
                t.eps = BigInt::from(0);
                !run(&t, &t.z0::<Fq>()).0
            });
            assert!(rejected, "p={p} {widths:?}: ε = 0 aceito em todos os folds");
        }
    }
}

#[test]
fn test_cs_agrees() {
    for (p, k) in PROFILES {
        let cs = setup(p, k, 3.9, 0.3, 32, Widths::Tight);
        let c = &cs[10];
        let mut tcs = TestConstraintSystem::<Fq>::new();
        let z: Vec<AllocatedNum<Fq>> = c
            .z0::<Fq>()
            .iter()
            .enumerate()
            .map(|(i, v)| AllocatedNum::alloc(tcs.namespace(|| format!("z{i}")), || Ok(*v)).unwrap())
            .collect();
        c.synthesize(&mut tcs, &z).unwrap();
        assert!(tcs.is_satisfied(), "p={p}: {:?}", tcs.which_is_unsatisfied());
    }
}
