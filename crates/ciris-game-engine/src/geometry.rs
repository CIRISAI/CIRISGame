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
/// Mirror of `assets/geometry/rhombic-dodeca-edges.ron` (the canonical full cage;
/// [`SPARSE_EDGES`] selects the subset actually drawn).
#[allow(dead_code)]
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

/// A **sparse** subset of [`EDGES`] — one diagonal strut per rhombic face (12 of
/// the 24 edges). The full cage reads as a busy wireframe; this sparser shell
/// just hints the cell boundary, separating the spheres and keeping sight-lines
/// open for the §4.8 fly-through (DESIGN_BRIEF §3.5).
const SPARSE_EDGES: [(usize, usize); 12] = [
    (8, 0),
    (8, 3),
    (9, 4),
    (9, 7),
    (10, 0),
    (10, 5),
    (11, 2),
    (11, 7),
    (12, 0),
    (12, 6),
    (13, 1),
    (13, 7),
];

/// Build the sparse rhombic-dodecahedron shell as tube geometry (DESIGN_BRIEF
/// §3.5): each [`SPARSE_EDGES`] strut becomes a thin prism so the plasma material
/// has surface to bloom on (a `LineList` renders as flat 1-px wire that can't read
/// as plasma). `NORMAL`/`UV_0` are filled for the material's vertex layout.
pub fn wireframe_mesh() -> Mesh {
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut uvs: Vec<[f32; 2]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    let radius = 0.02;
    let sides = 6;

    for &(a_idx, b_idx) in &SPARSE_EDGES {
        let a = Vec3::from(VERTICES[a_idx]);
        let b = Vec3::from(VERTICES[b_idx]);
        let dir = (b - a).normalize();

        let up_vec = if dir.y.abs() < 0.99 { Vec3::Y } else { Vec3::Z };
        let right = up_vec.cross(dir).normalize();
        let up = dir.cross(right).normalize();

        let base_idx = positions.len() as u32;

        for i in 0..sides {
            let angle = (i as f32) * std::f32::consts::TAU / (sides as f32);
            let (sin, cos) = angle.sin_cos();
            let normal = right * cos + up * sin;

            // vertex at A
            positions.push((a + normal * radius).into());
            normals.push(normal.into());
            uvs.push([0.0, 0.0]);

            // vertex at B
            positions.push((b + normal * radius).into());
            normals.push(normal.into());
            uvs.push([0.0, 1.0]);
        }

        for i in 0..sides {
            let next = (i + 1) % sides;
            let a0 = base_idx + i * 2;
            let b0 = base_idx + i * 2 + 1;
            let a1 = base_idx + next * 2;
            let b1 = base_idx + next * 2 + 1;

            indices.push(a0);
            indices.push(a1);
            indices.push(b0);

            indices.push(a1);
            indices.push(b1);
            indices.push(b0);
        }
    }

    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_indices(bevy::render::mesh::Indices::U32(indices))
}

/// Build a solid rhombic-dodecahedron mesh (for use by signets).
pub fn rhombic_dodecahedron() -> Mesh {
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut uvs: Vec<[f32; 2]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    // The 12 rhombic faces, defined by indices into VERTICES.
    // Each face consists of 4 vertices in counter-clockwise order.
    let faces: [[usize; 4]; 12] = [
        [10, 0, 8, 1],
        [5, 10, 1, 13],
        [4, 10, 5, 9],
        [10, 4, 12, 0],
        [3, 8, 2, 11],
        [13, 3, 11, 7],
        [9, 7, 11, 6],
        [2, 12, 6, 11],
        [8, 0, 12, 2],
        [13, 1, 8, 3],
        [6, 12, 4, 9],
        [9, 5, 13, 7],
    ];

    for face in &faces {
        let a = Vec3::from(VERTICES[face[0]]);
        let b = Vec3::from(VERTICES[face[1]]);
        let c = Vec3::from(VERTICES[face[2]]);
        let d = Vec3::from(VERTICES[face[3]]);

        let normal = (b - a).cross(c - a).normalize();

        let base_idx = positions.len() as u32;

        positions.push(a.into());
        positions.push(b.into());
        positions.push(c.into());
        positions.push(d.into());

        normals.push(normal.into());
        normals.push(normal.into());
        normals.push(normal.into());
        normals.push(normal.into());

        uvs.push([0.0, 0.0]);
        uvs.push([1.0, 0.0]);
        uvs.push([1.0, 1.0]);
        uvs.push([0.0, 1.0]);

        indices.push(base_idx);
        indices.push(base_idx + 1);
        indices.push(base_idx + 2);

        indices.push(base_idx);
        indices.push(base_idx + 2);
        indices.push(base_idx + 3);
    }

    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_indices(bevy::render::mesh::Indices::U32(indices))
}
