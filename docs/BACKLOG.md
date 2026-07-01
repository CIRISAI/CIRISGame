# CIRISGame — Backlog

What's left to build, in shipping order. Dependencies only. No time estimates.

---

## Tier 0 — Human input (playability blocker, unblocks everything else)

These three tasks are the only thing standing between "screensaver demo" and "playable game". They are self-contained changes to existing files; nothing in Tiers 1–7 depends on them being absent.

### 0a. Gate screensaver on player kind

`screensaver::drive` (`screensaver.rs`) runs unconditionally in every frame and every app state, including `AppScreen::Playing`. Add `roster: Res<RosterConfig>` to its signature and early-return when `roster.slots[board.0.current_steward().slot() as usize].kind == PlayerKind::Human`. The screensaver then drives only Computer/Agent turns; human turns fall through to 0b.

### 0b. Human click-to-place system

Add a new Bevy system (new file `gameplay.rs`, registered in `render.rs`) that runs only in `AppScreen::Playing`:

- Read `HoveredCell` (already computed by `hover.rs` every frame — the frontmost board cell under the cursor).
- On `MouseButton::Left` `just_pressed`, check that `roster.slots[current_steward().slot()].kind == PlayerKind::Human`.
- Confirm the coord is in `board.0.current_legal_moves()` (already filtered by hover glow, but double-check).
- Call `board.0.apply_move(Move::place(coord))` and set `BoardDirty(true)`.

The `HoveredCell` resource is `None` when the cursor is over an occupied / perma-dead / cross-blocked cell, so illegal clicks are already a no-op at the hover layer; the placement system only needs the legal-moves guard.

### 0c. Playing-mode turn indicator HUD

A minimal Bevy UI node that shows whose turn it is and whether they're thinking or waiting for input. Spawned `OnEnter(AppScreen::Playing)`, despawned `OnExit`. Reads `RosterConfig` and `board.0.current_steward()` each frame. Required so human players know when to act and when to wait for the AI. No design lock on the exact copy; suggest: steward color disc + pigment name + "your move" / "thinking…".

---

## Tier 1 — Foundation (parallel-safe)

### 1. Engine extraction [DONE]

Extract game logic from the Bevy crate into a `no_std + alloc` workspace member `ciris-game-engine-core`. Owns: lattice math, Algorithm A dispersal, mesh merge rules, atari at |M|=6, resonance trigger, perma-dead computation, MoveLog application, board_state_hash. **No bevy_*, no wgpu, no winit, no `std::collections::HashMap`** (iteration-order non-determinism). Deps: `serde + alloc + derive`, `rand_chacha`, `sha2`, `arrayvec`. The Bevy view layer `pub use`-re-exports core; public API unchanged.

Unblocks: every wasm32-wasip1 work (daily-seed Worker, future spectator stream replay).

### 2. RNG seed propagation [DONE]

Plumb `rand_chacha::ChaCha8Rng` through every randomness consumer: Gray-Scott texture seeding, agent particle phase, Morton tiebreaks in Algorithm A, daily-seed pre-existing perma-dead positions. Replay determinism depends on this. Cross-target CI: 1000 random `(seed, move_log)` pairs walked through both `wasm32-unknown-unknown` and `wasm32-wasip1`, asserting identical `(permadead_count, all_survivors, board_state_hash)`.

### 3. Resonance coefficient sweep [DONE]

Empirical α / β / γ / δ recalibration. Rust unit test walks `|M|, |N| ∈ [1, 14]` and confirms the resonance trigger (`δ · min(|M|, |N|) > 0.5 · (α·|M| + γ·|N|)`) fires within the expected mid-game band. Update DESIGN_BRIEF §4.1 starting defaults if the sweep finds better numbers.

### 4. Algorithm A sweep across N ∈ {3, 4, 5, 6, 7} [DONE]

Simulator sweep over board sizes. Identify pathological topologies on 6×6×6+ that exhibit excessive `SINGLE_LIVE` demotions. Either commit to A on big boards or specify a fallback partition. The 5×5×5 default ships unchanged regardless.

---

## Tier 2 — Core gameplay (parallel after Tier 1)

### 5. Lattice + materials (DESIGN_BRIEF §3) [DONE]

Bevy scene with rhombic-dodecahedral lattice at integer world positions, glass shells with `StandardMaterial.specular_transmission`, inner cores on bloom layer 1, pipes via `Capsule3d`, ghost wireframe via `bevy_polyline`. Custom rim term via `ExtendedMaterial<StandardMaterial, RimMaterial>`. Gray-Scott R-D per mesh at 96×96 ping-pong.

