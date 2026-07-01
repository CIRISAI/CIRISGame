//! `ciris-play` — stdin/stdout CLI driver for CIRISGame.
//!
//! ```text
//! ciris-play           # human (Sienna) vs 3 Easy AI
//! ciris-play --demo    # 4 Easy AI self-play; press Enter to step through each turn
//! ```
//!
//! The Easy AI is the same policy as the in-game screensaver: uniform-random among
//! legal placements, preferring cells that won't immediately collapse the mover's
//! own mesh (self-explosion pruning). No random-fallback path exists.
//!
//! Cell symbols:
//!   .  Empty       S  Sienna      L  Lapis
//!   V  Verdigris   K  Kaolin      *  atari (mesh of 6)
//!   t  TempDead (smouldering, 1 turn)    X  PermaDead (forever)

use std::io::{self, BufRead, Write};

use ciris_game_engine_core::{
    board::{CellState, Steward},
    engine::{GameState, Move},
    Coord, ATARI_SIZE, COLLAPSE_THRESHOLD, DEFAULT_BOARD_N,
};
use rand_chacha::ChaCha8Rng;
use rand_core::{RngCore, SeedableRng};

const N: u8 = DEFAULT_BOARD_N;
const NAMES: [&str; 4] = ["Sienna", "Lapis", "Verdigris", "Kaolin"];

// ── board rendering ──────────────────────────────────────────────────────────

fn cell_char(gs: &GameState, idx: usize) -> char {
    if let CellState::Live(s) = gs.board.get(idx) {
        if gs.board.mesh_containing(idx).len() == ATARI_SIZE {
            return '*';
        }
        return match s {
            Steward::Sienna => 'S',
            Steward::Lapis => 'L',
            Steward::Verdigris => 'V',
            Steward::Kaolin => 'K',
        };
    }
    match gs.board.get(idx) {
        CellState::Empty => '.',
        CellState::TempDead(_) => 't',
        CellState::PermaDead => 'X',
        CellState::Live(_) => unreachable!(),
    }
}

fn print_board(gs: &GameState) {
    let n = gs.board.n as usize;
    for j in (0..n).rev() {
        println!("  j={j}:");
        for k in 0..n {
            print!("    ");
            for i in 0..n {
                let c = Coord::new(i as u8, j as u8, k as u8);
                let idx = gs.board.index(c).unwrap();
                print!("{} ", cell_char(gs, idx));
            }
            println!("  k={k}");
        }
    }
    println!("  (i→, k↓)");
}

fn print_scores(gs: &GameState) {
    print!("  Scores: ");
    for (i, &s) in gs.scores.iter().enumerate() {
        print!("{}={} ", NAMES[i], s);
    }
    println!("(perma-dead created; lower wins)");
}

// ── Easy AI (same policy as screensaver) ────────────────────────────────────

/// Easy AI: uniform-random among legal moves, but prefer cells that don't
/// immediately collapse the mover's own mesh. Identical to `screensaver::choose_move`.
fn easy_move(gs: &GameState, rng: &mut ChaCha8Rng) -> Coord {
    let legal = gs.current_legal_moves();
    let steward = gs.current_steward();
    let safe: Vec<Coord> = legal
        .iter()
        .copied()
        .filter(|&c| {
            gs.board
                .index(c)
                .is_some_and(|idx| placed_mesh_size(&gs.board, steward, idx) < COLLAPSE_THRESHOLD)
        })
        .collect();
    let pool = if safe.is_empty() { &legal } else { &safe };
    pool[(rng.next_u32() as usize) % pool.len()]
}

