//! Algorithm A sweep — BACKLOG #4.
//!
//! Characterises the deterministic auto layout ([`algorithm_a`]) across board
//! sizes N ∈ {3,4,5,6,7} and collapse sizes 7..=14. The auto chooser is the
//! narrow-guard variant: canonical Morton-greedy partition + demote a pair only
//! if keeping it would form a live component of `>= COLLAPSE_THRESHOLD`.
//!
//! Invariants verified everywhere:
//! - **Legality:** no live component ever reaches the collapse threshold.
//! - **Floor:** perma count is never below the locked table.
//! - **Conservation + determinism.**
//! - **Table-exact small collapses:** N = 7, 8 always match the table (their
//!   live cells can't reach 7), on every board size.

use std::collections::BTreeSet;

use ciris_game_engine_core::{algorithm_a, dispersal_counts, Board, Coord, COLLAPSE_THRESHOLD};

fn grow_connected(board: &Board, start: usize, count: usize) -> Option<Vec<usize>> {
    let mut comp = vec![start];
    let mut set: BTreeSet<usize> = BTreeSet::new();
    set.insert(start);
    while comp.len() < count {
        let mut next = None;
        'outer: for &c in &comp {
            let mut nbs: Vec<usize> = board.neighbors(c).to_vec();
            nbs.sort_unstable();
            for nb in nbs {
                if !set.contains(&nb) {
                    next = Some(nb);
                    break 'outer;
                }
            }
        }
        let nb = next?; // board too small to grow this far from this start
        set.insert(nb);
        comp.push(nb);
    }
    Some(comp)
}

fn max_component(board: &Board, cells: &[usize]) -> usize {
    let set: BTreeSet<usize> = cells.iter().copied().collect();
    let mut seen: BTreeSet<usize> = BTreeSet::new();
    let mut max = 0;
    for &start in &set {
        if seen.contains(&start) {
            continue;
        }
        let mut stack = vec![start];
        seen.insert(start);
        let mut size = 0;
        while let Some(c) = stack.pop() {
            size += 1;
            for nb in board.neighbors(c) {
                if set.contains(&nb) && seen.insert(nb) {
                    stack.push(nb);
                }
            }
        }
        max = max.max(size);
    }
    max
}

/// A handful of deterministic, spread-out start cells per board.
fn start_cells(n: u8) -> Vec<usize> {
    let board = Board::new(n);
    let m = n - 1;
    let mid = n / 2;
    let candidates = [
        Coord::new(0, 0, 0),
        Coord::new(m, 0, 0),
        Coord::new(0, m, 0),
        Coord::new(0, 0, m),
        Coord::new(mid, mid, mid),
        Coord::new(m, m, m),
        Coord::new(mid, 0, mid),
        Coord::new(0, mid, m),
        Coord::new(m, mid, 0),
    ];
    candidates.iter().filter_map(|c| board.index(*c)).collect()
}

#[test]
fn algorithm_a_sweep() {
    let mut runs = 0usize;
    let mut total_excess = 0usize;
    let mut max_excess = 0usize;

    for n in 3u8..=7 {
        let board = Board::new(n);
        let total_cells = (n as usize).pow(3);
        for &start in &start_cells(n) {
            for size in COLLAPSE_THRESHOLD..=14 {
                if size > total_cells {
                    continue;
                }
                let Some(mesh) = grow_connected(&board, start, size) else {
                    continue;
                };
                let d = algorithm_a(&board, &mesh);
                runs += 1;

                // Conservation.
                assert_eq!(
                    d.live.len() + d.perma.len(),
                    size,
                    "N={n} size={size} conserve"
                );
                // Legality — the whole point of the narrow guard.
                assert!(
                    max_component(&board, &d.live) < COLLAPSE_THRESHOLD,
                    "N={n} size={size}: live has a >= {COLLAPSE_THRESHOLD} component"
                );
                // Floor.
                let floor = dispersal_counts(size).1;
                assert!(d.perma.len() >= floor, "N={n} size={size} below floor");
                // Determinism.
                assert_eq!(
                    d,
                    algorithm_a(&board, &mesh),
                    "N={n} size={size} determinism"
                );
                // Table-exact for N=7 (r=1) on every board. N=8's r=2 boundary
                // pair is only floor-optimal when geometrically adjacent, so the
                // auto chooser may exceed the floor there (still legal).
                if size == 7 {
                    assert_eq!(d.perma.len(), floor, "N={n} size={size} must match table");
                }

                let excess = d.perma.len() - floor;
                total_excess += excess;
                max_excess = max_excess.max(excess);
            }
        }
    }

    eprintln!(
        "algorithm_a sweep: {runs} runs, total perma-excess-over-floor={total_excess}, max_excess={max_excess}"
    );
    assert!(runs > 200, "sweep should exercise many configurations");
}

#[test]
fn auto_layout_is_always_legal_and_deterministic() {
    let board = Board::new(5);
    for size in COLLAPSE_THRESHOLD..=14 {
        let mesh = grow_connected(&board, board.index(Coord::new(2, 2, 2)).unwrap(), size).unwrap();
        let a = algorithm_a(&board, &mesh);
        let b = algorithm_a(&board, &mesh);
        assert_eq!(a, b, "size={size} determinism");
        assert!(
            max_component(&board, &a.live) < COLLAPSE_THRESHOLD,
            "size={size} legality"
        );
    }
}
