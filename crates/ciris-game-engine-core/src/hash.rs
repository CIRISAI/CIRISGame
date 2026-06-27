//! `board_state_hash` — a SHA-256 fingerprint of the game state used for replay
//! verification (DESIGN_BRIEF §8.4). Byte layout is fixed and target-independent.

use sha2::{Digest, Sha256};

use crate::board::CellState;
use crate::engine::GameState;

/// Encode a cell as one byte: empty 0; live 1..=4 by slot; temp-dead 5..=8 by
/// slot; perma-dead 9.
fn cell_byte(state: CellState) -> u8 {
    match state {
        CellState::Empty => 0,
        CellState::Live(s) => 1 + s.slot(),
        CellState::TempDead(s) => 5 + s.slot(),
        CellState::PermaDead => 9,
    }
}

/// SHA-256 over `(n, every cell byte in index order, scores LE, current slot)`.
pub fn board_state_hash(gs: &GameState) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update([gs.board.n]);
    for idx in 0..gs.board.len() {
        hasher.update([cell_byte(gs.board.get(idx))]);
    }
    for score in gs.scores {
        hasher.update(score.to_le_bytes());
    }
    hasher.update([gs.current]);

    let digest = hasher.finalize();
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest);
    out
}
