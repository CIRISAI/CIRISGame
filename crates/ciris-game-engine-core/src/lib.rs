//! # ciris-game-engine-core
//!
//! The deterministic heart of CIRISGame. No rendering, no Bevy, no `std`
//! collections with non-deterministic iteration order — everything here must
//! replay bit-identically across `wasm32-unknown-unknown` (browser),
//! `wasm32-wasip1` (the daily-seed verification Worker), and native CI.
//!
//! ## The rule (MISSION §2.2, DESIGN_BRIEF §4)
//!
//! Four stewards take turns placing one cell each on a rhombic-dodecahedral
//! (FCC) lattice — twelve face-neighbors per cell, no center. Same-color
//! face-adjacent cells form a *mesh*. A mesh that reaches [`COLLAPSE_THRESHOLD`]
//! (seven) cells undergoes a destructive transition: it dies and disperses via
//! [`dispersal::algorithm_a`] into live pairs of the steward's color plus
//! perma-dead spacer cells. Score = total perma-dead created; lowest wins; all
//! stewards at zero is the cooperative M-1 / WILD ending.
//!
//! ## Two invariants this crate enforces (DESIGN_BRIEF §4.10)
//!
//! * **Size-1 meshes are first-class.** Every placement is born `|M| = 1`.
//!   There is no minimum group size and no "lone stone is clear" demotion.
//! * **No capture.** A cell dies *only* when its own steward grows a mesh to
//!   seven. No liberties, no surround-to-kill, no enemy capture. A `Live` cell
//!   ringed entirely by other colors is inert but safe forever.

#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

extern crate alloc;

pub mod board;
pub mod crossing;
pub mod daily_seed;
pub mod dispersal;
pub mod engine;
pub mod hash;
pub mod lattice;
pub mod temperature;

pub use board::{Board, CellState, Coord, Steward};
pub use crossing::{is_crossing_illegal, opposite_diagonal};
pub use daily_seed::{derive_daily_seed, DailySeed, Difficulty};
pub use dispersal::{algorithm_a, dispersal_counts, validate_layout, Dispersal, LayoutError};
pub use engine::{GameState, Move, MoveError, MoveRecord, Outcome};

/// Default board edge length: 5×5×5 = 125 cells (DESIGN_BRIEF §3.1).
pub const DEFAULT_BOARD_N: u8 = 5;

/// A mesh that reaches this many cells destructively transitions. Fixed at 7 in
/// browser; configurable in native (CLAUDE.md locked list). MISSION §2.2.
pub const COLLAPSE_THRESHOLD: usize = 7;

/// A mesh of this size is one placement from collapse — the atari state that
/// drives the Kuramoto breath animation (DESIGN_BRIEF §4.9).
pub const ATARI_SIZE: usize = 6;

/// Number of stewards (board slots).
pub const STEWARD_COUNT: usize = 4;
