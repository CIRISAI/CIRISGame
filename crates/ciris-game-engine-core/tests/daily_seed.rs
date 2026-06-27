//! Tests for daily-seed derivation and replay determinism (BACKLOG #2).

use ciris_game_engine_core::{derive_daily_seed, Difficulty, GameState, Move, MoveRecord};
use rand_chacha::ChaCha8Rng;
use rand_core::{RngCore, SeedableRng};

// ---------------------------------------------------------------------------
// DailySeed structural invariants
// ---------------------------------------------------------------------------

#[test]
fn derive_daily_seed_is_deterministic() {
    let a = derive_daily_seed("2026-06-27", 5);
    let b = derive_daily_seed("2026-06-27", 5);
    assert_eq!(a, b, "same (date, n) must produce identical DailySeed");
}

#[test]
fn k_in_range_and_slot0_is_easy() {
    for date in &["2026-01-01", "2026-06-15", "2026-12-31"] {
        let ds = derive_daily_seed(date, 5);

        assert!(
            ds.k >= 3 && ds.k <= 15,
            "K={} out of [3,15] for date={}",
            ds.k,
            date
        );
        assert_eq!(ds.roster[0], Difficulty::Easy, "slot 0 must always be Easy");
    }
}

#[test]
fn perma_dead_count_distinct_sorted_inbounds() {
    let n = 5u8;
    let total = (n as usize).pow(3);
    let ds = derive_daily_seed("2026-06-27", n);

    assert_eq!(
        ds.perma_dead.len(),
        ds.k as usize,
        "perma_dead length must equal k"
    );

    // sorted
    let mut sorted = ds.perma_dead.clone();
    sorted.sort_unstable();
    assert_eq!(ds.perma_dead, sorted, "perma_dead must be sorted ascending");

    // in-bounds
    for &idx in &ds.perma_dead {
        assert!(idx < total, "index {idx} out of range for n={n}");
    }

    // distinct
    let unique_count = {
        let mut v = ds.perma_dead.clone();
        v.dedup();
        v.len()
    };
    assert_eq!(
        unique_count,
        ds.perma_dead.len(),
        "perma_dead indices must be distinct"
    );
}

#[test]
fn different_dates_produce_different_board_state_hashes() {
    let a = derive_daily_seed("2026-06-27", 5);
    let b = derive_daily_seed("2026-06-28", 5);
    assert_ne!(
        a.board_state_hash, b.board_state_hash,
        "adjacent dates must produce different board_state_hash (with overwhelming probability)"
    );
}

// ---------------------------------------------------------------------------
// GameState constructors
// ---------------------------------------------------------------------------

#[test]
fn with_perma_dead_places_cells_correctly() {
    use ciris_game_engine_core::CellState;

    let perma = &[0usize, 10, 50, 100, 124];
    let gs = GameState::with_perma_dead(5, [0u8; 32], perma);

    for &idx in perma {
        assert_eq!(
            gs.board.get(idx),
            CellState::PermaDead,
            "index {idx} should be PermaDead"
        );
    }
    // A cell not in the list should be Empty.
    assert_eq!(gs.board.get(1), CellState::Empty);
}

#[test]
fn from_daily_seed_places_perma_dead_matching_derive() {
    use ciris_game_engine_core::CellState;

    let date = "2026-06-27";
    let n = 5u8;
    let ds = derive_daily_seed(date, n);
    let gs = GameState::from_daily_seed(date, n, [42u8; 32]);

    for &idx in &ds.perma_dead {
        assert_eq!(
            gs.board.get(idx),
            CellState::PermaDead,
            "index {idx} should be PermaDead per daily seed"
        );
    }
    // Total empty cells should be n³ minus K perma-dead.
    let expected_empty = (n as usize).pow(3) - ds.k as usize;
    assert_eq!(gs.board.empty_count(), expected_empty);
}

// ---------------------------------------------------------------------------
// Replay determinism harness (~50 pseudo-random (seed, move_log) pairs)
// ---------------------------------------------------------------------------

/// Play up to `max_moves` random moves on a 5×5×5 board and return the
/// recorded history and final outcome.
fn play_random_moves(
    seed: [u8; 32],
    max_moves: usize,
) -> (Vec<MoveRecord>, ciris_game_engine_core::Outcome) {
    let mut gs = GameState::new(5, seed);
    let mut move_rng = ChaCha8Rng::from_seed(seed);

    for _ in 0..max_moves {
        if gs.is_over() {
            break;
        }
        let legal = gs.legal_moves();
        if legal.is_empty() {
            break;
        }
        let pick = move_rng.next_u32() as usize % legal.len();
        let c = legal[pick];
        let _ = gs.apply_move(Move::new(c.i, c.j, c.k));
    }

    let history = gs.history.clone();
    let outcome = gs.outcome();
    (history, outcome)
}

/// Replay a fixed move log from scratch on a 5×5×5 board and return the
/// final outcome.
fn replay_move_log(seed: [u8; 32], log: &[MoveRecord]) -> ciris_game_engine_core::Outcome {
    let mut gs = GameState::new(5, seed);
    for rec in log {
        let mv = if rec.dispersal.is_empty() {
            Move::place(rec.coord)
        } else {
            Move::rebuild(rec.coord, rec.dispersal.clone())
        };
        let _ = gs.apply_move(mv);
    }
    gs.outcome()
}

#[test]
fn replay_determinism_50_pairs() {
    // Master RNG seeds the per-iteration game seeds.
    let mut master = ChaCha8Rng::from_seed([0xab; 32]);

    for i in 0..50usize {
        let mut seed = [0u8; 32];
        for b in &mut seed {
            *b = master.next_u32() as u8;
        }

        let (log, outcome_a) = play_random_moves(seed, 60);
        let outcome_b = replay_move_log(seed, &log);

        assert_eq!(
            outcome_a.total, outcome_b.total,
            "pair {i}: replay total mismatch"
        );
        assert_eq!(
            outcome_a.all_survivors, outcome_b.all_survivors,
            "pair {i}: replay all_survivors mismatch"
        );
        assert_eq!(
            outcome_a.board_state_hash, outcome_b.board_state_hash,
            "pair {i}: replay board_state_hash mismatch"
        );
    }
}

/// Same determinism guarantee when the board has pre-seeded perma-dead cells
/// (the actual daily-seed code path).
#[test]
fn replay_determinism_with_daily_perma_dead() {
    let mut master = ChaCha8Rng::from_seed([0xcd; 32]);

    for i in 0..20usize {
        let mut seed = [0u8; 32];
        for b in &mut seed {
            *b = master.next_u32() as u8;
        }

        // Use a fixed date; only the game-rng seed varies.
        let date = "2026-06-27";
        let n = 5u8;
        let ds = derive_daily_seed(date, n);

        let play = |s: [u8; 32]| {
            let mut gs = GameState::with_perma_dead(n, s, &ds.perma_dead);
            let mut move_rng = ChaCha8Rng::from_seed(s);
            for _ in 0..50 {
                if gs.is_over() {
                    break;
                }
                let legal = gs.legal_moves();
                if legal.is_empty() {
                    break;
                }
                let pick = move_rng.next_u32() as usize % legal.len();
                let c = legal[pick];
                let _ = gs.apply_move(Move::new(c.i, c.j, c.k));
            }
            gs.outcome()
        };

        let a = play(seed);
        let b = play(seed);

        assert_eq!(
            a.board_state_hash, b.board_state_hash,
            "daily-seed pair {i}: board_state_hash differs on replay"
        );
        assert_eq!(
            a.total, b.total,
            "daily-seed pair {i}: total differs on replay"
        );
    }
}
