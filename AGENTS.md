# AGENTS.md — road to CIRISGame 1.0

Operational guide for any agent working toward a shippable **1.0 of the live
page** at <https://cirisai.github.io/CIRISGame/>. Read this with
[`CLAUDE.md`](./CLAUDE.md) (orientation + locked invariants),
[`docs/DESIGN_BRIEF.md`](./docs/DESIGN_BRIEF.md) (the spec), and
[`docs/BACKLOG.md`](./docs/BACKLOG.md) (the full forward build plan). This file
is the **1.0 cut**: the subset of the backlog that makes the deployed page a
real, playable, beautiful game — plus the dev harness you need to verify your
work before you push.

> CLAUDE.md's **locked list** and **refusals** still bind. Don't touch steward
> colors, the rule of seven, M-1 framing, Algorithm A, AGPL, or add any of the
> refused social/share/account surfaces.

---

## Current deployed state (2026-06)

What renders **today** on the github.io page:

- ✅ WebGPU wasm boots and renders (HDR + Bloom + AgX) full-window.
- ✅ Self-running **screensaver**: four AI stewards play forever, glass shells +
  emissive pigment cores, gas pipes, dark warm void.
- ✅ Front-of-house **text renders** (Inter / Source Serif 4 / JetBrains Mono are
  now shipped + loaded — see `fonts.rs`). The intro + setup wizard are legible.
- ✅ **Escape** dismisses the whole front-of-house straight to `Playing`.
- ✅ **Cursor-attention**: hovered cell glows + the plasma rushes inward
  (`hover.rs`). Backend-independent.

What is **not** there yet (the 1.0 gaps):

- ❌ The empty-cell cage still reads as a **flat wireframe**, not flowing plasma.
- ❌ **No human play.** It is a screensaver; you cannot place a stone. Roster
  config from the wizard is not yet what drives gameplay.
- ❌ **Modes are cosmetic.** `AppMode`/`RosterConfig`/`ViewConfig` are wired into
  the wizard but gameplay ignores them (always all-AI screensaver).
- ❌ The wizard is legible but **rough** (layout, stepper clarity, applied a11y).
- ❌ No endgame ceremony, audio, daily seed, native build, or real translations.

---

## Dev + verify harness (use this — it is how you avoid blind deploys)

**Build / run locally (native, Metal):**
```bash
cargo run -p ciris-game-engine                 # windowed app (default features)
cargo build -p ciris-game-engine --no-default-features --features webgl2 \
  --target wasm32-unknown-unknown              # deploy-shape compile gate
```

**Closed-loop screenshots (you CAN see your work).** `capture.rs` auto-saves
`screenshots/run-NNN.png` at fixed launch-relative times (native only;
gitignored). Run the app in the background, wait, kill it, then `Read` the PNG:
```bash
(cargo run -p ciris-game-engine >/tmp/run.log 2>&1 &) ; sleep 22 ; \
  pkill -f target/debug/ciris-game
```
To see a **full board** fast, temporarily lower `STEP_SECS` in `screensaver.rs`
(e.g. `0.35`) and/or change `capture.rs` `pending` times — **revert both before
committing**. A 15 KB PNG = magenta/error; ~1 MB = a real frame.

**Browser verification (real WebGPU + real input).** There is no browser in the
agent env by default, but Chrome can be driven from the shell via
`puppeteer-core` (headed → guaranteed Metal/WebGPU). This is the only way to
verify in-browser font loading, shader behavior, **and hover/click** (puppeteer
can `page.mouse.move` / `.click` / press Escape, then screenshot). Pattern lives
in `/tmp/cdp/` from prior sessions; launch with
`['--enable-unsafe-webgpu','--use-angle=metal']`, `headless:false`.

**Deploy = push.** `.github/workflows/pages.yml` redeploys on every push to
`main` (~12 min: the WebGPU wasm build dominates). Verify the run goes green
(`gh run list --workflow=pages.yml`), then re-check the live page.

### Non-obvious gotchas (each cost a debug cycle once)

