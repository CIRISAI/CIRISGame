//! Turn engine: move application, the interactive collapse → dispersal pipeline,
//! scoring, and end-state detection.
//!
//! Turn flow (DESIGN_BRIEF §4.6):
//! 1. A steward places one cell on an `Empty` target.
//! 2. If that grows the placed cell's mesh to [`COLLAPSE_THRESHOLD`], every cell
//!    of the mesh enters `TempDead` and the turn ends — the wreckage smoulders
//!    through the opponents' turns.
//! 3. On the collapsing steward's **next turn** they *choose the wreckage
//!    layout* (which crater cells come back live vs. perma-dead) AND place a new
//!    stone — both in one [`GameState::apply_move`] call. A `Move` with no
//!    `dispersal` lets the engine auto-pick a legal layout (computers / replay).

use alloc::vec::Vec;
use rand_chacha::ChaCha8Rng;
use rand_core::SeedableRng;
use serde::{Deserialize, Serialize};

use crate::board::{Board, CellState, Coord, Steward};
use crate::dispersal::{algorithm_a, validate_layout, Dispersal, LayoutError};
use crate::{COLLAPSE_THRESHOLD, STEWARD_COUNT};

/// A turn: place at `coord`, optionally with a chosen wreckage `dispersal`
/// (the perma-dead cells) when it's a rebuild turn.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Move {
    pub coord: Coord,
    /// On a rebuild turn: the cells the steward chooses to make perma-dead (the
    /// rest of the crater stays live). `None` → the engine auto-picks a legal
    /// layout. Ignored (must be `None`) on a normal turn.
    pub dispersal: Option<Vec<Coord>>,
}

impl Move {
    /// A plain placement (no chosen layout; auto-disperses if it's a rebuild turn).
    pub fn new(i: u8, j: u8, k: u8) -> Self {
        Move {
            coord: Coord::new(i, j, k),
            dispersal: None,
        }
    }

    /// A placement at `coord` with no chosen layout.
    pub fn place(coord: Coord) -> Self {
        Move {
            coord,
            dispersal: None,
        }
    }

    /// A rebuild turn: place at `coord` and make `perma` the crater's perma-dead.
    pub fn rebuild(coord: Coord, perma: Vec<Coord>) -> Self {
        Move {
            coord,
            dispersal: Some(perma),
        }
    }
}

/// Why a move was not applied.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MoveError {
    /// The placement coordinate is outside the board.
    OutOfBounds,
    /// The placement target is not `Empty`.
    Occupied,
    /// The placement would create a same-colour bond crossing an existing
    /// different-colour bond on the shared face (no-crossing rule, §4.11).
    CrossesBond,
    /// No legal placement remains; the game is over.
    GameOver,
    /// [`GameState::pass`] was called while the steward still has a legal move.
    /// There is no voluntary pass — a steward with ≥1 legal move must play.
    PassNotAllowed,
    /// A `dispersal` layout was supplied on a turn that is not a rebuild turn.
    UnexpectedDispersal,
    /// A chosen perma-dead coordinate is off the board.
    DispersalCoordOutOfBounds,
    /// A chosen perma-dead cell is not part of the collapsed crater.
    DispersalNotInFootprint,
    /// The same perma-dead cell was listed twice.
    DispersalDuplicate,
    /// Fewer perma-dead than the locked count floor (would score below the table).
    DispersalTooFewPerma,
    /// The kept live cells would form a component of `≥ COLLAPSE_THRESHOLD`.
    DispersalIllegalShape,
}

impl From<LayoutError> for MoveError {
    fn from(e: LayoutError) -> Self {
        match e {
            LayoutError::NotInFootprint => MoveError::DispersalNotInFootprint,
            LayoutError::Duplicate => MoveError::DispersalDuplicate,
            LayoutError::TooFewPerma => MoveError::DispersalTooFewPerma,
            LayoutError::IllegalShape => MoveError::DispersalIllegalShape,
        }
    }
}

