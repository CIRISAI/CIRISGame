//! Daily-seed derivation: deterministic starting state for the shared daily
//! puzzle (DESIGN_BRIEF §8.2).
//!
//! The public entry point is [`derive_daily_seed`]. Everything here is
//! `no_std`-compatible and byte-identical across `wasm32-unknown-unknown`,
//! `wasm32-wasip1`, and native.

use alloc::vec::Vec;
use rand_chacha::ChaCha8Rng;
use rand_core::{RngCore, SeedableRng};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// AI difficulty tier for a steward slot. The daily roster assigns one tier per
/// slot; slot 0 is always [`Difficulty::Easy`] (the kid-on-ramp guarantee,
/// DESIGN_BRIEF §8.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Difficulty {
    Easy,
    Medium,
    Hard,
    Brutal,
}

/// The fully-derived daily puzzle state. Produced by [`derive_daily_seed`] from
/// an ISO-8601 UTC date string (`"YYYY-MM-DD"`) and a board edge length.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DailySeed {
    /// Per-slot AI difficulty; `roster[0]` is always `Easy`.
    pub roster: [Difficulty; 4],
    /// Number of pre-existing perma-dead cells (`K ∈ [3, 15]`, DESIGN_BRIEF §8.2).
    pub k: u32,
    /// Sorted, distinct cell indices that start as `PermaDead`. Length == `k`;
    /// every index is `< board_n³`.
    pub perma_dead: Vec<usize>,
    /// SHA-256 over `(utc_date bytes, K as u32 LE, sorted perma-dead indices as u32 LE)`.
    /// Anchors the day's identity for cross-client comparison (DESIGN_BRIEF §8.2 step 4).
    pub board_state_hash: [u8; 32],
}

/// Derive the daily puzzle state deterministically from a UTC date string and
/// board edge length.
///
/// `utc_date_iso` must be an ISO-8601 date string, e.g. `"2026-06-27"`.
/// `board_n` sets the valid cell-index range (`0 .. board_n³`).
///
/// Draw order (DESIGN_BRIEF §8.2 — order is fixed for determinism across all
/// targets):
///
/// 1. **Roster** — slot 0 forced `Easy`; deterministic Fisher-Yates shuffle of
///    `[Medium, Hard, Brutal]` into slots 1–3 using two `rng.next_u32()` calls.
/// 2. **K** — `3 + (rng.next_u32() % 13)` → K ∈ \[3, 15\].
/// 3. **Positions** — K distinct indices via partial Fisher-Yates over `0..total`,
///    then sorted ascending.
/// 4. **`board_state_hash`** — SHA-256 over `(date_bytes, K as u32 LE, each
///    sorted index as u32 LE)`.
pub fn derive_daily_seed(utc_date_iso: &str, board_n: u8) -> DailySeed {
    // Derive the 32-byte ChaCha8 seed from the date.
    let mut hasher = Sha256::new();
    hasher.update(b"ciris-daily-");
    hasher.update(utc_date_iso.as_bytes());
    let seed_bytes: [u8; 32] = {
        let digest = hasher.finalize();
        let mut out = [0u8; 32];
        out.copy_from_slice(&digest);
        out
    };

    let mut rng = ChaCha8Rng::from_seed(seed_bytes);
    // Sample perma-dead from the actual FCC cell count (the even-parity points of
    // the n³ box), not n³ — indices must be valid board cell indices.
    let total = crate::board::Board::new(board_n).len();

    // 1. Roster: slot 0 = Easy; shuffle slots 1..=3 with Fisher-Yates (2 rng calls).
    let mut roster = [
        Difficulty::Easy,
        Difficulty::Medium,
        Difficulty::Hard,
        Difficulty::Brutal,
    ];
    // Standard Fisher-Yates over the 3-element sub-range [1..=3].
    // i = 3: j ∈ [1, 3], i = 2: j ∈ [1, 2].
    for i in (2usize..=3).rev() {
        let j = 1 + (rng.next_u32() as usize % i);
        roster.swap(i, j);
    }

    // 2. K ∈ [3, 15].
    let k = 3 + (rng.next_u32() % 13);
    let k_usize = k as usize;

    // 3. K distinct indices via partial Fisher-Yates over 0..total, then sorted.
    let mut indices: Vec<usize> = (0..total).collect();
    for i in 0..k_usize {
        let remaining = total - i;
        let j = i + (rng.next_u32() as usize % remaining);
        indices.swap(i, j);
    }
    let mut perma_dead: Vec<usize> = indices[..k_usize].to_vec();
    perma_dead.sort_unstable();

    // 4. board_state_hash over (date, K, sorted indices).
    let board_state_hash = {
        let mut h = Sha256::new();
        h.update(utc_date_iso.as_bytes());
        h.update(k.to_le_bytes());
        for &idx in &perma_dead {
            h.update((idx as u32).to_le_bytes());
        }
        let digest = h.finalize();
        let mut out = [0u8; 32];
        out.copy_from_slice(&digest);
        out
    };

    DailySeed {
        roster,
        k,
        perma_dead,
        board_state_hash,
    }
}
