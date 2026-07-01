//! Simple-cubic lattice geometry and Morton ordering.
//!
//! Cells sit at every integer point `(i, j, k) ∈ {0..N-1}³`. Two cells are
//! face-adjacent when their displacement has exactly one `±1` component and two
//! `0` components — the six axis-aligned face-neighbors. Interior cells have
//! exactly 6 neighbors; edge cells have 4; face cells have 5; corner cells have 3.

use crate::board::Coord;

/// Maximum face-neighbors for any cell (interior cells have exactly this many).
pub const MAX_NEIGHBORS: usize = 6;

/// The six face-neighbor displacements: exactly one of `(di, dj, dk)` is `±1`,
/// the other two are `0`. Listed in a fixed, deterministic order.
pub const NEIGHBOR_OFFSETS: [(i8, i8, i8); MAX_NEIGHBORS] = [
    (1, 0, 0),
    (-1, 0, 0),
    (0, 1, 0),
    (0, -1, 0),
    (0, 0, 1),
    (0, 0, -1),
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

/// True when `a` and `b` are face-adjacent on the simple-cubic lattice:
/// Manhattan distance exactly 1 (differ by ±1 on exactly one axis).
pub fn is_face_adjacent(a: Coord, b: Coord) -> bool {
    let di = (a.i as i16 - b.i as i16).abs();
    let dj = (a.j as i16 - b.j as i16).abs();
    let dk = (a.k as i16 - b.k as i16).abs();
    di + dj + dk == 1
}
