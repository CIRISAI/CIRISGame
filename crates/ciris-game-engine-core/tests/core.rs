//! Integration tests for the deterministic engine core.

use std::collections::BTreeSet;

use ciris_game_engine_core::lattice::is_face_adjacent;
use ciris_game_engine_core::{
    algorithm_a, dispersal_counts, validate_layout, Board, CellState, Coord, GameState, Move,
    MoveError, Steward, COLLAPSE_THRESHOLD,
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

/// Largest connected (face-adjacent) component wholly within `cells`.
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

#[test]
fn neighbor_counts() {
    let board = Board::new(5);
    let interior = board.index(Coord::new(2, 2, 2)).unwrap();
    assert_eq!(board.neighbors(interior).len(), 12);
    let corner = board.index(Coord::new(0, 0, 0)).unwrap();
    assert_eq!(board.neighbors(corner).len(), 3);
}

#[test]
fn adjacency_definition() {
    assert!(is_face_adjacent(Coord::new(0, 0, 0), Coord::new(1, 1, 0)));
    assert!(!is_face_adjacent(Coord::new(0, 0, 0), Coord::new(1, 0, 0)));
    assert!(!is_face_adjacent(Coord::new(0, 0, 0), Coord::new(1, 1, 1)));
}

#[test]
fn dispersal_count_table() {
    // (live_cells, perma_dead) — the locked count floor (§4.6).
    assert_eq!(dispersal_counts(7), (4, 3));
    assert_eq!(dispersal_counts(8), (6, 2));
    assert_eq!(dispersal_counts(13), (8, 5));
    assert_eq!(dispersal_counts(14), (10, 4));
}

#[test]
fn auto_layout_n7_matches_table_exactly() {
    let board = Board::new(5);
    // N=7 (r=1) always disperses to exactly 3 perma / 4 live: each triple yields
    // one spacer, the remainder one more, and 4 live can never reach the
    // threshold so the narrow guard never demotes. (N=8's r=2 boundary pair is
    // only floor-optimal when geometrically adjacent — see the large-collapse
    // test; a human/agent layout can always hit the floor 2 via validate_layout.)
    let mesh = grow_connected(&board, board.index(Coord::new(0, 0, 0)).unwrap(), 7);
    let d1 = algorithm_a(&board, &mesh);
    let d2 = algorithm_a(&board, &mesh);
    assert_eq!(d1.perma.len(), 3, "N=7 perma");
    assert_eq!(d1.live.len(), 4, "N=7 live");
    assert_eq!(d1, d2, "auto layout must be deterministic");
    // partition covers the mesh exactly
    let union: BTreeSet<usize> = d1.live.iter().chain(d1.perma.iter()).copied().collect();
    let original: BTreeSet<usize> = mesh.iter().copied().collect();
    assert_eq!(union, original);
}

#[test]
fn auto_layout_large_collapse_is_legal_and_above_floor() {
    let board = Board::new(5);
    // Large collapses may demote a few extra pairs to keep live cells legal, but
    // never go below the count floor, and never leave a >= 7 live component.
    for &n in &[8usize, 13, 14] {
        let mesh = grow_connected(&board, board.index(Coord::new(0, 0, 0)).unwrap(), n);
        let d = algorithm_a(&board, &mesh);
        assert_eq!(d.live.len() + d.perma.len(), n, "N={n} conservation");
        assert!(
            d.perma.len() >= dispersal_counts(n).1,
            "N={n} perma must be >= floor"
        );
        assert!(
            max_component(&board, &d.live) < COLLAPSE_THRESHOLD,
            "N={n} live must contain no >=7 component"
        );
    }
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
    assert_eq!(gs.board.mesh_containing(center).len(), 1);
    assert_eq!(gs.board.get(center), CellState::Live(Steward::Sienna));
}

/// Grow a seven-mesh under solo play and return (game, the seven cells).
fn solo_to_collapse() -> (GameState, Vec<usize>) {
    let mut gs = GameState::new(5, [7u8; 32]);
    gs.eliminated = [false, true, true, true]; // slot 0 plays every turn
    let seven = grow_connected(&gs.board, gs.board.index(Coord::new(0, 0, 0)).unwrap(), 7);
    for &idx in &seven {
        let c = gs.board.coord(idx);
        gs.apply_move(Move::new(c.i, c.j, c.k)).unwrap();
    }
    (gs, seven)
}

/// A distant empty cell not in (or adjacent to) `footprint`.
fn distant_empty(gs: &GameState, footprint: &BTreeSet<usize>) -> Coord {
    let idx = (0..gs.board.len())
        .find(|i| {
            gs.board.get(*i) == CellState::Empty
                && !footprint.contains(i)
                && gs
                    .board
                    .neighbors(*i)
                    .iter()
                    .all(|n| !footprint.contains(n))
        })
        .unwrap();
    gs.board.coord(idx)
}

#[test]
fn collapse_then_auto_rebuild_scores_table() {
    let (mut gs, seven) = solo_to_collapse();
    let seven_set: BTreeSet<usize> = seven.iter().copied().collect();

    assert!(gs.has_pending_dispersal(), "7th cell triggers collapse");
    assert!(gs.is_rebuild_turn(), "owning slot owes a rebuild");
    for &idx in &seven {
        assert_eq!(gs.board.get(idx), CellState::TempDead(Steward::Sienna));
    }
    assert_eq!(gs.scores[0], 0, "score accrues at rebuild, not at collapse");

    // Auto layout (Move with no dispersal) on the rebuild turn.
    let dc = distant_empty(&gs, &seven_set);
    gs.apply_move(Move::new(dc.i, dc.j, dc.k)).unwrap();

    assert_eq!(
        gs.scores[0], 3,
        "N=7 auto rebuild costs the table's 3 perma"
    );
    let live = seven
        .iter()
        .filter(|i| matches!(gs.board.get(**i), CellState::Live(Steward::Sienna)))
        .count();
    let perma = seven
        .iter()
        .filter(|i| gs.board.get(**i) == CellState::PermaDead)
        .count();
    assert_eq!((live, perma), (4, 3));
}

#[test]
fn player_chooses_wreckage_layout() {
    let (mut gs, seven) = solo_to_collapse();
    let seven_set: BTreeSet<usize> = seven.iter().copied().collect();

    let footprint = gs.pending_footprint().expect("a crater to lay out");
    assert_eq!(footprint.len(), 7);

    // Player picks the first 3 crater cells (Morton/coord order) as perma-dead.
    let mut chosen = footprint.clone();
    chosen.sort();
    let perma_choice: Vec<Coord> = chosen.into_iter().take(3).collect();

    let dc = distant_empty(&gs, &seven_set);
    gs.apply_move(Move::rebuild(dc, perma_choice.clone()))
        .unwrap();

    assert_eq!(gs.scores[0], 3);
    for c in &perma_choice {
        let idx = gs.board.index(*c).unwrap();
        assert_eq!(
            gs.board.get(idx),
            CellState::PermaDead,
            "chosen cell is perma"
        );
    }
    let perma = seven
        .iter()
        .filter(|i| gs.board.get(**i) == CellState::PermaDead)
        .count();
    assert_eq!(perma, 3, "exactly the player's 3 are perma-dead");
}

#[test]
fn rebuild_rejects_below_floor_and_leaves_state_intact() {
    let (mut gs, seven) = solo_to_collapse();
    let seven_set: BTreeSet<usize> = seven.iter().copied().collect();
    let footprint = gs.pending_footprint().unwrap();

    // Only 2 perma where the floor is 3 → rejected, nothing mutated.
    let too_few: Vec<Coord> = footprint.into_iter().take(2).collect();
    let dc = distant_empty(&gs, &seven_set);
    let err = gs.apply_move(Move::rebuild(dc, too_few)).unwrap_err();
    assert_eq!(err, MoveError::DispersalTooFewPerma);
    assert!(gs.is_rebuild_turn(), "crater still owed after rejection");
    assert_eq!(gs.scores[0], 0, "no score on a rejected move");
    for &idx in &seven {
        assert_eq!(gs.board.get(idx), CellState::TempDead(Steward::Sienna));
    }
}

#[test]
fn validate_layout_rejects_illegal_live_shape() {
    // A footprint big enough that keeping all non-floor cells live forms a >=7
    // connected blob: choose the floor count of perma at one tip so the live
    // remainder stays a single large component.
    let board = Board::new(5);
    let mesh = grow_connected(&board, board.index(Coord::new(0, 0, 0)).unwrap(), 13);
    let floor = dispersal_counts(13).1; // 5
                                        // Pick the floor-count perma as the LAST cells in grow order (one end), so
                                        // the first 8 (a connected prefix by construction) remain live.
    let perma: Vec<usize> = mesh.iter().rev().take(floor).copied().collect();
    let live: Vec<usize> = mesh.iter().take(mesh.len() - floor).copied().collect();
    // Only meaningful if that live remainder actually has a >=7 component.
    if max_component(&board, &live) >= COLLAPSE_THRESHOLD {
        let err = validate_layout(&board, &mesh, &perma).unwrap_err();
        assert_eq!(err, ciris_game_engine_core::LayoutError::IllegalShape);
    }
}
