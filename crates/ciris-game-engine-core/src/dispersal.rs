//! Algorithm A — Morton-greedy dispersal (DESIGN_BRIEF §4.6).
//!
//! When a mesh of `N ≥ 7` cells collapses, it disperses into live pairs of the
//! steward's color plus perma-dead spacers. The partition is fully deterministic:
//! Morton order is canonical and partner selection is lex-greedy, so the same
//! dead mesh always yields the same dispersal.
//!
//! ## Count contract (the locked strategic spine)
//!
//! With `k = N / 3` and `r = N mod 3`, the *ideal* perma-dead count is
//! `k + (1 if r == 1 else 0)`. The `r = 2` asymmetry (N = 8, 11, 14 cost less
//! than their `r = 1` neighbors) is intentional.
//!
//! | N  | k | r | live pairs | perma-dead |
//! |----|---|---|-----------:|-----------:|
//! | 7  | 2 | 1 | 2          | 3          |
//! | 8  | 2 | 2 | 3          | 2          |
//! | 13 | 4 | 1 | 4          | 5          |
//! | 14 | 4 | 2 | 5          | 4          |
//!
//! The table above shows the *ideal* counts before step-4 separation validation.
//! Step-4 may increase the perma count above the ideal when live pairs in Morton
//! order happen to be face-adjacent to each other (a common occurrence in
//! densely-connected blobs). See `algorithm_a` doc for details.
//!
//! The geometric pass can exceed the ideal count only in degenerate topologies
//! (a cell with no unconsumed face-adjacent partner, or a non-adjacent `r = 2`
//! remainder). Characterizing those on 6×6×6+ boards is BACKLOG #4; the 5×5×5
//! default matches the table after step-4 is accounted for.
//!
//! ## SINGLE_LIVE
//!
//! A *SINGLE_LIVE* cell is a live cell produced by dispersal that has no
//! face-adjacent live cell also in the dispersal result (it becomes a size-1
//! mesh). This occurs when the lex-greedy partner scan finds no adjacent neighbor
//! for `c[i]` — positions `i` and `i+1` both go live as two separate size-1
//! meshes. Size-1 meshes are first-class (DESIGN_BRIEF §4.10); `SINGLE_LIVE` is
//! not a new state, just a descriptive label for a lone live cell. No demotion
//! occurs.

use alloc::collections::BTreeSet;
use alloc::vec::Vec;

use crate::board::Board;

/// Ideal `(live_cells, perma_dead)` split for a dead mesh of `n` cells,
/// *before* step-4 separation validation. Step-4 may increase `perma_dead`
/// above the ideal in practice.
pub fn dispersal_counts(n: usize) -> (usize, usize) {
    let k = n / 3;
    let r = n % 3;
    let perma = k + if r == 1 { 1 } else { 0 };
    (n - perma, perma)
}

/// The result of dispersing one dead mesh: which cells respawn `Live` (the
/// steward's color) and which become `PermaDead`. Both lists are sorted.
///
/// `single_live` is the count of live cells that have no face-adjacent live
/// partner in this result — each forms a size-1 mesh (DESIGN_BRIEF §4.10).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dispersal {
    pub live: Vec<usize>,
    pub perma: Vec<usize>,
    /// Count of live cells with no face-adjacent live partner in `live`.
    /// These are size-1 "SINGLE_LIVE" meshes — first-class, not demoted.
    pub single_live: usize,
}