- **Magenta render**: a mix of HDR and non-HDR cameras renders magenta on Metal
  (bevy #6754). BOTH cameras must be `Hdr` + `Msaa::Off`, always. HDR
  Rgba16Float can't multisample, and HDR doesn't work on WebGL2 in-browser
  (#7352) — which is why the deploy target is **WebGPU**, not WebGL2.
- **`RangeError: failed to grow table by 4`**: Ubuntu's `apt` binaryen mis-binds
  the `__wbindgen_externrefs` export under `-Oz`. `pages.yml` pins the official
  **binaryen version_130** tarball. Do not revert to apt.
- **Blank UI text**: Bevy 0.19 text is Parley. `FontSource::Family("Inter")`
  resolves only if a `Font` asset with that embedded family name is loaded and
  its handle kept alive (`fonts.rs`). On wasm there are no system fonts, so
  shipping the faces is mandatory.
- **Shaders must be WebGL2-safe AND WebGPU-valid**: no compute, no dynamic
  loops, vec4-aligned uniforms, `AlphaMode::Blend` for translucent. Validate by
  compiling the webgl2 wasm target.
- Custom human nicknames stay **strictly local** (never in filenames, BoardView,
  leaderboards, OG cards, streams).

**Gates before every push:** `cargo fmt --all --check` ·
`cargo clippy -p ciris-game-engine --all-targets -- -D warnings` · the webgl2
wasm build above. Push only with all three green and a screenshot you've looked
at.

---

## Road to 1.0 — prioritized

Acceptance criteria are concrete and visual/behavioral. BACKLOG tier numbers in
`[brackets]` point at the deeper spec.

### P0 — make it a real, beautiful, playable game

**1. Turn the wireframe into proper plasma (the "prayer ball").** `[5]`
The empty-cell cage is a per-cell `LineList` of straight 1-px edges
(`geometry::wireframe_mesh`, painted with `PlasmaMaterial` from `plasma.rs` /
`assets/shaders/plasma.wgsl`). Color animation alone can't escape "wireframe"
because 1-px lines stay sharp. Pick an approach and commit:
  - (a) **Glowing strands**: replace edge lines with thin tube/`Capsule3d`
    geometry (or a fattened line technique) so the plasma shader has surface to
    bloom on; push peaks into HDR so Bloom bleeds them into soft filaments.
  - (b) **Fresnel shell**: a translucent sphere shell (or rhombic shell) with a
    flowing fresnel/plasma surface so the negative space itself glows — closer
    to a literal woven prayer ball.
  - Keep it **WebGL2-safe + WebGPU-valid**, translucent, and visible from any
    angle as the §4.8 fly-through passes through it.
  - **Acceptance**: in a Chrome screenshot it reads as a soft, flowing,
    ethereal cage — not a boxy grid — at rest, and the hover rush is obvious.

**2. Human play — make it actually playable.** `[6, 7]`
Today gameplay is `screensaver.rs` (all-AI). Add real input for human seats:
  - Reuse `hover.rs` picking (cursor → cell) for **click-to-place** on the
    current human steward's legal cells; highlight legal moves; reject illegal
    silently (no "invalid" copy — see §7.7).
  - A turn loop that, in `Playing`, advances **per `RosterConfig`**: human seats
    wait for a click, Computer seats use the AI (`[11]`), Agent seats use the
    API (`[12]`). Screensaver becomes the *attract* mode, not the only mode.
  - The §4.6 **player-chosen dispersal**: when a mesh hits seven, let the human
    lay out the crater's dead cells into any legal shape and place a stone the
    same turn. (Engine core already supports the rule; this is the UI.)
  - **Acceptance**: a human can play a full game to an end state in the browser.

**3. Wire the modes.** `[19]`
`AppMode` (Human/Agent), `RosterConfig` (4 seats × kind × difficulty), and
`ViewConfig` already exist (`state.rs`) and the wizard edits them — but gameplay
ignores them. Make `Playing` consume them. Honor `?mode=agent`. Add a mode
router (`#screensaver` / `#hot-seat` / `#d=DATE`) so the attract screensaver and
a real hot-seat game are both reachable.
  - **Acceptance**: choosing seats/difficulty in the wizard changes who plays;
    `?mode=agent` reframes step 2 as BoardView delivery.

**4. Improve the wizard.** `[8, 14]`
It's legible now but rough. Give it a clear 3-step stepper with obvious
Back/Next/Start, sensible spacing (the 29-language grid currently dominates —
make it a compact picker), apply `ViewConfig.text_scale` to the type scale, wire
RTL (`ar`/`fa`/`ur`), and make every control's selected state unmistakable.
Editable steward names per slot, **strictly local**.
  - **Acceptance**: a first-time player completes setup without confusion;
    Escape still always works as the escape hatch.

### P1 — depth + drama

**5. Hover/click feedback polish.** Tune `hover.rs` constants
(`PICK_RADIUS`, `TAU`, `GLOW_LUX`) against Chrome; add click ripple / placement
confirmation. **6. Endgame ceremony** `[9, 10]`: collapse → black mist → green
perma-dead mist, Algorithm A dispersal cascade, the WILD M-1 all-survive ending.
**7. Audio** `[16]`: 7 CC0 OGGs, Web Audio, `prefers-reduced-motion` auto-mute,
caption strip. **8. Camera remainder** `[7]`: minimap-in-sphere arcball, steward
seats ring at `1.80·N`, recenter, first-twist theater `[22]`.

### P2 — reach

**9. Real i18n**: replace English placeholders in the 29 `.ftl` files; add
**Noto** fallback faces for non-Latin scripts (current fonts are Latin-only, so
CJK/Arabic/Devanagari render tofu). **10. Daily seed + Worker** `[23, 24, 25]`:
wasm32-wasip1 replay, Cloudflare Worker aggregator (plain aggregator — no
federation attestation). **11. Native/Tauri** `[17]` packaging. **12. Mobile +
flat top-down a11y** `[20, 21]`.

### P3 — polish + perf

**13. Bundle/perf**: wasm is ~34 MB raw / ~10.5 MB gzip; trim (GitHub Pages is
gzip-only, no brotli). Implement the §2.3 two-camera **selective bloom** split
(TODO already marked in `render.rs`). **14. CI hardening**: add a post-build
**wasm smoke test** that asserts `__wbindgen_externrefs` binds to the externref
table (regression guard for the binaryen bug). **15. Spectator** `[26]`.

---

## Definition of 1.0

A visitor opens the page and sees a beautiful flowing prayer-ball attract mode;
can start a hot-seat or vs-AI game from a clear wizard and **play it to an
end**, including a collapse and the M-1 ending; an agent can drive it via
`?mode=agent`; it loads on any WebGPU browser; and nothing traps the player.
Daily seed, spectator, native, and full i18n are post-1.0 (`docs/BACKLOG.md`).
