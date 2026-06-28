//! Throwaway analysis harness for the PROPOSED "different-colour tubes cannot
//! cross" rule.
//!
//! This is NOT a rule change. The no-crossing rule is implemented purely as a
//! legal-move *filter* layered on top of the engine's real `legal_moves()`; the
//! shipped rules path in `engine.rs` is untouched. Run with:
//!
//! ```text
//! cargo run --release -p ciris-game-engine-core --example no_crossing_analysis
//! ```
//!
//! It plays a large sample of full games under two conditions (baseline vs the
//! no-crossing filter) and two policies (uniform-random, and the screensaver's
//! self-collapse-avoiding "Easy" policy), then prints min/max/mean/median for
//! game length, branching factor, score, the WILD rate, and how binding the rule
//! is. The findings are written up in `docs/analysis/NO_CROSSING_RULE.md`.

use ciris_game_engine_core::{
    is_crossing_illegal, Board, CellState, Coord, GameState, Move, Steward, COLLAPSE_THRESHOLD,
    DEFAULT_BOARD_N,
};
use rand_chacha::ChaCha8Rng;
use rand_core::{RngCore, SeedableRng};

/// Games played per (condition, policy) cell.
const N_GAMES: usize = 1000;

// The no-crossing predicate now lives in the engine core
// (`ciris_game_engine_core::crossing::is_crossing_illegal`) and is the SHIPPED
// rule. This harness imports it so the analysis can never drift from the engine.
// To measure the rule-off baseline faithfully, each game disables the engine's
// built-in enforcement (`set_no_crossing_rule(false)`) and re-applies the rule
// here as a legal-move filter only in the RULE condition — exactly the layering
// this document describes.

// ----------------------------------------------------------------------------
// Policies
// ----------------------------------------------------------------------------

/// Size the same-steward mesh would reach if `steward` placed a live cell at
/// `idx` (flood over live same-steward face-neighbours, counting the new cell).
/// Replicates `screensaver::placed_mesh_size`.
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

#[derive(Clone, Copy, PartialEq)]
enum Policy {
    Uniform,
    Easy,
}

/// Pick a move from an already-filtered legal set per `policy`.
fn choose(gs: &GameState, legal: &[Coord], policy: Policy, rng: &mut ChaCha8Rng) -> Coord {
    match policy {
        Policy::Uniform => legal[(rng.next_u32() as usize) % legal.len()],
        Policy::Easy => {
            let steward = gs.current_steward();
            let safe: Vec<Coord> = legal
                .iter()
                .copied()
                .filter(|c| match gs.board.index(*c) {
                    Some(idx) => placed_mesh_size(&gs.board, steward, idx) < COLLAPSE_THRESHOLD,
                    None => false,
                })
                .collect();
            let pool = if safe.is_empty() { legal } else { &safe };
            pool[(rng.next_u32() as usize) % pool.len()]
        }
    }
}

// ----------------------------------------------------------------------------
// One game
// ----------------------------------------------------------------------------

struct GameResult {
    /// Real placements made (one per filled empty cell).
    length: u32,
    /// Final total perma-dead.
    score: u32,
    wild: bool,
    /// Global deadlock: a full round (all four stewards) passed with empties
    /// still on the board — nobody could legally place anywhere.
    deadlock: bool,
    /// Single-steward passes: turns where the steward to move had no legal cell
    /// under the rule and skipped (a colour-local stall, not a global freeze).
    passes: u32,
    /// Per-turn branching = filtered legal-move count (0 on a pass turn).
    branching: Vec<u32>,
    /// Per-turn forbidden count = base_legal − filtered_legal.
    forbidden: Vec<u32>,
    /// Per-turn base legal count (before the filter).
    base_legal: Vec<u32>,
}

