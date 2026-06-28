//! The no-crossing rule: different-colour bonds may not cross on a shared face.
//!
//! A **bond** (the visual "tube") joins two face-adjacent same-colour live cells.
//! On the FCC lattice every face-neighbour displacement is a `(±1, ±1, 0)`-type
//! offset ([`crate::lattice::NEIGHBOR_OFFSETS`]), so every bond is the
//! **face-diagonal of a unit square** lying in an axis plane. The two diagonals
//! of a single face intersect at the face centre, so two bonds **cross** iff they
//! are the two diagonals of the same unit square.
//!
//! > **Rule (DESIGN_BRIEF §4.11).** A candidate placement of colour `X` at cell
//! > `C` is **illegal** if it would create any same-colour bond `C–N` (`N` a live
//! > colour-`X` neighbour) whose face's opposite diagonal `R–S` is *already* a
//! > live bond of a **different** colour `Y ≠ X` (both `R` and `S` live and
//! > colour `Y`). At most one diagonal per face may be a live cross-bond —
//! > first-come, first-served.
//!
//! The rule is **colour-dependent**: a cell forbidden to Sienna can be perfectly
//! legal for Lapis, because the bond it would create is a different diagonal.
//!
//! This predicate is the single source of truth for the rule. Both the shipped
//! engine ([`crate::engine::GameState::apply_move`]) and the analysis harness
//! (`examples/no_crossing_analysis.rs`) call it, so the two can never drift.

use crate::board::{Board, CellState, Coord, Steward};

/// The other two corners `(R, S)` of the unit square whose first diagonal is the
/// bond `c`–`nb`. `c` and `nb` are face-adjacent (differ in exactly two axes by
/// 1), so `R` and `S` are the cells reached by taking exactly one of those two
/// steps.
pub fn opposite_diagonal(c: Coord, nb: Coord) -> (Coord, Coord) {
    let cc = [c.i, c.j, c.k];
    let nn = [nb.i, nb.j, nb.k];
    let mut diff = [0usize; 2];
    let mut count = 0;
    for (ax, (a, b)) in cc.iter().zip(nn.iter()).enumerate() {
        if a != b {
            if count < 2 {
                diff[count] = ax;
            }
            count += 1;
        }
    }
    let mut r = cc;
    r[diff[0]] = nn[diff[0]];
    let mut s = cc;
    s[diff[1]] = nn[diff[1]];
    (Coord::new(r[0], r[1], r[2]), Coord::new(s[0], s[1], s[2]))
}

/// Would placing `color` at `cell` create any same-colour bond whose face's
/// opposite diagonal is already a live bond of a DIFFERENT colour? (At most one
/// diagonal per face may be a live cross-bond; first-come-first-served.)
pub fn is_crossing_illegal(board: &Board, cell: Coord, color: Steward) -> bool {
    let cidx = match board.index(cell) {
        Some(i) => i,
        None => return false,
    };
    for nb in board.neighbors(cidx) {
        // A bond forms only with a same-colour live neighbour.
        if board.get(nb) != CellState::Live(color) {
            continue;
        }
        let (r, s) = opposite_diagonal(cell, board.coord(nb));
        // R and S are corners of the same in-bounds unit square, so always valid.
        if let (Some(ri), Some(si)) = (board.index(r), board.index(s)) {
            if let (CellState::Live(yr), CellState::Live(ys)) = (board.get(ri), board.get(si)) {
                if yr == ys && yr != color {
                    return true;
                }
            }
        }
    }
    false
}
