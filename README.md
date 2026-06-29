# CIRISGame

CIRISGame is a quiet four-player game that plays out inside a glowing 3D crystal of glass marbles. It's also a small, playable picture of a serious idea from [CIRIS](https://github.com/CIRISAI): **when something grows too big and collapses, the collapse can *create*, not just destroy.**

## The idea in one line

**Don't let any of your groups grow to seven.**

## How to play

Four players — the **stewards** — each own one colour:

- **Sienna** (clay-orange)
- **Lapis** (blue)
- **Verdigris** (green)
- **Kaolin** (off-white)

On your turn you place one marble of your colour on any empty spot. Then:

- **Marbles connect.** When two of your marbles sit next to each other they automatically join into a group (a "mesh"), shown by a glowing glass tube between them. Groups can be any size and can merge.
- **The one rule — seven is too many.** The moment one of your groups reaches **seven** marbles, it **collapses**.
- **Collapse creates.** The collapsed group breaks apart: some marbles come back as small live **pairs** of your colour, and the rest become **dead green cells** that stay on the board forever — obstacles everyone has to play around.
- **Your score is your dead green cells.** Fewer is better. When the board fills up, the player with the **fewest** dead cells **wins**.
- **Or everybody wins (WILD).** If *every* player finishes with **zero** dead cells, the whole board lights up in a shared celebration — the cooperative ending.

You can never capture or remove another player's marbles. The only thing that can kill your marbles is your *own* group growing too big. So the game is about **restraint**: grow and connect, but don't tip over seven.

## The board

The play space is a 3D crystal lattice — a rhombic-dodecahedral (FCC) honeycomb, the 3D cousin of a hex grid. Each marble can connect to up to **12** nearby same-colour marbles. Marbles on the **edges and corners** have fewer neighbours, so a corner is a calmer (but lonelier) place to play. The default board has **63 spots**.

## Seeing it

- **Topology dial** (top-left): rotate the *same* game through different shapes — cube → sphere → cylinder → torus → Möbius — to read the structure from every angle. Click a marker to jump to a shape, or drag to scrub between them.
- **Steward signets**: four glowing glass shapes float out at the edges (a cube, a ring, a gem, a Möbius strip), one per player; the player whose turn it is glows brightest. With the coloured glow over the top and bottom of space, they tell you which way is up.
- **Tendrils**: hover over a marble to see soft tendrils of light reaching to the spots it could connect to.
- Everything floats in deep black space.

## Players

Humans or computer. Computer players come in Easy / Medium / Hard / Brutal — all think for the same 2 seconds. By default the game runs as a **screensaver**: four computer players, quietly playing on forever.

## Run it

Play in a browser: **https://cirisai.github.io/CIRISGame/**

Build and run locally (Rust + [Bevy](https://bevyengine.org)):

```bash
cargo run -p ciris-game-engine                    # native window
cargo run -p ciris-game-engine -- --screensaver   # boot straight into the screensaver
```

## Why

The "collapse is generative" idea comes from CIRIS's coherence-collapse work. The deeper story — and how the game maps onto it — is in [`MISSION.md`](./MISSION.md). The full spec is in [`docs/DESIGN_BRIEF.md`](./docs/DESIGN_BRIEF.md).

## License

AGPL-3.0-or-later.