fn play(seed_idx: u64, apply_rule: bool, policy: Policy) -> GameResult {
    let mut game_seed = [0u8; 32];
    game_seed[..8].copy_from_slice(&seed_idx.to_le_bytes());
    let mut gs = GameState::new(DEFAULT_BOARD_N, game_seed);
    // The harness layers the rule itself (as a filter), so turn off the engine's
    // built-in enforcement in BOTH conditions — baseline must be rule-free, and
    // the rule column already pre-filters every move it applies.
    gs.set_no_crossing_rule(false);

    let mut ai_seed = [0u8; 32];
    ai_seed[..8].copy_from_slice(&seed_idx.to_le_bytes());
    ai_seed[31] = 0xA5;
    let mut rng = ChaCha8Rng::from_seed(ai_seed);

    let mut res = GameResult {
        length: 0,
        score: 0,
        wild: false,
        deadlock: false,
        passes: 0,
        branching: Vec::new(),
        forbidden: Vec::new(),
        base_legal: Vec::new(),
    };

    // Consecutive stuck turns. Four in a row = a full round nobody could move =
    // a true global deadlock (board frozen with empties remaining).
    let mut consecutive_stuck = 0u32;
    let mut guard = 0u32;
    while !gs.is_over() {
        guard += 1;
        assert!(guard < 1_000_000, "runaway game loop");

        // Endgame rebuild-flush: board full but a crater is still pending. No
        // placement happens; resolve the owed layout with the auto chooser.
        if gs.board.empty_count() == 0 {
            let _ = gs.apply_move(Move::place(Coord::new(0, 0, 0)));
            consecutive_stuck = 0;
            continue;
        }

        let base = gs.legal_moves();
        let steward = gs.current_steward();
        let legal: Vec<Coord> = if apply_rule {
            base.iter()
                .copied()
                .filter(|c| !is_crossing_illegal(&gs.board, *c, steward))
                .collect()
        } else {
            base.clone()
        };

        // Record branching / binding for this decision turn (0 = a pass turn).
        res.base_legal.push(base.len() as u32);
        res.branching.push(legal.len() as u32);
        res.forbidden.push((base.len() - legal.len()) as u32);

        if legal.is_empty() {
            // Colour-local stall: this steward can't legally place anywhere, but
            // a different colour might. Pass to the next steward (no engine pass
            // rule exists; we model the natural extension). A full round of
            // passes is a global deadlock.
            res.passes += 1;
            consecutive_stuck += 1;
            if consecutive_stuck >= 4 {
                res.deadlock = true;
                break;
            }
            gs.current = (gs.current + 1) & 0b11;
            continue;
        }

        consecutive_stuck = 0;
        let pick = choose(&gs, &legal, policy, &mut rng);
        let _ = gs.apply_move(Move::place(pick));
        res.length += 1;
    }

    let outcome = gs.outcome();
    res.score = outcome.total;
    res.wild = outcome.all_survivors && !res.deadlock;
    res
}

// ----------------------------------------------------------------------------
// Stats
// ----------------------------------------------------------------------------

struct Stats {
    min: f64,
    max: f64,
    mean: f64,
    median: f64,
}

fn stats(v: &[f64]) -> Stats {
    if v.is_empty() {
        return Stats {
            min: 0.0,
            max: 0.0,
            mean: 0.0,
            median: 0.0,
        };
    }
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = s.len();
    let mean = s.iter().sum::<f64>() / n as f64;
    let median = if n % 2 == 1 {
        s[n / 2]
    } else {
        (s[n / 2 - 1] + s[n / 2]) / 2.0
    };
    Stats {
        min: s[0],
        max: s[n - 1],
        mean,
        median,
    }
}

/// Geometric mean of a positive-valued slice (the product-relevant average for
/// game-tree complexity).
fn geomean(v: &[f64]) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    let log_sum: f64 = v.iter().map(|&x| x.max(1.0).ln()).sum();
    (log_sum / v.len() as f64).exp()
}

