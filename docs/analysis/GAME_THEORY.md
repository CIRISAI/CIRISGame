# CIRISGame — A Game-Theoretic Analysis

**Scope.** This document places CIRISGame within the standard game-theory
taxonomies, estimates its complexity against Go, derives expected game duration
and compute cost, and catalogues its structurally novel properties. Every number
is accompanied by its assumptions; estimates are labelled as such.

**Grounding.** The analysis is built from the *actual* rules as implemented in
`crates/ciris-game-engine-core/`, not from a generic abstraction. The load-bearing
facts, with source:

| Fact | Value | Source |
|---|---|---|
| Board | 5×5×5 = 125 cells (default) | `lib.rs::DEFAULT_BOARD_N`, `board.rs::Board::new` |
| Lattice | rhombic-dodecahedral (FCC); 12 face-neighbors; displacement = two ±1, one 0 | `lattice.rs::NEIGHBOR_OFFSETS`, `is_face_adjacent` |
| Stewards | 4, fixed slot order, own colors | `board.rs::Steward` |
| Collapse threshold | mesh size 7 | `lib.rs::COLLAPSE_THRESHOLD` |
| Atari | mesh size 6 | `lib.rs::ATARI_SIZE` |
| Cell states | Empty / Live(4) / TempDead(transient) / PermaDead | `board.rs::CellState` |
| Score | total perma-dead created per steward; **lowest wins, ties allowed** | `engine.rs::Outcome`, `apply_move` |
| Cooperative ending | all-zero → `all_survivors` (WILD / M-1) | `engine.rs::Outcome::all_survivors` |
| No capture | a cell dies only via its **own** steward's collapse | `engine.rs::apply_move` (no opponent removal path), DESIGN_BRIEF §4.10 |
| Forced play | no pass; turn = one placement on an Empty cell | `engine.rs::apply_move`, `legal_moves` |
| Daily seed | pre-places `K ∈ [3,15]` perma-dead | `daily_seed.rs`, DESIGN_BRIEF §8.2 |
| Dispersal | **player-chosen layout** (count floor + no-≥7-live-component legality); auto = Algorithm A (Morton-greedy) | `dispersal.rs::validate_layout`, `algorithm_a` |
| End condition | no Empty cells **and** no pending crater | `engine.rs::is_over` |

A subtle structural fact that drives the complexity math: **every Empty cell is
always a legal move** (placement is never adjacency-constrained — `legal_moves`
is exactly the set of Empty cells), and a placement strictly decreases the Empty
count by one. Collapses convert Live→TempDead→{Live, PermaDead} but **never
return a cell to Empty**. Therefore the number of placement moves in any complete
game is exactly `125 − K` (every non-seed cell is filled exactly once), and the
game is a *forced fill to saturation*.

---

## 1. Taxonomic Placement

### 1.1 The master taxonomy, with CIRISGame mapped onto each axis

```
                         GAMES
                           |
        +------------------+--------------------+
        |                                       |
  CLASSICAL / STRATEGIC                  COMBINATORIAL (Conway)
  (payoff-matrix, n-player)              (perfect-info, no chance, alternating)
        |                                       |
  ┌─────┴──────────────┐               ┌────────┴─────────┐
  information:          |               partizan vs        |
   PERFECT  ◄═CIRIS     |               impartial:         |
   imperfect            |                PARTIZAN ◄═CIRIS   |
                        |                impartial          |
  chance:               |                                   |
   DETERMINISTIC ◄═CIRIS|               objective:          |
    (seed is pre-game)  |                normal-play (last-move-wins)
   stochastic           |                misère
                        |                SCORING/ECONOMIC ◄═CIRIS
  move order:           |                  (Milnor/Hanner/Conway "Economist's view")
   SEQUENTIAL ◄═CIRIS   |
   simultaneous         |               temperature theory:
                        |                hot / tepid / COLD-ish ◄═CIRIS (see §1.4)
  horizon:              |
   FINITE ◄═CIRIS       |
   infinite             |
                        |
  players:              |
   2-player             |
   N-PLAYER (n=4) ◄═CIRIS
                        |
  payoff sum:           |
   zero/constant-sum    |
   GENERAL-SUM ◄═CIRIS  ───────────► admits a COOPERATIVE solution (WILD / M-1)
   (non-constant; ties + all-survive)        relate: folk theorem, core, coalition

  cross-cutting structural lens:
   POTENTIAL / CONGESTION GAME ◄═CIRIS (partial — perma-dead = commons externality, §1.5)
```

