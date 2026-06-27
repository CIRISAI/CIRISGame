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
/// Resonance gain (DESIGN_BRIEF §4.1; nudged +0.002 from the starting default so
/// the near-atari pair (6,5) clears the trigger — BACKLOG #3 Gap 2).
pub const DELTA: f64 = 0.062;

/// Display normalization constant for [`t_vis`].
const VIS_SCALE: f64 = 1.40;

/// Scale-awareness floor for resonance (BACKLOG #3 Gap 1). Both meshes of a
/// face-pair must be at least this size before they can resonate, so small
/// groups near each other merely *vibe* while big groups near each other
/// *excite*. Mirrors `ITERATION_KNOBS.json` (`resonance.minMeshSize`).
pub const RESONANCE_MIN_SIZE: usize = 4;

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

/// Returns `true` when a face-pair `(M, N)` resonates (§4.3).
///
/// Two gates, both must pass:
/// 1. **Scale floor (Gap 1):** `min(|M|, |N|) ≥ RESONANCE_MIN_SIZE`. Small groups
///    near each other only vibe; big groups near each other can excite. This is
///    what makes resonance size-aware — without it the bare condition below is
///    scale-free for balanced pairs (size cancels), so (1,1) would resonate like
///    (6,6).
/// 2. **Trigger condition:** `δ · min(|M|, |N|) > 0.5 · (α·|M| + γ·|N|)`.
///
/// **Asymmetric when α ≠ γ.** The RHS weighs `|M|` by α and `|N|` by γ, so
/// `resonance_triggers(m, n)` can differ from `resonance_triggers(n, m)`.
/// Resonance direction is large → small (§4.3): the Hermite arc launches from
/// the larger side.
pub fn resonance_triggers(m_size: usize, n_size: usize) -> bool {
    if m_size.min(n_size) < RESONANCE_MIN_SIZE {
        return false;
    }
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