/// Run Algorithm A over `cells` (the collapsed mesh) on `board`.
///
/// The board is only read for geometry (coords + adjacency); callers apply the
/// returned partition to actual cell states.
///
/// ## Step-4 separation validation (§4.6 step 4)
///
/// After the initial live/perma partition, every live pair is checked in
/// Morton order against all previously accepted live pairs. If any cell in the
/// current pair is face-adjacent to any cell in an already-accepted pair, the
/// current pair is demoted to `PERMA_DEAD`. This prevents re-formed pairs from
/// starting the next turn already merged into a larger connected component.
///
/// Consequence: on densely-connected blobs (which are the common case), step-4
/// can raise the perma count above the `dispersal_counts` ideal — notably on
/// the canonical 5×5×5 board with simple corner-grown blobs. That excess is a
/// known property of the algorithm; the sweep (BACKLOG #4) quantifies it.
///
/// ## §4.6 step-2 loop bound
///
/// The brief says "while `i + 2 ≤ N`" (1-based). With 0-based indexing the
/// correct condition is `i + 2 < n` (i.e., the last valid triple consumes
/// positions `n-3, n-2, n-1`). This implementation uses `< n`.
pub fn algorithm_a(board: &Board, cells: &[usize]) -> Dispersal {
    let n = cells.len();
    let mut ordered = board.morton_sorted(cells);
    let mut perma: Vec<usize> = Vec::new();
    // `consumed[p]` marks positions already assigned in an earlier window.
    let mut consumed = alloc::vec![false; n];

    // Live pairs accumulated in Morton order for step-4 validation.
    // Each entry is (cell_a, cell_b, adjacent) where `adjacent` records
    // whether the pair is geometrically face-adjacent (false → SINGLE_LIVE × 2).
    let mut pairs: Vec<(usize, usize, bool)> = Vec::new();

    // Walk triples: positions i, i+1 -> live pair; position i+2 -> perma-dead.
    let mut i = 0;
    while i + 2 < n {
        // Try to make positions i and i+1 a face-adjacent pair. If c[i+1] is not
        // adjacent, pull in the lex-smallest unconsumed face-adjacent neighbor.
        if !board.adjacent(ordered[i], ordered[i + 1]) {
            let mut best: Option<usize> = None;
            for p in (i + 1)..n {
                if consumed[p] {
                    continue;
                }
                if board.adjacent(ordered[i], ordered[p]) {
                    best = match best {
                        None => Some(p),
                        Some(b) => {
                            if board.coord(ordered[p]) < board.coord(ordered[b]) {
                                Some(p)
                            } else {
                                Some(b)
                            }
                        }
                    };
                }
            }
            if let Some(p) = best {
                ordered.swap(i + 1, p);
            }
            // If no adjacent partner exists, positions i and i+1 still both go
            // live — two SINGLE_LIVE size-1 meshes (first-class per §4.10).
        }
        let adj = board.adjacent(ordered[i], ordered[i + 1]);
        pairs.push((ordered[i], ordered[i + 1], adj));
        consumed[i] = true;
        consumed[i + 1] = true;
        perma.push(ordered[i + 2]);
        consumed[i + 2] = true;
        i += 3;
    }

    // Remainder (§4.6 step 3).
    match n % 3 {
        1 => perma.push(ordered[n - 1]),
        2 => {
            // One extra live pair at the boundary if adjacent; else both perma.
            if board.adjacent(ordered[n - 2], ordered[n - 1]) {
                pairs.push((ordered[n - 2], ordered[n - 1], true));
            } else {
                perma.push(ordered[n - 2]);
                perma.push(ordered[n - 1]);
            }
        }
        _ => {}
    }

    // Step-4: separation validation (§4.6 step 4).
    //
    // Walk pairs in Morton order. If any cell in a pair is face-adjacent to a
    // cell in an already-accepted pair, demote the current pair to PERMA_DEAD.
    // Determinism is guaranteed: Morton order is canonical (same mesh → same
    // ordered sequence → same demotion decisions).
    //
    // Only the LATER pair is demoted (per spec). Earlier accepted pairs are never
    // revisited. A non-adjacent "pair" (SINGLE_LIVE × 2) participates in the
    // same check — if either singleton touches an accepted pair it is demoted.
    let mut accepted_cells: Vec<usize> = Vec::new();
    let mut demoted: Vec<usize> = Vec::new();

    for (a, b, _adj) in &pairs {
        let touching = accepted_cells
            .iter()
            .any(|&c| board.adjacent(c, *a) || board.adjacent(c, *b));
        if touching {
            demoted.push(*a);
            demoted.push(*b);
        } else {
            accepted_cells.push(*a);
            accepted_cells.push(*b);
        }
    }

    perma.extend_from_slice(&demoted);

    // Build final sorted partitions.
    let perma_set: BTreeSet<usize> = perma.iter().copied().collect();
    let live_cells: Vec<usize> = ordered
        .iter()
        .copied()
        .filter(|x| !perma_set.contains(x))
        .collect();

    // Count SINGLE_LIVE: live cells with no face-adjacent live neighbor in the
    // result.
    let live_set: BTreeSet<usize> = live_cells.iter().copied().collect();
    let single_live = live_cells
        .iter()
        .filter(|&&c| {
            board
                .neighbors(c)
                .iter()
                .all(|nb| !live_set.contains(nb))
        })
        .count();

    let mut live = live_cells;
    live.sort_unstable();
    let mut perma_sorted: Vec<usize> = perma_set.into_iter().collect();
    perma_sorted.sort_unstable();

    Dispersal {
        live,
        perma: perma_sorted,
        single_live,
    }
}
