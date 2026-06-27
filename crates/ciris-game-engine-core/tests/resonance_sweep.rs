//! Resonance coefficient sweep — BACKLOG #3.
//!
//! Walks `|M|, |N| ∈ [1, 14]²` to verify structural properties of the §4.3
//! resonance trigger condition and to surface calibration gaps.
//!
//! ## What the formula actually does (α=0.060, β=0.080, γ=0.050, δ=0.060)
//!
//! `δ · min(|M|, |N|) > 0.5 · (α·|M| + γ·|N|)`
//!
//! For `m ≤ n` (min = m): fires when `6m > 5n`.
//! For `m > n` (min = n): fires when `7n > 6m`.
//!
//! **Balanced pairs are scale-free.** When `m = n = s`, the condition reduces
//! to `δ > 0.5·(α + γ)` — size `s` cancels completely. With current defaults
//! `0.060 > 0.055`, so every balanced pair, including (1,1) and (2,2), resonates.
//!
//! **Asymmetry.** Because α ≠ γ, `resonance_triggers(m, n)` is not generally
//! equal to `resonance_triggers(n, m)`: (6, 7) fires but (7, 6) does not.
//!
//! ## Calibration gaps (BACKLOG #3 findings)
//!
//! 1. *Tiny balanced pairs fire.* Design intent says (1,1) and (2,2) should NOT
//!    resonate. They do, because the formula is scale-free. This gap cannot be
//!    closed by coefficient tuning alone — the formula would need a size-floor
//!    term. No coefficient change is made here; see final analysis below.
//!
//! 2. *(6,5) just misses.* Both meshes are near atari; design intent says they
//!    should resonate. With current δ=0.060: LHS=0.300, RHS=0.305 (margin −0.005).
//!    Raising δ to ≥ 0.062 would include (6,5), but gap #1 would remain.
//!
//! 3. *Asymmetric near-boundary.* (6,7) resonates; (7,6) does not. Whether this
//!    is a bug or intentional ("heat flows large→small") is a design question.
//!
//! ## 14×14 band summary
//!
//! With current defaults, exactly 31 of the 196 pairs in [1,14]² resonate:
//! all 14 diagonal pairs, 8 upper-off-diagonal (diff=1, m≥6), 2 upper-off-diagonal
//! (diff=2, m≥11), and 7 lower-off-diagonal (diff=1, n≥7).

use ciris_game_engine_core::temperature::{
    resonance_margin, resonance_triggers, temperature, ALPHA, DELTA, GAMMA,
};

/// Build the full 14×14 boolean resonance map (`map[m-1][n-1]` for pair `(m,n)`).
fn sweep_map() -> [[bool; 14]; 14] {
    let mut map = [[false; 14]; 14];
    for m in 1..=14usize {
        for n in 1..=14usize {
            map[m - 1][n - 1] = resonance_triggers(m, n);
        }
    }
    map
}

// ── Core design-intent properties ──────────────────────────────────────────

/// Large balanced pairs near the collapse threshold (7) must resonate.
#[test]
fn large_balanced_pairs_fire() {
    assert!(resonance_triggers(5, 5), "(5,5): deep mid-game balanced — must resonate");
    assert!(resonance_triggers(6, 6), "(6,6): atari-size balanced — must resonate");
    assert!(resonance_triggers(7, 7), "(7,7): collapse-size balanced — must resonate");
    assert!(resonance_triggers(14, 14), "max balanced pair — must resonate");
}

/// Very lopsided pairs do not resonate regardless of the larger mesh's size.
/// Heat bleeds from large to small (via the β term in temperature), but the
/// resonance arc only fires when both meshes are comparable in size.
#[test]
fn lopsided_large_pairs_do_not_fire() {
    assert!(!resonance_triggers(6, 1), "(6,1): lopsided — no resonance");
    assert!(!resonance_triggers(1, 6), "(1,6): lopsided — no resonance");
    assert!(!resonance_triggers(14, 1), "(14,1): heavily lopsided — no resonance");
    assert!(!resonance_triggers(14, 2), "(14,2): lopsided — no resonance");
    assert!(!resonance_triggers(10, 3), "(10,3): lopsided — no resonance");
    assert!(!resonance_triggers(7, 3), "(7,3): lopsided — no resonance");
    assert!(!resonance_triggers(5, 2), "(5,2): lopsided — no resonance");
}