fn fmt(s: &Stats) -> String {
    format!(
        "min {:.1} / max {:.1} / mean {:.2} / median {:.1}",
        s.min, s.max, s.mean, s.median
    )
}

fn run_cell(label: &str, apply_rule: bool, policy: Policy) {
    let mut lengths = Vec::new();
    let mut scores = Vec::new();
    let mut all_branching = Vec::new();
    let mut wild = 0usize;
    let mut deadlocks = 0usize;
    let mut games_with_pass = 0usize;
    let mut total_passes = 0u64;
    let mut binding_turns = 0usize;
    let mut total_turns = 0usize;
    let mut frac_sum = 0.0f64;
    let mut total_forbidden = 0u64;
    let mut total_base = 0u64;

    for g in 0..N_GAMES {
        let r = play(g as u64, apply_rule, policy);
        lengths.push(r.length as f64);
        scores.push(r.score as f64);
        if r.wild {
            wild += 1;
        }
        if r.deadlock {
            deadlocks += 1;
        }
        if r.passes > 0 {
            games_with_pass += 1;
        }
        total_passes += r.passes as u64;
        for &b in &r.branching {
            all_branching.push(b as f64);
        }
        for (i, &f) in r.forbidden.iter().enumerate() {
            total_turns += 1;
            if f > 0 {
                binding_turns += 1;
            }
            let base = r.base_legal[i];
            if base > 0 {
                frac_sum += f as f64 / base as f64;
            }
            total_forbidden += f as u64;
            total_base += r.base_legal[i] as u64;
        }
    }

    let len_s = stats(&lengths);
    let score_s = stats(&scores);
    let branch_s = stats(&all_branching);
    let gmean = geomean(&all_branching);

    println!("\n=== {label} (N={N_GAMES}) ===");
    println!("  length     : {}", fmt(&len_s));
    println!("  branching  : {} | geomean {:.2}", fmt(&branch_s), gmean);
    println!("  score      : {}", fmt(&score_s));
    println!(
        "  WILD       : {} / {} ({:.2}%)",
        wild,
        N_GAMES,
        100.0 * wild as f64 / N_GAMES as f64
    );
    println!(
        "  passes     : {} games had >=1 pass ({:.2}%); {:.2} passes/game avg",
        games_with_pass,
        100.0 * games_with_pass as f64 / N_GAMES as f64,
        total_passes as f64 / N_GAMES as f64,
    );
    println!(
        "  deadlocks  : {} / {} ({:.2}%) global (full round, all stuck)",
        deadlocks,
        N_GAMES,
        100.0 * deadlocks as f64 / N_GAMES as f64
    );
    println!(
        "  rule bind  : {:.2}% of turns removed >=1 move; mean forbidden frac/turn {:.4}; overall forbidden frac {:.4}",
        100.0 * binding_turns as f64 / total_turns.max(1) as f64,
        frac_sum / total_turns.max(1) as f64,
        total_forbidden as f64 / total_base.max(1) as f64,
    );

    // Game-tree complexity from measured numbers: log10( b^d ).
    let log10_arith = len_s.mean * branch_s.mean.log10();
    let log10_geom = len_s.mean * gmean.log10();
    println!(
        "  game-tree  : ~10^{:.1} (arith b={:.1}, d={:.1}); ~10^{:.1} (geom b={:.1})",
        log10_arith, branch_s.mean, len_s.mean, log10_geom, gmean
    );
}

fn main() {
    println!("CIRISGame — no-crossing rule analysis (N={N_GAMES} games/cell)");
    println!("Board: 5x5x5 = 125 cells, K=0 starting perma-dead (clean GameState::new).");

    run_cell("BASELINE / uniform-random", false, Policy::Uniform);
    run_cell("RULE     / uniform-random", true, Policy::Uniform);
    run_cell("BASELINE / Easy (avoid self-collapse)", false, Policy::Easy);
    run_cell("RULE     / Easy (avoid self-collapse)", true, Policy::Easy);
}
