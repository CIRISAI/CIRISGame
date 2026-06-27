//! Turn engine: move application, the collapse → dispersal pipeline, scoring,
//! and end-state detection.
//!
//! Turn flow (DESIGN_BRIEF §4.6):
//! 1. A steward places one cell on an `Empty` target.
//! 2. If that grows the placed cell's mesh to [`COLLAPSE_THRESHOLD`], every cell
//!    of the mesh enters `TempDead` and the steward's turn ends (the death moment).
//! 3. At the *start of the next turn* the pending dispersal resolves via
//!    Algorithm A: live pairs respawn in the steward's color, spacers become
//!    `PermaDead`, and the offending steward's score grows by the perma count.

use alloc::vec::Vec;
use rand_chacha::ChaCha8Rng;
use rand_core::SeedableRng;
use serde::{Deserialize, Serialize};

use crate::board::{Board, CellState, Coord, Steward};
use crate::dispersal::algorithm_a;
use crate::{COLLAPSE_THRESHOLD, STEWARD_COUNT};

/// A placement: the current steward claims `coord`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Move {
    pub coord: Coord,
}

impl Move {
    pub fn new(i: u8, j: u8, k: u8) -> Self {
        Move {
            coord: Coord::new(i, j, k),
        }
    }
}

/// Why a move was not applied.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MoveError {
    /// The coordinate is outside the board.
    OutOfBounds,
    /// The target cell is not `Empty`.
    Occupied,
    /// No legal placement remains; the game is over.
    GameOver,
}

/// One entry in the replayable move log.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MoveRecord {
    pub slot: u8,
    pub coord: Coord,
}

/// A collapse awaiting resolution at the start of the next turn.
#[derive(Clone, Debug)]
struct Pending {
    slot: u8,
    cells: Vec<usize>,
}

/// Final result of a game, the unit the daily-seed Worker compares against the
/// client assertion (DESIGN_BRIEF §8.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Outcome {
    /// Per-steward perma-dead created.
    pub permadead: [u32; STEWARD_COUNT],
    /// Total perma-dead across all stewards.
    pub total: u32,
    /// True when every steward ended at zero — the M-1 / WILD ending.
    pub all_survivors: bool,
    /// SHA-256 over the final board + scores; anchors replay identity.
    pub board_state_hash: [u8; 32],
}

/// Full game state. Not serialized directly — the serializable wire format is
/// the `BoardView` snapshot (built later for the AI-API, §7.2).
pub struct GameState {
    pub board: Board,
    /// Slot whose turn it is (`0..=3`).
    pub current: u8,
    /// Number of placements applied so far.
    pub turn: u32,
    /// Per-steward perma-dead created (the score; lowest wins).
    pub scores: [u32; STEWARD_COUNT],
    /// Stewards removed from the rotation. No base rule sets these; the field
    /// exists for `BoardView` parity and test scaffolding.
    pub eliminated: [bool; STEWARD_COUNT],
    pub history: Vec<MoveRecord>,
    pending: Option<Pending>,
    #[allow(dead_code)]
    rng: ChaCha8Rng,
}

impl GameState {
    /// A fresh game on an `n × n × n` board, seeded deterministically.
    pub fn new(n: u8, seed: [u8; 32]) -> Self {
        GameState {
            board: Board::new(n),
            current: 0,
            turn: 0,
            scores: [0; STEWARD_COUNT],
            eliminated: [false; STEWARD_COUNT],
            history: Vec::new(),
            pending: None,
            rng: ChaCha8Rng::from_seed(seed),
        }
    }

    /// Create a game with pre-placed perma-dead cells (e.g., from a daily
    /// puzzle layout). Every index in `perma_dead` must be `< n³`; panics
    /// otherwise. The game RNG is seeded with `seed`, independent of the
    /// perma-dead layout.
    pub fn with_perma_dead(n: u8, seed: [u8; 32], perma_dead: &[usize]) -> Self {
        let mut gs = GameState::new(n, seed);
        for &idx in perma_dead {
            assert!(
                idx < gs.board.len(),
                "perma_dead index {idx} out of range for n={n}"
            );
            gs.board.set(idx, CellState::PermaDead);
        }
        gs
    }