/// Balanced pairs resonate universally; far-off-diagonal pairs do not.
/// This confirms the "preferentially balanced" property (even if the
/// size-floor calibration gap means ALL balanced pairs fire, not just large ones).
#[test]
fn resonance_preferentially_balanced() {
    let map = sweep_map();

    // Every balanced pair (m, m) resonates.
    let diagonal_fires = (0..14).filter(|&i| map[i][i]).count();
    assert_eq!(diagonal_fires, 14, "all 14 diagonal (balanced) pairs resonate");

    // No heavily lopsided (m+1, 1) pair resonates.
    let col1_lopsided = (1..14).filter(|&m| map[m][0]).count();
    assert_eq!(col1_lopsided, 0, "no (≥2, 1) pair resonates");

    // No (1, n+1) pair resonates either.
    let row1_lopsided = (1..14).filter(|&n| map[0][n]).count();
    assert_eq!(row1_lopsided, 0, "no (1, ≥2) pair resonates");
}

// ── Band structure and exact counts ────────────────────────────────────────

/// The resonance band in [1,14]² consists of exactly 31 cells.
///
/// Derivation with current defaults:
///   m ≤ n: fires when 6m > 5n.
///   m > n: fires when 7n > 6m.
///
/// Cells:
///   diagonal (14) + upper-diff-1 m≥6 (8) + upper-diff-2 m≥11 (2)
///   + lower-diff-1 n≥7 (7) = 31.
#[test]
fn resonance_band_cell_count() {
    let map = sweep_map();
    let count: usize = map.iter().flat_map(|r| r.iter()).filter(|&&b| b).count();
    assert_eq!(
        count, 31,
        "exactly 31 of 196 pairs resonate with current defaults (α=0.060, γ=0.050, δ=0.060)"
    );
}

/// Near-diagonal off-diagonal behavior: first pair that fires above diagonal.
#[test]
fn upper_off_diagonal_threshold() {
    // Upper triangle (m < n): fires when 6m > 5n.
    // For diff-1 (n = m+1): needs m > 5.
    assert!(!resonance_triggers(5, 6), "(5,6): 30 > 30 is false — just below threshold");
    assert!(resonance_triggers(6, 7), "(6,7): 36 > 35 — first upper-diff-1 pair to fire");
    assert!(resonance_triggers(7, 8), "(7,8): 42 > 40 ✓");
    assert!(resonance_triggers(13, 14), "(13,14): 78 > 70 ✓");

    // For diff-2 (n = m+2): needs m > 10.
    assert!(!resonance_triggers(10, 12), "(10,12): 60 > 60 is false — boundary");
    assert!(resonance_triggers(11, 13), "(11,13): 66 > 65 ✓");
    assert!(resonance_triggers(12, 14), "(12,14): 72 > 70 ✓");

    // Diff-3 never fires in range.
    assert!(!resonance_triggers(11, 14), "(11,14): 66 > 70 is false");
}

/// Near-diagonal off-diagonal behavior below the diagonal.
#[test]
fn lower_off_diagonal_threshold() {
    // Lower triangle (m > n): fires when 7n > 6m.
    // For diff-1 (m = n+1): needs n > 6.
    assert!(!resonance_triggers(7, 6), "(7,6): 42 > 42 is false — strict inequality");
    assert!(resonance_triggers(8, 7), "(8,7): 49 > 48 — first lower-diff-1 pair to fire");
    assert!(resonance_triggers(9, 8), "(9,8): 56 > 54 ✓");
    assert!(resonance_triggers(14, 13), "(14,13): 91 > 84 ✓");

    // Diff-2 never fires in range: 7n > 6(n+2) → n > 12; smallest case n=13 → m=15 out of range.
    assert!(!resonance_triggers(14, 12), "(14,12): 84 > 84 is false — boundary");
}

