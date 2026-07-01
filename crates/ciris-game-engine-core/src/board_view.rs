//! BoardView — the canonical game-state snapshot delivered to every player type
//! (DESIGN_BRIEF §7.2). JSON via serde; ASCII via hand-coded slice printer.
//!
//! Multi-modal agents receive this in the POST body alongside an optional
//! base64-encoded PNG. The agent can request richer views for subsequent turns
//! via `view_opts` in their response.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

use crate::board::CellState;
use crate::engine::GameState;
use crate::temperature::{t_vis, temperature, temperature_word};
use crate::{ATARI_SIZE, COLLAPSE_THRESHOLD, STEWARD_COUNT};

// ── constants ────────────────────────────────────────────────────────────────

const DEFAULT_NAMES: [&str; 4] = ["Red", "Blue", "Green", "White"];
const PIGMENTS: [&str; 4] = ["#D97757", "#6A9BCC", "#788C5D", "#E8E6DC"];

// ── sub-types ────────────────────────────────────────────────────────────────

/// A live cell entry. Custom nicknames are never included (CLAUDE.md).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CellEntry {
    pub i: u8,
    pub j: u8,
    pub k: u8,
    pub slot_id: u8,
    /// Locked default steward name (never a custom nickname).
    pub default_name: String,
    pub pigment: String,
    /// Stable mesh id: smallest linear index in the mesh.
    pub mesh_id: usize,
}

/// A smouldering TempDead cell (one-turn crater).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TempDeadEntry {
    pub i: u8,
    pub j: u8,
    pub k: u8,
    /// Which steward owns the smouldering crater.
    pub slot_id: u8,
}

/// Per-mesh temperature and size summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshInfo {
    /// Stable id: smallest cell linear index in the mesh.
    pub id: usize,
    pub slot_id: u8,
    pub size: usize,
    pub temperature_float: f32,
    pub temperature_word: String,
    /// True if this mesh is exactly one placement from collapse (§4.9 atari).
    pub in_atari: bool,
}

/// The camera angle an agent may request for the next PNG/animation render.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CameraAngle {
    /// Default 3D isometric forward view.
    #[default]
    Isometric,
    /// Top-down orthographic (j-axis looking down).
    Top,
    /// Front face orthographic (k-axis looking in).
    Front,
    /// Side face orthographic (i-axis looking in).
    Side,
    /// Free rotation; `yaw_deg` and `pitch_deg` specify the angle.
    Custom,
}

/// Options the agent may embed in its response to shape the *next* POST body.
/// The game stores these per-slot and applies them on the following turn.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AgentViewOpts {
    /// Send a base64-encoded PNG with the next POST (default false).
    #[serde(default)]
    pub include_png: bool,
    /// Send a base64-encoded 6-frame animation with the next POST (default false).
    #[serde(default)]
    pub include_anim: bool,
    /// PNG/animation pixel size (default 128; options 96/128/192/256).
    #[serde(default = "default_png_size")]
    pub png_size: u32,
    /// Camera angle for the PNG/animation render.
    #[serde(default)]
    pub camera: CameraAngle,
    /// If set, only render the j-layer at this index rather than the full board.
    pub layer_j: Option<u8>,
}

fn default_png_size() -> u32 {
    128
}

// ── BoardView ────────────────────────────────────────────────────────────────

/// The canonical game-state snapshot (DESIGN_BRIEF §7.2).
///
/// Sent as JSON in every `POST /move` body. Never contains custom steward names
/// (CLAUDE.md strict-local invariant).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoardView {
    pub turn: u32,
    pub current_slot: u8,
    pub board_n: u8,
    pub is_rebuild_turn: bool,
    /// Live cells with steward attribution.
    pub cells: Vec<CellEntry>,
    /// Permanently-dead substrate cells (coord only).
    pub perma_dead: Vec<[u8; 3]>,
    /// Smouldering TempDead cells (one-turn crater).
    pub temp_dead: Vec<TempDeadEntry>,
    /// Legal placement coordinates for the current steward.
    pub legal_moves: Vec<[u8; 3]>,
    /// Crater footprint the current steward must lay out (rebuild turn only).
    pub pending_footprint: Vec<[u8; 3]>,
    pub scores: [u32; STEWARD_COUNT],
    pub eliminated: [bool; STEWARD_COUNT],
    /// Per-mesh temperature + size summary, all stewards.
    pub meshes: Vec<MeshInfo>,
    /// Last move applied.
    pub last_move: Option<LastMove>,
    /// Counts for context.
    pub collapse_threshold: usize,
    pub atari_size: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LastMove {
    pub slot: u8,
    pub i: u8,
    pub j: u8,
    pub k: u8,
}

