//! `ciris-play` — a stdin/stdout CLI driver for CIRISGame.
//!
//! Run with: `cargo run -p ciris-game-engine-core --features std --bin ciris-play`
//!
//! Seat 0 (Sienna) is the human player. Seats 1–3 (Lapis, Verdigris, Kaolin) are
//! Easy AI. On each human turn the board state is printed layer by layer and legal
//! moves are listed. Type a coordinate `i j k` (0-indexed) to place.
//!
//! Cell symbols:
//!   .  Empty        S  Sienna (you)    L  Lapis
//!   V  Verdigris    K  Kaolin          t  TempDead
//!   X  PermaDead

use std::io::{self, BufRead, Write};

use ciris_game_engine_core::{
    board::{CellState, Steward},
    engine::{GameState, Move},
    Coord, DEFAULT_BOARD_N,
};
use rand_chacha::ChaCha8Rng;
use rand_core::{RngCore, SeedableRng};

const N: u8 = DEFAULT_BOARD_N;

fn cell_char(s: CellState) -> char {
    match s {
        CellState::Empty => '.',
        CellState::Live(Steward::Sienna) => 'S',
        CellState::Live(Steward::Lapis) => 'L',
        CellState::Live(Steward::Verdigris) => 'V',
        CellState::Live(Steward::Kaolin) => 'K',
        CellState::TempDead(_) => 't',
        CellState::PermaDead => 'X',
    }
}

fn print_board(gs: &GameState) {
    let n = gs.board.n as usize;
    for j in (0..n).rev() {
        println!("  layer j={j}:");
        for k in 0..n {
            print!("    ");
            for i in 0..n {
                let c = Coord::new(i as u8, j as u8, k as u8);
                let idx = gs.board.index(c).unwrap();
                print!("{} ", cell_char(gs.board.get(idx)));
            }
            println!("  (k={k})");
        }
    }
    println!("  (i: →, k: ↓)");
}

fn print_scores(gs: &GameState) {
    let names = ["Sienna", "Lapis", "Verdigris", "Kaolin"];
    println!("\nScores (perma-dead created — lower is better):");
    for (i, &score) in gs.scores.iter().enumerate() {
        println!("  {}: {}", names[i], score);
    }
}

/// Easy AI: prefer cells that don't immediately collapse our mesh; otherwise any legal.
fn choose_move(gs: &GameState, rng: &mut ChaCha8Rng) -> Option<Coord> {
    use ciris_game_engine_core::COLLAPSE_THRESHOLD;
    let legal = gs.current_legal_moves();
    if legal.is_empty() {
        return None;
    }
    let steward = gs.current_steward();
    let safe: Vec<Coord> = legal
        .iter()
        .copied()
        .filter(|&c| {
            if let Some(idx) = gs.board.index(c) {
                placed_mesh_size(&gs.board, steward, idx) < COLLAPSE_THRESHOLD
            } else {
                false
            }
        })
        .collect();
    let pool = if safe.is_empty() { &legal } else { &safe };
    let pick = (rng.next_u32() as usize) % pool.len();
    pool.get(pick).copied()
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

fn main() {
    let stdin = io::stdin();
    let stdout = io::stdout();

    let mut gs = GameState::new(N, [0u8; 32]);
    let mut ai_rng = ChaCha8Rng::seed_from_u64(0xABCD_1234);

    println!("=== CIRISGame CLI ===");
    println!("You are Sienna (S). Lapis (L), Verdigris (V), Kaolin (K) are AI.");
    println!("One rule: don't let your mesh hit 7 cells. Lowest perma-dead wins.");
    println!("To place: type  i j k  (e.g. '2 2 2')  then Enter.");
    println!("Score shown after each turn. Type 'q' to quit.\n");

    let mut turn = 0u32;

    loop {
        if gs.is_over() {
            println!("\n=== Game Over ===");
            print_board(&gs);
            print_scores(&gs);
            let outcome = gs.outcome();
            println!("\nAll survivors: {}", outcome.all_survivors);
            if outcome.all_survivors {
                println!("WILD — M-1 achieved! All stewards cohabited without collapse.");
            } else {
                let min_score = outcome.permadead.iter().min().copied().unwrap_or(0);
                let names = ["Sienna", "Lapis", "Verdigris", "Kaolin"];
                let winners: Vec<&str> = outcome
                    .permadead
                    .iter()
                    .enumerate()
                    .filter(|&(_, &s)| s == min_score)
                    .map(|(i, _)| names[i])
                    .collect();
                println!("Winner(s): {} with {} perma-dead", winners.join(", "), min_score);
            }
            break;
        }

        let steward = gs.current_steward();
        turn += 1;

        if steward == Steward::Sienna {
            // Human turn.
            println!("\n── Turn {} — YOUR MOVE (Sienna) ──", turn);
            print_board(&gs);
            print_scores(&gs);

            let legal = gs.current_legal_moves();
            println!("\nLegal moves ({}):", legal.len());
            for (i, c) in legal.iter().enumerate() {
                if i < 20 {
                    print!("  ({},{},{}) ", c.i, c.j, c.k);
                }
                if i == 19 && legal.len() > 20 {
                    print!("…+{} more", legal.len() - 20);
                }
            }
            println!();

            if legal.is_empty() {
                println!("[forced pass]");
                let _ = gs.apply_move(Move::place(Coord::new(0, 0, 0)));
                continue;
            }

            loop {
                print!("place (i j k): ");
                let _ = stdout.lock().flush();
                let mut line = String::new();
                stdin.lock().read_line(&mut line).expect("stdin read");
                let trimmed = line.trim();
                if trimmed == "q" || trimmed == "quit" {
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
                        match gs.apply_move(Move::place(coord)) {
                            Ok(_) => break,
                            Err(e) => println!("Rejected: {:?}", e),
                        }
                    } else {
                        println!("Not a legal move. Try again.");
                    }
                } else {
                    println!("Enter three numbers, e.g.:  2 3 1");
                }
            }
        } else {
            // AI turn.
            let name = match steward {
                Steward::Lapis => "Lapis",
                Steward::Verdigris => "Verdigris",
                Steward::Kaolin => "Kaolin",
                Steward::Sienna => unreachable!(),
            };
            let mv = match choose_move(&gs, &mut ai_rng) {
                Some(c) => {
                    println!("  {name} → ({},{},{})", c.i, c.j, c.k);
                    Move::place(c)
                }
                None => {
                    println!("  {name} → [forced pass]");
                    Move::place(Coord::new(0, 0, 0))
                }
            };
            let _ = gs.apply_move(mv);
        }
    }
}
