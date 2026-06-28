# CIRISGame — The "No-Crossing" Rule: A Quantitative Analysis

**Status: adopted 2026-06-28.** The rule is now live in the shipped engine
([`ciris-game-engine-core`](../../crates/ciris-game-engine-core/src/crossing.rs))
with forced-pass turn handling. The harness still reproduces the figures below by
disabling the engine's built-in enforcement and re-applying the rule as a
legal-move filter; the §1 predicate is now the engine's exact, shared
`crossing::is_crossing_illegal`, so analysis and engine cannot drift. The prose
below is preserved as the original proposal-stage analysis.

**Scope.** This document evaluates a *proposed* new rule — **"different-colour
tubes cannot cross"** — against the live rules, by simulation. It measures how the
rule changes game length, branching factor, score, the WILD rate, and
complexity, and renders a validity/playability verdict. It extends
[`GAME_THEORY.md`](./GAME_THEORY.md); read that first for the baseline taxonomy
and complexity framework.

**Status of the rule.** This is a *proposal only*. It is **not** implemented in
the shipped rules path. The analysis implements it purely as a **legal-move
filter** layered on top of the engine's real `legal_moves()`, inside a throwaway
harness:
[`crates/ciris-game-engine-core/examples/no_crossing_analysis.rs`](../../crates/ciris-game-engine-core/examples/no_crossing_analysis.rs).
`engine.rs` is untouched. Reproduce with:

```bash
cargo run --release -p ciris-game-engine-core --example no_crossing_analysis
```

---

## 1. The rule, precisely

A **bond** (the visual "tube") joins two face-adjacent same-colour live cells.
On the FCC lattice every face-neighbour displacement is a `(±1, ±1, 0)`-type
offset (`lattice.rs::NEIGHBOR_OFFSETS`), so every bond is the **face-diagonal of
a unit square** lying in an axis plane.

Two bonds **cross** iff they are the *two diagonals of the same unit square
face* — they intersect at the face centre. For a bond `P→Q` (with `Q = P + δ`,
`δ` having exactly one zero component), the opposite diagonal joins the square's
other two corners `R, S` (each reached from `P` by exactly one of the two unit
steps in `δ`).

> **Proposed rule.** A candidate placement of colour `X` at cell `C` is
> **illegal** if it would create any same-colour bond `C–N` (`N` a live colour-`X`
> neighbour) whose face's opposite diagonal `R–S` is *already* a live bond of a
> **different** colour `Y ≠ X` (both `R` and `S` live and colour `Y`). At most one
> diagonal per face may be a live cross-bond — **first-come, first-served.**

Implemented as `is_crossing_illegal(board, cell, color) -> bool` and intersected
with `legal_moves()`. Note the rule is **colour-dependent**: a cell forbidden to
Sienna can be perfectly legal for Lapis, because the bond it would create is a
different diagonal.

---

## 2. Method

- **Board:** 5×5×5 = 125 cells, **K = 0** starting perma-dead (clean
  `GameState::new` — no daily-seed pre-placement, so the baseline isolates the
  rule's effect; cf. GAME_THEORY which assumes K ∈ [3,15]).
- **Conditions:** (A) **Baseline** = engine `legal_moves()` as-is; (B) **Rule** =
  baseline ∩ `!is_crossing_illegal`.
- **Policies:** (i) **uniform-random** legal move; (ii) **Easy** — the
  screensaver's self-collapse-avoiding policy (`screensaver.rs::choose_move`,
  faithfully replicated): prefer cells that don't immediately grow the mover's own
  mesh to 7; fall back to any legal cell if all options self-collapse.
- **N = 1000 games per (condition, policy)** cell; seed varied deterministically
  per game (ChaCha8, separate game/AI streams).
- **Pass semantics under the rule.** The engine has no pass. Because the rule is
  colour-dependent, a steward with no legal cell does *not* end the game — the
  natural extension is to **pass** that steward and continue. A **global
  deadlock** is recorded only when a *full round* (all four stewards) passes with
  empties still on the board (nobody can place anywhere). "Passes" (colour-local
  stalls) and "global deadlocks" are reported separately.

