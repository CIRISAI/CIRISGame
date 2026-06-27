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
