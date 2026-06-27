//! Integration tests for the deterministic engine core.

use std::collections::BTreeSet;

use ciris_game_engine_core::lattice::is_face_adjacent;
use ciris_game_engine_core::{
    algorithm_a, dispersal_counts, Board, CellState, Coord, GameState, Move, Steward,
};

/// Grow a connected component of `count` cells from `start`, always extending by
/// the smallest-index unused face-neighbor (deterministic). Returned in an order
/// where every cell after the first is adjacent to an earlier one.
fn grow_connected(board: &Board, start: usize, count: usize) -> Vec<usize> {
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
        let nb = next.expect("board too small to grow component");
        set.insert(nb);
        comp.push(nb);
    }
    comp
}

#[test]
fn neighbor_counts() {
    let board = Board::new(5);
    // Interior cell has all twelve face-neighbors.
    let interior = board.index(Coord::new(2, 2, 2)).unwrap();
    assert_eq!(board.neighbors(interior).len(), 12);
    // A corner has only the three offsets that stay in-bounds.
    let corner = board.index(Coord::new(0, 0, 0)).unwrap();
    assert_eq!(board.neighbors(corner).len(), 3);
}

#[test]
fn adjacency_definition() {
    // (0,0,0) and (1,1,0): two unit steps, one zero -> face-adjacent.
    assert!(is_face_adjacent(Coord::new(0, 0, 0), Coord::new(1, 1, 0)));
    // axis-aligned step is NOT a face-neighbor on this lattice.
    assert!(!is_face_adjacent(Coord::new(0, 0, 0), Coord::new(1, 0, 0)));
    // body diagonal is not adjacent either.
    assert!(!is_face_adjacent(Coord::new(0, 0, 0), Coord::new(1, 1, 1)));
}

#[test]
fn dispersal_count_table() {
    // (live_cells, perma_dead) — the locked strategic spine (§4.6).
    assert_eq!(dispersal_counts(7), (4, 3));
    assert_eq!(dispersal_counts(8), (6, 2));
    assert_eq!(dispersal_counts(13), (8, 5));
    assert_eq!(dispersal_counts(14), (10, 4));
}

#[test]
fn algorithm_a_n7_is_exact_and_deterministic() {
    let board = Board::new(5);
    let mesh = grow_connected(&board, board.index(Coord::new(0, 0, 0)).unwrap(), 7);

    let d1 = algorithm_a(&board, &mesh);
    let d2 = algorithm_a(&board, &mesh);

    // r == 1 -> perma count is exactly k + 1 = 3, independent of geometry.
    assert_eq!(d1.perma.len(), 3, "N=7 must produce 3 perma-dead");
    assert_eq!(d1.live.len(), 4, "N=7 must produce 4 live cells");
    assert_eq!(d1.live.len() + d1.perma.len(), 7);
    assert_eq!(d1, d2, "Algorithm A must be deterministic");

    // Live and perma partitions are disjoint and cover the mesh.
    let union: BTreeSet<usize> = d1.live.iter().chain(d1.perma.iter()).copied().collect();
    let original: BTreeSet<usize> = mesh.iter().copied().collect();
    assert_eq!(union, original);
}

#[test]
fn algorithm_a_n13_is_exact() {
    let board = Board::new(5);
    let mesh = grow_connected(&board, board.index(Coord::new(0, 0, 0)).unwrap(), 13);
    let d = algorithm_a(&board, &mesh);
    // r == 1 -> exactly k + 1 = 5 perma regardless of topology.
    assert_eq!(d.perma.len(), 5);
    assert_eq!(d.live.len(), 8);
}

#[test]
fn algorithm_a_r2_bounds() {
    let board = Board::new(5);
    for &(n, k) in &[(8usize, 2usize), (14, 4)] {
        let mesh = grow_connected(&board, board.index(Coord::new(0, 0, 0)).unwrap(), n);
        let d = algorithm_a(&board, &mesh);
        assert_eq!(d.live.len() + d.perma.len(), n);
        // r == 2: ideal is k perma (boundary pair adjacent); degenerate is k+2.
        assert!(
            d.perma.len() == k || d.perma.len() == k + 2,
            "N={n}: perma {} not in {{{k}, {}}}",
            d.perma.len(),
            k + 2
        );
    }
}

