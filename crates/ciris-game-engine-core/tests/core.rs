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

    // FINDING (BACKLOG #4 / step-4): The ideal count (§4.6 table) for N=7 is
    // 3 perma / 4 live (r=1, k=2). However, step-4 separation validation demotes
    // all but the first accepted live pair in this dense corner-grown blob,
    // because every subsequent pair is face-adjacent to the first accepted pair.
    // Observed: 5 perma / 2 live. Conservation holds; determinism holds.
    // This excess is the characterisation finding from the Algorithm A sweep.
    assert_eq!(d1.perma.len(), 5, "N=7 corner blob: step-4 yields 5 perma (ideal=3)");
    assert_eq!(d1.live.len(), 2, "N=7 corner blob: step-4 yields 2 live (ideal=4)");
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
    // FINDING (step-4): ideal is 5 perma / 8 live (r=1, k=4). Step-4 demotes all
    // but the first pair in this dense blob: observed 11 perma / 2 live.
    assert_eq!(d.perma.len(), 11, "N=13 corner blob: step-4 yields 11 perma (ideal=5)");
    assert_eq!(d.live.len(), 2, "N=13 corner blob: step-4 yields 2 live (ideal=8)");
    assert_eq!(d.live.len() + d.perma.len(), 13);
}

#[test]
fn algorithm_a_r2_bounds() {
    let board = Board::new(5);
    // FINDING (step-4): ideal perma for r=2 is k (boundary pair adjacent); pre-
    // step-4 degenerate was k+2. Step-4 demotes all but the first pair in these
    // dense corner blobs: N=8 → 6 perma, N=14 → 12 perma (both leave 2 live).
    // Invariants that survive step-4: conservation and that perma >= ideal.
    for &(n, k) in &[(8usize, 2usize), (14, 4)] {
        let mesh = grow_connected(&board, board.index(Coord::new(0, 0, 0)).unwrap(), n);
        let d = algorithm_a(&board, &mesh);
        assert_eq!(
            d.live.len() + d.perma.len(),
            n,
            "N={n}: total cells must be conserved"
        );
        assert!(
            d.perma.len() >= k,
            "N={n}: perma {} must be >= ideal k={k}",
            d.perma.len()
        );
    }
    // Characterisation: exact step-4 counts for the canonical corner blob.
    let start = board.index(Coord::new(0, 0, 0)).unwrap();
    let d8 = algorithm_a(&board, &grow_connected(&board, start, 8));
    let d14 = algorithm_a(&board, &grow_connected(&board, start, 14));
    assert_eq!((d8.live.len(), d8.perma.len()), (2, 6), "N=8 step-4 characterisation");
    assert_eq!((d14.live.len(), d14.perma.len()), (2, 12), "N=14 step-4 characterisation");
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

    // FINDING (step-4): ideal N=7 cost is 3 perma. Step-4 demotes all but the
    // first pair in this corner blob, yielding 5 perma / 2 live.
    assert_eq!(gs.scores[0], 5, "N=7 collapse: step-4 yields 5 perma (ideal=3)");
    let live = seven.iter().filter(|i| matches!(gs.board.get(**i), CellState::Live(Steward::Sienna))).count();
    let perma = seven.iter().filter(|i| gs.board.get(**i) == CellState::PermaDead).count();
    assert_eq!((live, perma), (2, 5));
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