---

## 3. Results

All figures are measured over N = 1000 games each. Length is in placements;
branching is the legal-move count per decision turn (0 on a pass turn under the
rule). Score is total perma-dead. Baseline branching is policy-independent (the
filter is off), so it is identical across both policies.

### 3.1 Headline table

| Metric (min / max / mean / median) | Baseline · Uniform | **Rule · Uniform** | Baseline · Easy | **Rule · Easy** |
|---|---|---|---|---|
| **Length** | 125 / 125 / 125.0 / 125 | 124 / 125 / **125.0** / 125 | 125 / 125 / 125.0 / 125 | 124 / 125 / **124.99** / 125 |
| **Branching** | 1 / 125 / 63.0 / 63 | 0 / 125 / **61.2** / 60 | 1 / 125 / 63.0 / 63 | 0 / 125 / **60.6** / 60 |
| **Branching (geomean)** | 47.23 | **43.91** | 47.23 | **42.25** |
| **Score** | 10 / 46 / 26.4 / 26 | 6 / 40 / **24.5** / 24 | 0 / 23 / 7.84 / 8 | 0 / 21 / **9.19** / 9 |
| **WILD rate** | 0.0% | 0.0% | 3.2% | **0.8%** |

### 3.2 Rule-binding and termination (Rule condition only)

| Quantity | Rule · Uniform | Rule · Easy |
|---|---|---|
| Turns that lost ≥1 legal move | **63.8%** | **67.1%** |
| Mean fraction of moves forbidden, per turn | 5.85% | 8.43% |
| Overall fraction of moves forbidden | **2.64%** | **3.36%** |
| Games with ≥1 colour-local pass | 27.5% | 40.6% |
| Mean passes per game | 0.36 | 0.62 |
| **Global deadlocks** (full round, all stuck) | **0.5%** | **1.4%** |

**Reading it.** The rule **touches almost every turn** (≈64–67% of turns lose at
least one legal cell) but **removes only a thin slice each time** (≈3% of all
moves overall). It is "broadly but lightly binding" — a constant low-grade
tactical constraint, not a chokehold. Length is essentially unchanged (median
125; mean ≈ 125.0): the game still fills the board to saturation in ≈99% of games.
Colour-local passes are common (a steward occasionally has to skip a turn) but
**global deadlock is rare** (0.5%–1.4%) and happens only deep in the endgame when
the few remaining empties are each cross-blocked for every colour.

---

## 4. Complexity

### 4.1 State-space complexity

The rule does **not** change the per-cell static state set
(`{Empty, Live×4, PermaDead}` = 6), so the crude ceiling is unchanged:
`6^125 ≈ 10^97.3` (GAME_THEORY §2.2). The rule adds a **pairwise face
constraint** (no two crossing live bonds of different colour share a face). This
prunes the reachable set, but weakly: only ≈3% of placement *moves* are ever
forbidden, so the forbidden *positions* are a small minority. The realistic
reachable estimate from GAME_THEORY (≈ **10^85–10^90**) is **unchanged to within
its own ±5-order uncertainty** — the rule shaves a fraction of an order of
magnitude at most. By state-space, the game remains **Go-13×13-class**.

### 4.2 Game-tree complexity

Game-tree ≈ `b^d`. With K = 0 the placement base is the full `125! = 10^209.3`,
whose 125th root is exactly the measured baseline geometric-mean branching
(47.23) — a clean cross-check. Computed from the measured numbers (geometric mean
is the product-relevant average):

| | Baseline | Rule · Uniform | Rule · Easy |
|---|---|---|---|
| Mean length d | 125.0 | 125.0 | 125.0 |
| Geomean branching b | 47.23 | 43.91 | 42.25 |
| **Game-tree ≈ b^d (log₁₀)** | **10^209.3** | **10^205.3** | **10^203.2** |
| (arith-mean b variant) | 10^224.9 | 10^223.3 | 10^222.8 |