### 1.2 Combinatorial game theory (Conway / CGT)

- **Partizan, not impartial.** In an impartial game (Nim, standard CGT Sprague–Grundy
  theory) the set of moves available from a position is identical for whichever
  player is to move. Here each steward owns a *color*: a placement by Sienna
  creates a Sienna cell that can only ever join Sienna meshes and can only ever
  collapse Sienna. The move *sets* are nearly symmetric (any player may place on
  any Empty cell), but the *consequences* are color-indexed and the *evaluation*
  of a resulting position differs by player. That is the definition of a
  **partizan** game. (It is not perfectly partizan in the "Left/Right disjoint
  move sets" Conway sense — placement targets are shared — but it is firmly on the
  partizan side because outcomes are player-relative.)

- **Scoring / economic, not last-move-wins.** Classical CGT (the
  Conway–Berlekamp–Guy *Winning Ways* program) is built for **normal-play**
  (last player to move wins) or **misère** (last to move loses) games, where the
  outcome is a *win/lose* determined by who runs out of moves. CIRISGame is
  neither: the game always runs to the same terminal (board saturation), the
  number of moves is essentially fixed (`125 − K`), and **the outcome is a
  numeric score** (`permadead_count`). This puts it in the **scoring-game** /
  **economic-game** branch of CGT — the Milnor (1953) / Hanner / Ettinger lineage,
  Conway's "Economist's view," where positions carry a *value in points* and the
  question is the optimal final margin, not parity of moves. The score here is
  *additive over independent collapse events*, which is exactly the setting
  scoring-CGT was built for.

- **Misère *flavor* in the comparator, not in the move rule.** The win condition
  is **lowest score wins** — you are racing to *minimize* the thing you produce.
  This is a minimization/misère sensibility ("the player who makes the least mess
  wins") layered on a scoring game, distinct from classical misère (where the
  *last move* loses). Combined with the cooperative all-zero ending it means the
  globally-optimal outcome is *everyone scoring zero* — a property no
  constant-sum CGT game can have.

- **Self-only "death."** A move can only ever damage the *mover's own* position
  (collapse is self-triggered, §4 below). In CGT terms there are no "Right options
  that hurt Left" through the board — the partizan tension is entirely about the
  *shared substrate* (perma-dead) and the *relative* score, never about direct
  removal of an opponent's pieces.

### 1.3 Classical / strategic game theory

CIRISGame is a **finite, deterministic, perfect-information, sequential,
n-player (n = 4), general-sum** game.

- **Perfect information.** Every player sees the complete board state through one
  canonical `BoardView` (DESIGN_BRIEF §7, MISSION §4.2). No hidden state, no
  private hands.
- **Deterministic during play.** No dice, no shuffle, no random draw *inside* a
  game. The only randomness — the daily seed's `K` perma-dead and the AI roster —
  is drawn *before* move 1 from a fixed ChaCha8 stream (`daily_seed.rs`). Once the
  position is set, the game tree is deterministic. (Computer-vs-Computer outcomes
  are a function of the deterministic AI policies; the *game* contains no chance
  node.)
- **Sequential, finite horizon.** Strict round-robin (`engine.rs::advance_turn`),
  forced play, terminating in `125 − K` plies. No infinite lines (no ko-like
  repetition — cells never return to Empty).