### 6. Game logic (DESIGN_BRIEF §4) [DONE]

Mesh tracking, same-color adjacency, pipe formation, mesh-merge, atari detection at |M|=6, destructive transition at |M|=7, Algorithm A dispersal, the no-crossing rule with forced-pass turn handling and all-pass deadlock termination (§4.11, `crossing::is_crossing_illegal`), score (`permadead_count`), end-state detection.

### 7. Camera systems (DESIGN_BRIEF §4.8) [DONE]

`bevy_panorbit_camera = "0.35"` base + custom systems for layer traversal (pinch/scroll moves camera through lattice, near-clip plane animates inside AABB), minimap-in-sphere arcball (1.6·N radius unlit sphere containing 1:8 scaled lattice replica on `RenderLayers::layer(2)`), steward seats (4 colored balls on a ring at `1.80·N` radius), re-center button.

### 8. Editable steward names (DESIGN_BRIEF §5.4)

Drawer text input per slot, 12 chars max. Persists to `localStorage["cirisgame.slots"]` + IndexedDB backstop. **Strictly local invariant** — never appears in filenames, `BoardView` JSON, or any network surface.

### 9. Mist materials (DESIGN_BRIEF §3.6)

Custom `Material` trait impl with `AsBindGroup` for raymarched volumetric mist. Black for temp-dead (1 turn, 0.6 units/s flow), green Verdigris for perma-dead (forever, 0.3 units/s). `AlphaMode::Opaque` with discard-on-noise threshold so it renders before `Transmissive3d` and composites correctly under the refracting shell.

### 10. Endgame animations (DESIGN_BRIEF §4.7) [DONE for screensaver; wire to Playing mode in 0c]

Mourn / celebrate / wild systems triggered when no legal placement remains. Wild bloom pulse temporarily moves ghost-lattice entities to layer 1 so they glow.

---

## Tier 3 — Player surfaces

### 11. Computer AI [Easy policy done in screensaver.rs; Medium/Hard/Brutal + Playing-mode wiring remain]

Easy (legal-random with self-explosion pruning), Medium (greedy local), Hard (minimax depth-2 with score-aware heuristic, understands the `r = 2` asymmetry), Brutal (MCTS within the full 2 s budget). Uniform 2 s wall-clock budget across all four; thinking pulse plays the full 2 s.

### 12. Agent HTTP wrapper

`POST /move` endpoint, optional bearer auth, 2000 ms timeout (forfeit to random legal move on timeout, logged `forfeit:timeout`). Caps: 1 `pick_move` per turn per agent; 20 renders per turn per agent.

### 13. AI-API render path (DESIGN_BRIEF §7)

`BoardView::{to_json, to_ascii, render_png, render_animation}`. Off-screen Bevy `Camera { target: RenderTarget::Image(handle) }` with `Readback::texture` for pixel readback. PNG via `image` + `png` crates. Animation as `Vec<Vec<u8>>` PNG sequence; optional APNG via the `apng` crate.

### 14. Unified ARIA-live region (DESIGN_BRIEF §7.7)

Single canvas-adjacent `<div role="status" aria-live="polite">` serving every announcement surface. String registry, grade-5 reading level. NEVER use "rejected", "invalid", "cheat", "verification failed".

---

## Tier 4 — Shareable + sound

### 15. Shareable replay (DESIGN_BRIEF §10)

