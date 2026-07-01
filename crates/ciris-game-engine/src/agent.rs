//! Agent HTTP client (DESIGN_BRIEF §7.6, native-only).
//!
//! Turn flow when a slot is `PlayerKind::Agent`:
//!   1. Build `BoardView` + optional PNG grid image.
//!   2. POST `{ view, ascii, png_b64? }` JSON to `<endpoint>/move`.
//!   3. Expect `{ "move": {...}, "view_opts"?: {...} }` within 2 s.
//!   4. Timeout or network error → forfeit to first legal move.
//!   5. Store any `view_opts` in `SlotViewOpts` for the next turn.
//!
//! Blocking HTTP runs on a `std::thread`; result returned via `mpsc::channel`.
//! `PendingAgentTask` wraps the `Receiver` in a `Mutex` for Bevy's `Resource`
//! bound (`Send + Sync`).

#[cfg(feature = "agent")]
mod inner {
    use std::sync::{mpsc, Mutex};
    use std::time::{Duration, Instant};

    use bevy::prelude::*;
    use ciris_game_engine_core::{AgentViewOpts, BoardView, Coord, GameState, Move};
    use serde::{Deserialize, Serialize};

    use crate::render::BoardDirty;
    use crate::state::SlotViewOpts;
    use crate::BoardResource;

    // ── wire types ────────────────────────────────────────────────────────────

    #[derive(Serialize)]
    struct AgentPost {
        view: BoardView,
        ascii: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        png_b64: Option<String>,
    }

    #[derive(Deserialize)]
    struct AgentMoveResponse {
        #[serde(rename = "move")]
        mv: AgentMoveInner,
        #[serde(default)]
        view_opts: Option<AgentViewOpts>,
    }

    #[derive(Deserialize)]
    struct AgentMoveInner {
        coord: CoordJson,
        #[serde(default)]
        dispersal: Option<Vec<CoordJson>>,
    }

    #[derive(Deserialize)]
    struct CoordJson {
        i: u8,
        j: u8,
        k: u8,
    }

    // ── task resource ─────────────────────────────────────────────────────────

    /// Holds an in-flight agent HTTP task. Mutex wraps Receiver for Send+Sync.
    #[derive(Resource, Default)]
    pub struct PendingAgentTask {
        receiver: Option<Mutex<mpsc::Receiver<Option<AgentMoveResponse>>>>,
        deadline: Option<Instant>,
        slot: usize,
    }

    // ── dispatch ──────────────────────────────────────────────────────────────

    pub fn dispatch_agent_move(
        gs: &GameState,
        slot: usize,
        endpoint_url: &str,
        view_opts: &AgentViewOpts,
        pending: &mut PendingAgentTask,
    ) {
        if pending.receiver.is_some() {
            return; // still waiting on previous call
        }

        let view = BoardView::from_game_state(gs);
        let ascii = view.to_ascii();
        let png_b64 = if view_opts.include_png {
            Some(render_png_grid(&view, view_opts))
        } else {
            None
        };

        let post = AgentPost {
            view,
            ascii,
            png_b64,
        };
        let body = match serde_json::to_string(&post) {
            Ok(s) => s,
            Err(e) => {
                warn!("agent: failed to serialize BoardView: {e}");
                return;
            }
        };
        let url = format!("{}/move", endpoint_url.trim_end_matches('/'));

        let (tx, rx) = mpsc::channel::<Option<AgentMoveResponse>>();
        std::thread::spawn(move || {
            let result = ureq::post(&url)
                .timeout(Duration::from_millis(2000))
                .set("Content-Type", "application/json")
                .send_string(&body)
                .ok()
                .and_then(|resp| resp.into_json::<AgentMoveResponse>().ok());
            let _ = tx.send(result);
        });

        pending.receiver = Some(Mutex::new(rx));
        pending.deadline = Some(Instant::now() + Duration::from_millis(2200));
        pending.slot = slot;
    }

    // ── poll ──────────────────────────────────────────────────────────────────

    pub fn poll_agent_task(
        mut pending: ResMut<PendingAgentTask>,
        mut board: ResMut<BoardResource>,
        mut slot_opts: ResMut<SlotViewOpts>,
        mut dirty: ResMut<BoardDirty>,
    ) {
        if pending.receiver.is_none() {
            return;
        }

        let timed_out = pending
            .deadline
            .map(|d| Instant::now() > d)
            .unwrap_or(false);

        let response: Option<AgentMoveResponse> = if timed_out {
            warn!(
                "agent slot {} timed out — forfeiting to first legal move",
                pending.slot
            );
            None
        } else {
            let rx = pending.receiver.as_ref().unwrap().lock().unwrap();
            match rx.try_recv() {
                Ok(r) => r,
                Err(mpsc::TryRecvError::Empty) => return,
                Err(mpsc::TryRecvError::Disconnected) => {
                    warn!(
                        "agent slot {} channel disconnected — forfeiting",
                        pending.slot
                    );
                    None
                }
            }
        };

        let slot = pending.slot;
        let mv = response
            .as_ref()
            .map(|r| {
                let c = Coord::new(r.mv.coord.i, r.mv.coord.j, r.mv.coord.k);
                let dispersal = r.mv.dispersal.as_ref().map(|ds| {
                    ds.iter()
                        .map(|d| Coord::new(d.i, d.j, d.k))
                        .collect::<Vec<_>>()
                });
                Move {
                    coord: c,
                    dispersal,
                }
            })
            .unwrap_or_else(|| {
                let legal = board.0.current_legal_moves();
                if legal.is_empty() {
                    Move::place(Coord::new(0, 0, 0))
                } else {
                    Move::place(legal[0])
                }
            });

        if board.0.apply_move(mv).is_ok() {
            dirty.0 = true;
        }

        if let Some(resp) = response {
            if let Some(opts) = resp.view_opts {
                slot_opts.0[slot] = opts;
            }
        }

        pending.receiver = None;
        pending.deadline = None;
    }