- **General-sum / non-constant-sum.** This is the crucial classification.
  - *Lowest-wins with ties.* The payoff vector is not a permutation of fixed
    places: two or more stewards can *tie* for the win (`ties allowed`,
    `engine.rs`), so the "1st/2nd/3rd/4th" payoff mass is not conserved.
  - *A strictly cooperative attractor exists.* The all-zero outcome
    (`all_survivors`, WILD / M-1) is **simultaneously optimal for all four
    players** — everyone wins at once. No zero-sum or constant-sum game can have a
    jointly-optimal terminal. This single feature is dispositive: CIRISGame is
    **not** constant-sum.
  - Practically the game lives on a spectrum between a *mild common-interest game*
    (when collapse is avoidable, everyone prefers the cooperative basin) and a
    *mild competitive game* (once someone has collapsed, the rest race to *not be
    worst*).

- **Cooperative / coalitional dimension.** The WILD ending is a **cooperative
  equilibrium sustained by self-restraint**: it requires *every* steward to keep
  every mesh ≤ 6 for the whole game. There is no enforcement mechanism and no
  binding agreement (it is a *non-cooperative* game in the formal sense — no
  contracts), so the all-survive outcome is a **folk-theorem-style cooperative
  equilibrium of the one-shot/repeated interaction**: it is individually rational
  (zero is the best possible personal score) and collectively optimal, but it is
  *not* the unique equilibrium — a single careless or malicious mesh-to-7 breaks
  it for that player only. In cooperative-solution-concept language the all-zero
  profile is in the **core** (no coalition can do better by deviating: you cannot
  score below zero), but it is **not enforceable** against a single defector who
  is willing to take a hit. This is the designed pedagogical point (MISSION §1):
  *coherence is a cooperative achievement that any one party can locally forfeit
  but no party can be forced into.*

### 1.4 Temperature framing (CGT thermography)

CGT *temperature* measures how urgent it is to move in a component (how much the
value swings with the next move). CIRISGame has an in-fiction "temperature"
(DESIGN_BRIEF §4.1) that is a *visual* heat metaphor, **not** the CGT thermograph
— but a genuine CGT-temperature reading is still meaningful:

- Most of the game is **cold/tepid**: placing on an isolated Empty cell that joins
  no mesh and threatens no collapse changes the score by 0 and is reversible in
  value (you can always place elsewhere; nothing is lost). Early game ≈ a cold
  position — moves are nearly interchangeable, swing ≈ 0.
- Positions **heat up** sharply near `|M| = 6` (atari): the next same-color
  adjacent placement swings the score by the full collapse cost (≥ 2 perma-dead,
  up to 5 for a 13-cell). A 6-mesh with an empty same-color-completing neighbor is
  a genuinely **hot** local component — there is real urgency to *not* be forced
  (but note: you are never *forced* to grow your own mesh; the heat is
  self-imposed, so unlike Go, the opponent cannot raise your local temperature).
- There is no "hot" in the adversarial sense of *the opponent gaining by moving
  first in my component* — because of no-capture (§4), opponents cannot move in
  your meshes at all. **CIRISGame's temperature is self-referential**: each
  steward sets and pays their own component's temperature.

### 1.5 Mechanism-design / potential-game / commons lens

- **Is there an exact potential function?** Not a global one in the
  Monderer–Shapley sense (a single Φ such that every player's unilateral payoff
  change equals ΔΦ). The score is *additive and monotone* — total perma-dead
  Φ = Σ scores only ever increases — which makes it a natural **global cost /
  congestion potential**, but each player's *own* score (not Φ) is what they
  minimize, and one player's collapse changes Φ without changing the others'
  payoffs symmetrically. So it is **potential-flavored** (a monotone non-decreasing
  global cost) rather than a textbook exact-potential game.

- **Perma-dead as a commons externality (the strong fit).** This is where the
  congestion/commons lens bites. Each perma-dead cell is a permanent removal from
  *everyone's* future legal-move set (`engine.rs`: PermaDead is never Empty, never
  reclaimable). When steward A collapses, A pays the score, but **all four**
  stewards inherit a smaller, more cratered board to route around — a classic
  **negative externality on a common-pool resource** (the open lattice). This is
  structurally a **tragedy-of-the-commons / congestion game**: the shared resource
  is *empty lattice cells*, congestion is *perma-dead density*, and over-grazing
  (reckless mesh growth) degrades the commons for all (Rosenthal 1973; Monderer &
  Shapley 1996). Unlike a pure congestion game, the *cost is asymmetric* (the
  collapser pays the scored cost; everyone shares the spatial cost), which is what
  makes the commons framing *more* apt than the pure-potential one: the social
  optimum (zero perma-dead = WILD) is the cooperative outcome, and any defection
  imposes an un-priced externality on the rest.