// ── construction ─────────────────────────────────────────────────────────────

impl BoardView {
    /// Build a `BoardView` from the current `GameState`.
    pub fn from_game_state(gs: &GameState) -> Self {
        let board = &gs.board;
        let n = board.n;

        // ── live + temp-dead cells ──────────────────────────────────────────
        let mut cells: Vec<CellEntry> = Vec::new();
        let mut perma_dead: Vec<[u8; 3]> = Vec::new();
        let mut temp_dead: Vec<TempDeadEntry> = Vec::new();

        for idx in 0..board.len() {
            let c = board.coord(idx);
            match board.get(idx) {
                CellState::Live(s) => {
                    let slot_id = s.slot();
                    let mesh = board.mesh_containing(idx);
                    let mesh_id = *mesh.iter().min().unwrap_or(&idx);
                    cells.push(CellEntry {
                        i: c.i,
                        j: c.j,
                        k: c.k,
                        slot_id,
                        default_name: DEFAULT_NAMES[slot_id as usize].to_string(),
                        pigment: PIGMENTS[slot_id as usize].to_string(),
                        mesh_id,
                    });
                }
                CellState::PermaDead => {
                    perma_dead.push([c.i, c.j, c.k]);
                }
                CellState::TempDead(s) => {
                    temp_dead.push(TempDeadEntry {
                        i: c.i,
                        j: c.j,
                        k: c.k,
                        slot_id: s.slot(),
                    });
                }
                CellState::Empty => {}
            }
        }

        // ── legal moves ─────────────────────────────────────────────────────
        let legal_moves: Vec<[u8; 3]> = gs
            .current_legal_moves()
            .into_iter()
            .map(|c| [c.i, c.j, c.k])
            .collect();

        // ── pending footprint ───────────────────────────────────────────────
        let pending_footprint: Vec<[u8; 3]> = gs
            .pending_footprint()
            .unwrap_or_default()
            .into_iter()
            .map(|c| [c.i, c.j, c.k])
            .collect();

        // ── mesh infos with temperature ─────────────────────────────────────
        let meshes = build_mesh_infos(gs);

        // ── last move ───────────────────────────────────────────────────────
        let last_move = gs.history.last().map(|rec| LastMove {
            slot: rec.slot,
            i: rec.coord.i,
            j: rec.coord.j,
            k: rec.coord.k,
        });

        BoardView {
            turn: gs.turn,
            current_slot: gs.current,
            board_n: n,
            is_rebuild_turn: gs.is_rebuild_turn(),
            cells,
            perma_dead,
            temp_dead,
            legal_moves,
            pending_footprint,
            scores: gs.scores,
            eliminated: gs.eliminated,
            meshes,
            last_move,
            collapse_threshold: COLLAPSE_THRESHOLD,
            atari_size: ATARI_SIZE,
        }
    }