#[test]
fn collapse_scores_and_disperses() {
    let mut gs = GameState::new(5, [7u8; 32]);
    // Solo play so one steward can grow a seven-mesh on consecutive turns.
    gs.eliminated = [false, true, true, true];

    let seven = grow_connected(&gs.board, gs.board.index(Coord::new(0, 0, 0)).unwrap(), 7);
    let seven_set: BTreeSet<usize> = seven.iter().copied().collect();

    // Placements 1..=6 grow the mesh; the 7th triggers the collapse.
    for (n, &idx) in seven.iter().enumerate() {
        let c = gs.board.coord(idx);
        gs.apply_move(Move::new(c.i, c.j, c.k)).unwrap();
        if n < 6 {
            assert!(!gs.has_pending_dispersal(), "no collapse before 7 cells");
        }
    }
    assert!(gs.has_pending_dispersal(), "7th cell must trigger collapse");
    // Every cell of the dead mesh is TempDead until dispersal resolves.
    for &idx in &seven {
        assert_eq!(gs.board.get(idx), CellState::TempDead(Steward::Sienna));
    }
    assert_eq!(gs.scores[0], 0, "score accrues at dispersal, not at collapse");

    // A distant placement opens the next turn, resolving the dispersal first.
    let distant = (0..gs.board.len())
        .find(|i| {
            gs.board.get(*i) == CellState::Empty
                && !seven_set.contains(i)
                && gs.board.neighbors(*i).iter().all(|n| !seven_set.contains(n))
        })
        .unwrap();
    let dc = gs.board.coord(distant);
    gs.apply_move(Move::new(dc.i, dc.j, dc.k)).unwrap();

    assert_eq!(gs.scores[0], 3, "N=7 collapse costs 3 perma-dead");
    let live = seven.iter().filter(|i| matches!(gs.board.get(**i), CellState::Live(Steward::Sienna))).count();
    let perma = seven.iter().filter(|i| gs.board.get(**i) == CellState::PermaDead).count();
    assert_eq!((live, perma), (4, 3));
}

#[test]
fn surrounded_cell_is_never_captured() {
    // §4.10: no capture. A lone cell ringed by other colors survives untouched.
    let mut gs = GameState::new(5, [0u8; 32]);
    let center = gs.board.index(Coord::new(2, 2, 2)).unwrap();
    gs.board.set(center, CellState::Live(Steward::Sienna));
    for nb in gs.board.neighbors(center) {
        gs.board.set(nb, CellState::Live(Steward::Lapis));
    }
    // The Sienna cell is a first-class size-1 mesh, fully enclosed, still alive.
    assert_eq!(gs.board.mesh_containing(center).len(), 1);
    assert_eq!(gs.board.get(center), CellState::Live(Steward::Sienna));
}

#[test]
fn replay_is_deterministic() {
    fn play(seed: [u8; 32], skip: Option<usize>) -> ciris_game_engine_core::Outcome {
        let mut gs = GameState::new(5, seed);
        let mut placed = 0;
        // Round-robin a handful of legal, non-colliding moves.
        for idx in 0..gs.board.len() {
            if Some(idx) == skip {
                continue;
            }
            if placed >= 12 {
                break;
            }
            let c = gs.board.coord(idx);
            if gs.apply_move(Move::new(c.i, c.j, c.k)).is_ok() {
                placed += 1;
            }
        }
        gs.outcome()
    }

    let a = play([42u8; 32], None);
    let b = play([42u8; 32], None);
    assert_eq!(a, b, "identical seed + moves -> identical outcome");
    assert!(a.all_survivors, "no collapse in a sparse opening");

    let c = play([42u8; 32], Some(0));
    assert_ne!(
        a.board_state_hash, c.board_state_hash,
        "different placements -> different board hash"
    );
}
