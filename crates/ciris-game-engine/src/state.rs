//! Front-of-house state machine and the two configuration resources the wizard
//! writes (DESIGN_BRIEF §6.2 mode routing, §6.3 per-slot defaults, §7 BoardView).
//!
//! The flow is three Bevy [`States`]: [`AppScreen::Intro`] → [`AppScreen::Setup`]
//! → [`AppScreen::Playing`]. The self-running screensaver (`screensaver.rs` +
//! `render.rs`) keeps advancing the board underneath in **all three** states;
//! these states only gate which UI overlays and whether the hero is masked to a
//! contained rectangle (Intro/Setup) or shown full-screen (Playing).
//!
//! [`AppMode`] records how the app was launched. A browser `?mode=agent` query
//! (or the `CIRISGAME_MODE=agent` env var on native) flips the player-slot
//! defaults to an Agent in seat 0 and re-frames wizard step 2 as §7 BoardView
//! delivery rather than §6.7 accessibility — same [`ViewConfig`], two labelings.

use bevy::prelude::*;
use ciris_game_engine_core::Difficulty;

/// The front-of-house screens. Default is [`AppScreen::Attract`] — the live
/// screensaver with a single "Play Now" button; clicking it opens the overlay.
#[derive(States, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AppScreen {
    /// Landing: full-screen live screensaver + a "Play Now" button. No overlay
    /// until the player asks for it.
    #[default]
    Attract,
    /// Click-through that teaches the one rule over the live hero.
    Intro,
    /// The setup wizard: players, view/accessibility config, language.
    Setup,
    /// The configured game; hero fills the screen, no overlay.
    Playing,
}

/// How the app was launched, which decides the wizard's defaults + framing.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AppMode {
    /// A person is playing: seat 0 defaults to Human; step 2 reads as
    /// accessibility settings.
    #[default]
    Human,
    /// An out-of-process agent is driving: seat 0 defaults to Agent; step 2
    /// reads as §7 BoardView delivery knobs.
    Agent,
}

/// Detect a "boot straight into the screensaver" launch: the `--screensaver` CLI
/// flag (or `CIRISGAME_MODE=screensaver`) on native, `?mode=screensaver` /
/// `#screensaver` in the URL on wasm. Skips the Attract button and the overlay,
/// landing directly on the clean ambient board (kiosk / display use).
pub fn detect_screensaver() -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(window) = web_sys::window() {
            if let Ok(search) = window.location().search() {
                if search.contains("screensaver") {
                    return true;
                }
            }
            if let Ok(hash) = window.location().hash() {
                if hash.contains("screensaver") {
                    return true;
                }
            }
        }
        false
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::env::args().any(|a| a == "--screensaver")
            || matches!(std::env::var("CIRISGAME_MODE"), Ok(v) if v.eq_ignore_ascii_case("screensaver"))
    }
}

/// Detect [`AppMode`] from the environment: the `?mode=agent` URL query on wasm,
/// the `CIRISGAME_MODE` env var on native. Anything but `agent` → [`AppMode::Human`].
pub fn detect_mode() -> AppMode {
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(window) = web_sys::window() {
            if let Ok(search) = window.location().search() {
                if search.contains("mode=agent") {
                    return AppMode::Agent;
                }
            }
        }
        AppMode::Human
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        match std::env::var("CIRISGAME_MODE") {
            Ok(value) if value.eq_ignore_ascii_case("agent") => AppMode::Agent,
            _ => AppMode::Human,
        }
    }
}

/// What kind of player occupies a steward seat (DESIGN_BRIEF §5.4 segmented
/// control). [`Difficulty`] is only meaningful for [`PlayerKind::Computer`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerKind {
    Human,
    Computer,
    Agent,
}

/// One steward seat's configuration.
#[derive(Debug, Clone, Copy)]
pub struct SlotConfig {
    pub kind: PlayerKind,
    /// Active only when `kind == Computer`; otherwise carried but unused.
    pub difficulty: Difficulty,
}

/// The four-seat roster the wizard assembles and hands to gameplay.
#[derive(Resource, Debug, Clone)]
pub struct RosterConfig {
    /// Seats in steward-slot order: Sienna, Lapis, Verdigris, Kaolin.
    pub slots: [SlotConfig; 4],
}

impl RosterConfig {
    /// The default roster for a launch mode: one Human (or Agent, in agent mode)
    /// in seat 0 and three Computer opponents — the "1 human + 3 AI" default
    /// (DESIGN_BRIEF §6.3), with a spread of difficulties to teach play strength.
    pub fn default_for(mode: AppMode) -> Self {
        let seat0 = match mode {
            AppMode::Agent => PlayerKind::Agent,
            AppMode::Human => PlayerKind::Human,
        };
        RosterConfig {
            slots: [
                SlotConfig {
                    kind: seat0,
                    difficulty: Difficulty::Easy,
                },
                SlotConfig {
                    kind: PlayerKind::Computer,
                    difficulty: Difficulty::Medium,
                },
                SlotConfig {
                    kind: PlayerKind::Computer,
                    difficulty: Difficulty::Hard,
                },
                SlotConfig {
                    kind: PlayerKind::Computer,
                    difficulty: Difficulty::Easy,
                },
            ],
        }
    }
}

