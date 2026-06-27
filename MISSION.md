# MISSION — CIRISGame

**Crate identifier**: `ciris-game-engine`.
**Repo**: [`CIRISAI/CIRISGame`](https://github.com/CIRISAI/CIRISGame).
**License**: AGPL-3.0-or-later, matching `CIRISServer` / `CIRISAgent` / `CIRISRegistry`.
**Cross-references**: [`CIRISAgent/ACCORD.md`](https://github.com/CIRISAI/CIRISAgent) §M-1 (meta-goal grounding); [`CIRISRegistry/FSD/FSD-002`](https://github.com/CIRISAI/CIRISRegistry) §3.4 (CEG 1+4 attestation primitive); [`coherence-ratchet`](https://github.com/CIRISAI/coherence-ratchet) (the pedagogical grounding); [`CEWP/README.md`](../CEWP/README.md) (holonomic federation framing).

---

## 1. Mission

### 1.1 Meta-Goal

CIRISGame serves **M-1** (*"Promote sustainable adaptive coherence — the living conditions under which diverse sentient beings may pursue their own flourishing in justice and wonder"*) by being **the in-game visceral conveyance of CIRIS's coherence-collapse work** — collapse is generative, not annihilating. A mesh that over-grows undergoes a destructive transition, and from the destruction smaller groups are *created*. The substrate carries permanent traces of the failure (perma-dead cells everyone must route around), and the player keeps playing with whatever survived. Players who avoid coherence collapse altogether share the cooperative WILD ending — the federation held.

That is M-1 made playable. A five-year-old can hold the rule; an LLM can play it from the same JSON every human sees; a federation steward can witness their network's resilience as a dispersed pair surviving an over-grown mesh. **One rule, three audiences, one substrate**.

### 1.2 What CIRISGame is

A turn-based 3D game on a rhombic-dodecahedral lattice. Four stewards take turns placing cells; same-color adjacency auto-forms meshes via glass pipes; a mesh that hits seven cells **destructively transitions** and disperses into live pairs + perma-dead spacers; lowest perma-dead count at game end wins; all-zero is the cooperative ending.

A single Rust crate built on Bevy 0.19, two WASM artifacts (WebGPU + WebGL2), native via Tauri 2 (macOS / Linux / Windows), headless feature-gated for CI and AI tournaments.

Three player types per slot — **Computer** (4 difficulty levels), **Agent** (HTTP), **Human** — all consuming the same `BoardView` representation. Default mode is screensaver: four Computers on uniform 2-second compute budget, scene shimmering quietly on a shelf.

### 1.3 What CIRISGame is not

- Not a simulation engine — `CEWP/FSD/SIMULATION_ENGINE.md` owns that scope.
- Not a benchmarking tool — `CIRISBench` owns that scope.
- Not a federation primitive — does not author any substrate row, signs no envelopes, holds no federation-wide authority. It consumes the CEG vocabulary (§3) but contributes nothing back.

---

## 2. The lattice and the rule

### 2.1 The lattice — rhombic-dodecahedral honeycomb

The play space is the FCC Voronoi tessellation: cells positioned at integer lattice points, each cell face-adjacent to twelve neighbors (the displacements where exactly two of `(±1, 0)` are non-zero and one is zero). Space-filling, isotropic, no preferred axis, no edge-effect favoritism. The starter board is **5×5×5** (125 cells) — large enough for the M-1 cooperative ending to be reachable with good play, small enough to learn the geometry in a single game.

The lattice is **holonomic by construction**: there is no central cell, no canonical viewing axis, no privileged steward seat. Re-root the camera anywhere and the game shape is preserved.

### 2.2 The rule — don't let your mesh hit seven

A steward owns one or more *meshes* — the connected components of their colored cells. Each turn the steward places one cell. Same-color adjacent cells auto-link via glass pipes (`delegates_to` envelopes in CEG terms). A mesh that reaches **seven cells** undergoes a **destructive transition**: the mesh dies, and from the destruction smaller groups are *created*. The steward keeps playing with whatever survived.

**Dispersal mechanic** (canonical algorithm in `docs/DESIGN_BRIEF.md` §4.6). For a dead mesh of `N` cells, with `k = N ÷ 3` and `r = N mod 3`:

- `k` **live pairs** of the steward's color — 2k cells re-spawned as separated 2-cell meshes.
- `k` **perma-dead spacers** — cells turn neutral, rendered as green ethereal mist forever, cannot be reclaimed by any steward.
- Remainder: `r = 0` → done; `r = 1` → one extra perma-dead; `r = 2` → one extra live pair at the boundary.

The transition runs over two turns: **black mist for one turn** (the death moment, dramatic), then dispersal into **live pairs + perma-dead spacers carrying green mist forever**.

**Scoring**: each steward's score is the **total perma-dead stones they created** across the game — magnitude matters. An 8-cell explosion costs 2; a 13-cell costs 5. Lowest score wins; ties allowed. If every steward ends at zero — nobody triggered a coherence collapse — the federation held; the M-1 *sustained adaptive coherence* ending fires.

**Atari**: a mesh of **six** cells is one placement from collapse. The agent particles around its cells (DESIGN_BRIEF §3.9, §4.9) carry this as a discrete visual state — synchronized orbits, locked-phase breathing at 0.6 Hz, a faint Verdigris foreshadowing ring. Players scan the board for atari at a glance, the way Go players do.

Why dispersal with permanent substrate damage, not erasure: erasing the steward's color would model collapse as catastrophic *for the agent only*. Dispersal models it as catastrophic *for the substrate* — perma-dead cells are a permanent audit trail every player must route around. The steward's identity survives; the trust topology they built does not. CEG-faithful: every `delegates_to` envelope in the dead mesh gets a `withdraws` cascade; the cells remain in the lattice as neutral substrate.

The threshold seven is steward-chosen, not derived: small enough that a five-year-old can count to it without losing the count, large enough that two 6-cell meshes joined yield a 13-cell explosion (5 perma-dead stones — heavily punitive), large enough to require *thinking ahead* about which adjacencies will auto-link on the next placement.

---

## 3. Trust shape — CEG envelopes are the move surface

Every game-state-changing event lives as an envelope in the CEG 1+4 grammar (CIRISRegistry FSD-002 §3.4). Placement is a `Scores` envelope on the placing steward's identity; pipe formation is a `DelegatesTo` between the two cells; dispersal is a `Withdraws` cascade over the dead mesh's delegations. The engine emits these envelopes internally; future iterations may persist them to the federation substrate when CIRISGame cohabits with a CIRIS Agent.

Pre-cohabitation, the envelopes are an internal representation: faithful to the spec but not signed, not federated, not externally observable. The point is that the move format is the federation's format. When the cohabitation arc lands (see §5), nothing in the move surface changes — only the persistence target.

---

## 4. Surface — what CIRISGame exposes

### 4.1 Three player types via one trait

```rust
pub trait Player: Send {
    fn name(&self) -> &str;
    fn pick_move(&mut self, view: BoardView) -> Result<Move, MoveError>;
}
```

- **Computer** — built-in heuristic AI at four difficulty levels (Easy / Medium / Hard / Brutal), uniform 2-second compute budget.
- **Agent** — HTTP wrapper; external AI consumes `BoardView` JSON, returns a `Move`. Same 2-second budget; timeout forfeits to a random legal move.
- **Human** — mouse/touch/keyboard input; no time limit in hot-seat.

The trait signature is the same for all three. The recursive Golden Rule binds them equally: **no privileged "developer" player slot, no preferential access to game state, no Computer difficulty unavailable to a human opponent**. The screensaver's four Computers play under exactly the same constraints a Human or Agent would face.

### 4.2 BoardView — four output formats, same canonical state

Every player sees the same board through one of four representations:

- **JSON** — canonical wire format; cells with `(slot_id, default_name, pigment)` triples (never custom names), perma-dead positions, current steward, legal moves, mesh sizes, temperatures with numeric value + word label.
- **ASCII** — text dump for LLMs without image input; five z-slices, legend, mesh table, temperature words.
- **PNG** — single frame, default 128×128 at N=5 (~8–12 KB).
- **Animation** — 6 fps × 10 frames default (~100 KB); for LLMs to see motion they can't infer from a single frame.

All four are produced by the same Bevy off-screen rendering path (one render target, one rasterizer); no second renderer, no headless fallback. Format parity is the Golden-Rule bite — humans get what AIs get and vice versa.

### 4.3 Three modes

- **Screensaver** — four Computers, leisurely 2.5 s/move, no chrome past the legend. Default.
- **Hot-seat** — humans / mixed, full HUD, click or tap to place.
- **Headless** — engine only, no renderer, no window; CI / tournaments.

Mode is the only top-level state. URL hash routing in browser (`#screensaver`, `#hot-seat`, `#d=YYYY-MM-DD`); CLI flag in native.

### 4.4 Daily seed

A pure-client deterministic seed per UTC day produces an identical opening cohort everyone in the world plays. The seed sets the AI difficulty roster (slot 1 is forced Easy — the kid-on-ramp guarantee), the count `K ∈ [3, 15]` of pre-existing perma-dead spots, and their positions. The substrate-scar count varies daily; some days reward conservative split-mesh play, others reward early commitment.

Score submission goes to a Cloudflare Worker that compiles the engine to `wasm32-wasip1` and replays `(seed, move_log)` to compute ground-truth — closing the cheat surface without the federation envelope path. The Worker is a plain HTTP aggregator; no signed assertions, no public verifier endpoint, no leaderboard percentile.

### 4.5 Spectator URL

`play.ciris.ai/watch/{slug}` opens a read-only view of an in-progress game via a WebSocket `BoardView` JSON stream. Pairs with the daily seed for ambient AI-tournament viewing.

### 4.6 Shareable replay

At game end, a 384×384 APNG centered on the dispersal cascade plus score reveal, save-or-share via the OS-native flow. Deterministic from `(seed, move_log)`. No hosted gallery, no Twitter intent, no auto-download.

---

## 5. Cohabitation trajectory

CIRISGame ships as a standalone Bevy crate today. The substrate dependencies are intentionally light:

- The CEG vocabulary from CIRISRegistry FSD-002 §3.4 — internal representation only.
- The CIRIS visual identity tokens (Anthropic Clay, Bone-cream, Verdigris) — shared with the broader CIRIS web surface.

Future cohabitation arc — when CIRISAgent grows an "agent-as-tutor" surface, CIRISGame folds in-process as a teaching artifact. At fold:

- Move envelopes persist to the federation substrate via `CIRISPersist`.
- Player identity may bind to a CIRIS-federation identity (Sovereign or Registered path per CIRISRegistry MISSION §1.1), enabling federation-attested play history.
- The agent-as-tutor can demonstrate rule consequences in the same lattice it talks the human through.

Pre-fold, none of this is committed. The game's job is to ship a beautiful, playable, viscerally-correct conveyance of M-1 — the substrate fold is the agent's surface, not the game's.

---

## 6. Recursive Golden Rule — how it bites in the game

The Golden Rule ("we owe ourselves what we offer to others; no principal is exempt from the standard they impose on others") is operational here at specific primitives:

- **All player types share one `BoardView` trait.** No Computer slot sees state that an Agent or Human cannot. No Agent slot can bypass the four AI-API output formats. A developer-controlled `Computer` cannot quietly outgrow a human opponent because every player type is rate-limited by the same 2-second compute budget on the same legal-move set.
- **The `permadead_count` is symmetric across stewards.** No steward's perma-dead counts less than another's; no scoring bonus for any role; no AI player exempt from the same destructive transition mechanic.
- **The daily seed is identical for everyone.** Every player on Earth at the same UTC date sees the same opening substrate scar layout. No "tournament seed" privilege.
- **Custom human nicknames stay strictly local.** They never enter filenames, BoardView JSON, leaderboards, OG cards, or spectator streams — the same strict-local invariant a federation principal would expect of their PII.

---

## 7. Open questions

- **Resonance coefficient calibration.** The α / β / γ / δ defaults in DESIGN_BRIEF §4.1 are starting points pending an empirical sweep over `|M|, |N| ∈ [1, 14]` to confirm the resonance trigger fires within the expected mid-game band.
- **Algorithm A on bigger boards.** Morton-greedy dispersal is verified on 5×5×5. Pathological topologies on 6×6×6+ may exhibit excessive `SINGLE_LIVE` fallbacks; needs simulator sweep before committing to A on the larger presets.
- **D1 vs KV** for the daily-seed counter store. D1 single-statement transactions are committed; KV's atomic increment is the alternative if dogfood reveals contention.

---

## 8. References

### Within CIRISGame
- [`docs/DESIGN_BRIEF.md`](docs/DESIGN_BRIEF.md) — the full visual + interaction spec
- [`docs/ITERATION_KNOBS.json`](docs/ITERATION_KNOBS.json) — every tunable value with `applyMode`
- [`docs/BACKLOG.md`](docs/BACKLOG.md) — shipping order, dependencies, no time estimates
- [`CLAUDE.md`](CLAUDE.md) — session bootstrap for fresh Claude sessions

### Federation context
- [`CIRISAgent/ACCORD.md`](https://github.com/CIRISAI/CIRISAgent/blob/main/ACCORD.md) §M-1 — meta-goal grounding
- [`CIRISRegistry/MISSION.md`](https://github.com/CIRISAI/CIRISRegistry/blob/main/MISSION.md) — Sovereign vs Registered attestation paths
- [`CIRISRegistry/FSD/FSD-002_FEDERATION_SURFACE.md`](https://github.com/CIRISAI/CIRISRegistry/blob/main/FSD/FSD-002_FEDERATION_SURFACE.md) §3.4 — the CEG 1+4 primitive
- [`CIRISNodeCore/CIRIS_FEDERATION.md`](https://github.com/CIRISAI/CIRISNodeCore/blob/main/CIRIS_FEDERATION.md) — the system claim
- [`CEWP/README.md`](../CEWP/README.md) — the holonomic-federation framing

### Pedagogical grounding
- [`coherence-ratchet`](https://github.com/CIRISAI/coherence-ratchet) — the empirical work the game makes playable
- [`ciris-website/src/app/coherence-collapse-analysis`](../ciris-website/src/app/coherence-collapse-analysis) — public-facing framing
