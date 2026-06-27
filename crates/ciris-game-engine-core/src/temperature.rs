//! Mesh temperature (DESIGN_BRIEF §4.1).
//!
//! ```text
//! T(M) = α·|M| + Σ_{N ∈ adj(M)} [ γ·|N|         if |N| ≥ |M|
//!                                 −β·(|M|−|N|)   if |M| >  |N|
//!                                 + δ·min(|M|,|N|) ]
//! ```
//!
//! Coefficients are starting defaults pending the empirical resonance sweep
//! (BACKLOG #3); they mirror `ITERATION_KNOBS.json` (`temperature.*`).

/// Internal Brownian gain.
pub const ALPHA: f64 = 0.060;
/// Large-bleeds-to-small rate.
pub const BETA: f64 = 0.080;
/// Heated-by-larger gain.
pub const GAMMA: f64 = 0.050;
/// Resonance gain (reconciled to DESIGN_BRIEF §4.1).
pub const DELTA: f64 = 0.060;

/// Display normalization constant for [`t_vis`].
const VIS_SCALE: f64 = 1.40;

/// Raw temperature of a mesh of size `m_size` whose face-adjacent enemy meshes
/// have the given sizes.
pub fn temperature(m_size: usize, enemy_sizes: &[usize]) -> f64 {
    let m = m_size as f64;
    let mut t = ALPHA * m;
    for &n_size in enemy_sizes {
        let n = n_size as f64;
        if n_size >= m_size {
            t += GAMMA * n;
        } else {
            t -= BETA * (m - n);
        }
        t += DELTA * m.min(n);
    }
    t
}

/// Display-normalized temperature in `[0, 1]`: `1 − exp(−max(T,0) / 1.40)`.
pub fn t_vis(t: f64) -> f64 {
    let v = 1.0 - libm::exp(-t.max(0.0) / VIS_SCALE);
    v.clamp(0.0, 1.0)
}

/// Coarse word label for a normalized temperature (§7.2 `calm/lively/hot/chaotic`).
pub fn temperature_word(t_vis: f64) -> &'static str {
    match t_vis {
        v if v < 0.25 => "calm",
        v if v < 0.50 => "lively",
        v if v < 0.75 => "hot",
        _ => "chaotic",
    }
}

/// Returns `true` when a face-pair `(M, N)` resonates under the §4.3 condition.
///
/// Resonance condition: `δ · min(|M|, |N|) > 0.5 · (α·|M| + γ·|N|)`.
///
/// **Scale-free for balanced pairs.** When `|M| = |N| = s` the condition reduces
/// to `δ > 0.5·(α + γ)`, with `s` cancelling. With current defaults (δ = 0.060,
/// α = 0.060, γ = 0.050) this is `0.060 > 0.055`, so every balanced pair
/// resonates regardless of size. See BACKLOG #3 analysis for the calibration gap.
///
/// **Asymmetric when α ≠ γ.** Because the RHS weighs `|M|` by α and `|N|` by γ,
/// `resonance_triggers(m, n)` can differ from `resonance_triggers(n, m)` when
/// `m ≠ n` (e.g., (6, 7) resonates but (7, 6) does not with current defaults).
///
/// Resonance direction is large → small (§4.3): heat and attestations flow down
/// the size gradient; the triggered Hermite arc launches from the larger side.
pub fn resonance_triggers(m_size: usize, n_size: usize) -> bool {
    let (lhs, rhs) = resonance_margin(m_size, n_size);
    lhs > rhs
}

/// Returns the raw `(lhs, rhs)` of the §4.3 resonance condition for calibration.
///
/// Resonance fires when `lhs > rhs`. A positive `lhs − rhs` margin indicates
/// resonance; negative means no resonance. Pairs near zero are on the trigger
/// boundary and are sensitive to coefficient changes.
pub fn resonance_margin(m_size: usize, n_size: usize) -> (f64, f64) {
    let m = m_size as f64;
    let n = n_size as f64;
    let lhs = DELTA * m.min(n);
    let rhs = 0.5 * (ALPHA * m + GAMMA * n);
    (lhs, rhs)
}
