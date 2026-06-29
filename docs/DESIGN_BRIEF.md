# CIRISGame — Design Brief

**License**: AGPL-3.0-or-later.
**Repo**: `/Users/macmini/CIRISGame/`.
**Cross-references**: [CIRISAgent/ACCORD.md](https://github.com/CIRISAI/CIRISAgent) §M-1, [CIRISRegistry/FSD/FSD-002](https://github.com/CIRISAI/CIRISRegistry) §3.4 (CEG 1+4 primitive), [CEWP/README.md](../CEWP/README.md).

---

## 0. Capsule

CIRISGame is a four-steward turn-based game on a 3D rhombic-dodecahedral lattice — the space-filling 3D analog of a hex grid, twelve face-neighbors per cell, no center, holonomic by construction. One rule a five-year-old can hold: **don't let your mesh hit seven cells**. Same-color adjacent cells auto-link through glass pipes; under the hood those pipes are `delegates_to` envelopes in the CEG 1+4 grammar. A mesh that hits seven undergoes a **destructive transition** — not erasure: from the destruction, smaller groups are *created*. This is the in-game visceral conveyance of CIRIS's [coherence-collapse-analysis](https://github.com/CIRISAI/coherence-ratchet) work. The dispersed cells either return as 2-cell live pairs in the steward's color or become perma-dead substrate (green ethereal mist forever) every other player must route around. Lowest count of perma-dead stones at game end wins; if every steward ends at zero, the whole board lights up in a cooperative WILD celebration — the M-1 *sustained adaptive coherence* ending. The default is screensaver: four Computer players on a uniform 2-second compute budget, no chrome past a small legend and a sun/moon toggle, the scene shimmering. **Agent particles inside each sphere are load-bearing** — a six-cell mesh reads as visibly *holding its breath*, the way a Go group with one liberty does. In your hand it feels like a warm-clay glass lab on a Bone-cream desk — heavy, quiet, slightly alive — and the only colors that matter are the four pigment cores: Sienna, Lapis, Verdigris, and ringed-Kaolin.

---

## 1. Engine and Build

**Engine**: a single Rust crate `ciris-game-engine` built on **Bevy 0.19**. The view layer is pure Bevy ECS; the `BoardView` snapshot is an internal `Resource` mutated by the rules system and observed by render systems in the same process.

**Backends, shipped as two WASM artifacts** (Bevy issue #13168 prevents a single binary from selecting WebGPU or WebGL2 at runtime):

- `app.webgpu.wasm` — features `webgpu,tonemapping_luts,bloom,pbr_transmission_textures`. Primary target.
- `app.webgl2.wasm` — same features minus `webgpu`. Fallback; screen-space refraction softer than WebGPU but acceptable.

A small `index.html` shim probes `navigator.gpu` and ES-module-imports the matching `.wasm`. No runtime branching inside the engine.

**Native**: Bevy's stock `winit` window targets macOS / Linux / Windows. **Headless mode**: feature-gated, links no rendering code; used for AI tournaments and CI.

**Build**:

```bash
rustup target add wasm32-unknown-unknown
cargo build --release --target wasm32-unknown-unknown --features 'webgpu,tonemapping_luts,bloom,pbr_transmission_textures'
wasm-bindgen --target web --out-dir dist/webgpu/ target/wasm32-unknown-unknown/release/ciris-game-engine.wasm
wasm-opt -Oz -o dist/webgpu/app.webgpu.wasm dist/webgpu/ciris-game-engine_bg.wasm
# Repeat for webgl2 features → dist/webgl2/app.webgl2.wasm

cargo build --release                                      # native
cargo build --release --no-default-features --features 'headless'   # headless
```

CI matrix covers linux-x86_64 / macOS arm64 / macOS x64 / windows-x64 / wasm32-unknown-unknown. Bundle target: ~5–9 MB gzipped per WASM artifact (~4–6 MB brotli); native ~14–22 MB raw.

---

## 2. Palette and Lighting

### 2.1 Tokens

All tokens are CSS custom properties on `:root` and shader uniforms. Steward hexes are the canonical mapping; they appear identically in JSON state and in rendered pixels.

| token | hex | name | intent |
|---|---|---|---|
| `bg.deep` | `#FAF9F5` | Bone | Light-mode page background |
| `bg.mid` | `#E8E6DC` | Linen | Side-panel and HUD chrome fill |
| `surface.glass-tint` | `#EAF2EE` | Borosilicate | Multiplied over shell transmission |
| `ink.primary` | `#141413` | Ink | Body type, sphere rim ring, pipe joints |
| `ink.secondary` | `#5F5C55` | Slate | Hairlines, secondary type, lattice wireframe |
| `accent.warm` | `#D97757` | Clay | Turn indicator, hover ring, victory flash |
| `accent.cool` | `#6A9BCC` | Lapis | Focus state, "thinking" pulse |
| `steward.1` | `#D97757` | Burnt Sienna | Steward 1 core (Gray-Scott seed: spots) |
| `steward.2` | `#6A9BCC` | Lapis Lazuli | Steward 2 core (seed: stripes) |
| `steward.3` | `#788C5D` | Verdigris | Steward 3 core (seed: labyrinth) |
| `steward.4` | `#E8E6DC` | Kaolin | Steward 4 core (seed: spirals); mandatory 2 px Ink ring |

**Light mode is the default.** Dark mode flips on `prefers-color-scheme: dark` or via the HUD sun/moon toggle (persisted to `localStorage["cirisgame.palette"]`). Dark overrides invert `bg.*` and `ink.*` and bump key intensity, rim intensity, and bloom strength so cores carry the room.

### 2.2 Lighting rig (N = 5 board baseline; scale by N/5 for other sizes)

- **Key** — warm clay, upper-right-front. `pos = vec3(6.67, 9.17, 5.00)`, `tint = #FFE5CC`, `intensity = 1.6`, soft area.
- **Fill** — cool linen, lower-left, half-strength. `pos = vec3(-5.83, 2.50, 4.17)`, `tint = #DCE5EF`, `intensity = 0.55`.
- **Rim** — accent rake from behind. `pos = vec3(0.83, 3.67, -7.50)`, `tint = #FFD6A8`, `intensity = 1.2`, narrow cone.
- **Sky** — hemispheric gradient overhead, `pos = vec3(0, 13.33, 0)`, `intensity = 0.35`.

HDR via the `Hdr` marker component on the camera (RGBA16F implicit). Tone mapping: `Tonemapping::AgX` enum variant on every camera with `ColorGrading.global.exposure = 0.0` and `post_saturation = 1.0` as neutral starting points.

### 2.3 Bloom

Bevy's `Bloom` component does not honor `RenderLayers` on entities (issue #7361), so selective bloom is achieved with **two cameras over the same render target**:

- **Camera A** — layer 0, no HDR, no bloom, `Tonemapping::AgX`. Renders shells, pipes, lattice ghosts, HUD.
- **Camera B** — `RenderLayers::layer(1)`, `Hdr`, `Bloom { intensity: 0.15, composite_mode: EnergyConserving }`, `Tonemapping::AgX`. Renders the emissive cores.

Both cameras must be spawned in one tuple to avoid HDR-mismatch issues. Bloom tint multiplier `#FFF1E0` carries the warm temperature into the glow.

---

## 3. Materials and Geometry

### 3.1 Lattice geometry (board-size independent)

The lattice is the **rhombic-dodecahedral honeycomb = a single FCC lattice**. Cells occupy the **FCC sublattice** of the cubic box: integer positions `(i, j, k) ∈ {0..N-1}³` **with `i + j + k` even** (one parity class), `→ world pos = (i − (N−1)/2, j − (N−1)/2, k − (N−1)/2)`. Twelve face-neighbors per interior cell; a cell at `(i, j, k)` is face-adjacent to `(i+di, j+dj, k+dk)` where exactly two of `(di, dj, dk)` are `±1` and one is `0` — every such displacement changes `i+j+k` by an even amount, so a cell's twelve neighbors are **other FCC cells of the same parity**, and they are its **twelve nearest cells** (√2 away). The skipped odd-parity integer points (distance 1) are **not cells**.

**Why the parity restriction (locked).** Using *all* integer points would make the 12-face-diagonal adjacency split the board into two interleaved, never-connecting parity sub-lattices (each bond preserves `i+j+k` parity), and the visually-nearest cells (axis-aligned) would not be neighbors at all. Restricting to one parity is the actual rhombic-dodecahedral honeycomb (a single FCC lattice): one connected board where adjacency equals nearest-neighbor, so proximity reads true. A direct corollary used downstream: **on a single parity class the two diagonals of any unit face can never both be cells** (they are opposite parities), so two bonds can never cross — see §4.11.

**Boundary liberties (preserved).** The board is a *finite* FCC solid, so it keeps real sides and corners: an interior cell has the full **12** face-neighbors, a box-face cell fewer, a box **corner** cell as few as **3**. Corners and edges therefore have fewer liberties / potential tubes than the interior — a genuine positional feature (a corner stone is hard to grow toward a collapse, but also hard to connect). This is geometric boundary structure, not axis favoritism: the lattice stays isotropic (no preferred axis).

| primitive | value |
|---|---|
| cubic-axis pitch | 1.0 world unit |
| face-neighbor center distance | √2 ≈ 1.4142 |
| inradius (geometric shell-max) | √2/2 ≈ 0.7071 |
| **glass shell radius** | 0.42 |
| **inner core radius** | 0.25 |
| **channel** between face-neighbor shells | 0.574 |
| **pipe radius** | 0.055 |
| pipe length (shell-surface to shell-surface) | 0.574 |

Board AABB scales linearly: span = N world units. The cell count is the number of even-parity points of the `N³` box (the FCC sublattice), **not** `N³`:

| N | FCC cells (`i+j+k` even) |
|---|---|
| 3 | 14 |
| 4 | 32 |
| **5 (default)** | **63** |
| 6 | 108 |
| 7 | 172 |

Default **N = 5 → 63 cells**. Alternates 3, 4, 6, 7. N = 7 is native-only (mobile browsers may struggle).

### 3.2 Glass marble — `OrbMaterial` (custom shader)

Each live cell is **one opaque clear-glass marble**: a neon swirling-gas core (§3.3) at the centre, wrapped by a thick clear-glass edge. The glass **refracts and reflects only the surrounding starfield (§3.8) — never other game objects.** A marble never shows the rest of the board's jumble through it; to see what is behind a marble you rotate or re-orient the camera (§4.8), not look through the glass. This is a deliberate departure from screen-space transmission of the live scene: the environment map is fixed to the enclosure, so each marble reads as a self-contained gas-in-glass orb rather than a lens onto the board.

Implemented as a custom fragment material (`orb.wgsl` / `OrbMaterial`), not screen-space `specular_transmission` (which would lens the live scene through the glass). The shader composites, opaque, in one surface:

```text
gas core (centre, §3.3)
  + clear glass edge that REFRACTS + REFLECTS the starfield env only (§3.8)
  + fresnel rim catch  pow(1 - dot(N,V), 3) — a visionOS-style edge outline
  + optional X-cube dichroic prism on that rim (knob spheres.prism)
knobs: glass.ior, glass.reflect, spheres.core_size, spheres.gas_luma/gas_sat
```

Head-on reads as a clean lens onto the gas core; oblique reads as a bright glass edge against the stars. The **optional X-cube dichroic prism** rides the fresnel rim, splitting the rim catch into spectral colour at grazing angles (knob `spheres.prism`).

### 3.3 Gas core — `StandardMaterial` on bloom layer 1

At the centre of each marble, a **neon swirling-gas core** carries the steward's color. `base_color = steward.hex`, `emissive = steward.hex × emissive_intensity` (default 0.6, range [0.4, 1.8]). `RenderLayers::from_layers(&[0, 1])` so the core receives PBR shading on Camera A *and* glows on Camera B. The core radius is **tunable** (knob `spheres.core_size`) as a fraction of the marble: a small core reads as a bright nucleus deep in clear glass; a large core fills most of the marble with gas. Gas luminance and saturation are likewise tunable (`spheres.gas_luma`, `spheres.gas_sat`).

The gas's motion is a Gray-Scott reaction-diffusion texture (one shared texture per mesh, 96×96 R8G8) sampled via an extended material. The R-D pattern is the "automata thang" swirling inside the glass — see §4.2.

### 3.4 Tubes

A bond between two face-adjacent same-color cells renders as a **single straight tube** — a Bevy `Capsule3d`, length 0.574, radius 0.055 (knob `layout.tube_width`) — running directly between the two marble centres. The tube uses the **same glass-marble material as the spheres (§3.2)**: the neon gas (§3.3) fills the *whole* tube, so the bond never vanishes at grazing angles and stays visually continuous with the gas in the two spheres it joins. Tubes sit in the channel between shells; up to 12 per node, spreading naturally along surface normals.

Tubes always run **straight**. The no-crossing rule (§4.11) guarantees two different-color bonds never share a face diagonal, so there is never a crossing to dodge — no bowed or two-segment routing is needed.

### 3.5 Ghost cells (empty lattice)

`LineSegments` rhombic-dodecahedron wireframe via `bevy_polyline`, in Slate `#5F5C55` at 18 % alpha. Distance fade: `mix(0.35, 0.05, smoothstep(6.0, 18.0, camDist))`.

### 3.6 Dead-group mist — temp (black) and perma (green)

**Temp-dead (until the steward's rebuild turn).** The instant a mesh hits 7, all its cells enter `TEMP_DEAD` and the crater smoulders through the opponents' turns until the owning steward rebuilds it (§4.6). Shell saturation drops 60 %; core dims to 30 % emissive; a black volumetric mist (raymarched fragment, 32 steps, 3D simplex noise at octave 2, freq 1.4) flows inside the shell at 0.6 units/s. Pipes inside the dead mesh go fully opaque black. Custom `Material` trait impl with `AlphaMode::Opaque + discard-on-noise-threshold`, rendered in `Opaque3d` before `Transmissive3d` so refraction composites correctly.

**Perma-dead (forever).** After dispersal (§4.6), flagged `PERMA_DEAD` cells take shell `attenuation_color = Verdigris #788C5D`, core hidden, green mist flowing at 0.3 units/s (slower; at rest). Incident pipes drop to Stone `#B0AEA5` × 0.45 alpha, no inner mist. Perma-dead cells are removed from the legal-move set permanently.

### 3.7 Last-placed indicator

A bloom-layer-1 halo sphere of radius 0.55 over the just-placed cell. Opacity tweens `0 → 0.6 → 0` over 800 ms in hot-seat, 1500 ms in screensaver.

### 3.8 Starfield enclosure

The play area sits inside a deep-space enclosure — **pure-black space** with white twinkling stars in **two parallax layers** (a near layer and a far layer that shift at different rates as the camera orbits, giving honest depth). A slow whole-field **drift** is available but **default OFF**: the starfield is an orientation reference, and a still field reads as a fixed frame the player navigates against. Capping the vertical axis are two **pole nebulae** — a cool-hue glow over the +Y pole and a warm-hue glow under the −Y pole — with black around the horizon between them, so the up/down axis carries a fixed color signature.

Together with the four steward signets (§3.10) on the horizontal cardinals, the pole nebulae give every one of the six spatial directions a distinct visual anchor — four colored horizons, a cool zenith, a warm nadir — so the player never loses orientation while flying through the lattice (§4.8).

The starfield is also the **only** thing the glass marbles refract and reflect (§3.2) — never other game objects — so the enclosure doubles as the marbles' environment map. (It replaces the earlier warm horizon dome.) Star density and brightness, twinkle rate, drift, nebula strength, and the two pole hues are tunable (`space.*` knobs).

### 3.9 Agent particles around a node

- Count: `3 + min(9, link_count)`, hard cap **5**.
- Color: OKLCH L*+8 % shift of the steward hex (computed via the `palette` crate at startup).
- Orbit angular speed: `0.6 + 0.25 · T_vis` rad/s where `T_vis` is the normalized temperature from §4.1.

**Atari override** when `|M| = 6` — one move from destructive transition. The particles are load-bearing as the discrete atari signal; a 6-mesh must read as visibly *holding its breath*.

- Count locks to **5**.
- Orbits Kuramoto-phase-couple (β = 0.4) over ~600 ms; the whole 6-mesh breathes as one.
- Orbit speed locks at **0.35 rad/s** (slower than calm).
- Particles shift inward to `r = 0.50·R`.
- Emissive pulse: `1.0 + 0.4·sin(2π · 0.6 · t)` — a 0.6 Hz breath.
- A faint Verdigris ring around each cell at 25 % alpha foreshadows the green mist.

End-state animation (§4.7) overrides atari at game end.

### 3.10 Steward signets

Four glowing emblems — one per steward, each in that steward's pigment color — float **outside the play area** at the four horizontal cardinal directions (E / W / N / S), on the seat ring (§4.8). Each signet is a billboarded emblem that always faces the camera. The signet of the steward **whose turn it is** burns ≈ **4× brighter** than the three idle signets (knob `signets.active_boost`), so the active player is legible from any camera pose without reading the HUD.

With the up/down pole nebulae (§3.8) these are the player's orientation anchors: the four colored cardinals plus a cool zenith and a warm nadir mark all six directions in space. Brightness floor, active boost, size, and ring distance are tunable (`signets.bright`, `signets.active_boost`, `signets.size`, `signets.distance`).

---

## 4. Motion and Dynamics

### 4.1 Mesh temperature — canonical formula

For a mesh `M` with face-adjacent enemy meshes `N`:

```
T(M) = α·|M|
     + Σ_{N ∈ adj(M)} [
           γ·|N|              if |N| ≥ |M|
         − β·(|M| − |N|)      if |M| >  |N|
         + δ·min(|M|, |N|)
       ]
```

Defaults: **α = 0.060, β = 0.080, γ = 0.050, δ = 0.062** (δ nudged +0.002 from the starting 0.060 by the BACKLOG #3 resonance sweep so the near-atari pair `(6,5)` clears the trigger — Gap 2). Display normalization: `T_vis = 1 − exp(−max(T, 0) / 1.40)`, clamp `[0, 1]`.

Three defenses, one formula: statistical mechanics (larger thermal mass changes less per packet), information thermodynamics (higher-entropy source dominates lower-entropy receiver), CEG attestation flow (size gradient *is* the attestation gradient).

### 4.2 Gray-Scott pattern — per mesh, not per cell

One R-D simulation per mesh at 96×96 R8G8 on a WebGL2 ping-pong target. All cells of the mesh sample the same texture at a per-cell UVW offset. Iterations per render frame: 16 at 60 fps, 8 at 30 fps, 4 at 6 fps export.

| T_vis band | regime | F | k |
|---|---|---|---|
| `[0.00, 0.20)` | calm spots | 0.040 | 0.060 |
| `[0.20, 0.40)` | drifting blobs | 0.034 | 0.058 |
| `[0.40, 0.60)` | stripes | 0.029 | 0.057 |
| `[0.60, 0.80)` | writhing | 0.025 | 0.055 |
| `[0.80, 1.00]` | mitosis chaos | 0.014 | 0.054 |

Crossfade F and k linearly across bands over 600 ms. Per-steward seed pattern fixed at mesh birth: Sienna spots / Lapis stripes / Verdigris labyrinth / Kaolin spirals. The pattern family is a WCAG redundant channel — surviving 96×96 PNG chroma degradation.

### 4.3 Agent orbits

Spherical coords `(r, θ, φ)` around the node center; `r = 0.62 · shell_radius` at rest. Per-particle phase advances by `dt × orbit_omega`. Wobble adds a perlin-style noise amplitude that scales with `T_vis`. A face-pair **resonates** when both gates pass — `min(|M|, |N|) ≥ 4` (scale floor: small groups near each other only *vibe*, big groups near each other *excite*; knob `resonance.minMeshSize`) **and** `δ · min(|M|, |N|) > 0.5 · (α·|M| + γ·|N|)`. On resonance, one particle per side launches a Hermite arc to a midpoint shell between the two nodes, dwells 800 ms, returns. Resonance direction: **large → small** (heat flows down the gradient; attestations flow down the size gradient). The scale floor is the BACKLOG #3 Gap-1 fix — the bare condition is scale-free for balanced pairs (size cancels), so without the floor `(1,1)` would resonate like `(6,6)`.

### 4.4 Camera (N = 5 baseline; distance scales as 1.80 · N)

Default orbit: `(yaw 0.785 rad ≈ 45°, pitch 0.35 rad, distance 9.00)`. The 45° yaw opens the view looking *between* two adjacent steward signets (§3.10); the pitch gives a gentle downward tilt onto the board. Mouse-drag smoothing τ = 0.18 s (yaw/pitch), 0.22 s (distance). On placement: 200 ms ease-out-back zoom-in to the placed cell. Screensaver: continuous yaw at 0.05 rad/s + a 0.030-unit breath at 0.030 Hz.

Camera control via `bevy_panorbit_camera = "0.35"` plus custom systems for the layer-traversal, minimap arcball, and seat-return mechanics in §4.8.

### 4.5 Turn transitions

| event | duration | easing |
|---|---|---|
| Inter-move pause (screensaver) | 2500 ms | linear |
| Inter-move pause (CPU-vs-CPU non-screensaver) | 250 ms | linear |
| Inter-move pause (hot-seat human) | 0 ms | — |
| Node appear (scale 0 → 1) | 600 ms | ease-out-cubic |
| Shell alpha 0 → 0.85 | first 480 ms of appear | linear |
| Core alpha 0 → 1 | full 600 ms of appear | linear |
| Pipe delay after node | +200 ms | — |
| Pipe extrude | 400 ms | ease-in-out-cubic |
| Mesh-merge re-color | 1000 ms | ease-in-out-quart |

The smaller mesh's R-D target is released after merge; the larger keeps its target. Pipe-join receives a one-frame specular pop.

### 4.6 Dispersal — destructive transition (player-chosen layout)

When a mesh `M` of size `|M| ≥ 7` is created, the steward *rebuilds their own wreckage* on their next turn:

**Step 1 — immediate.** All cells of `M` enter `TEMP_DEAD` (§3.6 black mist). Pipes severed. The steward's turn ends. The crater smoulders through the opponents' turns.

**Step 2 — the steward's next turn (the rebuild turn).** In one move the steward both **lays out the crater** and **places a new stone** elsewhere. The layout assigns each crater cell to live (their color) or perma-dead, subject to two rules:

1. **Count floor (the locked score spine).** At least `floor = k + (1 if r=1 else 0)` perma-dead, where `k = N÷3`, `r = N mod 3`. A clever layout can never score *below* the table — only the *placement* of the spacers is the player's, not the amount.
2. **Legality.** The live cells the steward keeps may not form a connected component of `7` or more — dispersal can never hand back an already-collapse-sized mesh.

Human and agent players choose the layout (constrained to the crater's own cells); **Computers and any no-choice caller get the deterministic auto layout, "Algorithm A (Morton-greedy)":**

1. Order cells of `M` in 3D Morton (Z-order) sequence: `c[0], c[1], …, c[N-1]`.
2. Walk `i = 0, 3, 6, …` while `i + 2 < N` (0-based: the last triple consumes `c[N-3..=N-1]`):
   - Pair `c[i]` with `c[i+1]` if face-adjacent; else scan forward for the lex-smallest unconsumed face-adjacent neighbor of `c[i]` and swap it into `i+1`.
   - `c[i], c[i+1]` → candidate live pair; `c[i+2]` → perma-dead.
3. Remainder `r = N mod 3`: `r=1` → `c[N-1]` perma-dead; `r=2` → `c[N-2], c[N-1]` a live pair if face-adjacent, else both perma-dead.
4. **Narrow separation guard.** Demote a candidate live pair to perma-dead **only if** keeping it would connect live cells into a component of `≥ 7`. (This replaces the original wholesale "demote any touching pair" step, which BACKLOG #4 found destroyed the count table on dense blobs.)

Determinism: Morton order is canonical, the swap rule is lex-greedy, the guard is order-deterministic. Same crater → same auto layout (replay-safe across targets). The chosen or auto-resolved perma cells are recorded in the move log so a replay reproduces the exact layout.

**Step 3 — animated, ~1200 ms total.** (timings unchanged)

| transition | timing |
|---|---|
| TEMP_DEAD → LIVE | mist fades 600 ms; core reappears 400 ms |
| TEMP_DEAD → PERMA_DEAD | mist cross-fades black → green over 800 ms; shell `attenuation_color` lerps to Verdigris |
| `permadead_count` legend tick-up | per-cell, 50 ms stagger, 800 ms total |
| Pipes severed → new live pipes | 400 ms extrude |

**Strategic count floor** (k = N ÷ 3, r = N mod 3, min perma-dead = `k + (1 if r=1 else 0)`):

| N | k | r | live | min perma-dead | total |
|---|---|---|---|---|---|
| 7 | 2 | 1 | 4 | 3 | 7 |
| 8 | 2 | 2 | 6 | 2 | 8 |
| 13 | 4 | 1 | 8 | 5 | 13 |
| 14 | 4 | 2 | 10 | 4 | 14 |

The `r = 2` asymmetry (N = 8, 11, 14 cost less than the `r = 1` neighbors) is the strategic spine. On small collapses (N = 7, 8) the floor is always achievable (live cells can't reach 7); on large collapses, legality may force a few extra perma-dead beyond the floor. A skilled player chooses the spacer positions to hit the floor exactly and shape the surviving live groups; the auto chooser matches the floor for N = 7 and stays at-or-above it otherwise.

### 4.7 Endgame — mourn, celebrate, wild

Triggered when no steward has a legal placement. For each surviving mesh `M`:

- `proximity_to_dead(M) = 1` if any cell of `M` is face-adjacent to any `PERMA_DEAD`.
- `any_dead_globally = true` if any `PERMA_DEAD` exists on the board.

**MOURN** — `proximity_to_dead == 1` AND `any_dead_globally`. Orbit × 0.3; core desaturated 40 %, hue shifted +8° toward Lapis; bloom × 0.7; 0.3 Hz inhale envelope. 8 s before "New Game" affordance auto-fades in.

**CELEBRATE** — `proximity_to_dead == 0` AND `any_dead_globally`. Orbit × 1.5; particles hue-cycle through the 4-steward palette at 0.4 Hz; 4 Hz sparkle bursts; bloom × 1.3. 8 s.

**WILD** — `any_dead_globally == false`. Nobody triggered collapse. Cross-mesh resonance arcs unleashed; 5-second screen-wide bloom pulse to `luminanceThreshold = 0.35`; 540° camera sweep over 12 s; all four Gray-Scott seeds desynchronize and recombine. The score table is replaced with one line: **"The federation held."** 18 s before auto-restart. Ghost-lattice entities move to layer 1 during the bloom pulse so they glow with the rest.

### 4.8 Layer traversal, minimap arcball, steward seats

Each player has a fixed **steward seat** outside the board at one of four cardinal positions on a ring of radius `1.80 · N` at elevation `0.20 · N`, looking at the board center. Each seat is a `Mesh3d` sphere of radius 0.22, full `StandardMaterial` in the steward color, emissive on bloom layer 1, with the steward name floating 0.40 above as a 3D-billboarded label. In hot-seat the active steward's seat pulses radius ±3 % at 0.4 Hz; inactive seats render at 35 % opacity.

**Layer traversal.** Pinch-in (touch) or scroll-up (mouse) moves the camera FORWARD along view direction (toward board center). Forward speed cap 1.5 world-units/s, smoothing τ = 0.20 s. When the camera position is inside the AABB ± 0.05, `Camera::near = camera_to_facing_AABB_face + 0.05` so layers behind the camera get clipped; outside the AABB, near stays at 0.10 so the placement zoom-in doesn't fight. A subtle 1 % vignette darkens screen edges while inside the lattice volume, fading in over 200 ms when crossing the boundary.

**Twist / rotate — minimap-in-sphere arcball.** Two-finger twist or right-mouse-drag activates a virtual trackball. A translucent `12% Bone-tinted` sphere fades in over 200 ms at gaze center, radius `1.6 · N`. **The sphere contains a live 1:8 scaled replica of the entire lattice** — every cell, every mesh color, every perma-dead spacer, every glass pipe, and replicas of all four seat balls — as `RenderLayers::layer(2)` unlit entities (no PBR recursion, no bloom). The minimap rotates 1:1 with the user's twist gesture; what you see in the sphere is what the main camera will frame on release. On release, camera eases to the final POV over 300 ms.

**Re-center button.** HUD glyph bottom-right (32 px Bone disc, 1.5 px Clay stroke). Click eases the camera back to the player's seat over 700 ms (ease-out-cubic), simultaneously dismissing the arcball sphere if active. Auto-hides 4 s after camera returns. Keyboard: `Space` in non-hot-seat modes; `Tab` in hot-seat (Esc reserved for drawer).

**Selection glint.** The cursor — or a touch selection — emits a light that glints **consistently and strongly off every surface type**: glass spheres, tubes, and empty-position markers alike. The single coherent glint says *"you are selecting here, and these are the positions this selection touches,"* rather than a per-surface highlight that reads differently on glass than on a marker. Intensity is tunable (`layout.select_glow`).

### 4.9 Atari timing

Entry (mesh just grew to 6): orbit-speed lerps 400 ms ease-in-out; Kuramoto phase-lock 600 ms; Verdigris foreshadowing ring fades in over 400 ms. Steady state: inhale/exhale 0.6 Hz, ±0.4 emissive; phase locked. Exit: destructive transition supersedes immediately at turn 7, or end-state animation overrides at game end.

### 4.10 Cell states, size-1 meshes, and the no-capture invariant

The five cell states are exhaustive: `EMPTY` (ghost lattice, §3.5), `LIVE(steward)`, `TEMP_DEAD(steward)` (smoulders until the owner's rebuild turn, §4.6), `PERMA_DEAD` (forever, neutral, §3.6). A *mesh* is a connected component of one steward's `LIVE` cells under face-adjacency; `EMPTY` is a distinct state, never a zero-size mesh.

**Size-1 meshes are first-class.** Every placement is born a `|M| = 1` mesh until it links to a same-color face-neighbor. The temperature formula (§4.1) is defined at `|M| = 1`; dispersal *creates* `|M| = 2` live pairs. There is no minimum group size and no "lone stone is clear" demotion.

**No capture — the only death is self-triggered collapse.** CIRISGame has no liberties, no surround-to-kill, no enemy capture of any kind. A cell dies **only** when its *own* steward grows a mesh to seven and triggers the destructive transition (§4.6). A steward can never remove an opponent's cell. Consequence: a `LIVE` cell whose twelve face-neighbors are all occupied by other colors is **inert but safe** — it persists untouched for the rest of the game, contributes `|M| = 1` to temperature, and costs zero perma-dead. Placement is never adjacency-constrained, so a surrounded cell never traps its steward; a steward may place on any `EMPTY` cell anywhere.

The Go analogy in §0 ("holding its breath, the way a Go group with one liberty does") is the **atari *animation* metaphor only** — the held-breath particle pulse at `|M| = 6`. It is not a capture rule. This invariant is load-bearing for M-1 (MISSION §2.2): dispersal models collapse as self-inflicted and generative, never as adversarial annihilation. It is locked under the same "one rule" constraint as the rule of seven.

### 4.11 No-crossing rule and forced pass

A **bond** (the visual "tube", §4.6 pipes) joins two face-adjacent same-color `LIVE` cells. Every face-neighbor displacement on the FCC lattice is a `(±1, ±1, 0)`-type offset, so every bond is the **face-diagonal of a unit square** lying in an axis plane; the two diagonals of one face intersect at its center.

**The rule.** A placement of color `X` at cell `C` is **illegal** if it would create any same-color bond `C–N` (`N` a `LIVE` color-`X` face-neighbor) whose face's *opposite diagonal* `R–S` is **already** a `LIVE` bond of a different color `Y ≠ X` (both `R` and `S` `LIVE` and color `Y`). At most one diagonal per face may carry a live cross-bond — **first-come, first-served**. The rule is **color-dependent**: a cell forbidden to one steward can be legal for another, because the bond each would form is a different diagonal. It introduces the game's first indirect-attack lever (positional denial / "tube-fencing") without touching the no-capture invariant (§4.10) — collapse is still self-only. Quantitative validity, complexity, and playability analysis: [`docs/analysis/NO_CROSSING_RULE.md`](./analysis/NO_CROSSING_RULE.md). The canonical predicate is `ciris_game_engine_core::crossing::is_crossing_illegal`, the single source of truth shared by the engine and the analysis harness.

**Forced pass — and only forced.** A steward **passes if and only if they have no legal placement** (every `EMPTY` cell is occupied/dead or forbidden by the no-crossing rule). There is **no voluntary pass**: a steward with ≥1 legal move must play. A pass advances the turn to the next steward and leaves any owed crater (§4.6) pending. Termination is preserved: empties only ever decrease, so the game still saturates to the board limit; and if a full round of all four stewards passes with empties still on the board (a global cross-deadlock — rare, deep-endgame, ≈0.5–1.4% of games), the game is **over**, scored as it stands.

Like the rule of seven, this rule is **fixed-on in browser** and exposed as a native-only toggle (`noCrossingRule`, §12 knob, default on).

---

## 5. UI and Typography

### 5.1 Type stack

- **Display**: Inter, weights 400/500/700. Used for HUD labels, steward names, legend rows, button text.
- **Body / editorial**: Source Serif 4 (SIL OFL). Used for the rulebook line, end-screen taglines, caption strip.
- **Mono**: JetBrains Mono. Used for JSON state, mesh-size counters, endpoint URLs, debug overlays.

Tabular numerics always on for the mesh-size readout (`3/7`, `5/7`, `6/7`) so digits don't jitter between frames.

Size scale, 1× = 16 px root: `text-2xs 10`, `text-xs 12`, `text-sm 14`, `text-base 16`, `text-md 18`, `text-lg 22`, `text-xl 32`, `text-2xl 48`.

### 5.2 Screensaver HUD

The default scene. Idle 3 s → cursor + chrome auto-hide; mouse-move restores.

- **Top-left**: 10 px steward-color disc + steward name in Inter 500 14 px Ink. Format: `Sienna · thinking…` with verb in Verdigris italic; ellipsis animates at 1.5 Hz.
- **Top-right**: 200 px legend panel, Linen at 88 % alpha over the scene, 1 px Stone hairline, 8 px radius. Header `STEWARDS` in Inter 500 12 px Stone; four rows 28 px tall, each `[disc] [name] [count "n/7"] [status dot]`. Status dot Clay = alive, Stone = eliminated.
- **Top-right above legend**: sun/moon glyph (16 × 16) toggling palette. Below it: a small gear opening the drawer.
- **Bottom-right**: 32 px Bone disc with 1.5 px Clay stroke — the **re-center button** (§4.8).
- **Caption strip, bottom-centered**: Source Serif 12 px Ink at 32 % alpha — *"Don't let your mesh reach seven."*

No buttons beyond the legend rows (click to open the drawer), the sun/moon toggle, and the re-center glyph.

### 5.3 Hot-seat HUD

Same legend, plus:

- **Hover ghost sphere** at the cursor's nearest legal cell, 35 % opacity. Illegal cells render no ghost.
- **No place-node button** — click commits; the ghost is the affordance.
- **No pass button** — rules forbid skipping.
- **Keyboard**: `Space` commits hovered cell · `Esc` clears hover (and from end screen, opens drawer) · `Tab` cycles legal cells · `1`–`4` opens drawer to that slot · `R` restarts from end screen · `U` submits today's daily score.

### 5.4 Stewards drawer

Top-of-viewport panel sliding down 240 px. Bone background, 1 px Stone bottom hairline, drop shadow `0 8px 24px rgba(20,20,19,0.06)`. Title `Stewards` in Inter 500 22 px. Four columns, 16 px gutter.

Per slot:

- 3-way segmented control: `Computer · Agent · Human`.
- **Computer**: 4-way segment `Easy · Medium · Hard · Brutal`.
- **Agent**: mono 13 px endpoint URL input + optional bearer-token field + trailing ping badge (`42 ms` Verdigris on success, `timeout` Clay on failure).
- **Human**: 12-char `nickname` field; defaults `Sky`, `Rose`, `Mint`, `Sun`. Custom names persist to `localStorage` (and IndexedDB tier as ITP backstop) — **STRICTLY LOCAL**; never appear in filenames, leaderboards, or `BoardView` JSON.

Footer: `Cancel` (Inter 500 Ink, underline on hover) and `OK` (Clay filled, Bone text, 36 px tall, 16 px horizontal padding, 6 px radius). Tab order: `Cancel → Reset → OK`. `OK` applies on next game.

**Advanced panel** (collapsed by default; opens via `?` keystroke, three-finger tap, or the gear glyph) — runtime sliders for every knob in `docs/ITERATION_KNOBS.json`. Writes to `localStorage["cirisgame.knobs"]` JSON and triggers hot-reload. Available in both native and browser. The live-tunable visual knobs are grouped into seven families: **Space** (star density, star brightness, twinkle, drift, nebula, up hue, down hue · §3.8), **Spheres** (gas luma, gas sat, prism, core size · §3.3), **Glass** (IOR, thickness, reflect, rough · §3.2), **Layout** (marble size, peer distance, tube width, select glow · §3.1/§3.4/§4.8), **Post** (bloom · §2.3), and **Signets** (bright, active boost, size, distance · §3.10).

### 5.5 End screen — score table, no epigram

No epigram. The animation IS the message (§4.7 mourn / celebrate / wild).

- **Sub-line, top-center** (Inter 500 14 px Stone, tabular numerics): `Game 0247 · 31 turns · 4m 12s`.
- **Score table**, centered, 320 px wide, Linen at 88 % alpha, 8 px radius, 16 px padding. Four rows, one per steward, ordered ASCENDING by `permadead_count` (winners on top). Each row 36 px tall: `[12 px disc] [name 14 px Inter 500] [spacer] [count 18 px mono]`. Lowest-count row(s) — ties allowed — render with a Clay 12 %-alpha row background + 1 px Clay hairline + small Clay laurel glyph (8 px).
- **Special case — all zeros (WILD)**: replace the table with `The federation held.` in Source Serif 32 px Ink, the WILD animation underneath.
- **Below the table**: a 400 × 400 px Cream card with 2 px Clay border, 8 px radius, rendering the inline replay APNG (§10) auto-looping at 6 fps. Card materializes 600 ms after the score table settles. Two buttons under the card, 36 px tall: **Save replay** (download glyph) and **Share** (system-share glyph).
- **`New game` button**, bottom-center. Auto-restart at 18 s in WILD mode, 10 s otherwise, with a Verdigris countdown ring stroked around the button.

### 5.6 AI / Agent thinking indicator

Inline spinner in the legend row for the active Agent slot: 10 px Verdigris ring, 1.5 px stroke, 720 ms rotation. At ≥ 3 s, a mono `1.2s` latency readout appears. At ≥ 10 s, the row background tints Clay at 8 % and a small `skip` link appears (clicking forfeits to a random legal move).

For Computer slots the thinking pulse plays for the full 2 s regardless of whether the algorithm returned early — this is the brand rhythm guarantee.

---

## 6. Layout and Modes

### 6.1 Three modes

**Screensaver.** Four Computer stewards, ~2.5 s per move, animated. No chrome past the legend, the active-steward indicator, the sun/moon toggle, and the re-center glyph. Default for browser (no hash) and native (no flag). Idle hide at 3 s.

**Hot-seat.** Mixed roster, full HUD, click or tap to place. The Stewards drawer auto-opens for 8 s at game start. Active steward's slot pulses with a 1 px Clay hairline. URL `#hot-seat`; native `--mode hot-seat`.

**Headless.** Engine only, no renderer, no window. AI tournaments and CI smoke tests. Native flag `--headless`. Emits canonical JSON to stdout per move. Exit codes: 0 on M-1 survival, 1 on last-standing, 2 on illegal input.

### 6.2 Mode switching

Browser uses a hash router: `#screensaver`, `#hot-seat`, `#d=YYYY-MM-DD` (daily seed §8). No hash → screensaver. Switching modes from the drawer rewrites the hash without reloading; the engine resets state, the view rebuilds.

Native: `--mode {screensaver|hot-seat|headless}`. `--headless` implies no window and silences the renderer crate via feature gating.

### 6.3 Per-slot defaults (screensaver)

Heterogeneous by design — the default scene teaches play strength.

| slot | color | default | name | role |
|---|---|---|---|---|
| 1 | Sienna `#D97757` | Computer Easy | Sky | Loses charmingly; a five-year-old can spot the mistake. |
| 2 | Lapis `#6A9BCC` | Computer Medium | Rose | Greedy local; reasonable opening play. |
| 3 | Verdigris `#788C5D` | Computer Hard | Mint | Minimax depth-2 with mesh-size heuristic. |
| 4 | Kaolin `#E8E6DC` | Computer Easy | Sun | Closes the lattice; pairs with Sky for symmetry. |

Computer difficulty:

- **Easy** — legal-random with light "don't trigger your own destructive transition" pruning. ~50 ms.
- **Medium** — greedy local heuristic minimizing own `permadead_count` next turn. ~150 ms.
- **Hard** — minimax depth-2 with score-aware heuristic; understands the `r = 2` asymmetry. ~600 ms.
- **Brutal** — MCTS within the full 2 s budget. ~10–20 k playouts on mid-tier wasm.

All four consume a **uniform 2-second compute budget per move** (§7.5). The thinking pulse plays for the full 2 s regardless.

### 6.4 Game length

Total placements = `total_cells − total_perma_dead`. Forced play, no pass.

| board | cells | typical turns | comparable Go (total moves) |
|---|---|---|---|
| 3×3×3 | 27 | 22–26 | between 7×7 and 9×9 |
| 4×4×4 | 64 | 56–62 | ≈ 9×9 to 11×11 |
| **5×5×5** | **125** | **110–122** | **≈ 13×13** |
| 6×6×6 | 216 | 195–212 | between 13×13 and 19×19 |
| 7×7×7 | 343 | 315–340 | ≈ 19×19 |

M-1 (all-zero cooperative) frequency rises with board size — rare on 3×3×3, common on 5×5×5+.

### 6.5 Browser embed

Minimal `index.html`: `<title>`, viewport meta, `og:image` (1200 × 630), single `<canvas id="glcanvas">`, `<noscript>` fallback with a static screenshot. Touch parity is mandatory — tap = click, pinch = zoom + layer traversal, two-finger drag = camera orbit, two-finger twist = arcball.

### 6.6 Native CLI

```
ciris-game                                                      # screensaver, fullscreen, Esc quits
ciris-game --mode hot-seat                                      # windowed
ciris-game --headless --turns 100 --seed 42 \
  --p1 computer:easy --p2 computer:hard \
  --p3 agent:http://localhost:9000 --p4 human                   # CI / tournaments
```

`human` in `--headless` exits 64 (`EX_USAGE`).

### 6.7 Mobile portrait

Below 768 px wide and portrait aspect, activated on `matchMedia('(max-width: 768px) and (orientation: portrait) and (pointer: coarse)')`:

- Camera tilts to 60° down-angle (above-front rather than orbit).
- HUD collapses: legend pill at top respecting `env(safe-area-inset-top)`. Bottom-right thumb cluster — re-center glyph (44 px Bone disc, 2 px Clay stroke) + drawer hamburger (44 px disc, three 16 × 2 px Clay hairlines).
- Pinch-in/out unchanged.
- Twist becomes one-finger long-press (400 ms) + drag — two-finger gestures conflict with mobile system gestures.
- Minimap arcball appears smaller (`1.2 · N` instead of `1.6 · N`) to leave HUD space.
- Rotating to landscape REVERTS to the desktop pose.

A **flat top-down accessibility alternate** is available in the drawer for users who can't parse 3D depth cues: 90° look-down + 2-layer cross-section view + ARIA announcements on slice navigation.

---

## 7. AI-API

Every player type — Computer, Agent, Human — sees the same board through the same trait.

### 7.1 Player trait

```rust
pub trait Player: Send {
    fn name(&self) -> &str;
    fn pick_move(&mut self, view: BoardView) -> Result<Move, MoveError>;
}

impl BoardView {
    pub fn to_json(&self) -> String;
    pub fn to_ascii(&self) -> String;
    pub fn render_png(&self, opts: RenderOpts, after: Option<Move>) -> Vec<u8>;
    pub fn render_animation(&self, opts: AnimOpts, after: Option<Move>) -> Vec<Vec<u8>>;
}
```

### 7.2 JSON canonical schema

Includes: occupied cells with `(slot_id, default_name, pigment)` triples (never custom names), perma-dead positions, current steward, eliminated flags, turn number, legal moves, mesh sizes per steward, temperatures per mesh (numeric + word `calm/lively/hot/chaotic`), `last_move`, history. Stable mesh ids: smallest cell id lex-ordered within the mesh.

### 7.3 ASCII text dump

For LLMs with no image input. Five z-slices (5×5 of single-letter steward marks for N=5), legend, mesh table, temperature words. ~600 chars at N=5.

### 7.4 PNG and animation

PNG single frame: default **128 × 128** at N=5 (~8–12 KB). Optional 96, 192, 256. Camera default isometric forward; alternatives top, side. `after = Some(Move)` parameter clones state, applies, renders.

Animation: default **6 fps × 10 frames at 128 × 128** (~100 KB). For LLMs: today's models can see motion they can't infer from a single frame. PNG sequence as `Vec<Vec<u8>>`; optional APNG bundle.

Off-screen rendering via Bevy `Camera { target: RenderTarget::Image(handle), order: -1 }` whose target is `Image::new_target_texture(W, H, TextureFormat::Rgba8UnormSrgb, ..)`. A `Readback::texture(handle)` component fires `ReadbackComplete` with `Vec<u8>` for PNG encoding via the `image` + `png` crates.

### 7.5 Compute budget — uniform across player types

Every Computer difficulty AND every Agent has the same wall-clock budget per move: **2000 ms**. Humans have no time limit in hot-seat; Human in screensaver is rejected.

- Computer: returns early if the algorithm finishes; thinking pulse plays the full 2 s.
- Agent: HTTP timeout 2000 ms. On timeout, engine applies a random legal move (logged `forfeit:timeout`).

Headless can override: `--turn-budget-ms 100` for fast tournaments, `--turn-budget-ms 0` for as-fast-as-possible.

### 7.6 HTTP wrapper (out-of-process agents)

```
POST /move
  body: { view: BoardView, opts?: { include_png: bool, include_anim: bool, png_size: u32 } }
  response: { move: Move }

GET  /render/png?turn=N&after_move=cell-x-y-z&size=128
GET  /render/anim?turn=N&fps=6&frames=10&size=128
```

Optional bearer auth. Caps: 1 `pick_move` per turn per agent; 20 renders per turn per agent (cached per `(turn, move)`).

### 7.7 Unified ARIA-live region

A single canvas-adjacent `<div role="status" aria-live="polite">` DOM region serves every announcement surface (legend updates, daily-seed pill, sound captions, mobile-portrait slice changes, replay-saved toast). One region only — prevents NVDA / JAWS announcement races. Strings live in a registry, grade-5 reading level.

---

## 8. Daily Seed

### 8.1 The mode

URL `play.ciris.ai/#d=YYYY-MM-DD`. Missing hash falls through to screensaver. Everyone in the world plays the same seed on the same UTC day.

### 8.2 Seed derivation (pure-client, deterministic)

```
seed_bytes = SHA-256("ciris-daily-" || utc_date_iso8601)
rng = ChaCha8Rng::from_seed(seed_bytes[..32])
```

From this RNG, drawn in order:

1. **Difficulty roster** — shuffle `[Medium, Hard, Brutal]` across slots 2–4; **slot 1 is forced Easy** (the kid-on-ramp guarantee, non-negotiable).
2. **Pre-existing perma-dead count** — `K = 3 + (rng.next_u32() % 13)` → `K ∈ [3, 15]`.
3. **Pre-existing perma-dead positions** — `K` distinct cell indices via `rand::seq::index::sample(&mut rng, 125, K)`. Each cell starts in `PERMA_DEAD`, rendered with green ethereal mist per §3.6, attributed to no steward.
4. **`board_state_hash`** = SHA-256 over `(utc_date, K, sorted_perma_dead_indices)`. Anchors the day's identity.

Strategic implication: between 2.4 % and 12 % of the board is pre-claimed substrate, varying daily. The landing page reveals K so players know what they're walking into (`today: 7 substrate scars`).

### 8.3 Landing page

Cream background. Single Clay `44 px` `"Play today"` button dominating the fold. Source Serif 24 px `Today — 2026-06-27` above. Stone 14 px sub-line `next puzzle in 3h 12m`. Below the button, quiet Stone 13 px **coherence ribbon**: `today: 1,847 plays — 412 reached all-survivors` pulled from `GET /v1/daily/:date/aggregates`, refreshed at end-of-game. When the endpoint is unavailable the ribbon hides silently; seed and play work fully offline.

First-visit-ever: a 4-line panel (`place stones` / `don't let your mesh hit 7` / `same seed for everyone today` / `lowest permadead wins`) replaces the auto-dismiss toast.

### 8.4 Server-side replay verification

A Cloudflare Worker provides cheat-resistant aggregation. The engine compiles to **`wasm32-wasip1`** (pure CPU, no GPU, no bevy_render) as a workspace member `ciris-game-engine-core`.

```
POST /v1/daily/:date/submit
  body: { seed, move_log, slot_metadata, client_assertion: { permadead, all_survivors, board_state_hash, client_nonce } }
```

Pre-replay validation (before loading the wasi blob): `move_log.len() <= total_cells - K`, every cell index `< total_cells`, round-robin slot ordering, forfeit count `<= 4`. Failure → `204 + audit log`, no D1 increment.

Pass → load wasi blob into the V8 isolate (warm-cached), run `replay(seed, move_log) → Outcome { permadead, all_survivors, board_state_hash }`. Compare to `client_assertion`. Match → single-statement D1 transaction increments `plays` and `m1_count`. Mismatch or timeout (200 ms hard cap) → `204 + audit log`, no increment, no discrepancy leaked to client.

Client UX: end-screen renders immediately. Submit button labeled `"Submit today's score"`; on press a Stone 13 px pill appears with microcopy `"Score sending."` Client-side timeout 8000 ms. On success (`204 + aggregates GET refresh`): pill `"Score counted."` On mismatch: `"Score saved on this device."` (no accusation). On rate-limit (429): `"Daily ribbon full for this network. Score saved here."` On client timeout: `"Will retry on next visit."` Retry writes to `IndexedDB["cirisgame.daily.pendingSubmit.YYYY-MM-DD"]`; date-rollover retry still targets the original date (Worker accepts up to 24 h late).

ARIA-live announcements via §7.7. **NEVER** use "rejected", "invalid", "cheat", "verification failed", "tampering". The ribbon never carries `"verified"`, `"official"`, percentile rank, or `"you beat X%"` copy.

### 8.5 What's refused

No streak counter. No inviter parameter. No leaderboard percentile. No accounts. The Worker is a plain HTTP aggregator — no federation envelopes, no signed assertions, no public verifier endpoint. No moderation surface; the client never displays user-generated text.

---

## 9. Spectator URL

`play.ciris.ai/watch/{slug}` opens a read-only view of an in-progress game. The playing client opens a WebSocket to a Cloudflare Durable Object instance keyed by `slug`; the spectator client subscribes and renders identical Bevy scene from streamed `BoardView` JSON.

Stream payload per turn: the move, the resulting `BoardView` diff, and the temperature snapshot. ~1–2 KB per move at N=5. Auto-pause on tab blur. Server-side rate-limit: 10 streams per IP per hour.

Sub-features:

- Read-only legend mirror (no slot interaction, no drawer).
- Spectator HUD: small "watching {default_name}" banner top-left; room-id pill top-right.
- `?seed=YYYY-MM-DD` deep-link parsing so a spectator URL identifies which daily seed is being played.

Refusals: no chat, no viewer count badge, no friend-list / presence roster, no spectator-to-playing backchannel.

---

## 10. Shareable Replay

At game end, generate a **384 × 384 APNG** of the dispersal cascade plus score reveal. The animation centers on the highest-`|M|` event of the game (typically the final dispersal), 8 turns padded symmetrically, 6 fps, total **52 frames** = 8 turns × 6 frames + 4-frame score-reveal tail. WILD endings capture the final 12 turns + celebration with a 2-frame Source Serif title card prepend `The federation held.`

Encoded via the `apng` crate. Size cap **500 KB**; if exceeded, drop alternate frames and re-encode (max 3 passes). Deterministic from `(seed, move_log)` — re-render produces bit-identical output.

**Bundle on save** (atomic — all three): the `.apng`, a `-card.png` static cover frame for embed-stripper fallback (the `prefers-reduced-motion` default), and a `-receipt.txt` ≤ 240-char shape-glyph receipt `CIRISGame {seed} · {score} perma · ●{s} ■{l} ▲{v} ⬡{k} · cascade@{t} · {url}`. Filenames `cirisgame-{score}perma-{seed}-{YYYYMMDD}.{ext}` (WILD: `cirisgame-WILD-{seed}-{YYYYMMDD}.{ext}`).

**Save path**. Web: Blob → `URL.createObjectURL` → synthetic anchor `download` click (NEVER auto-triggered). Native: OS save dialog via Tauri 2's `dialog` plugin, defaulting to `~/Documents/cirisgame/`. **Share**: `navigator.share({ files: [apng, card, txt], text: receipt })` on mobile and modern desktop; clipboard fallback elsewhere. **Save** never touches the clipboard.

Keyboard `R` invokes Save. ARIA-live `"Replay saved. Receipt copied."` on Share, `"Replay saved."` on Save.

Refusals: no auto-download. No hosted gallery URL. No Twitter / Bluesky / X share-intent buttons. No "Share to X" / "Epic!" copy. Custom steward names NEVER enter the filename (the strict-local invariant per §5.4). No watermark by default.

---

## 11. Sound

OFF by default. Respects `prefers-reduced-motion: reduce` (auto-mute). Vendored at `assets/sound/` from **highest-quality CC0 picks on freesound.org** — filter `license:Creative Commons 0` + `format:wav`, sort by rating, audition top 5 per slot against the descriptive spec, vendor as OGG q=4. Per-sample `attribution.json` notes the source URL, author handle, sample ID for traceability.

| sample | duration | spec | search keywords |
|---|---|---|---|
| `placement_tick.ogg` | 3 ms | Clay-soft ~800 Hz tick | "soft wood tap", "marble place" |
| `pipe_extrude.ogg` | 220 ms | Bone-tinted breath shimmer | "glass swell", "wind chime stir" |
| `dispersal_settle.ogg` | 480 ms | low Verdigris hum settling | "deep bowl hum", "low gong tail" |
| `perma_dead_hum.ogg` | 1.4 s loop | sub-bass Stone | "drone bass loop", "subterranean hum" |
| `seat_pulse_breath.ogg` | 1.6 s loop | 0.6 Hz inhale/exhale (matches `atari.breathFreq`) | "soft breath loop", "ambient inhale exhale" |
| `dispersal_resolve.ogg` | sting | descend-up resolved chord | "warm minor resolved" |
| `wild_chord.ogg` | sting | Sienna/Lapis/Verdigris triad on WILD | "celebratory chord swell" |

Plus an **ambient layer** for screensaver mode: 3 × 22 s stitched OGG variants, crossfade on screensaver entry, ducking under sting events. Default volume bus 0.15.

**Bus topology**: `master = clamp(0, 1, foley + sting + ambient)`. Welcome chime gates on `audioContext.state === 'running'`.

**Welcome banner**: "Sound off — tap to enable" lives in the Stewards drawer label strip (passive, not over canvas), fires once per browser per first interaction. `localStorage["cirisgame.audio.welcomed"]` boolean.

**Captions** (deaf-blind audio replacement): every audible event has a corresponding ARIA-live polite announcement via §7.7. A visible caption strip toggle is available in the drawer for hearing-difference users who want visible cues.

**Web Audio API** via `web_sys::AudioContext`. **Native sound via `rodio`** (Tauri builds emit the same OGG bundle).

**Android**: optional haptic vibration on placement — `navigator.vibrate(8ms)`, opt-in via drawer.

Refusals: no quiet-hours auto-duck. No analytics. APNG `shareable_replay` exports never carry audio.

---

## 12. Persistence and Knobs

**Configuration sources** (precedence: later overrides earlier):

1. Compiled defaults.
2. `~/.config/ciris-game/config.toml` (native) or `localStorage["cirisgame.knobs"]` JSON (browser).
3. URL hash parameters: `#hot-seat&bloom.strength=0.7&temperature.alpha=0.08`.
4. Stewards drawer **Advanced panel** — runtime sliders for every knob; writes to (2), triggers hot-reload.

**Storage tiers** (browser):

- `localStorage["cirisgame.slots"]`, `["cirisgame.mode"]`, `["cirisgame.palette"]`, `["cirisgame.knobs"]`, `["cirisgame.audio.welcomed"]`.
- **IndexedDB** as backup tier (Safari ITP evicts localStorage after 7 days of no visits); on browser open, hydrate localStorage from IndexedDB if localStorage is empty.

**Hot-reload**:

- Native: `notify` crate watches `config.toml`; on change, validate against the knob schema in `docs/ITERATION_KNOBS.json`, apply.
- Browser: `storage` event fires across tabs; `hashchange` event fires on URL change. Same apply path.

**Knob schema**: every entry in `docs/ITERATION_KNOBS.json` carries `{ knob, defaultValue, range, effect, section, applyMode }`. `applyMode` is `live` (next animation frame), `next-move` (next placement), or `next-game` (next New Game).

---

## 13. Position in CIRIS Architecture

Application tier above CEWP's substrate trio (Verify / Persist / Edge). Not substrate. Pedagogical purpose: make M-1 viscerally playable for everyone from five-year-olds to LLMs.

### Sibling repos

- [CIRISAgent](../CIRISAgent/) — the agent; release CI mirrored here
- [CIRISRegistry](../CIRISRegistry/) — canonical CEG 1+4 primitive (FSD-002 §3.4)
- [CIRISServer](../CIRISServer/) — Rust sibling; bevy_panorbit_camera precedent
- [CIRISNodeCore](../CIRISNodeCore/) — federation consensus spec
- [CEWP](../CEWP/) — the holonomic-federation framing
- [CIRISVerify](../CIRISVerify/) — hardware-rooted identity verification
- [CIRISPersist](../CIRISPersist/) — federation directory substrate
- [coherence-ratchet](https://github.com/CIRISAI/coherence-ratchet) — the pedagogical grounding
