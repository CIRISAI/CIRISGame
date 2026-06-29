//! # ciris-game-engine
//!
//! The Bevy 0.19 view layer for CIRISGame. The deterministic rules live in
//! [`ciris_game_engine_core`]; this crate re-exports that API unchanged and adds
//! the rendered presentation of the rhombic-dodecahedral lattice (DESIGN_BRIEF
//! §3). Rendering is entirely behind the `render` feature so the `headless`
//! build (CI / AI tournaments) links no GPU code.
//!
//! This is a first cut of BACKLOG #5: glass shells, emissive steward cores, and
//! faint ghost markers under the §2.2 lighting rig and a panorbit camera. The
//! richer §3/§4 material work (Gray-Scott R-D, the `ExtendedMaterial` rim, the
//! dead-group mist) is intentionally deferred — see the `// TODO §3.x` markers.

#![forbid(unsafe_code)]

// Re-export the deterministic core unchanged. `engine_core::` reaches the whole
// crate; the glob lifts the common API (Board, Coord, Steward, GameState, …)
// into this crate's root so callers can `use ciris_game_engine::Board`.
pub use ciris_game_engine_core as engine_core;
pub use ciris_game_engine_core::*;

#[cfg(feature = "render")]
pub mod palette;

#[cfg(feature = "render")]
mod attract;

#[cfg(feature = "render")]
mod cube;

#[cfg(feature = "render")]
mod effects;

// Dev screenshot capture is native-only: `save_to_disk` writes a file on native
// but triggers a browser download on wasm (see `capture.rs`).
#[cfg(all(feature = "render", not(target_arch = "wasm32")))]
mod capture;

#[cfg(feature = "render")]
mod endgame;

// `environment` (the old warm horizon dome) is retired — replaced by the
// deep-space starfield enclosure in `cube`. File kept for reference, not compiled.

#[cfg(feature = "render")]
mod fonts;

// Dormant for now — the plasma wireframe was retired in favour of empty-position
// orbs; kept for a future scaffold attempt.
#[cfg(feature = "render")]
#[allow(dead_code)]
mod geometry;

#[cfg(feature = "render")]
mod hover;

#[cfg(feature = "render")]
mod i18n;

#[cfg(feature = "render")]
mod intro;

#[cfg(feature = "render")]
mod lighting;

#[cfg(feature = "render")]
mod materials;

#[cfg(feature = "render")]
mod mist;

#[cfg(feature = "render")]
mod navigation;

#[cfg(feature = "render")]
mod orb;

#[cfg(feature = "render")]
mod plasma;

#[cfg(feature = "render")]
mod render;

#[cfg(feature = "render")]
mod screensaver;

#[cfg(feature = "render")]
mod signets;

#[cfg(feature = "render")]
mod tendrils;

#[cfg(feature = "render")]
mod state;

#[cfg(feature = "render")]
mod topology;

#[cfg(feature = "render")]
mod ui_theme;

#[cfg(feature = "render")]
mod wizard;

/// The live game, wrapped as a Bevy `Resource`. The screensaver driver
/// (`screensaver.rs`) advances it; the render sync system (`render.rs`) observes
/// it. Available in every build because `bevy_ecs` links regardless of the
/// render feature. Not `Clone`/`Debug` — `GameState` carries an RNG.
#[derive(bevy::ecs::resource::Resource)]
pub struct BoardResource(pub GameState);

/// A 32-byte game seed derived from a `u64` counter. Deterministic and distinct
/// per screensaver round (the counter advances on every restart).
pub(crate) fn seed_from_counter(counter: u64) -> [u8; 32] {
    let mut seed = [0u8; 32];
    seed[..8].copy_from_slice(&counter.to_le_bytes());
    seed
}

/// Build and run the CIRISGame application.
///
/// With `render` (native / wasm) this opens a window and draws the lattice. In a
/// `headless` build it spins up a `MinimalPlugins` app with no rendering — the
/// shape AI tournaments and CI link against.
#[cfg(feature = "render")]
pub fn run() {
    render::run_app();
}

/// Headless entry point: a `MinimalPlugins` app holding the board, no GPU.
#[cfg(not(feature = "render"))]
pub fn run() {
    use bevy::app::{App, PluginGroup, ScheduleRunnerPlugin};
    use bevy::MinimalPlugins;

    App::new()
        .add_plugins(MinimalPlugins.set(ScheduleRunnerPlugin::run_once()))
        .insert_resource(BoardResource(GameState::new(
            DEFAULT_BOARD_N,
            seed_from_counter(0),
        )))
        .run();
}
