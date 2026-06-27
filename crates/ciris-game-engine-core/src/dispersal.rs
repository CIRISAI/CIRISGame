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
//! The geometric pass can exceed the ideal count only in degenerate topologies
//! (a cell with no unconsumed face-adjacent partner, or a non-adjacent `r = 2`
//! remainder). Characterizing those on 6×6×6+ boards is BACKLOG #4; the 5×5×5
//! default matches the table.
//!
//! Separation validation (§4.6 step 4 — demoting touching live pairs) is **not**
//! yet applied here; it is owned by the Algorithm A sweep (BACKLOG #4). Adjacent
//! live pairs merely form a larger (still legal, `< 7`) live component, which
//! this crate's model permits.

use alloc::collections::BTreeSet;
use alloc::vec::Vec;

use crate::board::Board;

/// Ideal `(live_cells, perma_dead)` split for a dead mesh of `n` cells.
pub fn dispersal_counts(n: usize) -> (usize, usize) {
    let k = n / 3;
    let r = n % 3;
    let perma = k + if r == 1 { 1 } else { 0 };
    (n - perma, perma)
}

/// The result of dispersing one dead mesh: which cells respawn `Live` (the
/// steward's color) and which become `PermaDead`. Both lists are sorted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dispersal {
    pub live: Vec<usize>,
    pub perma: Vec<usize>,
}

/// Run Algorithm A over `cells` (the collapsed mesh) on `board`.
///
/// The board is only read for geometry (coords + adjacency); callers apply the
/// returned partition to actual cell states.
pub fn algorithm_a(board: &Board, cells: &[usize]) -> Dispersal {
    let n = cells.len();
    let mut ordered = board.morton_sorted(cells);
    let mut perma: Vec<usize> = Vec::new();
    // `consumed[p]` marks positions already assigned in an earlier window.
    let mut consumed = alloc::vec![false; n];

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
            // live — two size-1 meshes (first-class per §4.10).
        }
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
            if !board.adjacent(ordered[n - 2], ordered[n - 1]) {
                perma.push(ordered[n - 2]);
                perma.push(ordered[n - 1]);
            }
        }
        _ => {}
    }

    let perma_set: BTreeSet<usize> = perma.iter().copied().collect();
    let mut live: Vec<usize> = ordered
        .iter()
        .copied()
        .filter(|x| !perma_set.contains(x))
        .collect();
    live.sort_unstable();
    let mut perma: Vec<usize> = perma_set.into_iter().collect();
    perma.sort_unstable();

    Dispersal { live, perma }
}
