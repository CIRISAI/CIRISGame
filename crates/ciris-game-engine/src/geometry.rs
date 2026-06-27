//! Rhombic-dodecahedron ghost-cell wireframe (DESIGN_BRIEF §3.5).
//!
//! The canonical edge list lives in `assets/geometry/rhombic-dodeca-edges.ron`
//! (14 vertices, 24 edges, inradius √2/2 ≈ 0.7071). `bevy_polyline` — which would
//! draw that `.ron` with its built-in distance fade — has no Bevy-0.19 release,
//! so we fall back to a built-in `LineList` `Mesh` and mirror the same vertex /
//! edge data here as `const`s (single source of truth: keep the two in sync).
//!
//! The vertices are unit-cell coordinates: the inradius 0.7071 is exactly half
//! the FCC face-neighbour pitch (√2 between adjacent cell centres, §3.1), so the
//! wireframe drawn at native scale tiles the empty lattice cell-for-cell with no
//! per-cell scaling.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Mesh, PrimitiveTopology};
use bevy::prelude::*;

/// The 14 rhombic-dodecahedron vertices (8 cube corners ±0.5, 6 axis apices ±1.0).
/// Mirror of `assets/geometry/rhombic-dodeca-edges.ron`.
const VERTICES: [[f32; 3]; 14] = [
    [0.5, 0.5, 0.5],    // 0  cube
    [0.5, 0.5, -0.5],   // 1
    [0.5, -0.5, 0.5],   // 2
    [0.5, -0.5, -0.5],  // 3
    [-0.5, 0.5, 0.5],   // 4
    [-0.5, 0.5, -0.5],  // 5
    [-0.5, -0.5, 0.5],  // 6
    [-0.5, -0.5, -0.5], // 7
    [1.0, 0.0, 0.0],    // 8  +x apex
    [-1.0, 0.0, 0.0],   // 9  -x apex
    [0.0, 1.0, 0.0],    // 10 +y apex
    [0.0, -1.0, 0.0],   // 11 -y apex
    [0.0, 0.0, 1.0],    // 12 +z apex
    [0.0, 0.0, -1.0],   // 13 -z apex
];

/// The 24 edges — each apex joins the four cube corners sharing its axis sign.
/// Mirror of `assets/geometry/rhombic-dodeca-edges.ron`.
const EDGES: [(usize, usize); 24] = [
    (8, 0),
    (8, 1),
    (8, 2),
    (8, 3),
    (9, 4),
    (9, 5),
    (9, 6),
    (9, 7),
    (10, 0),
    (10, 1),
    (10, 4),
    (10, 5),
    (11, 2),
    (11, 3),
    (11, 6),
    (11, 7),
    (12, 0),
    (12, 2),
    (12, 4),
    (12, 6),
    (13, 1),
    (13, 3),
    (13, 5),
    (13, 7),
];

/// Build the rhombic-dodecahedron wireframe as a `LineList` `Mesh` (DESIGN_BRIEF
/// §3.5). Positions are emitted as one segment per edge; `NORMAL` and `UV_0` are
/// filled so the `StandardMaterial` vertex layout is satisfied on every backend
/// (the material is unlit, so their values are immaterial).
pub fn wireframe_mesh() -> Mesh {
    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(EDGES.len() * 2);
    for &(a, b) in &EDGES {
        positions.push(VERTICES[a]);
        positions.push(VERTICES[b]);
    }
    let n = positions.len();
    Mesh::new(
        PrimitiveTopology::LineList,
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 1.0, 0.0]; n])
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0, 0.0]; n])
}
