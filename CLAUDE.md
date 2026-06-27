# CLAUDE.md — CIRISGame session bootstrap

You're being dropped into the CIRISGame repo. This file is the canonical orientation.

## What CIRISGame is

A four-steward turn-based 3D game on a rhombic-dodecahedral lattice (FCC honeycomb, 12 face-neighbors per cell, no center). One rule: **don't let your mesh hit seven cells**. A mesh that hits seven undergoes a destructive transition — black mist for 1 turn, then Morton-greedy dispersal into live pairs of the steward's color + perma-dead spacer cells (green mist forever). Score = total perma-dead created; lowest wins. All-zero across all four stewards triggers WILD — the M-1 cooperative ending.

It's the in-game visceral conveyance of CIRIS's [coherence-collapse-analysis](https://github.com/CIRISAI/coherence-ratchet) — collapse is generative, not annihilating.

## The stack

- **Engine**: Bevy 0.19, pure Rust, two WASM artifacts (`app.webgpu.wasm`, `app.webgl2.wasm`). Bevy issue #13168 blocks a single binary from runtime backend selection.
- **Camera**: `bevy_panorbit_camera = "0.35"` + custom systems for layer traversal + minimap-in-sphere arcball + steward seats.
- **Default board**: 5×5×5 = 125 cells.
- **Four stewards**: Sienna `#D97757` (Anthropic Clay), Lapis `#6A9BCC`, Verdigris `#788C5D`, Kaolin `#E8E6DC` (with mandatory 2 px Ink ring — Kaolin is near-Bone and needs the rim to read).
- **Native**: Tauri 2 wraps the same Bevy app for macOS / Linux / Windows.
- **Headless**: feature-gated, links no rendering code; used for AI tournaments and CI.
- **AI players**: uniform 2-second compute budget across Easy / Medium / Hard / Brutal; identical thinking pulse.
- **AI-API**: identical `BoardView` for every player type — JSON, ASCII, PNG (default 128×128 at N=5), 6 fps × 10-frame animation.
- **Dispersal**: Algorithm A (Morton-greedy) — `k = N÷3` live pairs + `k` perma-dead spacers + remainder per `r = N mod 3`.
- **Score**: `permadead_count` — magnitude matters (an 8-cell explosion costs 2, a 13-cell costs 5).
- **Atari at |M| = 6**: synchronized particle Kuramoto breath at 0.6 Hz with foreshadowing Verdigris ring.

## Documents — read in this order

1. **[`MISSION.md`](./MISSION.md)** — what + why; M-1 grounding; CEG envelope shape; cohabitation trajectory. Read this FIRST.
2. **[`docs/DESIGN_BRIEF.md`](./docs/DESIGN_BRIEF.md)** — the full spec. Thirteen sections, stable numbering. Skim §0 capsule + §1 engine + §13 architecture position before diving deeper.
3. **[`docs/BACKLOG.md`](./docs/BACKLOG.md)** — shipping order with dependencies. No time estimates.
4. **[`docs/ITERATION_KNOBS.json`](./docs/ITERATION_KNOBS.json)** — every numeric default has an entry with `applyMode ∈ {live, next-move, next-game}`. Hot-reload via §12 (Persistence).

## Build

```bash
rustup target add wasm32-unknown-unknown

# WebGPU artifact (primary)
cargo build --release --target wasm32-unknown-unknown \
  --features 'webgpu,tonemapping_luts,bloom,pbr_transmission_textures'
wasm-bindgen --target web --out-dir dist/webgpu/ \
  target/wasm32-unknown-unknown/release/ciris-game-engine.wasm
wasm-opt -Oz -o dist/webgpu/app.webgpu.wasm dist/webgpu/ciris-game-engine_bg.wasm

# WebGL2 fallback (same flow, swap features)
cargo build --release --target wasm32-unknown-unknown \
  --features 'webgl2,tonemapping_luts,bloom,pbr_transmission_textures'
# ... same wasm-bindgen + wasm-opt flow → dist/webgl2/app.webgl2.wasm

# Native (macOS / Linux / Windows via Tauri 2)
cargo build --release

# Headless (no rendering)
cargo build --release --no-default-features --features 'headless'

# Server-side replay (Cloudflare Worker)
cargo build --release -p ciris-game-engine-core --target wasm32-wasip1 \
  --no-default-features --features 'wasi-replay'
```

CI matrix in `.github/workflows/build.yml` covers linux-x86_64 / macOS arm64 / macOS x64 / windows-x64 / wasm32-unknown-unknown / wasm32-wasip1. Bundle target: ~5–9 MB gzipped per WASM artifact (~4–6 MB brotli); native ~14–22 MB raw.

## What's locked — do not change without explicit user direction

- The four stewards' colors and pigment names
- The rule of seven (configurable in native, fixed 7 in browser)
- M-1 framing as the cooperative-all-survive ending
- The CEG 1+4 primitive shape: `scores` + `delegates_to` / `supersedes` / `withdraws` / `recants`
- Bevy 0.19 + `bevy_panorbit_camera = "0.35"` stack
- Score = total perma-dead created (NOT explosion count)
- Daily seed's 3–15 random perma-dead-at-start mechanic (seed-deterministic via ChaCha8)
- Dispersal Algorithm A (Morton-greedy)
- 2-second uniform AI compute budget across all difficulties
- `prefers-reduced-motion` auto-mute for audio
- Custom human nicknames stay strictly local (never in filenames, leaderboards, BoardView JSON, OG cards, or spectator streams)
- AGPL-3.0-or-later license

## Refusals — never propose adding these

- Twitter / Bluesky / X share-intent buttons
- "Share to X" / "Epic!" / hype copy anywhere
- Auto-download on game end
- Hosted gallery `cirisgame.ai/replay/{hash}`
- Streak counter on daily seed
- Inviter parameter / K-factor
- Chat in spectator mode
- Viewer count badge
- Custom steward colors
- "You beat X%" / percentile ranking
- Account system (anonymous-hash aggregation only)
- Real-time multiplayer (out of scope for the lifetime of the game)
- Federation envelope attestation in the daily-seed POST (the Worker is a plain aggregator)
- PQC / ML-DSA-65 hybrid signing (not day-1 for a game)
- HUMANITY_ACCORD glyph in BoardView (federation surface, not game surface)
- Watermark on shareable replay

## Common starting points

- **"Run the game"** → §1 build + §6.5 browser embed / §6.6 native CLI
- **"Change a knob"** → §12 config sources + hot-reload + Advanced panel
- **"What does the AI player see?"** → §7 AI-API
- **"How does dispersal work?"** → MISSION §2.2 + brief §4.6
- **"Why this game design?"** → MISSION §1 M-1 grounding + brief §0 capsule
- **"Visual identity"** → §0 capsule, §2 color & lighting, §3 materials, §4 motion
- **"Daily seed start state"** → §8.2 seed derivation (3–15 perma-dead)
- **"Daily score submission"** → §8.4 server replay (wasm32-wasip1 Worker)
- **"Sound assets"** → §11 freesound.org CC0 curation methodology
- **"Camera controls"** → §4.8 layer traversal + minimap arcball + seats

## Sibling repos on this machine

- `/Users/macmini/CIRISAgent` — Python headless agent runtime; release CI mirrored here
- `/Users/macmini/CIRISRegistry` — canonical CEG 1+4 primitive ([FSD-002 §3.4](https://github.com/CIRISAI/CIRISRegistry/blob/main/FSD/FSD-002_FEDERATION_SURFACE.md))
- `/Users/macmini/CIRISServer` — Rust sibling; closer to our build shape; `bevy_panorbit_camera` precedent
- `/Users/macmini/CIRISNodeCore` — federation consensus spec
- `/Users/macmini/CEWP` — the "no center" framing; `examples/scale_model.rs` is the scaling-math reference
- `/Users/macmini/CIRISVerify` — hardware-rooted identity verification
- `/Users/macmini/CIRISPersist` — federation directory substrate

## Working style with this codebase

- The brief is the source of truth. Every numeric value in the brief should have a corresponding knob in `ITERATION_KNOBS.json` with an `applyMode`.
- MISSION.md is sibling-conformant; match the canonical structure (cross-references with relative paths, recursive Golden Rule bites, references section).
- BACKLOG.md is the single forward-only build plan. No version-history scarring.
- If you're unsure whether to change something locked, ask the user. The locked list is short on purpose.

## First steps in a fresh session

1. Read `MISSION.md` fully.
2. Skim `docs/DESIGN_BRIEF.md` §0 capsule + §1 engine + §13 architecture position.
3. Read `docs/BACKLOG.md` to know what's next.
4. Verify `cargo` and `wasm32-unknown-unknown` are installed.
5. Pick the leftmost item in BACKLOG and start.

## If something doesn't make sense

The brief is intentionally dense. If a section's intent is unclear, look for:
- the same concept in `MISSION.md` (more explanation, less spec)
- the relevant ITERATION_KNOBS entry (the `effect` field carries the design intent)

If still unclear: surface to user. The brief evolved through a long iteration sequence before this clean cut; some context lives in the user's head and didn't quite make it to disk.