### 1.6 Multiplayer (n = 4) instability

Four-player, non-constant-sum, lowest-wins games carry the classic n ≥ 3
pathologies, and CIRISGame's no-capture rule shapes them distinctively:

- **Attacking is only ever indirect.** With **no capture**, you cannot reduce an
  opponent's score or remove their cells. The *only* way to "win" relative to an
  opponent is (a) keep your own score lower, and (b) hope/induce them to collapse.
  You can influence (b) only indirectly — by consuming the Empty cells they would
  want, by shaping perma-dead substrate that boxes their growth options, or by
  occupying the cells that would have given them a safe split. This is **purely
  positional, constructive pressure** — there is no fork, no atari-the-opponent,
  no capture race.
- **King-making.** Because you cannot attack, a trailing player's choices can
  still decide *which* of the leaders wins (by which Empty cells they leave),
  without improving their own standing — textbook **king-making**, and it is
  unusually pure here since the king-maker has no offensive lever, only the
  allocation of remaining commons.
- **Coalition / tacit collusion.** Three players cannot gang up to *capture* the
  fourth (no mechanism), but they can tacitly coordinate toward WILD (everyone
  stays ≤ 6) — a **self-enforcing-only** coalition (it dissolves the instant one
  member defects, and defection harms *only the defector*). The coalition is
  therefore stable against accidental break but offers no protection or punishment
  — there is no way to punish a defector, which (per the folk theorem) is exactly
  why the cooperative outcome is an *equilibrium that exists* but is *not the
  unique or enforced one*.

**Taxonomic verdict (one line).** CIRISGame is a *finite, deterministic,
perfect-information, sequential, 4-player, general-sum **scoring/economic
combinatorial game** with a **partizan** (color-indexed) structure, a **misère
sensibility** (lowest-score-wins), a **commons/congestion externality** (the
perma-dead substrate), and a genuine **cooperative attractor** (the all-zero WILD
/ M-1 ending) — a combination that sits outside every single classical bucket and
is best described as a* ***cooperative-attractor congestion game on a partizan
scoring lattice.***

---

## 2. Complexity vs Go

**Convention.** Following Wikipedia / the Allis framework, *state-space
complexity* = number of legal positions reachable from the start;
*game-tree complexity* = number of leaf nodes of the (full-width) game tree
≈ number of distinct complete play sequences. Logs are base-10. All CIRISGame
numbers are *my* estimates with assumptions stated; Go numbers are cited.

### 2.1 Authoritative Go reference numbers (cited)

| Board | Legal positions (state-space) | Game-tree complexity | Branch / length |
|---|---|---|---|
| Go 9×9 | **1.039 × 10³⁸** (exact, Tromp) | ≈ 10⁸⁵ (commonly cited estimate) | — |
| Go 13×13 | **3.72 × 10⁷⁹** (exact, Tromp) | ≈ 10²¹⁰ (my b^d estimate, b≈110,d≈110) | — |
| Go 19×19 | **2.08 × 10¹⁷⁰** (exact, Tromp) | **10⁵⁰⁵** (Wikipedia) / **10³⁶⁰** (classic Allis) | b ≈ 250, d ≈ 211 |

Sources: Tromp's exact legal-position counts (9×9 = 1.039×10³⁸; 13×13 = 3.72×10⁷⁹;
19×19 = 2.08×10¹⁷⁰); Wikipedia *Game complexity* table for 19×19 (state-space
10¹⁷⁰, game-tree 10⁵⁰⁵, branching 250, length 211). The older Allis estimate of
10³⁶⁰ for the 19×19 game tree is the historically-quoted figure. 9×9 and 13×13
game-tree figures are not authoritatively tabulated; the values above are my own
b^d estimates and are labelled as such.