/// BoardView delivery format (DESIGN_BRIEF §7.2–§7.4). For humans this same field
/// chooses the §6.7 flat top-down / 2D alternate (Ascii) vs. the full 3D view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewFormat {
    Json,
    Ascii,
    Png,
    Animation,
}

/// Visual effects budget. For humans this is the "effects quality" control; for
/// agents it is folded into the graphics yes/no knob.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quality {
    Low,
    Medium,
    High,
}

/// UI text size (DESIGN_BRIEF §5.1 scale multiplier for accessibility).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextScale {
    Small,
    Normal,
    Large,
}

impl TextScale {
    /// Multiplier applied to the §5.1 type scale.
    pub fn factor(self) -> f32 {
        match self {
            TextScale::Small => 0.85,
            TextScale::Normal => 1.0,
            TextScale::Large => 1.25,
        }
    }
}

/// The single shared view/accessibility configuration. The wizard presents it two
/// ways — §7 BoardView delivery for agents, §6.7/§7.7 accessibility for humans —
/// but both edit *this one resource*. "Same machinery, two labelings."
#[derive(Resource, Debug, Clone)]
pub struct ViewConfig {
    /// Render graphics at all (agent: graphics yes/no; human: effects quality > none).
    pub graphics: bool,
    /// Deliver motion (agent: video/animation; human: inverse of reduced-motion).
    pub animation: bool,
    /// Delivery format / flat-view alternate.
    pub format: ViewFormat,
    /// Animation frame rate, fps (DESIGN_BRIEF §7.4 default 6).
    pub framerate: u8,
    /// Image size in px (DESIGN_BRIEF §7.4 default 128).
    pub size: u32,
    /// Unified ARIA-live announcements (DESIGN_BRIEF §7.7).
    pub screen_reader: bool,
    /// Sound captions strip.
    pub captions: bool,
    /// High-contrast emphasis.
    pub high_contrast: bool,
    /// Colorblind emphasis (extra non-color cues).
    pub colorblind: bool,
    /// UI text size.
    pub text_scale: TextScale,
    /// Mute audio (also forced by `prefers-reduced-motion`, handled elsewhere).
    pub audio_muted: bool,
    /// Effects quality when `graphics` is on.
    pub quality: Quality,
}

impl Default for ViewConfig {
    fn default() -> Self {
        // Mirrors the §7 BoardView defaults: graphics on, a single PNG frame at
        // 128 px, animation off (6 fps / 10 frames when enabled).
        ViewConfig {
            graphics: true,
            animation: false,
            format: ViewFormat::Png,
            framerate: 6,
            size: 128,
            screen_reader: false,
            captions: false,
            high_contrast: false,
            colorblind: false,
            text_scale: TextScale::Normal,
            audio_muted: false,
            quality: Quality::High,
        }
    }
}

/// The framerate options cycled by the wizard control.
pub const FRAMERATES: [u8; 3] = [6, 12, 24];
/// The image-size options cycled by the wizard control (DESIGN_BRIEF §7.4).
pub const SIZES: [u32; 4] = [96, 128, 192, 256];

/// Register the state machine and seed the configuration resources from the
/// detected launch mode. Added by `render.rs` before the UI plugins.
pub fn plugin(app: &mut App) {
    let mode = detect_mode();
    // A `--screensaver` launch drops straight onto the clean ambient board;
    // otherwise land on Attract (screensaver + a "Play Now" button).
    let start = if detect_screensaver() {
        AppScreen::Playing
    } else {
        AppScreen::Attract
    };
    app.insert_state(start)
        .insert_resource(mode)
        .insert_resource(RosterConfig::default_for(mode))
        .insert_resource(ViewConfig::default())
        .add_systems(Update, skip_to_game);
}

/// Escape hatch: pressing Escape from Intro or Setup jumps straight to
/// [`AppScreen::Playing`], dismissing the whole front-of-house. The roster/view
/// resources already hold working defaults, so skipping is always safe — and it
/// guarantees the overlay can never trap the player, whatever the wizard state.
fn skip_to_game(
    keys: Res<ButtonInput<KeyCode>>,
    screen: Res<State<AppScreen>>,
    mut next: ResMut<NextState<AppScreen>>,
) {
    if *screen.get() != AppScreen::Playing && keys.just_pressed(KeyCode::Escape) {
        next.set(AppScreen::Playing);
    }
}