The rule shrinks the game tree by only **≈4–6 orders of magnitude on a ~209-order
axis — under 3% on the log scale**. Negligible. (These K = 0 figures sit a little
above GAME_THEORY's headline ~10^190, which assumes K ≈ 9 → `115!`; rescaling for
K reproduces that figure. The *relative* baseline→rule shift is what matters here
and is K-independent.)

### 4.3 Comparison to Go (consistent with GAME_THEORY §2)

| Measure (log₁₀) | Go 9×9 | Go 13×13 | **CIRIS baseline** | **CIRIS + rule** | Go 19×19 |
|---|---|---|---|---|---|
| State-space | 38 | 79 | ~85–90 | ~85–90 | 170 |
| Game-tree | ~85 | ~210 (est) | ~205–209 | ~203–205 | 360–505 |

The no-crossing rule leaves CIRISGame **firmly in the Go-13×13 complexity class
on both axes**. It does not move the game between complexity tiers.

---

## 5. Validity & playability verdict

**The game remains valid and playable under the no-crossing rule.**

- **Termination.** The game still terminates essentially always. Length stays
  pinned at the saturation value (median 125, mean ≈ 125.0); ≈99% of games fill
  the board exactly as the baseline does. The monotone "fill to saturation"
  property (GAME_THEORY's key structural fact — empties only ever decrease) is
  preserved, so the rule cannot create infinite play; the worst case is an early
  freeze, and even that is rare.
- **No degenerate forcing.** Branching stays **healthy**: geomean ≈ 42–44 vs the
  baseline 47, mean ≈ 61 vs 63. It is nowhere near collapsing to ~1 — players
  retain dozens of choices nearly every turn. End states stay varied (score
  spreads of 6–40 / 0–21 across games, comparable to baseline 10–46 / 0–23).
- **Deadlock is rare, not absent.** True global deadlock occurs in only 0.5%
  (uniform) to 1.4% (Easy) of games, always in the deep endgame. Colour-local
  passes (a single steward skipping a turn) are more frequent (28–41% of games
  see at least one) but are a *feature* of the design space, not a breakdown — and
  they require a pass rule to be added if the proposal is ever adopted, since the
  engine currently has none.
- **WILD remains reachable** but **rarer** (Easy: 3.2% → 0.8%). The rule
  occasionally strips a steward of their *safe* extension cells, nudging them
  toward a self-collapse they would otherwise avoid — which is exactly why the
  Easy-policy mean score **rises** under the rule (7.84 → 9.19) even though the
  uniform-policy mean *falls* (26.4 → 24.5). WILD is harder to hold, but still
  attainable.

**Qualitative shift.** The rule introduces the game's **first genuine indirect-
attack lever**. GAME_THEORY §1.6 notes the base game has *no* way to raise an
opponent's temperature — "no atari-the-opponent," purely self-inflicted risk.
Under no-crossing, an opponent's bond `R–S` now **forbids** your crossing bond
`C–N`: a player can deliberately lay a cross-bond to deny an opponent a
connection or, in the endgame, to strip their safe cells and pressure them toward
collapse. This converts CIRISGame from *purely constructive* toward *constructive
+ positional denial* — more tactical, with a new "tube-fencing" mini-game over
shared faces — while keeping the core invariants (no capture; self-only collapse
of one's *own* mesh; lowest-score-wins; cooperative WILD attractor) intact.
Quantitatively the change is **mild**: length unchanged, branching down ≈7%, game
tree down <3% on the log axis, score shifted by single-digit perma-dead, WILD
preserved but ~4× rarer under skilled play.

**Bottom line.** A safe, low-risk addition that *enriches* tactics (adds denial,
slightly raises the floor on achievable scores, makes WILD a harder cooperative
target) without threatening termination, variety, or the complexity class — with
the one caveat that adopting it requires also adding an explicit **pass rule** to
handle the ≈28–41% of games where a steward is briefly cross-blocked.

---

*All CIRISGame figures are measured by the harness cited above (N = 1000
games/cell, ChaCha8-seeded, reproducible). Go figures are cited in
[`GAME_THEORY.md`](./GAME_THEORY.md) §2.*
