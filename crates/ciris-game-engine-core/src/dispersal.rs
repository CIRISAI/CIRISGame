//! Dispersal — turning a collapsed mesh into live cells + perma-dead spacers
//! (DESIGN_BRIEF §4.6).
//!
//! ## Player-chosen layout (the mechanic)
//!
//! When a mesh of `N ≥ 7` collapses, its cells go dark (`TempDead`). On the
//! collapsing steward's **next turn** they *choose the wreckage layout*: which of
//! the crater's cells come back alive and which become perma-dead — subject to:
//!
//! 1. **Count floor (the locked score spine).** At least [`dispersal_counts`]
//!    perma-dead must be created, so a clever layout can never score *below* the
//!    table. With `k = N/3`, `r = N mod 3`, the floor is `k + (1 if r==1 else 0)`.
//!    The `r = 2` asymmetry (N = 8, 11, 14 cost less than their `r = 1`
//!    neighbours) is the strategic spine.
//!
//!    | N  | k | r | min perma-dead | live |
//!    |----|---|---|---------------:|-----:|
//!    | 7  | 2 | 1 | 3              | 4    |
//!    | 8  | 2 | 2 | 2              | 6    |
//!    | 13 | 4 | 1 | 5              | 8    |
//!    | 14 | 4 | 2 | 4              | 10   |
//!
//! 2. **Legality.** The live cells the player keeps may not form a connected
//!    component of [`COLLAPSE_THRESHOLD`] or more — dispersal must never hand back
//!    an already-collapse-sized live mesh.
//!
//! [`validate_layout`] enforces both for human/agent choices.
//!
//! ## The auto chooser ([`algorithm_a`])
//!
//! Computers (and any caller that supplies no layout) get a deterministic legal
//! layout: the canonical Morton-greedy partition (live pairs + spacers), then a
//! **narrow separation guard** that demotes a pair to perma-dead only if keeping
//! it would connect live cells into a component of `≥ COLLAPSE_THRESHOLD`. On the
//! common small collapse (N = 7, 8) the live cells can never reach 7, so the auto
//! layout always equals the table exactly. Larger collapses demote the minimum
//! needed to stay legal.
//!
//! Determinism: Morton order is canonical and the guard is order-deterministic,
//! so the same mesh always yields the same auto layout — replay-safe across
//! targets.

use alloc::collections::BTreeSet;
use alloc::vec::Vec;

use crate::board::Board;
use crate::COLLAPSE_THRESHOLD;

/// Minimum `(live_cells, perma_dead)` split for a dead mesh of `n` cells — the
/// locked count floor. Legality (no live component `≥ COLLAPSE_THRESHOLD`) may
/// force a few more perma-dead on large collapses, but never fewer.
pub fn dispersal_counts(n: usize) -> (usize, usize) {
    let k = n / 3;
    let r = n % 3;
    let perma = k + if r == 1 { 1 } else { 0 };
    (n - perma, perma)
}

/// A resolved dispersal: which crater cells respawn `Live` and which become
/// `PermaDead`. Both lists are sorted ascending.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dispersal {
    pub live: Vec<usize>,
    pub perma: Vec<usize>,
    /// Count of live cells with no face-adjacent live partner — first-class
    /// size-1 meshes (DESIGN_BRIEF §4.10), never demoted.
    pub single_live: usize,
}

/// Why a player-supplied dispersal layout was rejected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayoutError {
    /// A chosen perma-dead cell is not part of the collapsed crater.
    NotInFootprint,
    /// The same cell was listed twice.
    Duplicate,
    /// Fewer perma-dead than the locked count floor (would score below the table).
    TooFewPerma,
    /// The kept live cells form a component of `≥ COLLAPSE_THRESHOLD` — illegal.
    IllegalShape,
}

/// Largest connected component (face-adjacency) wholly within `set`.
fn max_component_size(board: &Board, set: &BTreeSet<usize>) -> usize {
    let mut seen: BTreeSet<usize> = BTreeSet::new();
    let mut max = 0;
    for &start in set {
        if seen.contains(&start) {
            continue;
        }
        let mut stack = alloc::vec![start];
        seen.insert(start);
        let mut size = 0;
        while let Some(c) = stack.pop() {
            size += 1;
            for nb in board.neighbors(c) {
                if set.contains(&nb) && !seen.contains(&nb) {
                    seen.insert(nb);
                    stack.push(nb);
                }
            }
        }
        if size > max {
            max = size;
        }
    }
    max
}