### 2.2 CIRISGame state-space complexity (5×5×5)

**Per-cell static states.** For a *static* legal position the relevant states are
`{Empty, Live(Sienna), Live(Lapis), Live(Verdigris), Live(Kaolin), PermaDead}` =
**6**. `TempDead` is excluded: it is a transient one-turn marker that always
resolves on the owner's next move, so it does not characterize stable positions
(including it would multiply some intermediate counts but does not change the
order of magnitude).

**Crude upper bound.**
```
6^125  =  10^(125 · log10 6)  =  10^97.3  ≈  2.4 × 10^97
```
This ignores all constraints and counts impossible boards (e.g. all-Sienna). It
is a hard ceiling.

**More realistic reachable estimate.** Three reductions apply:

1. **No live component ≥ 7** (the rule of seven; `validate_layout` /
   `apply_move` guarantee no live mesh ever persists at ≥ 7). On the FCC lattice
   (coordination 12, site-percolation threshold ≈ 0.20) each color sits at density
   ≈ 1/6 ≈ 0.17 — just *below* percolation, so large monochromatic components are
   uncommon but not negligible. I estimate this prunes **~1–3 orders of
   magnitude**.
2. **Color balance from round-robin.** At any ply the four live-cell counts differ
   by at most the number of collapses since parity broke; positions are
   approximately balanced, removing the extreme-imbalance configurations —
   **~2–5 orders**.
3. **Perma-dead provenance.** PermaDead only arises from the seed (`K ∈ [3,15]`)
   or from collapses (which come in dispersal-shaped clusters), not arbitrarily —
   **~1–2 orders**.

Net realistic reachable state-space estimate: **≈ 10⁸⁵ – 10⁹⁰** (point estimate
10⁸⁸ ≈ `5^125`, with honest ±5 orders of uncertainty). The neat coincidence that
`5^125 = 10^87.4` lands inside this band is a useful mnemonic but not the
derivation.

**Comparison.** On the log axis: Go 9×9 = 38, Go 13×13 = 79, **CIRISGame 5×5×5 ≈
85–90**, Go 19×19 = 170. So by state-space, **5×5×5 sits just above Go 13×13 and
roughly 80 orders of magnitude *below* Go 19×19** — squarely "13×13-class," not
19×19-class. (Go's 19×19 wins on raw state-space because 361 points × 3 states
beats 125 cells × 6 states: `3^361 = 10^172` vs `6^125 = 10^97` at the ceiling.)

### 2.3 CIRISGame game-tree complexity (5×5×5)

**Branching factor.** Legal moves = Empty cells (`legal_moves`). This starts at
`125 − K` (≈ 110–122) and **decreases by exactly 1 each placement** (placements
never restore Empty cells). The arithmetic mean branching is ≈ `(125−K)/2 ≈ 55–60`;
the *geometric* mean (the one that matters for the product) is
`(115!)^{1/115} ≈ 43.5`.

**Leaf count (distinct complete games), placement choices only.** Because every
Empty cell is always legal and each placement removes exactly one, the number of
distinct placement *orderings* of a game is:
```
(125 − K)!     for K ∈ [3,15]
  ⇒  110!  =  10^178.2   (K = 15)
     122!  =  10^203.0   (K = 3)
     115!  ≈  10^188.5   (K = 9, midpoint)
```
So the placement-tree alone is **≈ 10¹⁷⁸ – 10²⁰³**, point estimate **10¹⁸⁸**.
(The color of each stone is *determined* by the round-robin turn order, so it adds
no factor.)

