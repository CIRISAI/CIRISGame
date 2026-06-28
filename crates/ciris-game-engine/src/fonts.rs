//! UI font loading (DESIGN_BRIEF §5.1 type stack).
//!
//! `ui_theme` builds text with `FontSource::Family("Inter" | "Source Serif 4" |
//! "JetBrains Mono")`. Parley resolves those names against its font collection,
//! and a family only enters that collection when a `Font` asset carrying that
//! embedded family name is loaded (`bevy_text::load_font_assets_into_font_
//! collection`). On a bare wasm host there are **no** system fonts, so without
//! shipping + loading these faces every `Family(...)` look-up resolves to
//! nothing and the UI renders blank — which is exactly what happened in the
//! browser. So: load all three at startup and keep their handles alive.

use bevy::prelude::*;
use bevy::text::Font;

/// The three UI faces, in `ui_theme` DISPLAY / EDITORIAL / MONO order. The
/// embedded family name of each file matches the `ui_theme` constants exactly
/// (verified: "Inter", "Source Serif 4", "JetBrains Mono").
const FONT_PATHS: [&str; 3] = [
    "fonts/Inter.ttf",
    "fonts/SourceSerif4.ttf",
    "fonts/JetBrainsMono.ttf",
];

/// Holds the UI font handles for the lifetime of the app. While these handles
/// live, the faces stay registered in Parley's collection; dropping them would
/// unregister the families and blank the text.
#[derive(Resource)]
struct UiFonts(#[allow(dead_code)] Vec<Handle<Font>>);

pub(crate) fn plugin(app: &mut App) {
    app.add_systems(Startup, load_fonts);
}

fn load_fonts(mut commands: Commands, asset_server: Res<AssetServer>) {
    let handles = FONT_PATHS.iter().map(|p| asset_server.load(*p)).collect();
    commands.insert_resource(UiFonts(handles));
}