/// Count live cells with no face-adjacent live partner (SINGLE_LIVE).
fn count_single_live(board: &Board, live: &BTreeSet<usize>) -> usize {
    live.iter()
        .filter(|&&c| board.neighbors(c).iter().all(|nb| !live.contains(nb)))
        .count()
}

/// Assemble a [`Dispersal`] from a perma-dead set over a footprint.
fn assemble(board: &Board, footprint: &[usize], perma_set: &BTreeSet<usize>) -> Dispersal {
    let live_set: BTreeSet<usize> = footprint
        .iter()
        .copied()
        .filter(|c| !perma_set.contains(c))
        .collect();
    let single_live = count_single_live(board, &live_set);
    let mut live: Vec<usize> = live_set.into_iter().collect();
    live.sort_unstable();
    let mut perma: Vec<usize> = perma_set.iter().copied().collect();
    perma.sort_unstable();
    Dispersal {
        live,
        perma,
        single_live,
    }
}

/// Validate a player-chosen layout: `perma_choice` are the cells the steward
/// wants turned to perma-dead; the rest of `footprint` stays live.
///
/// Enforces the count floor and legality (see module docs). Returns the resolved
/// [`Dispersal`] or a [`LayoutError`].
pub fn validate_layout(
    board: &Board,
    footprint: &[usize],
    perma_choice: &[usize],
) -> Result<Dispersal, LayoutError> {
    let fp: BTreeSet<usize> = footprint.iter().copied().collect();
    let perma_set: BTreeSet<usize> = perma_choice.iter().copied().collect();

    if perma_set.len() != perma_choice.len() {
        return Err(LayoutError::Duplicate);
    }
    if !perma_set.is_subset(&fp) {
        return Err(LayoutError::NotInFootprint);
    }
    let floor = dispersal_counts(footprint.len()).1;
    if perma_set.len() < floor {
        return Err(LayoutError::TooFewPerma);
    }
    let live_set: BTreeSet<usize> = fp.difference(&perma_set).copied().collect();
    if max_component_size(board, &live_set) >= COLLAPSE_THRESHOLD {
        return Err(LayoutError::IllegalShape);
    }
    Ok(assemble(board, footprint, &perma_set))
}

/// Deterministic auto layout for computers / no-choice callers (the locked
/// "Algorithm A — Morton-greedy"). Always legal; equals the table exactly for
/// small collapses. See module docs.
pub fn algorithm_a(board: &Board, cells: &[usize]) -> Dispersal {
    let n = cells.len();
    let mut ordered = board.morton_sorted(cells);
    let mut perma: Vec<usize> = Vec::new();
    // Candidate live pairs in Morton order; consumed positions are skipped.
    let mut consumed = alloc::vec![false; n];
    let mut pairs: Vec<(usize, usize)> = Vec::new();

    // Walk triples: positions i, i+1 -> candidate live pair; i+2 -> perma-dead.
    let mut i = 0;
    while i + 2 < n {
        if !board.adjacent(ordered[i], ordered[i + 1]) {
            // Pull the lex-smallest unconsumed face-adjacent neighbour into i+1.
            let mut best: Option<usize> = None;
            for p in (i + 1)..n {
                if consumed[p] || !board.adjacent(ordered[i], ordered[p]) {
                    continue;
                }
                best = match best {
                    Some(b) if board.coord(ordered[b]) <= board.coord(ordered[p]) => Some(b),
                    _ => Some(p),
                };
            }
            if let Some(p) = best {
                ordered.swap(i + 1, p);
            }
        }
        pairs.push((ordered[i], ordered[i + 1]));
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
            if board.adjacent(ordered[n - 2], ordered[n - 1]) {
                pairs.push((ordered[n - 2], ordered[n - 1]));
            } else {
                perma.push(ordered[n - 2]);
                perma.push(ordered[n - 1]);
            }
        }
        _ => {}
    }

    // Narrow separation guard (§4.6 step 4, table-preserving): demote a pair only
    // if keeping it would connect live cells into a component of >= threshold.
    let mut accepted: BTreeSet<usize> = BTreeSet::new();
    for (a, b) in &pairs {
        let mut tentative = accepted.clone();
        tentative.insert(*a);
        tentative.insert(*b);
        if max_component_size(board, &tentative) >= COLLAPSE_THRESHOLD {
            perma.push(*a);
            perma.push(*b);
        } else {
            accepted = tentative;
        }
    }

    let perma_set: BTreeSet<usize> = perma.into_iter().collect();
    assemble(board, cells, &perma_set)
}