**Extra branching from player-chosen dispersal.** Each collapse hands the
collapsing steward a sub-game: choose which crater cells become perma-dead,
subject to the count floor and the no-≥7-live-component rule. For a 7-cell crater
the legal layouts = subsets of size ≥ 3 (the floor) of 7 cells, all legal
(≤ 4 live can never form a 7-component) = **99 distinct layouts**. An 8-crater
(floor 2) has **247**. So each collapse multiplies the tree by ≈ 10²·⁰–10²·⁴. With
a typical `C` collapses per game:
```
dispersal multiplier ≈ 100^C  ≈  10^(2C)
  C = 0 (WILD):     × 1
  C = 4 (typical):  × 10^8
  C = 8 (aggressive): × 10^16
```
This is *small* relative to the 10¹⁸⁸ placement base — player-chosen dispersal
enriches the game *qualitatively* (it turns every collapse into a real decision)
far more than it enlarges the tree *quantitatively*.

**Total game-tree complexity (5×5×5): ≈ 10¹⁸⁰ – 10²⁰⁵, point estimate ≈ 10¹⁹⁰.**

**Comparison.** This lands **between Go 13×13 (my est. ~10²¹⁰) and well below
Go 19×19 (10³⁶⁰–10⁵⁰⁵)** — again *13×13-class*. Note a structural caveat that
inflates CIRISGame's tree for its size: **it has essentially no move-legality
pruning** — no ko, no suicide rule, no eye-filling pointlessness, no
adjacency constraint. Every Empty cell is a real branch. Go's effective branching
is throttled by these rules and by the fact that most legal Go moves are
strategically dead; CIRISGame's branching is the *raw* combinatorial fan-out,
which is why a 125-cell game reaches a ~10¹⁹⁰ tree.

### 2.4 The §6.4 "≈ 13×13" claim — confirmed, and by which measure

DESIGN_BRIEF §6.4 asserts 5×5×5 ≈ Go 13×13. This analysis **confirms it on three
independent measures**:

| Measure | Go 9×9 | Go 13×13 | **CIRISGame 5×5×5** | Go 19×19 |
|---|---|---|---|---|
| Game length (plies) | ~45–60 | ~90–130 | **110–122** | ~211 |
| State-space (log₁₀) | 38 | 79 | **~85–90** | 170 |
| Game-tree (log₁₀) | ~85 | ~210 (est) | **~180–205** | 360–505 |

5×5×5 matches 13×13 most tightly on **game length** (the comparison §6.4 actually
draws), sits **just above** 13×13 on state-space, and **just below** 13×13 on the
game tree. The honest summary: **CIRISGame 5×5×5 is a 13×13-Go-class game by
complexity, decisively below 19×19 Go.**

### 2.5 Board-size scaling (3³, 4³, 5³, 6³, 7³)

State-space upper bound `6^(n³)`; game-tree `(n³ − K)!` (placement orderings,
K small relative to large boards). Compared to Go state-space.

| Board | Cells | State-space ≤ `6^cells` (log₁₀) | Game-tree `≈(cells−K)!` (log₁₀) | Nearest Go by state-space |
|---|---|---|---|---|
| 3×3×3 | 27 | **21.0** | ~23.8 (24!) | below 9×9 (38) |
| 4×4×4 | 64 | **49.8** | ~80.1 (59!) | ≈ 9×9 (38), a bit above |
| **5×5×5** | **125** | **97.3** (realistic ~85–90) | **~188.5** (115!) | just above 13×13 (79) |
| 6×6×6 | 216 | **168.1** | ~381.8 (203!) | ≈ **19×19 (170)** |
| 7×7×7 | 343 | **266.9** | ~681.9 (327!) | **above 19×19** |

Two notes worth flagging against §6.4 (which compares by *game length*):

- By **state-space**, 6×6×6 already matches Go 19×19 (10¹⁶⁸ vs 10¹⁷⁰), whereas
  §6.4's *length*-based comparison puts 6×6×6 "between 13×13 and 19×19." Both are
  correct on their respective axes — length grows linearly with cells, state-space
  exponentially, so the larger boards "feel" relatively bigger on state-space.
- By **game-tree**, 7×7×7 (~10⁶⁸²) *exceeds* Go 19×19 (10³⁶⁰–10⁵⁰⁵), again because
  CIRISGame's unpruned raw branching produces a factorial-shaped tree.

