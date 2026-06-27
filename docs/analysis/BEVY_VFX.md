# Bevy VFX techniques for CIRISGame (liquid / smoke / motes / plasma)

Research notes for the visual passes. **Governing constraint up front**, then per-effect recommendations.

## The governing constraint: WebGL2 has no compute shaders

CIRISGame ships **two wasm artifacts** (DESIGN_BRIEF §1): `app.webgpu.wasm` (primary) and
`app.webgl2.wasm` (fallback); `index.html` probes `navigator.gpu` and loads the match. This
split is exactly what lets us have rich effects *and* broad compatibility — **but only if we
gate compute-dependent effects to the WebGPU artifact and give WebGL2 a fragment-shader / CPU
fallback.**

| Technique | WebGPU wasm | WebGL2 wasm | Native |
|---|---|---|---|
| Compute shaders (GPU particles, GPU fluid, compute R-D) | ✅ | ❌ (none exist) | ✅ |
| Fragment-shader raymarching (volumetric mist/smoke/plasma) | ✅ | ✅ | ✅ |
| Render-to-texture **fragment** ping-pong (R-D the WebGL way) | ✅ | ✅ | ✅ |
| CPU-driven, GPU-batched particles | ✅ | ✅ | ✅ |

The first Pages deploy is the **webgl2** artifact (compatibility), so the *baseline* visual
stack must be webgl2-safe; the webgpu artifact can layer richer compute effects on top.

## Per-effect recommendations

### Motes / agent particles / sparkle (§3.9, §4.3, §4.9 atari, §4.7 celebrate)
- **Use `bevy_firework`** — CPU-driven, GPU-batched, **explicitly "WASM and WebGL compatible"**,
  integrates with PBR (fog/shadows/lighting), soft particle edges, one-shot + continuous
  emission. Right tool for the webgl2 deploy. (`bevy_enoki` is a 2D alternative also "works well
  in wasm webgl2 and mobile".)
- **Avoid `bevy_hanabi` on webgl2** — it's GPU-compute based and its docs state wasm support is
  **WebGPU-only** ("compute shaders are not available via the legacy WebGL2 renderer"). Great for
  the *native/webgpu* artifact if we want million-particle scale, but not the webgl2 baseline.
- Counts here are tiny (a few motes per node), so even hand-rolled instanced billboards work; use
  `bevy_firework` for the dispersal sparkle / WILD bursts where counts grow.

### Smoke / mist (§3.6 temp-dead black & perma-dead green, §4.6 dispersal)
- **Custom `Material` + `AsBindGroup`, fragment-shader raymarch** — exactly what the brief already
  specifies (§3.6: raymarched fragment, 32 steps, 3D simplex noise octave 2, freq 1.4,
  `AlphaMode::Opaque` + discard-on-noise-threshold). Fragment raymarch is **webgl2-safe**. This is
  the per-cell mist; do it as a per-mesh material, not a global effect.
- Bevy has a **built-in `VolumetricFog`** (`bevy::light::VolumetricFog`, `step_count` + `jitter`
  TAA) with a 3d/volumetric-fog example — but it's a global/light-shaft (god-ray) system, better
  for the horizon/atmosphere than for per-cell mist. Consider it for ambiance, not the dead-cell mist.

### Plasma / living surface (§4.2 Gray-Scott per mesh, core shimmer, WILD recombination)
- **WebGL2 way = fragment-shader render-to-texture ping-pong** (NOT compute). The brief's "96×96
  R8 ping-pong target" is the classic WebGL R-D pattern: render quad A→B→A each frame with the
  Gray-Scott update in the fragment shader, sample the result as the mesh's surface texture. This
  is webgl2-safe. Caveat: Bevy ping-pong between two textures may need a small custom render-graph
  node or per-frame render-to-image swap (community notes: can require light wgpu/manual pipeline
  config — see Bevy discussion #3294).
- **Cheaper Tier-A/B approximation** (already in use): map the imported static `gs-seed-*.png` per
  steward as the surface texture; animate with scrolling/warp in the fragment shader for a "living"
  feel without the full sim. Upgrade to live ping-pong on the webgpu artifact (or webgl2 fragment
  ping-pong) as a later pass.
- For the WILD "all four R-D seeds desynchronize and recombine" moment, animated procedural plasma
  (layered noise + sin warp in a fragment shader) reads as "plasma" cheaply and is webgl2-safe.

### Liquid (the mist *flow*, glass interior, dispersal cascade)
- True fluid sim (`bevy_salva` SPH, `bevy_eulerian_fluid`, MLS-MPM) is **overkill and mostly
  compute/native** — not webgl2-friendly and far heavier than the game needs. Don't.
- The game's "liquid" is really **flowing volumetric mist inside the glass shell** → use the same
  fragment-raymarch material as smoke with an animated noise flow field (the brief's 0.6 / 0.3
  units/s flow). For surface "liquid glass," that's the StandardMaterial `specular_transmission`
  already in Tier-A. `bevy_water` exists for ocean surfaces but isn't relevant here.

## Recommended architecture (per §1's two-artifact design)
- **WebGL2 baseline (deployed first):** `bevy_firework` motes + custom fragment-raymarch mist +
  static/animated GS textures (or fragment ping-pong R-D) + StandardMaterial glass. All webgl2-safe.
- **WebGPU artifact (richer):** same, optionally upgraded with `bevy_hanabi` for large particle
  counts and compute-based R-D if we want it. Feature-gate behind the `webgpu` cargo feature.
- Keep every effect behind the crate's `render` feature so `headless` stays GPU-free.

## Crates
- `bevy_firework` — CPU particles, webgl2-safe — https://github.com/mbrea-c/bevy_firework
- `bevy_enoki` — 2D particles, webgl2/mobile — https://github.com/Lommix/bevy_enoki
- `bevy_hanabi` — GPU-compute particles, **webgpu-only on wasm** — https://github.com/djeedai/bevy_hanabi
- Bevy built-in `VolumetricFog` — https://bevy.org/examples/3d-rendering/volumetric-fog/

## Sources
- bevy_hanabi (GPU particles; wasm=WebGPU-only): https://github.com/djeedai/bevy_hanabi/blob/main/docs/wasm.md
- bevy_firework (CPU, WASM+WebGL): https://github.com/mbrea-c/bevy_firework
- bevy_enoki (webgl2/mobile): https://github.com/Lommix/bevy_enoki
- Bevy VolumetricFog: https://docs.rs/bevy/latest/bevy/light/struct.VolumetricFog.html · https://github.com/bevyengine/bevy/blob/main/examples/3d/volumetric_fog.rs
- Bevy ping-pong rendering discussion: https://github.com/bevyengine/bevy/discussions/3294
- Gray-Scott shader R-D (technique): https://pierre-couy.dev/simulations/2024/09/gray-scott-shader.html