fn placed_mesh_size(
    board: &ciris_game_engine_core::board::Board,
    steward: Steward,
    idx: usize,
) -> usize {
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

// ── event announcements ──────────────────────────────────────────────────────

fn snapshot(gs: &GameState) -> Vec<CellState> {
    (0..gs.board.len()).map(|i| gs.board.get(i)).collect()
}

fn announce_events(snap: &[CellState], gs: &GameState, slot: usize) {
    let name = NAMES[slot];
    let mut collapse_size = 0usize;
    let mut perma_born = 0usize;
    let mut live_born = 0usize;

    for (idx, &before) in snap.iter().enumerate().take(gs.board.len()) {
        let after = gs.board.get(idx);
        if before == after {
            continue;
        }
        match (before, after) {
            (CellState::Live(_), CellState::TempDead(_)) => collapse_size += 1,
            (CellState::TempDead(_), CellState::Live(_)) => live_born += 1,
            (CellState::TempDead(_), CellState::PermaDead) => perma_born += 1,
            _ => {}
        }
    }

    if collapse_size > 0 {
        println!();
        println!("  !! COLLAPSE: {name} mesh of {collapse_size} cells turns TempDead (smouldering 1 turn)");
        println!("     → {name} will rebuild next turn: Algorithm A → live pairs + perma-dead");
    }
    if perma_born > 0 || live_born > 0 {
        println!();
        println!("  >> REBUILD complete: {live_born} cells reborn as live, {perma_born} become PermaDead");
        println!(
            "     Score +{perma_born} for {name}  (total: {})",
            gs.scores[slot]
        );
    }

    // Atari warnings after this move.
    for s in Steward::ALL {
        let n = gs.atari_meshes(s).len();
        if n > 0 {
            println!(
                "  ⚠  {} has {n} mesh(es) at size 6 — next placement there = collapse!",
                NAMES[s.slot() as usize]
            );
        }
    }
}

// ── main ─────────────────────────────────────────────────────────────────────

fn main() {
    let demo = std::env::args().any(|a| a == "--demo");
    let stdin = io::stdin();
    let stdout = io::stdout();

    let mut gs = GameState::new(N, [0u8; 32]);
    let mut rng = ChaCha8Rng::seed_from_u64(0xCAFE_BABE);

    if demo {
        println!("=== CIRISGame demo — 4 Easy AI, press Enter to step ===");
        println!("Symbols: . empty  S/L/V/K live  * atari  t temp-dead  X perma-dead");
    } else {
        println!("=== CIRISGame — Sienna (you) vs 3 Easy AI ===");
        println!("Symbols: . empty  S/L/V/K live  * atari  t temp-dead  X perma-dead");
        println!("Place: type  i j k  (0-indexed). Type 'q' to quit.");
    }
    println!();

    let mut global_turn = 0u32;

    loop {
        if gs.is_over() {
            println!("\n═══════════════════════════════════════");
            println!("GAME OVER — final board:");
            print_board(&gs);
            println!();
            print_scores(&gs);
            let outcome = gs.outcome();
            println!();
            if outcome.all_survivors {
                println!(
                    "✦ WILD — M-1 achieved! All stewards cohabited without a single collapse."
                );
            } else {
                let min = *outcome.permadead.iter().min().unwrap();
                let winners: Vec<&str> = outcome
                    .permadead
                    .iter()
                    .enumerate()
                    .filter(|&(_, &s)| s == min)
                    .map(|(i, _)| NAMES[i])
                    .collect();
                println!("Winner(s): {} with {} perma-dead", winners.join(", "), min);
            }
            break;
        }

        let steward = gs.current_steward();
        let slot = steward.slot() as usize;
        let name = NAMES[slot];
        global_turn += 1;

        let is_rebuild = gs.is_rebuild_turn();
        let snap = snapshot(&gs);

        // ── header + board ───────────────────────────────────────────────
        println!("────────────────────────────────────────");
        if is_rebuild {
            println!(
                "Turn {global_turn}: {} — REBUILD TURN \
                 (TempDead cells will be replaced by Algorithm A layout + 1 new placement)",
                name
            );
        } else {
            println!("Turn {global_turn}: {}", name);
        }

        if demo {
            print_board(&gs);
            print_scores(&gs);
            print!("[Enter] next  [q] quit > ");
            let _ = stdout.lock().flush();
            let mut line = String::new();
            stdin.lock().read_line(&mut line).expect("stdin");
            if line.trim() == "q" {
                println!("Quitting.");
                return;
            }
        }

        // ── make move ────────────────────────────────────────────────────
        if !demo && steward == Steward::Sienna {
            // Human input.
            if !demo {
                print_board(&gs);
                print_scores(&gs);
            }
            let legal = gs.current_legal_moves();
            println!("\nLegal moves ({}):", legal.len());
            for (i, c) in legal.iter().enumerate().take(20) {
                print!("({},{},{}) ", c.i, c.j, c.k);
                if i == 19 && legal.len() > 20 {
                    print!("…+{}", legal.len() - 20);
                }
            }
            println!();
            loop {
                print!("place (i j k): ");
                let _ = stdout.lock().flush();
                let mut line = String::new();
                stdin.lock().read_line(&mut line).expect("stdin");
                let trimmed = line.trim();
                if trimmed == "q" {
                    println!("Quitting.");
                    return;
                }
                let parts: Vec<u8> = trimmed
                    .split_whitespace()
                    .filter_map(|s| s.parse().ok())
                    .collect();
                if parts.len() == 3 {
                    let coord = Coord::new(parts[0], parts[1], parts[2]);
                    if legal.contains(&coord) {
                        let _ = gs.apply_move(Move::place(coord));
                        announce_events(&snap, &gs, slot);
                        break;
                    } else {
                        println!("Not a legal move.");
                    }
                } else {
                    println!("Enter three numbers, e.g.: 2 3 1");
                }
            }
        } else {
            // Easy AI.
            let coord = easy_move(&gs, &mut rng);
            println!("  {name} places at ({},{},{})", coord.i, coord.j, coord.k);
            let _ = gs.apply_move(Move::place(coord));
            announce_events(&snap, &gs, slot);
        }
    }
}