---

## 3. Game Duration & Compute

### 3.1 Placements per game

`placements = 125 − K` with `K ∈ [3,15]` ⇒ **110–122 placements**, point estimate
**116** (K = 9). This matches `is_over` (saturation) and DESIGN_BRIEF §6.4's
"110–122 typical turns" exactly. Perma-dead created *during* play does not change
the placement count (crater cells were already placed once); it only changes how
many of the filled cells end PermaDead vs Live. Decision *points* exceed
placements by the number of collapses `C` only in the sense that `C` of the
placement turns are *also* crater-layout turns (the rebuild and the new placement
happen in one `apply_move`); they are richer turns, not extra turns.

### 3.2 Wall-clock by assumption

Assumes 116 moves (K = 9). Budgets per DESIGN_BRIEF §6.1, §7.5.

| Assumption | Per-move time | Total moves | Total wall-clock |
|---|---|---|---|
| Uniform 2 s AI compute budget (§7.5) | 2.0 s | 116 | **~3.9 min** |
| Screensaver pacing (§6.1, 2.5 s inter-move) | 2.5 s | 116 | **~4.8 min** |
| Headless tournament `--turn-budget-ms 100` | 0.1 s | 116 | **~12 s** |
| Headless `--turn-budget-ms 0` (heuristic, no think) | <1 ms | 116 | **<1 s** |
| Human hot-seat (≈10 s deliberation/move) | ~10 s | 116 | **~19 min** |
| Human hot-seat, deep think (≈30 s/move) | ~30 s | 116 | **~58 min** |

Player-chosen dispersal adds *decision weight* but not separate turns; in
human play, collapse turns take longer (you are laying out a crater *and*
placing), so a game with several collapses skews the human numbers upward by a
few minutes. In all AI budgets the dispersal sub-choice is resolved inside the
same 2 s.

### 3.3 Search feasibility and "solvability"

- **Branching vs budget.** Early branching ≈ 110–122, settling to a geometric
  mean ≈ 43. On mid-tier WASM a heuristic node eval is ~µs–tens of µs; 2 s buys
  ~10⁵–10⁷ node evaluations.
- **Minimax depth.** With effective b ≈ 43–55, full-width depth in 2 s is
  `log_b(10^6…10^7) ≈ 3–4 ply`. DESIGN_BRIEF §6.3 confirms Hard = **depth-2
  minimax**; that is honest, not a placeholder — depth-2 is near the practical
  full-width ceiling at this branching, and the 4-player tree makes deeper
  full-width search exponentially worse (each "ply" is one of four opponents).
- **MCTS.** Brutal = MCTS, ~10–20 k playouts in 2 s (§6.3). Against a tree of
  ~10¹⁹⁰ leaves and average game length ~116, 10–20 k random playouts is *very*
  sparse sampling — strong tactically (it will reliably avoid self-collapse and
  spot the `r = 2` floor asymmetry) but nowhere near game-theoretic optimality.
- **Solvability.**
  - **5×5×5: not remotely solvable.** State-space ~10⁸⁵–10⁹⁰ rules out any
    tablebase; game-tree ~10¹⁹⁰ rules out full search. It is in the same
    "unsolvable in practice" tier as Go 13×13.
  - **3×3×3: not solved either, but the only candidate.** State-space ~10¹⁸–10²¹
    is still far beyond exhaustive enumeration (a 10²¹-entry tablebase is
    infeasible), and the 4-player general-sum structure means "solving" requires a
    full equilibrium profile, not a single value. Endgames (last ~6–10 empty
    cells) *are* exhaustively searchable in budget, so 3×3×3 has solvable
    *endgames* but no solved *game*. There is no board size at which CIRISGame is
    fully solved.

---

## 4. Novel / Structurally Interesting Properties

