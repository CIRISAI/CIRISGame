# CIRISGame — Game-Theoretic Analysis

**Scope.** This document places CIRISGame within standard game-theory taxonomies,
estimates its complexity against Go, characterises collapse risk and strategic
depth, and catalogues structurally novel properties. Numbers are accompanied by
their assumptions; estimates are labelled as such.

**Grounding.** Analysis is built from the actual rules as implemented, not from a
generic abstraction.

| Fact | Value |
|---|---|
| Board | 5×5×5 = 125 cells |
| Lattice | Simple cubic; ±x/±y/±z bonds only; corner=3 neighbors, edge=4, face-center=5, interior=6 |
| Stewards | 4, strict round-robin |
| Collapse threshold | mesh size 7 |
| Atari | mesh size 6 (Kuramoto breath at 0.6 Hz) |
| Cell states | Empty / Live(×4) / TempDead(×4, transient) / PermaDead = 10 total |
| Score | perma-dead created per steward; **lowest wins** |
| Dispersal | Algorithm A (Morton-greedy): k = N÷3 live pairs + k perma-dead spacers + remainder |
| Cooperative ending | all-zero → WILD (M-1) |
| No capture | cells die only via their own steward's collapse |
| No-crossing rule | a same-color bond may not cross a live different-color bond on a shared face |
| Daily seed | K ∈ [3,15] perma-dead pre-placed, ChaCha8-derived |
| End condition | no Empty cells and no pending craters |

**Dispersal count table (locked):**

| Collapse size N | Live cells after | Perma-dead created |
|---|---|---|
| 7 | 4 | **3** |
| 8 | 6 | **2** |
| 13 | 8 | **5** |
| 14 | 10 | **4** |

Rule: k = ⌊N/3⌋; r = N mod 3. k live pairs + k perma spacers; remainder r=1 goes
to perma, r=2 becomes a live pair. Score cost ranges from 2 (N=8) to 5 (N=13) —
larger explosions are not always more costly, which drives interesting late-game
sizing tactics.

---

## 1. Taxonomic Placement

CIRISGame is a **finite, deterministic, perfect-information, sequential, 4-player,
general-sum scoring game** with a partizan (color-indexed) structure.

**Partizan, not impartial.** Each steward's placement creates a cell that only
ever joins their own mesh. The move *sets* are symmetric (any steward may place on
any legal Empty cell) but consequences are color-indexed — the definition of
partizan.

**Scoring/economic, not last-move-wins.** The game always runs to saturation
(125 − K placements); the outcome is a numeric score additive over independent
collapse events. This sits in the Milnor/Hanner/Conway "scoring game" branch of
CGT, not normal-play or misère.

**Misère sensibility.** The win condition is *lowest score wins* — a minimisation
orientation layered on a scoring game. Unlike classical misère (last move loses),
here every player can individually guarantee a score of zero by keeping all meshes
≤ 6.

**General-sum, not constant-sum.** Ties are allowed; a strictly cooperative
outcome exists (all-zero = WILD). No constant-sum game can have a jointly-optimal
terminal.

**Commons/congestion externality.** Perma-dead cells are removed from *every*
player's future legal-move set, but the *scored* cost falls only on the collapser.
Each collapse imposes an un-priced externality on the commons (open lattice
cells). This fits the Rosenthal congestion / tragedy-of-the-commons frame better
than an exact potential game.

**Verdict.** CIRISGame is a *cooperative-attractor congestion game on a partizan
scoring lattice* — a combination that sits outside every classical single bucket.

---

## 2. State Space

**Absolute upper bound.** With 10 states per cell:
```
10^125  ≈  10^125
```
**Stable-position upper bound** (excluding transient TempDead, which resolves
within one turn; 6 states: Empty / Live×4 / PermaDead):
```
6^125  =  10^(125 × log₁₀6)  ≈  10^97
```
**Realistic reachable estimate.** Three constraints reduce this materially:

1. *No live component ≥ 7.* Simple cubic site-percolation threshold ≈ 0.312;
   each color's density is ≈ 30/125 = 0.24, just below threshold. Large
   monochromatic clusters are uncommon. Prunes ~1–3 orders.
2. *Round-robin color balance.* Counts differ by at most the collapse imbalance
   — extreme-imbalance configurations are unreachable. Prunes ~2–4 orders.
3. *Perma-dead provenance.* PermaDead only arises from the seed or from
   dispersal-shaped clusters, never arbitrarily. Prunes ~1–2 orders.

**Realistic state-space estimate: ≈ 10⁸⁵ – 10⁹⁰.**

**Comparison to Go:**

