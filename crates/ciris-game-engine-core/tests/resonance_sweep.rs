//! Resonance sweep — BACKLOG #3 (post-fix).
//!
//! The §4.3 trigger now has two gates: a scale floor (`min(|M|,|N|) ≥
//! RESONANCE_MIN_SIZE`, Gap 1) and the condition `δ·min > 0.5·(α·M + γ·N)` with
//! δ = 0.062 (Gap 2). Net effect: small groups near each other only vibe; big
//! groups near each other excite.

use ciris_game_engine_core::temperature::{
    resonance_triggers, temperature, ALPHA, DELTA, GAMMA, RESONANCE_MIN_SIZE,
};

fn sweep_map() -> [[bool; 14]; 14] {
    let mut map = [[false; 14]; 14];
    for m in 1..=14usize {
        for n in 1..=14usize {
            map[m - 1][n - 1] = resonance_triggers(m, n);
        }
    }
    map
}

#[test]
fn scale_floor_gates_small_pairs() {
    // Gap 1: nothing below the size floor resonates, no matter the partner.
    for m in 1..=14usize {
        for n in 1..=14usize {
            if m.min(n) < RESONANCE_MIN_SIZE {
                assert!(
                    !resonance_triggers(m, n),
                    "({m},{n}) below floor must not fire"
                );
            }
        }
    }
}

#[test]
fn big_balanced_pairs_excite_small_ones_vibe() {
    // Tiny balanced pairs now vibe (the Gap-1 fix).
    assert!(!resonance_triggers(1, 1));
    assert!(!resonance_triggers(2, 2));
    assert!(!resonance_triggers(3, 3));
    // Big balanced pairs excite, from the floor up to the max.
    for s in RESONANCE_MIN_SIZE..=14usize {
        assert!(resonance_triggers(s, s), "({s},{s}) should excite");
    }
    // Exactly (14 - floor + 1) balanced pairs fire.
    let diag_fires = (1..=14usize).filter(|&s| resonance_triggers(s, s)).count();
    assert_eq!(diag_fires, 14 - RESONANCE_MIN_SIZE + 1);
}

#[test]
fn near_atari_pairs_fire() {
    // Gap 2: both meshes near atari should resonate.
    assert!(resonance_triggers(6, 5), "(6,5) near-atari must fire");
    assert!(resonance_triggers(5, 6), "(5,6) near-atari must fire");
    assert!(resonance_triggers(6, 6));
    assert!(resonance_triggers(5, 5));
    assert!(resonance_triggers(7, 7));
}

#[test]
fn lopsided_pairs_do_not_fire() {
    for &(m, n) in &[(6, 1), (1, 6), (14, 1), (14, 2), (10, 3), (7, 3), (8, 2)] {
        assert!(
            !resonance_triggers(m, n),
            "({m},{n}) lopsided must not fire"
        );
    }
}

#[test]
fn formula_asymmetry_alpha_ne_gamma() {
    // With α ≠ γ the trigger is direction-sensitive even above the floor:
    // (4,5) fires but (5,4) does not.
    assert!(resonance_triggers(4, 5), "(4,5) fires");
    assert!(!resonance_triggers(5, 4), "(5,4) does not");
    assert!(
        (ALPHA - GAMMA).abs() > 1e-12,
        "α ({ALPHA}) ≠ γ ({GAMMA}) drives the asymmetry"
    );
}

#[test]
fn every_firing_pair_is_above_the_floor() {
    let map = sweep_map();
    for m in 1..=14usize {
        for n in 1..=14usize {
            if map[m - 1][n - 1] {
                assert!(
                    m.min(n) >= RESONANCE_MIN_SIZE,
                    "firing pair ({m},{n}) must clear the floor"
                );
            }
        }
    }
    // Sanity: the band is non-empty but far from saturating the 14×14 grid.
    let count: usize = map.iter().flat_map(|r| r.iter()).filter(|&&b| b).count();
    assert!(count > 0 && count < 196, "band size {count} out of range");
}

#[test]
fn balanced_condition_uses_delta_above_floor() {
    // For balanced pairs above the floor the condition reduces to δ > 0.5·(α+γ).
    let balanced_rhs = 0.5 * (ALPHA + GAMMA);
    assert!(
        DELTA > balanced_rhs,
        "δ ({DELTA:.3}) > 0.5·(α+γ) ({balanced_rhs:.3}): big balanced pairs excite"
    );
}

#[test]
fn temperature_monotone_lone_mesh() {
    for s in 1usize..14 {
        assert!(
            temperature(s + 1, &[]) > temperature(s, &[]),
            "temperature must increase with lone-mesh size at {s}"
        );
    }
}

#[test]
fn temperature_enemy_heats_smaller_mesh() {
    assert!(
        temperature(3, &[6]) > temperature(3, &[]),
        "an adjacent larger enemy raises temperature"
    );
}
