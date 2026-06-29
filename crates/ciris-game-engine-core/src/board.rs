//! Board state: cells, stewards, coordinate math, and mesh detection.
//!
//! Determinism note: mesh detection uses [`BTreeSet`] (sorted iteration), never
//! a hash set, so connected-component results are byte-identical across targets.

use alloc::collections::BTreeSet;
use alloc::vec::Vec;
use arrayvec::ArrayVec;
use serde::{Deserialize, Serialize};

use crate::lattice::{morton_code, MAX_NEIGHBORS, NEIGHBOR_OFFSETS};

/// One of the four stewards. Slot order is fixed (DESIGN_BRIEF §6.3); pigment
/// names and hex are locked (CLAUDE.md).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Steward {
    /// Slot 0 — Anthropic Clay `#D97757`.
    Sienna,
    /// Slot 1 — `#6A9BCC`.
    Lapis,
    /// Slot 2 — `#788C5D`.
    Verdigris,
    /// Slot 3 — `#E8E6DC` (rendered with a 2px Ink ring).
    Kaolin,
}

impl Steward {
    /// All stewards in slot order.
    pub const ALL: [Steward; 4] = [
        Steward::Sienna,
        Steward::Lapis,
        Steward::Verdigris,
        Steward::Kaolin,
    ];

    /// Slot index `0..=3`.
    pub fn slot(self) -> u8 {
        match self {
            Steward::Sienna => 0,
            Steward::Lapis => 1,
            Steward::Verdigris => 2,
            Steward::Kaolin => 3,
        }
    }

    /// Steward for a slot index (wraps mod 4).
    pub fn from_slot(slot: u8) -> Steward {
        Steward::ALL[(slot & 0b11) as usize]
    }

    /// Locked pigment hex.
    pub fn pigment(self) -> &'static str {
        match self {
            Steward::Sienna => "#D97757",
            Steward::Lapis => "#6A9BCC",
            Steward::Verdigris => "#788C5D",
            Steward::Kaolin => "#E8E6DC",
        }
    }
}

/// The exhaustive set of cell states (DESIGN_BRIEF §4.10). `Empty` is distinct
/// from a steward's size-1 mesh; it is never a zero-size mesh.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum CellState {
    /// Ghost lattice — a legal placement target.
    Empty,
    /// A live cell owned by a steward.
    Live(Steward),
    /// Dead for exactly one turn after collapse, before dispersal resolves.
    TempDead(Steward),
    /// Permanently neutral substrate. Never reclaimable, never a legal target.
    PermaDead,
}

/// An integer lattice coordinate. `Ord` is lexicographic on `(i, j, k)` — the
/// tiebreak Algorithm A uses for "lex-smallest" partner selection.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Coord {
    pub i: u8,
    pub j: u8,
    pub k: u8,
}

impl Coord {
    pub fn new(i: u8, j: u8, k: u8) -> Self {
        Coord { i, j, k }
    }
}

/// The lattice and its per-cell state.
///
/// The cells are the **FCC sublattice** of the `n³` box: only integer points with
/// `i + j + k` even (DESIGN_BRIEF §1). That is the true rhombic-dodecahedral
/// honeycomb — one connected lattice whose 12 face-neighbours are a cell's nearest
/// cells. `coords` maps a linear index → its coordinate; `cubic_to_idx` is the
/// inverse lookup over the full `n³` box (`-1` where the point is not a cell).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Board {
    /// Bounding-box edge length. The board is the even-parity points of `n³`.
    pub n: u8,
    cells: Vec<CellState>,
    coords: Vec<Coord>,
    cubic_to_idx: Vec<i32>,
}

impl Board {
    /// An empty board: the even-parity (FCC) cells of the `n × n × n` box.
    pub fn new(n: u8) -> Self {
        let nn = n as usize;
        let mut coords = Vec::new();
        let mut cubic_to_idx = alloc::vec![-1i32; nn * nn * nn];
        // Fixed (k, j, i) order → deterministic indices.
        for k in 0..n {
            for j in 0..n {
                for i in 0..n {
                    if (i as u16 + j as u16 + k as u16).is_multiple_of(2) {
                        let cubic = i as usize + nn * (j as usize + nn * k as usize);
                        cubic_to_idx[cubic] = coords.len() as i32;
                        coords.push(Coord::new(i, j, k));
                    }
                }
            }
        }
        let count = coords.len();
        Board {
            n,
            cells: alloc::vec![CellState::Empty; count],
            coords,
            cubic_to_idx,
        }
    }