| Game | State-space (log₁₀) |
|---|---|
| Go 9×9 | 38 (exact, Tromp) |
| Go 13×13 | 79 (exact, Tromp) |
| **CIRISGame 5×5×5** | **~85–90 (est.)** |
| Go 19×19 | 170 (exact, Tromp) |

5×5×5 sits just above Go 13×13 and roughly 80 orders of magnitude below Go 19×19.

---

## 3. Branching Factor and Game Length

**Game length is fixed at exactly 125 − K placements** (K ∈ [3,15]). Every
placement fills one Empty cell permanently; dispersal never recycles cells to
Empty. Total turns: **110–122**, point estimate **116** (K = 9). Each steward
places ~29 cells on average.

**Branching factor.** Legal moves = Empty cells minus cells forbidden by the
no-crossing rule. Early game: branching ≈ 110–120 (no-crossing rarely triggers
when cells are sparse). Late game: branching approaches 1. Geometric mean
(the relevant figure for tree size): **≈ 40–55** after no-crossing pruning, vs
~43 on the raw empty-cell count.

**Game-tree complexity.** Without the no-crossing constraint, placement orderings
alone give:
```
(125 − K)!  ≈  115!  ≈  10^188
```
With no-crossing trimming this modestly in dense positions, the practical game
tree is estimated at **≈ 10^180 – 10^195**. This lands just below Go 13×13
(estimated ~10^210). Unlike Go, there is no ko or suicide rule; CIRISGame's tree
is driven by raw combinatorial fan-out rather than legality-pruning, which is why
a 125-cell game reaches 10^180+.

---

## 4. Collapse Probability

**Simple cubic hard ceiling.** The maximum number of distinct cells one cell can
be simultaneously connected to is 6 (interior cell). The densest 7-mesh is a
"star" — one center + all 6 axis-aligned neighbors — which is both the minimum
and a natural configuration.

**Risk by cell type.** An interior cell at (i,j,k) with 2≤i,j,k≤3 has 6
neighbors and can complete a 7-mesh most easily. Corner cells (3 neighbors) are
the safest expansion territory; face-center cells (5 neighbors) are dangerous in
the late game.

**Against random play.** Each steward places ~29 cells. With simple-cubic site
percolation threshold ~0.312 and steward density ~0.24 (below threshold), the
expected largest single-color cluster is small, but with 29 cells spread over 125
and round-robin interleaving, accidental 7-mesh formation is a real risk — not
automatic, but not rare. A rough estimate: under pure random play, each steward
has a 40–70% probability of triggering at least one collapse during a full game.
The no-crossing rule provides additional mesh separation by preventing color
interpenetration across axis-aligned planes, which naturally keeps steward regions
from bleeding into each other.

**Against attentive play.** Atari warning (|M|=6) fires well before the 7-mesh.
An attentive steward can always avoid collapse by placing away from an
atari-flagged mesh, since the no-crossing rule means there is no mechanism for an
opponent to force growth into your own mesh. **Collapse risk is entirely
endogenous.**

---

## 5. Complexity Class

**Decision problem.** "Can player P achieve a score ≤ k by turn T given optimal
play by all parties?" This is almost certainly **PSPACE-hard** — it requires
reasoning about an exponentially large strategy tree in a multi-agent setting.
An exact proof would require reduction from a known PSPACE-complete game (e.g.
Generalized Geography, TQBF), which has not been attempted; the classification
is by analogy with Go and territorial games of similar depth.

**4-player structure.** Classical minimax is ill-defined for n=4 general-sum;
equilibrium concepts (Nash, correlated) apply instead. This makes "optimal play"
harder to define than in 2-player games, potentially placing exact solution in
EXP rather than PSPACE — but the practical unsolvability dominates at any
classification.

**Practical solvability.** State-space ~10^88 rules out any endgame tablebase.
Game-tree ~10^188 rules out exhaustive search. CIRISGame 5×5×5 is
**definitively unsolvable** in the same tier as Go 13×13.

---

## 6. Strategic Depth

**Simple cubic geometry creates clear lanes.** Bonds are strictly ±x/±y/±z.
This organises the board into three independent families of planes (xy, xz, yz).
A steward who "owns" a band of cells along the x-axis is naturally separated from
a steward who owns a band along the y-axis — cleaner territorial partitioning
than the FCC lattice's 12-direction fan.

**Connectivity gradient.** The corner → edge → face-center → interior gradient
(3→4→5→6 neighbors) is immediately legible and maps directly to risk. Strategic
depth comes from balancing between safe peripheral placement (low connectivity,
low collapse risk, low strategic value) and valuable interior placement (high
connectivity, high risk, high board control).

**No-crossing as a natural border.** The no-crossing rule prevents a same-color
bond from piercing a live different-color bond on a shared face. In the axis-aligned
grid, this creates soft barriers along the planes where different-color bonds run.
Players effectively negotiate territory across these planes without any explicit
border-drawing mechanism.