    /// Create a game state for the given UTC-date daily seed (DESIGN_BRIEF §8.2).
    /// Derives the daily layout via [`crate::daily_seed::derive_daily_seed`] and
    /// pre-places all perma-dead cells. The game RNG is seeded independently with
    /// `seed` so AI / particle randomness is decoupled from the layout derivation.
    pub fn from_daily_seed(utc_date_iso: &str, board_n: u8, seed: [u8; 32]) -> Self {
        let daily = crate::daily_seed::derive_daily_seed(utc_date_iso, board_n);
        GameState::with_perma_dead(board_n, seed, &daily.perma_dead)
    }

    /// The steward whose turn it is.
    pub fn current_steward(&self) -> Steward {
        Steward::from_slot(self.current)
    }

    /// Whether a collapse is awaiting dispersal at the next turn.
    pub fn has_pending_dispersal(&self) -> bool {
        self.pending.is_some()
    }

    /// The game ends when no placement remains and nothing is pending.
    pub fn is_over(&self) -> bool {
        self.pending.is_none() && self.board.empty_count() == 0
    }

    /// All empty (legal) placement targets, ascending by linear index.
    pub fn legal_moves(&self) -> Vec<Coord> {
        (0..self.board.len())
            .filter(|&i| self.board.get(i) == CellState::Empty)
            .map(|i| self.board.coord(i))
            .collect()
    }

    /// Meshes of `steward` currently in atari (`|M| == ATARI_SIZE`).
    pub fn atari_meshes(&self, steward: Steward) -> Vec<Vec<usize>> {
        self.board
            .meshes_of(steward)
            .into_iter()
            .filter(|m| m.len() == crate::ATARI_SIZE)
            .collect()
    }

    /// Apply the current steward's placement, resolving any pending dispersal
    /// first (DESIGN_BRIEF §4.6).
    pub fn apply_move(&mut self, mv: Move) -> Result<(), MoveError> {
        if self.is_over() {
            return Err(MoveError::GameOver);
        }

        // Step 3 of the previous collapse: resolve at the start of this turn.
        self.resolve_pending();
        if self.board.empty_count() == 0 {
            return Err(MoveError::GameOver);
        }

        let idx = self.board.index(mv.coord).ok_or(MoveError::OutOfBounds)?;
        if self.board.get(idx) != CellState::Empty {
            return Err(MoveError::Occupied);
        }

        let steward = self.current_steward();
        self.board.set(idx, CellState::Live(steward));
        self.history.push(MoveRecord {
            slot: self.current,
            coord: mv.coord,
        });

        // Step 1/2: did this placement push a mesh to the threshold?
        let mesh = self.board.mesh_containing(idx);
        if mesh.len() >= COLLAPSE_THRESHOLD {
            for &c in &mesh {
                self.board.set(c, CellState::TempDead(steward));
            }
            self.pending = Some(Pending {
                slot: self.current,
                cells: mesh,
            });
        }

        self.turn += 1;
        self.advance_turn();
        Ok(())
    }

    /// Resolve a pending collapse: live pairs respawn, spacers go perma-dead,
    /// the offending steward's score grows by the perma count.
    fn resolve_pending(&mut self) {
        let Some(pending) = self.pending.take() else {
            return;
        };
        let steward = Steward::from_slot(pending.slot);
        let d = algorithm_a(&self.board, &pending.cells);
        for &c in &d.live {
            self.board.set(c, CellState::Live(steward));
        }
        for &c in &d.perma {
            self.board.set(c, CellState::PermaDead);
        }
        self.scores[pending.slot as usize] += d.perma.len() as u32;
    }

    /// Advance `current` to the next non-eliminated slot (round-robin).
    fn advance_turn(&mut self) {
        for _ in 0..STEWARD_COUNT {
            self.current = (self.current + 1) & 0b11;
            if !self.eliminated[self.current as usize] {
                break;
            }
        }
    }

    /// Snapshot the final result.
    pub fn outcome(&self) -> Outcome {
        let total: u32 = self.scores.iter().sum();
        Outcome {
            permadead: self.scores,
            total,
            all_survivors: total == 0,
            board_state_hash: crate::hash::board_state_hash(self),
        }
    }
}
