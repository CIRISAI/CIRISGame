//! Attract screen (the default landing): the full-screen live screensaver with a
//! single "Play Now" button at the bottom centre. Clicking it opens the
//! front-of-house overlay ([`AppScreen::Intro`] → Setup → Playing). A
//! `--screensaver` launch (`state::detect_screensaver`) skips this entirely and
//! drops straight onto the clean ambient board.
//!
//! The Play Now button cycles through all 29 language labels every 2.5 s (English
//! every 4th slot) so the attract screen telegraphs multilingual support even to
//! a first-time visitor. English at every 4th position keeps the button legible
//! to the majority. Non-Latin scripts will render as TOFU until a richer font is
//! bundled, but the cycling rhythm and the English fallback still read correctly.

use bevy::prelude::*;

use crate::i18n::LANGS;
use crate::state::AppScreen;
use crate::ui_theme as theme;

/// How long each language label is shown on the attract button.
const CYCLE_SECS: f32 = 2.5;

/// Root of the attract overlay (just the Play Now button), despawned on exit.
#[derive(Component)]
struct AttractRoot;

/// The Play Now button marker.
#[derive(Component, Clone, Copy)]
struct PlayNow;

/// Persistent state for the Play Now label cycle. Survives attract entry/exit so
/// the rotation is continuous and never resets on re-entry.
#[derive(Resource)]
struct PlayNowState {
    timer: Timer,
    /// How many ticks have elapsed since startup (includes ticks during other
    /// screens so the cycle always advances when attract is re-entered).
    seq: u32,
}

impl Default for PlayNowState {
    fn default() -> Self {
        Self {
            timer: Timer::from_seconds(CYCLE_SECS, TimerMode::Repeating),
            seq: 0,
        }
    }
}

pub(crate) fn plugin(app: &mut App) {
    app.init_resource::<PlayNowState>()
        .add_systems(OnEnter(AppScreen::Attract), enter_attract)
        .add_systems(
            Update,
            (play_now_action, cycle_play_now).run_if(in_state(AppScreen::Attract)),
        );
}

/// Index into LANGS for sequence position `seq`.
///
/// Pattern: [L, L, L, EN] repeating — English appears at every 4th slot (3, 7,
/// 11, …) so the button stays legible. The 28 non-English languages fill the
/// other three slots in each group of four, cycling indefinitely.
fn lang_for_seq(seq: u32) -> usize {
    if seq % 4 == 3 {
        return 0; // English
    }
    let non_en_idx = ((seq / 4) * 3 + seq % 4) as usize % 28;
    non_en_idx + 1 // indices 1..=28 are the non-English languages
}

/// The label to show on the attract button for LANGS index `idx`.
/// English → localized "Play Now"; other languages → their own endonym.
fn play_now_label(idx: usize) -> &'static str {
    if idx == 0 {
        "Play Now"
    } else {
        LANGS[idx].1 // endonym in native script (e.g. "Español", "Français")
    }
}

/// Spawn the bottom-centre Play Now button over the live screensaver.
fn enter_attract(mut commands: Commands, state: Res<PlayNowState>) {
    let root = commands
        .spawn((
            AttractRoot,
            DespawnOnExit(AppScreen::Attract),
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(0.0),
                left: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::FlexEnd,
                padding: UiRect::bottom(Val::Px(52.0)),
                ..default()
            },
            GlobalZIndex(100),
        ))
        .id();

    let label = play_now_label(lang_for_seq(state.seq));
    theme::button(
        &mut commands,
        root,
        PlayNow,
        label,
        theme::SIZE_LG,
        theme::BtnSpec::filled(),
    );
}

/// Advance the cycle every `CYCLE_SECS` and update the button label in-place.
fn cycle_play_now(
    mut state: ResMut<PlayNowState>,
    time: Res<Time>,
    btn: Query<&Children, With<PlayNow>>,
    mut texts: Query<&mut Text>,
) {
    if !state.timer.tick(time.delta()).just_finished() {
        return;
    }
    state.seq += 1;
    let label = play_now_label(lang_for_seq(state.seq));
    let Ok(children) = btn.single() else {
        return;
    };
    for child in children.iter() {
        if let Ok(mut text) = texts.get_mut(child) {
            *text = Text::new(label);
            return;
        }
    }
}

/// Open the front-of-house overlay when Play Now is pressed.
fn play_now_action(
    actions: Query<&Interaction, (Changed<Interaction>, With<PlayNow>)>,
    mut next: ResMut<NextState<AppScreen>>,
) {
    for interaction in &actions {
        if *interaction == Interaction::Pressed {
            next.set(AppScreen::Intro);
        }
    }
}