    /// Total cell count (the even-parity points of `n³`, not `n³`).
    pub fn len(&self) -> usize {
        self.cells.len()
    }

    /// Whether the board has zero cells.
    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    /// Linear index for a coordinate, or `None` if it is out of bounds or not an
    /// FCC cell (odd parity).
    pub fn index(&self, c: Coord) -> Option<usize> {
        let n = self.n;
        if c.i < n && c.j < n && c.k < n {
            let nn = n as usize;
            let cubic = c.i as usize + nn * (c.j as usize + nn * c.k as usize);
            match self.cubic_to_idx[cubic] {
                -1 => None,
                idx => Some(idx as usize),
            }
        } else {
            None
        }
    }

    /// Coordinate for a linear index. Panics if out of range.
    pub fn coord(&self, idx: usize) -> Coord {
        self.coords[idx]
    }

    pub fn get(&self, idx: usize) -> CellState {
        self.cells[idx]
    }

    pub fn set(&mut self, idx: usize, state: CellState) {
        self.cells[idx] = state;
    }

    /// In-bounds face-neighbors of `idx`, in [`NEIGHBOR_OFFSETS`] order.
    pub fn neighbors(&self, idx: usize) -> ArrayVec<usize, MAX_NEIGHBORS> {
        let c = self.coord(idx);
        let n = self.n as i16;
        let mut out = ArrayVec::new();
        for (di, dj, dk) in NEIGHBOR_OFFSETS {
            let ni = c.i as i16 + di as i16;
            let nj = c.j as i16 + dj as i16;
            let nk = c.k as i16 + dk as i16;
            if (0..n).contains(&ni) && (0..n).contains(&nj) && (0..n).contains(&nk) {
                let nc = Coord::new(ni as u8, nj as u8, nk as u8);
                // unwrap safe: bounds checked above
                out.push(self.index(nc).unwrap());
            }
        }
        out
    }

    /// True when `a` and `b` are face-adjacent cells.
    pub fn adjacent(&self, a: usize, b: usize) -> bool {
        self.neighbors(a).contains(&b)
    }

    /// Count of empty (placeable) cells.
    pub fn empty_count(&self) -> usize {
        self.cells
            .iter()
            .filter(|c| **c == CellState::Empty)
            .count()
    }

    /// The connected component of same-steward `Live` cells containing `idx`,
    /// sorted ascending. Empty if `idx` is not `Live`.
    pub fn mesh_containing(&self, idx: usize) -> Vec<usize> {
        let steward = match self.cells[idx] {
            CellState::Live(s) => s,
            _ => return Vec::new(),
        };
        let mut seen = BTreeSet::new();
        let mut stack = alloc::vec![idx];
        seen.insert(idx);
        while let Some(cur) = stack.pop() {
            for nb in self.neighbors(cur) {
                if !seen.contains(&nb) {
                    if let CellState::Live(s) = self.cells[nb] {
                        if s == steward {
                            seen.insert(nb);
                            stack.push(nb);
                        }
                    }
                }
            }
        }
        seen.into_iter().collect()
    }

    /// All meshes belonging to `steward`, each component sorted ascending, the
    /// outer list ordered by smallest cell index (stable mesh ids, §7.2).
    pub fn meshes_of(&self, steward: Steward) -> Vec<Vec<usize>> {
        let mut visited = BTreeSet::new();
        let mut out = Vec::new();
        for idx in 0..self.cells.len() {
            if visited.contains(&idx) {
                continue;
            }
            if self.cells[idx] == CellState::Live(steward) {
                let comp = self.mesh_containing(idx);
                for &c in &comp {
                    visited.insert(c);
                }
                out.push(comp);
            }
        }
        out
    }

    /// Morton-sorted copy of `cells` (helper for dispersal).
    pub(crate) fn morton_sorted(&self, cells: &[usize]) -> Vec<usize> {
        let mut v = cells.to_vec();
        v.sort_by_key(|&idx| morton_code(self.coord(idx)));
        v
    }
}