    // ── PNG grid renderer ─────────────────────────────────────────────────────

    /// 5 j-layers side by side, 14×14 px per cell. Returns base64 PNG.
    fn render_png_grid(view: &BoardView, opts: &AgentViewOpts) -> String {
        const CELL: u32 = 14;
        const GAP: u32 = 3;

        let n = view.board_n as u32;
        let j_range: Vec<u32> = if let Some(lj) = opts.layer_j {
            vec![lj as u32]
        } else {
            (0..n).collect()
        };
        let layers = j_range.len() as u32;
        let w = layers * n * CELL + (layers - 1) * GAP;
        let h = n * CELL;

        let mut img = image::RgbImage::new(w, h);
        for px in img.pixels_mut() {
            *px = image::Rgb([10u8, 10, 12]);
        }

        // Build index → color lookup.
        let idx = |i: u32, j: u32, k: u32| (i + n * (j + n * k)) as usize;
        let total = (n * n * n) as usize;
        const EMPTY: [u8; 3] = [28, 28, 30];
        const PERMA: [u8; 3] = [30, 55, 30];
        const TEMP: [u8; 3] = [60, 22, 10];
        const STEWARD: [[u8; 3]; 4] = [
            [217, 119, 87],
            [106, 155, 204],
            [120, 140, 93],
            [232, 230, 220],
        ];

        let mut colors: Vec<[u8; 3]> = vec![EMPTY; total];
        for pd in &view.perma_dead {
            colors[idx(pd[0] as u32, pd[1] as u32, pd[2] as u32)] = PERMA;
        }
        for td in &view.temp_dead {
            colors[idx(td.i as u32, td.j as u32, td.k as u32)] = TEMP;
        }
        for cell in &view.cells {
            let base = STEWARD[cell.slot_id as usize];
            let in_atari = view
                .meshes
                .iter()
                .any(|m| m.id == cell.mesh_id && m.in_atari);
            let rgb = if in_atari {
                base.map(|c| (c as u16 * 130 / 100).min(255) as u8)
            } else {
                base
            };
            colors[idx(cell.i as u32, cell.j as u32, cell.k as u32)] = rgb;
        }

        for (col, &j) in j_range.iter().enumerate() {
            let x_off = col as u32 * (n * CELL + GAP);
            for k in 0..n {
                for i in 0..n {
                    let rgb = colors[idx(i, j, k)];
                    let px0 = x_off + i * CELL;
                    let py0 = k * CELL;
                    for dy in 0..CELL {
                        for dx in 0..CELL {
                            let border = dx == 0 || dy == 0 || dx == CELL - 1 || dy == CELL - 1;
                            let color = if border {
                                image::Rgb([rgb[0] / 3, rgb[1] / 3, rgb[2] / 3])
                            } else {
                                image::Rgb(rgb)
                            };
                            if px0 + dx < w && py0 + dy < h {
                                img.put_pixel(px0 + dx, py0 + dy, color);
                            }
                        }
                    }
                }
            }
        }

        let mut buf: Vec<u8> = Vec::new();
        if image::DynamicImage::ImageRgb8(img)
            .write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
            .is_err()
        {
            return String::new();
        }
        use base64::Engine as _;
        base64::engine::general_purpose::STANDARD.encode(&buf)
    }
} // mod inner

// ── stub for non-agent builds ─────────────────────────────────────────────────

#[cfg(not(feature = "agent"))]
mod stub {
    use bevy::prelude::*;
    use ciris_game_engine_core::{AgentViewOpts, GameState};

    use crate::render::BoardDirty;
    use crate::state::SlotViewOpts;
    use crate::BoardResource;

    #[derive(Resource, Default)]
    pub struct PendingAgentTask;

    pub fn dispatch_agent_move(
        _gs: &GameState,
        _slot: usize,
        _url: &str,
        _opts: &AgentViewOpts,
        _pending: &mut PendingAgentTask,
    ) {
        warn!("agent player type requires the `agent` feature (not in this build)");
    }

    pub fn poll_agent_task(
        _pending: ResMut<PendingAgentTask>,
        _board: ResMut<BoardResource>,
        _slot_opts: ResMut<SlotViewOpts>,
        _dirty: ResMut<BoardDirty>,
    ) {
    }
}

// ── re-exports ────────────────────────────────────────────────────────────────

#[cfg(feature = "agent")]
pub(crate) use inner::{dispatch_agent_move, poll_agent_task, PendingAgentTask};

#[cfg(not(feature = "agent"))]
pub(crate) use stub::{dispatch_agent_move, poll_agent_task, PendingAgentTask};
