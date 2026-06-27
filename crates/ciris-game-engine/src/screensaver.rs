//! Self-running screensaver mode (DESIGN_BRIEF §6.3 default). A timer advances
//! the game one Easy-policy move at a time; when the game ends it holds on the
//! final board for a few seconds, then reseeds and loops forever. Everything is
//! deterministic (seeded `ChaCha8Rng`) and panic-free.

use bevy::prelude::*;
use rand_chacha::ChaCha8Rng;
use rand_core::{RngCore, SeedableRng};

use crate::render::BoardDirty;
use crate::{seed_from_counter, BoardResource};
use ciris_game_engine_core::{
    Board, CellState, Coord, GameState, Move, Steward, COLLAPSE_THRESHOLD, DEFAULT_BOARD_N,
};

/// Inter-move pacing in screensaver (knob `turn.pause_screensaver_ms` = 2500).
const STEP_SECS: f32 = 2.5;
/// Hold on the final board before restarting (knob `endgame.autoRestart.normal`
/// = 10 s).
const HOLD_SECS: f32 = 10.0;

/// Drives the endless screensaver loop: a repeating step timer for moves and a
/// one-shot hold timer for the post-game pause.
#[derive(Resource)]
pub struct ScreensaverState {
    step: Timer,
    hold: Timer,
    /// True while holding on a finished board, waiting to restart.
    holding: bool,
    /// Round counter; seeds each fresh game distinctly.
    round: u64,
}

impl ScreensaverState {
    pub fn new() -> Self {
        ScreensaverState {
            step: Timer::from_seconds(STEP_SECS, TimerMode::Repeating),
            hold: Timer::from_seconds(HOLD_SECS, TimerMode::Once),
            holding: false,
            round: 0,
        }
    }
}

impl Default for ScreensaverState {
    fn default() -> Self {
        Self::new()
    }
}

/// Deterministic AI RNG, distinct from the game's own internal dispersal RNG.
#[derive(Resource)]
pub struct AiRng(pub ChaCha8Rng);

impl AiRng {
    pub fn new(round: u64) -> Self {
        AiRng(ChaCha8Rng::from_seed(ai_seed(round)))
    }
}

/// AI seed for `round`, kept separate from the game seed so the two streams
/// never coincide.
fn ai_seed(round: u64) -> [u8; 32] {
    let mut seed = seed_from_counter(round);
    seed[31] = 0xA5;
    seed
}

/// The screensaver step system (`Update`). Ticks the timers, applies one move
/// per step, and reseeds after the hold.
pub fn drive(
    time: Res<Time>,
    mut state: ResMut<ScreensaverState>,
    mut board: ResMut<BoardResource>,
    mut rng: ResMut<AiRng>,
    mut dirty: ResMut<BoardDirty>,
) {
    let dt = time.delta();

    if state.holding {
        if state.hold.tick(dt).just_finished() {
            state.round += 1;
            board.0 = GameState::new(DEFAULT_BOARD_N, seed_from_counter(state.round));
            rng.0 = ChaCha8Rng::from_seed(ai_seed(state.round));
            state.step.reset();
            state.holding = false;
            dirty.0 = true;
        }
        return;
    }

    if state.step.tick(dt).just_finished() {
        if board.0.is_over() {
            state.hold.reset();
            state.holding = true;
            return;
        }
        if step_ai(&mut board.0, &mut rng.0) {
            dirty.0 = true;
        }
    }
}

/// Apply one Easy-policy move for the current steward. Returns whether the board
/// changed. Never panics (a rejected move is simply a no-op).
fn step_ai(gs: &mut GameState, rng: &mut ChaCha8Rng) -> bool {
    let mv = match choose_move(gs, rng) {
        Some(coord) => Move::place(coord),
        // No legal placement remains but a crater is still owed: resolve it with
        // an auto-layout. The placement coord is ignored when the board is full.
        None => Move::place(Coord::new(0, 0, 0)),
    };
    gs.apply_move(mv).is_ok()
}

/// Easy policy: a uniformly-random legal cell, but prefer cells that do *not*
/// immediately collapse the mover's own mesh (|M| reaching [`COLLAPSE_THRESHOLD`])
/// when at least one safe legal cell exists. Falls back to any legal cell when
/// every option would collapse.
fn choose_move(gs: &GameState, rng: &mut ChaCha8Rng) -> Option<Coord> {
    let legal = gs.legal_moves();
    if legal.is_empty() {
        return None;
    }
    let steward = gs.current_steward();
    let safe: Vec<Coord> = legal
        .iter()
        .copied()
        .filter(|c| match gs.board.index(*c) {
            Some(idx) => placed_mesh_size(&gs.board, steward, idx) < COLLAPSE_THRESHOLD,
            None => false,
        })
        .collect();
    let pool = if safe.is_empty() { &legal } else { &safe };
    let pick = (rng.next_u32() as usize) % pool.len();
    pool.get(pick).copied()
}

/// Size the same-steward mesh would have if `steward` placed a live cell at
/// `idx` — a flood fill over live same-steward face-neighbors, counting the
/// hypothetical new cell. Read-only; allocates a visited bitset per call.
fn placed_mesh_size(board: &Board, steward: Steward, idx: usize) -> usize {
    let mut visited = vec![false; board.len()];
    let mut stack = vec![idx];
    visited[idx] = true;
    let mut count = 0;
    while let Some(cur) = stack.pop() {
        count += 1;
        for nb in board.neighbors(cur) {
            if !visited[nb] {
                if let CellState::Live(s) = board.get(nb) {
                    if s == steward {
                        visited[nb] = true;
                        stack.push(nb);
                    }
                }
            }
        }
    }
    count
}