/// Formula asymmetry: with α ≠ γ, (m, n) and (n, m) can differ.
#[test]
fn formula_asymmetry_alpha_ne_gamma() {
    // (6,7): m≤n branch: 6·6 > 5·7 → 36 > 35. Fires.
    assert!(resonance_triggers(6, 7));
    // (7,6): m>n branch: 7·6 > 6·7 → 42 > 42. Does NOT fire (strict inequality).
    assert!(!resonance_triggers(7, 6));

    // If α = γ, the formula would be symmetric. Document the current asymmetry.
    assert!(
        (ALPHA - GAMMA).abs() > 1e-12,
        "α ({ALPHA}) ≠ γ ({GAMMA}): asymmetry is expected"
    );
}

// ── Calibration gaps (documented, not fixed) ───────────────────────────────

/// GAP #1: All balanced pairs fire — including tiny (1,1) and (2,2).
///
/// Design intent: tiny balanced pairs should NOT resonate (only mid-game 4–6 range).
/// Current state: every balanced pair fires because δ > 0.5·(α+γ) is size-independent.
/// Fix: requires a size-floor in the formula, not achievable by coefficient tuning alone.
#[test]
fn gap_tiny_balanced_pairs_fire() {
    // Verify the structural cause: the balanced-pair condition is size-independent.
    let balanced_rhs = 0.5 * (ALPHA + GAMMA); // = 0.055 with current defaults
    assert!(
        DELTA > balanced_rhs,
        "δ ({DELTA:.3}) > 0.5·(α+γ) ({balanced_rhs:.3}): all balanced pairs resonate"
    );

    // Consequence: (1,1) and (2,2) resonate contrary to design intent.
    assert!(
        resonance_triggers(1, 1),
        "GAP: (1,1) resonates (δ > 0.5·(α+γ) is size-independent)"
    );
    assert!(
        resonance_triggers(2, 2),
        "GAP: (2,2) resonates (same structural reason)"
    );
    assert!(
        resonance_triggers(3, 3),
        "GAP: (3,3) resonates (same structural reason)"
    );
}

/// GAP #2: (6,5) just misses the threshold.
///
/// Design intent: both meshes near atari should resonate.
/// Current state: margin is −0.005 (LHS=0.300, RHS=0.305).
/// Fix: raising δ to ≥ 0.062 closes this, but gap #1 remains.
#[test]
fn gap_near_collapse_pair_just_misses() {
    let (lhs, rhs) = resonance_margin(6, 5);
    // lhs = δ·min(6,5) = 0.060·5 = 0.300
    // rhs = 0.5·(α·6 + γ·5) = 0.5·(0.360 + 0.250) = 0.305
    assert!(
        lhs < rhs,
        "(6,5) should miss: lhs={lhs:.4}, rhs={rhs:.4}"
    );
    assert!(
        !resonance_triggers(6, 5),
        "GAP: (6,5) does not resonate; design intent says it should (margin = {:.4})",
        lhs - rhs
    );
    // (5,6) likewise: LHS=0.300, RHS=0.5·(0.300+0.300)=0.300 — strict false.
    assert!(!resonance_triggers(5, 6), "GAP: (5,6) also does not resonate");
}

// ── Temperature monotonicity ────────────────────────────────────────────────

/// A lone mesh with no face-adjacent enemies has `T(M) = α·|M|`, which is
/// strictly increasing with mesh size. Verifies the α-gain term is positive.
#[test]
fn temperature_monotone_lone_mesh() {
    for s in 1usize..14 {
        let t_s = temperature(s, &[]);
        let t_s1 = temperature(s + 1, &[]);
        assert!(
            t_s1 > t_s,
            "temperature not monotone: T({s}) = {t_s:.4} ≥ T({}) = {t_s1:.4}",
            s + 1
        );
    }
}

/// Temperature with a large enemy neighbor is higher than a lone mesh at the
/// same size — the γ·|N| term adds heat when the enemy is larger or equal.
#[test]
fn temperature_enemy_heats_smaller_mesh() {
    // A size-3 mesh next to a size-6 enemy should run hotter than alone.
    let alone = temperature(3, &[]);
    let adjacent = temperature(3, &[6]);
    assert!(
        adjacent > alone,
        "adjacent large enemy raises temperature: alone={alone:.4}, adjacent={adjacent:.4}"
    );
}