384×384 APNG, 52 frames (8 turns × 6 fps + 4-frame tail), 500 KB cap with frame-drop fallback (max 3 passes). Save (disk only) + Share (clipboard image + receipt text on web; native share sheet via Tauri 2's `dialog` plugin). Bundle is atomic: `.apng` + `-card.png` + `-receipt.txt`. Filename excludes custom names.

### 16. Sound bed (DESIGN_BRIEF §11)

Vendor 7 OGG samples from freesound.org CC0 picks per the curation methodology. Web Audio API via `web_sys::AudioContext`. Welcome banner in drawer label strip. `prefers-reduced-motion` auto-mute. Caption strip toggle.

### 17. Native sound

`rodio` integration for Tauri builds. Same OGG bundle, same bus topology.

### 18. Android haptic on placement

`navigator.vibrate(8ms)`, opt-in via drawer. Resolves the haptic open question.

---

## Tier 5 — Modes + mobile

### 19. Mode router (DESIGN_BRIEF §6.2)

Browser hash router (`#screensaver`, `#hot-seat`, `#d=YYYY-MM-DD`). Native `--mode` CLI flag. Switching modes from drawer rewrites hash without reloading the wasm module.

### 20. Mobile portrait (DESIGN_BRIEF §6.7)

`matchMedia('(max-width: 768px) and (orientation: portrait) and (pointer: coarse)')` triggers 60° down-angle camera, thumb-reach HUD, long-press twist, smaller arcball (1.2·N).

### 21. Flat top-down accessibility view (DESIGN_BRIEF §6.7)

Drawer toggle: 90° look-down + 2-layer cross-section + ARIA announcements on slice navigation. For users who can't parse 3D depth cues.

### 22. First-twist theater (DESIGN_BRIEF §4.8 with cinematic reveal)

Once per browser per session. First arcball activation gets an 800 ms fade-in (vs 200 ms steady) with a 1.2 s camera dolly orbiting 30° around the sphere as it materializes. After first reveal, drop to spec'd timing. `localStorage["cirisgame.onboarding.firstTwistSeen"]` boolean.

---

## Tier 6 — Daily seed

### 23. Seed derivation + landing page (DESIGN_BRIEF §8.2, §8.3)

ChaCha8-deterministic seed produces (difficulty roster, K ∈ [3, 15] perma-dead count, K cell positions, board_state_hash). Landing page reveals K as `today: 7 substrate scars`. First-visit panel.

### 24. Server-side replay verification (DESIGN_BRIEF §8.4)

Cloudflare Worker hosting the `wasm32-wasip1` engine blob. `POST /v1/daily/:date/submit` with pre-replay validation, V8 isolate replay (200 ms hard cap), D1 single-statement transaction on match. Mismatch / timeout → silent 204 + audit log. Client-side IndexedDB retry on timeout.

### 25. Coherence ribbon

`GET /v1/daily/:date/aggregates` returns `{plays_count, m1_count}`. No median, no percentile, no streak. Ribbon hides silently when endpoint unavailable.

---

## Tier 7 — Spectator

### 26. Spectator URL (DESIGN_BRIEF §9)

`play.ciris.ai/watch/{slug}`. Cloudflare Durable Object instance keyed by `slug`. Playing client opens WebSocket and pushes `BoardView` JSON diff per turn. Spectator client subscribes and renders identical Bevy scene. Auto-pause on tab blur. Rate-limit 10 streams per IP per hour.

---

## Build order — waterfall (dependencies only)

```
Tier 0 (ship NOW — unblocks human play)
  0a. Gate screensaver    0b. Human click-to-place    0c. Turn indicator HUD

Tier 1 (parallel) [all DONE]
  1. Engine extraction    2. RNG plumbing    3. Resonance sweep    4. Algorithm A sweep

Tier 2 (needs Tier 1, parallel internally) [5–7 DONE; 8–10 remain]
  5. Lattice + materials    6. Game logic    7. Camera    8. Editable names
  9. Mist materials   10. Endgame animations

Tier 3 (needs Tier 2) [11 partial; 12–14 remain]
  11. Computer AI    12. Agent HTTP    13. AI-API render    14. ARIA-live region

Tier 4 (parallel after Tier 3)
  15. Shareable replay    16. Sound bed    17. Native sound    18. Android haptic

Tier 5 (parallel after Tier 3)
  19. Mode router    20. Mobile portrait    21. Flat top-down a11y    22. First-twist theater

Tier 6 (needs Tier 1 wasm32-wasip1 work + Tier 3 game logic stable)
  23. Seed derivation + landing    24. Server replay Worker    25. Coherence ribbon

Tier 7 (needs Tier 6 daily-seed deterministic state + Tier 3 BoardView stable)
  26. Spectator URL
```

---

## Out of scope

- Real-time multiplayer (WebRTC, CRDT, matchmaking, party invites)
- Federation envelope attestation in daily-seed submissions
- HUMANITY_ACCORD recognition in BoardView JSON
- ML-DSA-65 / PQC hybrid signing
- Hosted replay gallery
- Twitter / Bluesky / X share intents
- Streak counter, percentile ranking, "you beat X%" copy
- Account system
- Inviter K-factor parameter
- Profanity filter / moderation surface (the client never displays user-generated text)
- macOS NSSharingServicePicker (clipboard fallback parity is sufficient)
- WILD title-card A/B (prepended is the locked choice)
- Quiet-hours auto-duck on audio