**No direct attack.** Since no capture exists and dispersal is automatic (Algorithm
A), all strategy is constructive and positional. Players compete by occupying
valuable cells and managing their own mesh topology — never by threatening
opponents' pieces. King-making (a trailing player determining which leader wins by
cell allocation) is the main indirect-interaction mechanism.

---

## 7. WILD (M-1) Analysis

**Condition.** All four stewards must end the game with zero perma-dead created.
This means no steward ever triggers a 7-mesh at any point during the game.

**Under cooperative play.** Straightforwardly achievable: any strategy that keeps
all meshes at ≤5 cells guarantees zero collapses. With ~29 placements per steward
over 125 cells, this only requires spreading cells across multiple small clusters
rather than building one connected chain. A cooperative player disperses
deliberately and avoids adjacent same-color cells in interior regions.

**Under random play.** Given the 40–70% per-steward collapse probability estimated
in §4, the probability that *all four* stewards avoid collapse is roughly
(0.30–0.60)^4 ≈ 1–13%. Under fully random play, WILD is rare but not impossible.
The cooperative equilibrium is individually rational (zero is the minimum possible
score) and collectively optimal, but not enforced — any single steward can
defect by careless growth, harming only themselves.

**Game-theoretic characterisation.** WILD is a Nash equilibrium: given that the
other three stewards are keeping meshes ≤6, it is optimal to do the same (score 0
is unimprovable). It is not the *unique* Nash equilibrium — any profile where no
one benefits from changing their mesh strategy is also an equilibrium. The WILD
outcome is in the core (no coalition can do strictly better by deviating), but it
is not enforceable against a unilateral defector.

---

## 8. Comparison to FCC (Prior Lattice)

The previous design used a rhombic-dodecahedral (FCC) lattice — 12 face-neighbors
per cell, coordination number 4× higher than simple cubic interior.

**What was lost.**
- *Routing richness.* 12 directions gave stealthy mesh-building paths that were
  hard to anticipate. Strategic depth was partially hidden in the geometry.
- *Higher state-space density.* FCC state-space was in the same order-of-magnitude
  range but with far more legal configurations per unit volume.

**What was gained.**
- *Immediate legibility.* Axis-aligned ±x/±y/±z bonds map to the natural xyz
  mental model. Players grasp the topology in the first few moves.
- *Cleaner risk gradient.* The 3/4/5/6 neighbor-count progression by cell position
  is instantly understood; the FCC gradient was non-intuitive and hard to convey
  visually.
- *Safer collapse dynamics.* FCC's percolation threshold ~0.20 means each
  steward's density (~0.25) *exceeds* threshold, making large monochromatic
  clusters statistically expected. Simple cubic threshold ~0.31 keeps the same
  density *below* threshold — large clusters are atypical, making collapse a
  deliberate-or-careless event rather than an emergent inevitability.
- *No-crossing rule cleanly defined.* With axis-aligned bonds, the shared-face
  crossing constraint has unambiguous geometry.

**Verdict.** The FCC lattice was geometrically richer but punishing to learn.
Simple cubic sacrifices some hidden depth in exchange for a game that communicates
its mechanics clearly, where strategic errors feel legible and the cooperative
ending (WILD) is visibly achievable rather than statistically improbable.

---

## Sources

- John Tromp, *Number of legal Go positions* — exact state-space counts for Go
  9×9 / 13×13 / 19×19: https://tromp.github.io/go/legal.html
- Wikipedia, *Game complexity* — Go 19×19 branching 250, length 211, game-tree
  10^505: https://en.wikipedia.org/wiki/Game_complexity
- L. V. Allis, *Searching for Solutions in Games and Artificial Intelligence*
  (1994) — state-space/game-tree complexity framework.
- R. W. Rosenthal, *A class of games possessing pure-strategy Nash equilibria*
  (1973) — congestion games.
- D. Monderer & L. S. Shapley, *Potential Games*, Games and Economic Behavior
  (1996).
- J. Milnor, *Sums of positional games* (1953); E. Berlekamp, J. Conway, R. Guy,
  *Winning Ways* — scoring/economic combinatorial games.
- G. Hardin, *The Tragedy of the Commons*, Science (1968).
- M. F. Sykes & J. W. Essam, simple cubic site-percolation threshold p_c ≈ 0.3116:
  *J. Math. Phys.* **5**, 1117 (1964); confirmed by subsequent Monte Carlo studies.

*All CIRISGame complexity and duration figures are estimates from the rules
described above; assumptions are stated inline and uncertainties are explicit.
Go figures are cited from Tromp and Wikipedia.*