/// One entry in the replayable move log. `dispersal` records the *resolved*
/// perma-dead cells of a rebuild turn (empty on a normal turn) so a replay
/// reproduces the exact layout regardless of the auto chooser.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MoveRecord {
    pub slot: u8,
    pub coord: Coord,
    pub dispersal: Vec<Coord>,
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
    /// Per-slot smouldering crater awaiting that steward's rebuild turn.
    pending: [Option<Vec<usize>>; STEWARD_COUNT],
    /// Whether the no-crossing rule (§4.11) is enforced. Always `true` in browser
    /// (fixed-on, like the rule of seven); a native-only knob may switch it off.
    /// The analysis harness sets it `false` to measure the rule-off baseline.
    no_crossing: bool,
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
            pending: [None, None, None, None],
            no_crossing: true,
            rng: ChaCha8Rng::from_seed(seed),
        }
    }

    /// Create a game with pre-placed perma-dead cells (e.g., from a daily
    /// puzzle layout). Every index in `perma_dead` must be `< n³`; panics
    /// otherwise. The game RNG is seeded with `seed`, independent of the layout.
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
    pub fn from_daily_seed(utc_date_iso: &str, board_n: u8, seed: [u8; 32]) -> Self {
        let daily = crate::daily_seed::derive_daily_seed(utc_date_iso, board_n);
        GameState::with_perma_dead(board_n, seed, &daily.perma_dead)
    }

    /// The steward whose turn it is.
    pub fn current_steward(&self) -> Steward {
        Steward::from_slot(self.current)
    }

    /// Whether the no-crossing rule (§4.11) is being enforced.
    pub fn no_crossing_rule(&self) -> bool {
        self.no_crossing
    }

    /// Enable/disable the no-crossing rule. Browser builds keep it fixed-on; this
    /// exists for the native-only knob and the analysis harness (rule-off
    /// baseline). Does not affect the board hash or determinism.
    pub fn set_no_crossing_rule(&mut self, enabled: bool) {
        self.no_crossing = enabled;
    }

    /// Whether any steward has a smouldering crater awaiting rebuild.
    pub fn has_pending_dispersal(&self) -> bool {
        self.pending.iter().any(Option::is_some)
    }

    /// Whether the current steward must lay out a crater this turn.
    pub fn is_rebuild_turn(&self) -> bool {
        self.pending[self.current as usize].is_some()
    }

    /// The crater the current steward must lay out (cell coords), if any.
    pub fn pending_footprint(&self) -> Option<Vec<Coord>> {
        self.pending[self.current as usize]
            .as_ref()
            .map(|cells| cells.iter().map(|&i| self.board.coord(i)).collect())
    }

    /// The game ends when the board is saturated with no crater pending, or when
    /// every steward is forced to pass with empties remaining (a global deadlock —
    /// see [`is_deadlocked`](Self::is_deadlocked)). The latter cannot occur unless
    /// the no-crossing rule is active.
    pub fn is_over(&self) -> bool {
        (self.board.empty_count() == 0 && !self.has_pending_dispersal()) || self.is_deadlocked()
    }

    /// All empty placement targets, ascending by linear index. This is the raw
    /// target set; it ignores the colour-dependent no-crossing rule. For the
    /// moves a specific steward may actually play, use
    /// [`legal_moves_for`](Self::legal_moves_for) /
    /// [`current_legal_moves`](Self::current_legal_moves).
    pub fn legal_moves(&self) -> Vec<Coord> {
        (0..self.board.len())
            .filter(|&i| self.board.get(i) == CellState::Empty)
            .map(|i| self.board.coord(i))
            .collect()
    }

    /// Placement targets `steward` may legally take: empty cells minus any the
    /// no-crossing rule (§4.11) forbids for that colour. Ascending by index, so
    /// deterministic. With the rule off this equals [`legal_moves`](Self::legal_moves).
    pub fn legal_moves_for(&self, steward: Steward) -> Vec<Coord> {
        let empties = self.legal_moves();
        if !self.no_crossing {
            return empties;
        }
        empties
            .into_iter()
            .filter(|&c| !crate::crossing::is_crossing_illegal(&self.board, c, steward))
            .collect()
    }

    /// Legal placements for the steward whose turn it is.
    pub fn current_legal_moves(&self) -> Vec<Coord> {
        self.legal_moves_for(self.current_steward())
    }

    /// Whether the steward to move has no legal placement and must therefore pass
    /// (forced pass — there is no voluntary pass). Only true while empties remain
    /// and the game is not already over.
    pub fn must_pass(&self) -> bool {
        !self.is_over() && self.board.empty_count() > 0 && self.current_legal_moves().is_empty()
    }

    /// Global deadlock: empties remain but **no** non-eliminated steward has any
    /// legal placement (every empty cell is cross-blocked for every colour). The
    /// board can never progress, so the game is treated as terminal. Always
    /// `false` when empties are exhausted (the saturation endgame handles that) or
    /// when the no-crossing rule is off (raw empties are always placeable).
    pub fn is_deadlocked(&self) -> bool {
        if self.board.empty_count() == 0 || !self.no_crossing {
            return false;
        }
        Steward::ALL
            .iter()
            .enumerate()
            .all(|(slot, &s)| self.eliminated[slot] || self.legal_moves_for(s).is_empty())
    }

    /// Skip the current steward's turn. Legal **only** as a forced pass — i.e.
    /// when [`must_pass`](Self::must_pass) holds. Advances to the next steward
    /// without touching the board (any owed crater stays pending). Returns
    /// [`MoveError::PassNotAllowed`] if the steward still has a legal move, or
    /// [`MoveError::GameOver`] if the game is already over.
    pub fn pass(&mut self) -> Result<(), MoveError> {
        if self.is_over() {
            return Err(MoveError::GameOver);
        }
        if !self.must_pass() {
            return Err(MoveError::PassNotAllowed);
        }
        self.advance_turn();
        Ok(())
    }

    /// Meshes of `steward` currently in atari (`|M| == ATARI_SIZE`).
    pub fn atari_meshes(&self, steward: Steward) -> Vec<Vec<usize>> {
        self.board
            .meshes_of(steward)
            .into_iter()
            .filter(|m| m.len() == crate::ATARI_SIZE)
            .collect()
    }

    /// Apply the current steward's turn: first the wreckage layout (if this is a
    /// rebuild turn), then a placement. All validation happens before any board
    /// mutation, so a rejected move leaves the state untouched.
    pub fn apply_move(&mut self, mv: Move) -> Result<(), MoveError> {
        if self.is_over() {
            return Err(MoveError::GameOver);
        }
        let cur = self.current as usize;
        let steward = self.current_steward();
        let has_empty = self.board.empty_count() > 0;

        // --- compute (don't yet apply) the rebuild layout, if owed one ---
        let rebuild: Option<Dispersal> = match self.pending[cur].clone() {
            Some(footprint) => Some(match &mv.dispersal {
                Some(perma_coords) => {
                    let mut perma_idx = Vec::with_capacity(perma_coords.len());
                    for c in perma_coords {
                        perma_idx.push(
                            self.board
                                .index(*c)
                                .ok_or(MoveError::DispersalCoordOutOfBounds)?,
                        );
                    }
                    validate_layout(&self.board, &footprint, &perma_idx)?
                }
                None => algorithm_a(&self.board, &footprint),
            }),
            None => {
                if mv.dispersal.is_some() {
                    return Err(MoveError::UnexpectedDispersal);
                }
                None
            }
        };

        // --- validate the placement (crater cells are TempDead, never Empty,
        // so a valid target is always outside the crater) ---
        let place_idx = if has_empty {
            let idx = self.board.index(mv.coord).ok_or(MoveError::OutOfBounds)?;
            if self.board.get(idx) != CellState::Empty {
                return Err(MoveError::Occupied);
            }
            // No-crossing rule (§4.11): reject a placement whose same-colour bond
            // would cross an existing different-colour bond on the shared face.
            // Checked against the pre-move board (matching the analysis predicate)
            // before any mutation, so a rejected move leaves state untouched.
            if self.no_crossing
                && crate::crossing::is_crossing_illegal(&self.board, mv.coord, steward)
            {
                return Err(MoveError::CrossesBond);
            }
            Some(idx)
        } else {
            None // endgame: a rebuild-only turn with no remaining placement
        };

        // --- apply the rebuild ---
        let mut resolved_perma: Vec<Coord> = Vec::new();
        if let Some(d) = rebuild {
            for &c in &d.live {
                self.board.set(c, CellState::Live(steward));
            }
            for &c in &d.perma {
                self.board.set(c, CellState::PermaDead);
            }
            self.scores[cur] += d.perma.len() as u32;
            resolved_perma = d.perma.iter().map(|&i| self.board.coord(i)).collect();
            self.pending[cur] = None;
        }

        // --- apply the placement (may itself trigger a fresh collapse) ---
        if let Some(idx) = place_idx {
            self.board.set(idx, CellState::Live(steward));
            let mesh = self.board.mesh_containing(idx);
            if mesh.len() >= COLLAPSE_THRESHOLD {
                for &c in &mesh {
                    self.board.set(c, CellState::TempDead(steward));
                }
                self.pending[cur] = Some(mesh);
            }
        }

        self.history.push(MoveRecord {
            slot: self.current,
            coord: mv.coord,
            dispersal: resolved_perma,
        });
        self.turn += 1;
        self.advance_turn();
        Ok(())
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
