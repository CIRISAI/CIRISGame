# TOPOLOGY_DIAL — a rotary dial through the embedding family

Research/design note. No code shipped. Grounds the "rotate the lattice through
higher dimensions" dial in real math, then gives a concrete, WebGL2/Bevy-friendly
parameterization, ordering, and anti-overlap numbers.

Scope reminder: the lattice **topology** (the adjacency graph — 125 cells, 12
face-neighbours each) is fixed forever. The dial only changes the **embedding**:
the map `coord (i,j,k) → Vec3`. Today `topology.rs` has four discrete embeddings
(Cube, Sphere, Torus, Möbius) and morphs between them on a button press
(`embed_one` + a dual-axis `tumble`). The ask: replace the button with a
continuous angular dial.

---

## 1. What "rotational exploration of topological space" actually means

The design phrase conflates three different objects. Pulling them apart is the
whole game:

| Object | What it is | Does the dial change it? |
|---|---|---|
| **Intrinsic topology** | the fixed graph (who is adjacent to whom) | **No. Never.** |
| **Embedding** `f: coord → R³` | a placement of the cells in space | **Yes — this is the dial.** |
| **The path of embeddings** `f_θ` as θ sweeps | a *homotopy* (a continuous 1-parameter family of maps) | the dial **is** this path |

So the honest framing is: **the dial sweeps a 1-parameter family (a homotopy) of
embeddings of a fixed graph.** You are not exploring "topological space" (there is
only one topology here); you are exploring **the space of embeddings of one
topology**, which is exactly what makes it visually legible — same state, many
shadows.

Concepts in play, real vs. evocative:

- **Homotopy / isotopy of embeddings** — REAL and central. A homotopy is any
  continuous `f_θ`. An *isotopy* additionally keeps every `f_θ` injective (no two
  cells ever collide). Our morphs are homotopies but **not** isotopies — cells do
  pass through each other mid-morph. That is fine for a glass-marble visualizer
  (it reads as "rotating through a higher dimension") but it is the precise reason
  torus/Möbius look crammed: at some θ the map is non-injective. ([Regular
  homotopy, Wikipedia](https://en.wikipedia.org/wiki/Regular_homotopy))
- **Regular homotopy / sphere eversion** — EVOCATIVE only. Smale's eversion is
  about *immersions of a surface* (preserving an immersion's tangent data); our
  cells are points, not an immersed surface, so eversion theory doesn't directly
  apply. Good as a mood reference, not a spec. ([Sphere eversion,
  Wikipedia](https://en.m.wikipedia.org/wiki/Sphere_eversion))
- **4D rotation (double / isoclinic rotation) projected to R³** — REAL and the
  best literal realization of "rotate through a higher dimension." A rotation in
  R⁴ acts in two orthogonal planes at once; the *isoclinic* case (equal angles)
  is the smoothest. Lift each cell to 4D, rotate, project to R³ → you genuinely
  watch the structure swing through a 4th axis. ([Rotations in 4D, Wikipedia](https://en.wikipedia.org/wiki/Rotations_in_4-dimensional_Euclidean_space))
- **Hopf fibration / Clifford torus / stereographic S³→R³** — REAL, and it is the
  *principled* way to get the torus: a flat (Clifford) torus living in S³,
  stereographically projected to R³, becomes the familiar ring torus, and a 4D
  rotation of it traces Villarceau circles. This is a much cleaner torus than the
  hand-rolled `(big + rr·cosφ)` one. ([Hopf fibration, Wikipedia](https://en.wikipedia.org/wiki/Hopf_fibration); [Clifford torus, Wikipedia](https://en.wikipedia.org/wiki/Clifford_torus))
- **Cube → sphere as an area-preserving map** — REAL; the standard
  "spherified cube" / inverse-Lambert construction spreads a cube grid onto S²
  with near-zero distortion (better than the current Chebyshev-radius trick, which
  bunches cells toward face centres). ([Uniform spherical grids via equal-area
  cube→sphere projection (PDF)](https://num.math.uni-goettingen.de/plonka/pdfs/cubsphere3.pdf); [cube-to-sphere survey](https://link.springer.com/article/10.1007/s00371-019-01708-4))

**The one hard truth (read before believing the marketing).** A *single rigid R⁴
rotation cannot* morph cube → sphere → torus. Rigid rotations preserve pairwise
distances; cube→sphere and the torus wrap are **non-isometric** (they stretch and
glue). Therefore the shape change is a **non-rigid blend (homotopy)**, and the 4D
rotation is a **flourish layered on top**, not the thing that produces the shapes.
"One big 4D rotation that visits every shape" is false. "A blend between shapes,
carried by a real 4D rotation that makes it feel four-dimensional" is true and is
what we should build.

---

## 2. Principled ordering + angular layout

Order by a single monotone quantity: **how much the embedding glues the lattice's
boundary to itself** (identifications), then by twist. This makes adjacent stops
minimally different, so any blend between neighbours is gentle.

| Surface evoked | Boundary identifications | Genus / character | Why it sits here |
|---|---|---|---|
| **Cube** | none | g=0, flat block | the raw lattice; identity reference |
| **Sphere** | none | g=0, rounded | same topology as cube, just radial rounding — tiny step from cube |
| **Cylinder** | 1 pair glued, no flip | open torus | first gluing; bridges sphere→torus so it isn't a jump |
| **Torus** | 2 pairs glued, no flip | g=1 | full wrap |
| **Möbius** | 1 pair glued *with flip* | non-orientable, w/ boundary | introduces the half-twist |

Cube and sphere are genuinely the *same* topology (both genus 0), so they belong
adjacent. Cylinder is the natural in-between that the current set is missing and
is exactly what removes the harsh sphere→torus crush.

### Recommended layout — palindrome sweep (ship this)

A full 360° dial that goes out and back. Closure is **free and exact** because the
two ends are literally the same embedding (cube = cube), so there is zero seam.

| Angle | Stop |
|---|---|
| 0° / 360° | Cube |
| 60° | Sphere |
| 120° | Cylinder |
| 180° | Torus |
| 240° | Möbius |
| 300° | Cylinder (return) |
| → 360° | Sphere → Cube (return) |

Between stops, blend with a smoothstep; at each stop the blend weight is exactly
0 or 1 so the named shape is shown cleanly. The dial reads "cube unfolds all the
way out to its most twisted form and re-settles" — the exact design intent — and
is guaranteed C⁰-closed.

### Alternative — monotone ring (purer, riskier)

If you want every stop distinct on a 72° ring `Cube→Sphere→Cylinder→Torus→Möbius→
(wrap to Cube)`, it closes only if the Möbius→Cube morph is acceptable (it is the
largest single motion). Use this only if the palindrome's repeated stops feel
redundant. The 4D flourish (below) hides the Möbius→Cube jump well.

**Blend behaviour between stops:** linear interpolation of the two stop positions,
eased by smoothstep `s = local·local·(3−2·local)`, where `local ∈ [0,1)` is the
fraction between the two bracketing stops. This is what `embed` already does for
`t`; the dial just makes `local` a function of the angle instead of a timer.

---

## 3. Continuity / "tubes stay connected" guarantee

Each tube is drawn endpoint-to-endpoint between two cells' *current* positions
(`position_pipes` re-fits every frame). Therefore:

- **The only requirement is that `embed(coord, θ)` is continuous in θ.** Any
  continuous map keeps every tube attached to its two marbles — connectivity is a
  property of the graph, not the geometry, and the graph is fixed. There is no
  extra condition to satisfy.
- The blend (lerp of two continuous embeddings, eased by a continuous smoothstep)
  is continuous. The 4D flourish (below) is continuous and returns to identity at
  each stop, so it does not break continuity at stop boundaries. ✓

What continuity does **not** buy you is **injectivity** (no overlaps). The current
torus and Möbius overlap precisely because those embeddings are non-injective at
the chosen scales — different cells land within a marble-radius of each other.
That is a *spacing* bug, not a *connectivity* bug, and §5 fixes it.

Why the current torus/Möbius cram (diagnosed from `embed_one`, n=5, marble
radius 0.42 → diameter 0.84):

- **Torus** maps the third lattice axis `k` to tube radius `rr ∈ [0.5, 1.4]`. Over
  5 shells that is spacing `0.9/4 = 0.225` — but marbles are 0.84 across. The
  nested tube shells overlap ~4×. The inner minor ring at `rr=0.5` has
  circumference `2π·0.5 ≈ 3.14`, i.e. `0.63` per cell < 0.84 → also overlapping.
- **Möbius** maps `k` (thickness) to `p.z·0.14`, range ±0.14 → spacing `0.07`
  over 5 cells vs. 0.84 diameter → ~12× overlap in the thin direction. Width and
  loop are fine; only the ribbon thickness collides.

---

## 4. Concrete parameterization

A single `embed(coord, n, dial) → Vec3`. Two layers: a **keyframe blend** (which
surface) and an **honest 4D rotation flourish** (the "higher dimension" feel).

```rust
const STOPS: [Shape; 6] = [Cube, Sphere, Cylinder, Torus, Mobius, Cylinder];
// (palindrome; index 5 == Cylinder return; the wrap 5→0 is Cylinder→Cube,
//  i.e. the Sphere/Cube return half. Use 8 entries for the full palindrome.)

fn embed(c: Coord, n: u8, dial: f32) -> Vec3 {
    let k = STOPS.len() as f32;
    let x = dial.rem_euclid(TAU) / TAU * k;     // continuous stop coordinate
    let i0 = x.floor() as usize % STOPS.len();
    let i1 = (i0 + 1) % STOPS.len();
    let local = x - x.floor();                  // 0..1 within this segment
    let s = local * local * (3.0 - 2.0 * local); // smoothstep ease

    let a = embed_one(c, n, STOPS[i0]);
    let b = embed_one(c, n, STOPS[i1]);
    let base = a.lerp(b, s);                     // continuous, connectivity-safe

    // Flourish completes once per segment: identity at s=0 and s=1, so stops are
    // shown clean and continuity across stop boundaries is preserved.
    rot4_flourish(base, norm(c, n), s)
}
```

### The 4D flourish (the literally-real "rotate through a higher dimension")

Lift the R³ point to R⁴ with a 4th coordinate `w`, apply an **isoclinic rotation**
by `α = s·TAU` in the orthogonal planes (x,w) and (y,z), then project back by
dropping `w`:

```rust
fn rot4_flourish(p: Vec3, np: Vec3, s: f32) -> Vec3 {
    // w lift: outer shells swing furthest through the 4th axis. cheb radius in
    // [0,1] -> w in [-L, +L]; centred so the whole body rotates, not just one side.
    let cheb = np.x.abs().max(np.y.abs()).max(np.z.abs());
    let w = (cheb - 0.5) * 2.0 * L_LIFT;        // L_LIFT ~ SCALE (=2.0)
    let a = s * TAU;                             // isoclinic angle, 0 at both stops
    let (sa, ca) = a.sin_cos();
    Vec3::new(
        p.x * ca - w * sa,      // (x,w) plane
        p.y * ca - p.z * sa,    // (y,z) plane
        p.y * sa + p.z * ca,
    ) // w' = p.x*sa + w*ca is dropped (orthographic projection R4 -> R3)
}
```

Because `α = s·TAU`, at `s=0` and `s=1` the rotation is the identity → `base` is
shown untouched at every stop, and the dial is C⁰ across the whole 360°. During a
segment the structure genuinely rotates through `w` (an isoclinic double rotation,
the smoothest 4D rotation — [Rotations in 4D](https://en.wikipedia.org/wiki/Rotations_in_4-dimensional_Euclidean_space)).
This replaces the current 3D `Quat` tumble with a real 4D one; it is pure
per-vertex arithmetic (sin/cos + multiplies), so WebGL2/Bevy-friendly with no
compute shader.

Tunables: `L_LIFT` controls how far cells swing through 4D (0 = flat blend, only
the shape morph; `≈SCALE` = strong four-dimensional swing). Expose as a knob.

### Optional upgrade for the torus stop (principled, not required)

If you want the torus to be the *real* one, build it as a Clifford torus in S³,
stereographically projected: place `(cosθ, sinθ, cosφ, sinφ)/√2` in R⁴ (θ from
`i`, φ from `j`, radius from `k`), project `(x1,x2,x3,x4) → (x1,x2,x3)/(1−x4)`.
This yields the Villarceau-circle ring torus and composes naturally with the same
4D rotation. ([Hopf fibration](https://en.wikipedia.org/wiki/Hopf_fibration))
For shipping, the existing analytic torus with the §5 scaling is sufficient.

---

## 5. Anti-overlap recommendation (numbers, n=5, marble r=0.42, dia 0.84)

Design rule: **normalize every embedding so the minimum nearest-neighbour spacing
≥ marble diameter + a glass gap.** The cube is the baseline — its neighbour
spacing is `SCALE · 2/(n−1) = 2·0.5 = 1.0`, comfortably > 0.84. Target every other
shape to ≈1.0 minimum spacing.

**Sphere.** Already ~fine, but the Chebyshev trick bunches cells at face centres.
Either leave it or switch to the equal-area spherified-cube map for even spacing
([cube→sphere equal-area](https://num.math.uni-goettingen.de/plonka/pdfs/cubsphere3.pdf)).
No overlap today; low priority.

**Torus.** The binding constraints and the fix:

- Minor-ring spacing at smallest tube radius: `2π·rr_min / n ≥ 1.0` → `rr_min ≥ n/2π ≈ 0.80`.
- Radial (k-shell) spacing: `(rr_max − rr_min)/(n−1) ≥ d`.
- Self-intersection: ring torus needs `R > rr_max`.

Two options:
1. **True non-overlap, big torus:** `rr ∈ [0.8, 4.8]` (spacing 1.0), `R ≈ 5.5`.
   Diameter ~20 — must zoom the camera out for this mode.
2. **Ship this — compact torus + per-mode marble shrink:** `R = 3.2`,
   `rr ∈ [0.8, 2.6]`, and set `MarbleSize ≈ 0.5` in torus mode (effective
   diameter 0.42). Then radial spacing `(2.6−0.8)/4 = 0.45 ≥ 0.42` ✓, inner minor
   ring `2π·0.8/5 = 1.0` ✓, major `R·(0.85·2π)/5 = 3.4` ✓, and `R > rr_max`
   (3.2 > 2.6) ✓. Keep the `0.85` seam so no false wrap-around bonds.

   Replace in `embed_one` Torus arm: `rr = 0.8 + (p.z*0.5+0.5)*1.8; big = 3.2;`.

**Möbius.** Only the thickness (`k`) axis overlaps. Required thickness factor `f`
on the normal so spacing `2f/(n−1) ≥ d`: with `MarbleSize 0.5` (d=0.42), `f ≥ 0.84`.

- **Ship this:** change `normal * (p.z * 0.14)` → `normal * (p.z * 0.84)` and run
  Möbius at `MarbleSize ≈ 0.5`. Width `±2.1` and loop `R=2.8` are already fine
  (loop spacing `2π·2.8/5 = 3.5`, width spacing `4.2/4 = 1.05`). The ribbon
  becomes a thin *slab* (~1.7 thick) rather than a sheet — unavoidable: five
  marbles cannot stack in a paper-thin ribbon without overlap. Accept the slab or
  shrink marbles further.

**Mechanism.** Drive the per-mode marble shrink off the existing `MarbleSize`
resource, blended by the same `s`/stop weight so it eases in with the shape (e.g.
`marble_scale = lerp(1.0, shape_min_scale[stop], s)`). No new system needed —
`position_cells` already applies `MarbleSize`.

---

## 6. What's real vs. evocative (one-screen summary)

**Real (build on these):**
- Fixed graph topology + a continuous family of embeddings; tubes stay connected
  iff `embed` is continuous in the dial angle. Guaranteed.
- A 4D isoclinic rotation of a 4D lift, projected to R³, is a genuine "rotation
  through a higher dimension" and is cheap per-vertex math.
- Cube↔sphere (radial or equal-area) and the Clifford-torus-via-stereographic
  constructions are real, standard maps.
- Ordering by boundary-identification count (genus/twist) is a real, monotone
  way to make neighbouring stops minimally different.

**Evocative only (don't oversell):**
- "Exploring topological space" — the topology never changes; we explore the
  *embedding* space of one fixed topology. Different stops merely *evoke* surfaces
  of different genus; the underlying object's topology is constant.
- "One 4D rotation visits all the shapes" — false. A rigid rotation is isometric;
  cube→sphere→torus are non-isometric. The shapes come from a non-rigid blend; the
  4D rotation is the carrier/flourish, not the generator.
- Sphere eversion / regular homotopy — nice mood, wrong category (those are about
  immersed surfaces, not a point-lattice).

---

## Recommendation in five lines

1. Make the dial a continuous angle that walks a **palindrome ring** of stops
   `Cube→Sphere→Cylinder→Torus→Möbius→…→Cube`, smoothstep-blended — closure is
   exact and free because the ends are the same embedding.
2. Add the missing **Cylinder** stop so sphere→torus isn't a crush.
3. Replace the 3D `tumble` with a real **R⁴ isoclinic rotation projected to R³**,
   phased to be identity at every stop — that is the literal "rotate through a
   higher dimension," continuity preserved.
4. Fix overlap by **normalizing each embedding to ≥0.84 nearest-neighbour spacing**:
   torus `R=3.2, rr∈[0.8,2.6]`, Möbius thickness factor `0.84`, both at
   `MarbleSize≈0.5` eased in with the morph.
5. Be honest in copy: it's an **exhaustive sweep of the embedding family of one
   fixed lattice**, carried by a genuine 4D rotation — not a change of topology.

## Sources

- Regular homotopy — https://en.wikipedia.org/wiki/Regular_homotopy
- Sphere eversion — https://en.m.wikipedia.org/wiki/Sphere_eversion
- Rotations in 4-dimensional Euclidean space — https://en.wikipedia.org/wiki/Rotations_in_4-dimensional_Euclidean_space
- Hopf fibration — https://en.wikipedia.org/wiki/Hopf_fibration
- Clifford torus — https://en.wikipedia.org/wiki/Clifford_torus
- Uniform spherical grids via equal-area cube→sphere projection (PDF) — https://num.math.uni-goettingen.de/plonka/pdfs/cubsphere3.pdf
- Survey of cube-mapping methods (cube→sphere) — https://link.springer.com/article/10.1007/s00371-019-01708-4