    /// Serialize to canonical JSON (requires `serde_json`).
    #[cfg(feature = "json")]
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".to_string())
    }

    /// Five z-slice ASCII dump (~600 chars at N=5, DESIGN_BRIEF §7.3).
    ///
    /// Each layer is a `j`-slice (horizontal plane). Symbols:
    /// `.` empty, `S/L/V/K` live, `*` atari, `t` temp-dead, `X` perma-dead.
    pub fn to_ascii(&self) -> String {
        let n = self.board_n as usize;
        // Build fast lookup: (i,j,k) → char.
        // Use a flat Vec<char> indexed by i + n*(j + n*k).
        let mut grid: Vec<u8> = alloc::vec![b'.'; n * n * n];
        for pd in &self.perma_dead {
            let (i, j, k) = (pd[0] as usize, pd[1] as usize, pd[2] as usize);
            grid[i + n * (j + n * k)] = b'X';
        }
        for td in &self.temp_dead {
            let (i, j, k) = (td.i as usize, td.j as usize, td.k as usize);
            grid[i + n * (j + n * k)] = b't';
        }
        for cell in &self.cells {
            let (i, j, k) = (cell.i as usize, cell.j as usize, cell.k as usize);
            let ch = match cell.slot_id {
                0 => b'S',
                1 => b'L',
                2 => b'V',
                3 => b'K',
                _ => b'?',
            };
            grid[i + n * (j + n * k)] = ch;
        }
        // Mark atari meshes with '*'.
        for m in &self.meshes {
            if m.in_atari {
                for cell in &self.cells {
                    if cell.mesh_id == m.id {
                        let (i, j, k) = (cell.i as usize, cell.j as usize, cell.k as usize);
                        grid[i + n * (j + n * k)] = b'*';
                    }
                }
            }
        }

        let mut out = String::new();
        let steward_names = ["Red", "Blue", "Green", "White"];
        let cur = self.current_slot as usize;
        out.push_str(&alloc::format!(
            "Turn {} — {}'s move  scores: S={} L={} V={} K={}\n",
            self.turn,
            steward_names[cur],
            self.scores[0],
            self.scores[1],
            self.scores[2],
            self.scores[3],
        ));
        out.push_str("(. empty  S/L/V/K live  * atari  t temp-dead  X perma-dead)\n\n");

        for j in (0..n).rev() {
            out.push_str(&alloc::format!("  layer j={}\n", j));
            out.push_str("    ");
            for _ in 0..n {
                out.push_str(" k→");
            }
            out.push('\n');
            for k in 0..n {
                out.push_str("  i→  ");
                for i in 0..n {
                    out.push(grid[i + n * (j + n * k)] as char);
                    out.push(' ');
                }
                out.push('\n');
            }
            out.push('\n');
        }

        // Mesh table.
        if !self.meshes.is_empty() {
            out.push_str("  Meshes:\n");
            for m in &self.meshes {
                let name = steward_names[m.slot_id as usize];
                let atari_flag = if m.in_atari { " !ATARI" } else { "" };
                out.push_str(&alloc::format!(
                    "    {} mesh#{} size={} temp={} ({}){}\n",
                    name, m.id, m.size, m.temperature_float, m.temperature_word, atari_flag,
                ));
            }
        }

        out
    }
}

// ── mesh info builder ─────────────────────────────────────────────────────────

fn build_mesh_infos(gs: &GameState) -> Vec<MeshInfo> {
    let mut infos: Vec<MeshInfo> = Vec::new();
    let board = &gs.board;
    let mut visited: Vec<bool> = alloc::vec![false; board.len()];

    for idx in 0..board.len() {
        if visited[idx] {
            continue;
        }
        if let CellState::Live(steward) = board.get(idx) {
            let mesh = board.mesh_containing(idx);
            for &ci in &mesh {
                visited[ci] = true;
            }
            let mesh_id = *mesh.iter().min().unwrap_or(&idx);
            let m_size = mesh.len();

            // Enemy mesh sizes (all live meshes not owned by this steward).
            let enemy_sizes: Vec<usize> = {
                let mut seen: Vec<usize> = Vec::new();
                let mut ev: Vec<bool> = alloc::vec![false; board.len()];
                for ei in 0..board.len() {
                    if ev[ei] {
                        continue;
                    }
                    if let CellState::Live(es) = board.get(ei) {
                        if es != steward {
                            let em = board.mesh_containing(ei);
                            for &ec in &em {
                                ev[ec] = true;
                            }
                            seen.push(em.len());
                        }
                    }
                }
                seen
            };

            let t = temperature(m_size, &enemy_sizes);
            let tv = t_vis(t) as f32;
            let word = temperature_word(tv as f64);

            infos.push(MeshInfo {
                id: mesh_id,
                slot_id: steward.slot(),
                size: m_size,
                temperature_float: tv,
                temperature_word: word.to_string(),
                in_atari: m_size == ATARI_SIZE,
            });
        }
    }
    infos
}
