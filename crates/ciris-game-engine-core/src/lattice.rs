//! Rhombic-dodecahedral (FCC) lattice geometry and Morton ordering.
//!
//! Cells sit at integer positions `(i, j, k) ∈ {0..N-1}³`. Two cells are
//! face-adjacent when their displacement has exactly two `±1` components and one
//! `0` component — the twelve FCC face-neighbors (DESIGN_BRIEF §3.1).

use crate::board::Coord;

/// Maximum face-neighbors for any cell (interior cells have exactly this many).
pub const MAX_NEIGHBORS: usize = 12;

/// The twelve face-neighbor displacements: exactly two of `(di, dj, dk)` are
/// `±1`, the third is `0`. Listed in a fixed, deterministic order.
pub const NEIGHBOR_OFFSETS: [(i8, i8, i8); MAX_NEIGHBORS] = [
    // k held at 0, vary i and j
    (1, 1, 0),
    (1, -1, 0),
    (-1, 1, 0),
    (-1, -1, 0),
    // j held at 0, vary i and k
    (1, 0, 1),
    (1, 0, -1),
    (-1, 0, 1),
    (-1, 0, -1),
    // i held at 0, vary j and k
    (0, 1, 1),
    (0, 1, -1),
    (0, -1, 1),
    (0, -1, -1),
];

/// Spread the low 10 bits of `v` so each occupies every third bit position
/// (`abc` → `a..b..c`). Supports board sizes up to N = 1024.
fn spread3(v: u32) -> u32 {
    let mut x = v & 0x3ff;
    x = (x | (x << 16)) & 0x030000ff;
    x = (x | (x << 8)) & 0x0300f00f;
    x = (x | (x << 4)) & 0x030c30c3;
    x = (x | (x << 2)) & 0x09249249;
    x
}

/// 3D Morton (Z-order) code for a cell. Canonical, monotone, and
/// platform-independent — the ordering Algorithm A walks (DESIGN_BRIEF §4.6).
pub fn morton_code(c: Coord) -> u32 {
    spread3(c.i as u32) | (spread3(c.j as u32) << 1) | (spread3(c.k as u32) << 2)
}

/// True when `a` and `b` are face-adjacent: displacement is a permutation of
/// `(±1, ±1, 0)`.
pub fn is_face_adjacent(a: Coord, b: Coord) -> bool {
    let di = (a.i as i16 - b.i as i16).abs();
    let dj = (a.j as i16 - b.j as i16).abs();
    let dk = (a.k as i16 - b.k as i16).abs();
    let sum = di + dj + dk;
    let zeros = (di == 0) as i16 + (dj == 0) as i16 + (dk == 0) as i16;
    // exactly two unit steps and one zero
    sum == 2 && di <= 1 && dj <= 1 && dk <= 1 && zeros == 1
}