1. **No capture — purely constructive conflict.** `apply_move` has no path by
   which one steward removes another's cell; the *only* death is a steward's own
   mesh reaching 7 (DESIGN_BRIEF §4.10). This is unusual among territorial board
   games (Go, Hex, Reversi all feature capture/conversion). Strategy is therefore
   100% positional and constructive: you compete by *building well and consuming
   shared space*, never by destroying. A `Live` cell ringed by 12 enemy colors is
   "inert but safe" forever — a property with no Go analogue (in Go it would be
   captured).

2. **Self-inflicted-only collapse.** Risk is fully endogenous and self-priced: no
   opponent can raise your local CGT temperature, and you can guarantee a personal
   score of zero by never growing a mesh past 6. This makes the *floor* of every
   player's payoff individually controllable — rare in multiplayer games, and the
   formal basis for the cooperative attractor.

3. **The perma-dead commons as a shared externality.** Collapses degrade a
   resource (open lattice) that *all* players draw from, while the *scored* cost
   falls only on the collapser (§1.5). This dual cost-structure (private score +
   public congestion) is what makes the congestion/tragedy-of-the-commons lens fit
   better than a pure potential game, and it is the mechanic that makes "route
   around the scars" a genuine multiplayer dynamic.

4. **A cooperative attractor (WILD / M-1).** A jointly-optimal terminal where
   everyone wins simultaneously (`all_survivors`) — impossible in constant-sum
   games. It is in the core but not enforceable (any single defector can break it,
   harming only themselves), a clean playable instantiation of a
   folk-theorem-style cooperative equilibrium and of the M-1 thesis that coherence
   is a cooperative achievement no party can be coerced into.

5. **Generative collapse.** Collapse is not annihilation: a dead 7-mesh *creates*
   live pairs plus permanent substrate (`dispersal.rs`). The destructive
   transition is a *productive* state change — the central pedagogical metaphor
   (MISSION §1.1) rendered as a literal game rule.

6. **Player-chosen dispersal = a sub-game per collapse.** Each collapse opens a
   constrained combinatorial layout decision (`validate_layout`: count floor +
   no-≥7-live-component). This converts a former deterministic event into a real
   decision node — ~99 legal layouts for a 7-crater — letting a skilled player
   shape *where* the scars land (boxing their own future or, indirectly, others')
   while never escaping the locked score floor. It enriches the game's decision
   texture far more than its raw tree size.

7. **Factorial-shaped, unpruned game tree.** Because every Empty cell is always
   legal and never recycles, the tree is essentially a constrained permutation of
   the fill order — giving a game-tree complexity unusually large for the cell
   count (~10¹⁹⁰ at 125 cells), with none of the legality-pruning that throttles
   Go's effective branching.

---

## Sources

- Wikipedia, *Game complexity* — state-space/game-tree definitions; Go 19×19
  state-space 10¹⁷⁰, game-tree 10⁵⁰⁵, branching 250, length 211:
  https://en.wikipedia.org/wiki/Game_complexity
- John Tromp, *Number of legal Go positions* (exact counts: 9×9 =
  1.039×10³⁸, 13×13 = 3.72×10⁷⁹, 19×19 = 2.08×10¹⁷⁰):
  https://tromp.github.io/go/legal.html ; method paper:
  https://tromp.github.io/go/gostate.pdf
- L. V. Allis, *Searching for Solutions in Games and Artificial Intelligence*
  (1994) — the classic 10³⁶⁰ Go game-tree estimate and the state-space/game-tree
  complexity framework.
- R. W. Rosenthal, *A class of games possessing pure-strategy Nash equilibria*
  (1973) — congestion games.
- D. Monderer & L. S. Shapley, *Potential Games*, Games and Economic Behavior
  (1996) — exact/ordinal potential games, finite improvement property.
- J. Milnor, *Sums of positional games* (1953); E. Berlekamp, J. Conway, R. Guy,
  *Winning Ways* — scoring/economic combinatorial games, temperature theory,
  partizan vs impartial.
- G. Hardin, *The Tragedy of the Commons*, Science (1968) — the commons
  externality framing.

*All CIRISGame complexity and duration figures are this author's estimates from
the rules in `crates/ciris-game-engine-core/`; assumptions are stated inline and
uncertainties are explicit. Go figures are cited.*
