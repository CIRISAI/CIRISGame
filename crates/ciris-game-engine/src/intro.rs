//! The intro: a 3-screen click-through that teaches the one rule over a contained
//! live hero (DESIGN_BRIEF §8.3 first-visit panel, §5 typography).
//!
//! The screensaver keeps running full-window underneath; this screen overlays
//! opaque Bone panels that mask everything except a framed rectangle near the top
//! — so the resting hero reads as "rendered in ~a quarter of the screen,
//! contained". Each screen teaches one rule line; the final Play button (and the
//! Skip shortcut) advance to [`AppScreen::Setup`].
//!
//! The UI is rebuilt from scratch whenever the screen index changes — there are a
//! couple dozen nodes, so the rebuild is free and the spawn code reads top-down.

use bevy::prelude::*;

use crate::i18n::Localization;
use crate::palette;
use crate::state::{AppScreen, ViewConfig};
use crate::ui_theme as theme;

/// How many click-through screens the intro has.
const SCREENS: usize = 3;

/// Which intro screen is showing (`0..SCREENS`). Mutating it triggers a rebuild.
#[derive(Resource)]
struct IntroScreen(usize);

/// Marks the intro UI root so the rebuild can despawn the whole tree.
#[derive(Component)]
struct IntroRoot;

/// What a clicked intro button does.
#[derive(Component, Clone, Copy)]
enum IntroAction {
    Back,
    Next,
    /// Skip the teaching and go straight to setup.
    Skip,
    /// Finish the teaching and go to setup.
    Play,
}

/// Register the intro screen's resources and systems.
pub fn plugin(app: &mut App) {
    app.insert_resource(IntroScreen(0))
        .add_systems(OnEnter(AppScreen::Intro), enter_intro)
        .add_systems(
            Update,
            (rebuild_intro, intro_actions).run_if(in_state(AppScreen::Intro)),
        );
}

/// Reset to the first screen on (re-)entry; the assignment marks the resource
/// changed, so `rebuild_intro` paints it this frame.
fn enter_intro(mut screen: ResMut<IntroScreen>) {
    screen.0 = 0;
}

/// Handle clicks: advance / retreat the screen index or move on to setup.
fn intro_actions(
    actions: Query<(&Interaction, &IntroAction), Changed<Interaction>>,
    mut screen: ResMut<IntroScreen>,
    mut next: ResMut<NextState<AppScreen>>,
) {
    for (interaction, action) in &actions {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match action {
            IntroAction::Back => screen.0 = screen.0.saturating_sub(1),
            IntroAction::Next => screen.0 = (screen.0 + 1).min(SCREENS - 1),
            IntroAction::Skip | IntroAction::Play => next.set(AppScreen::Setup),
        }
    }
}

/// Despawn the old tree and rebuild for the current screen when it changes.
fn rebuild_intro(
    screen: Res<IntroScreen>,
    roots: Query<Entity, With<IntroRoot>>,
    i18n: Res<Localization>,
    view: Res<ViewConfig>,
    mut commands: Commands,
) {
    if !screen.is_changed() {
        return;
    }
    for root in &roots {
        commands.entity(root).despawn();
    }
    build_intro(&mut commands, screen.0, &i18n, view.text_scale.factor());
}

/// Build the full intro overlay for `screen`.
fn build_intro(commands: &mut Commands, screen: usize, i18n: &Localization, scale: f32) {
    let rtl = i18n.current_rtl();

    // Standard overlay: framed live hero at the top, opaque Bone content panel
    // below. The intro centres its few elements vertically.
    let panel = theme::hero_overlay(
        commands,
        (IntroRoot, DespawnOnExit(AppScreen::Intro)),
        40.0,
        JustifyContent::Center,
    );

    // Tagline only on the first screen (editorial Slate).
    if screen == 0 {
        theme::text(
            commands,
            panel,
            i18n.t("intro-tagline"),
            theme::font(theme::EDITORIAL, theme::SIZE_MD * scale, FontWeight::NORMAL),
            palette::SLATE_SRGB,
        );
    }

    // Heading (display bold Ink) — reuse the screen's rule-line title.
    let (title_key, body_key) = match screen {
        0 => ("intro-screen-1-title", "intro-screen-1-body"),
        1 => ("intro-screen-2-title", "intro-screen-2-body"),
        _ => ("intro-screen-3-title", "intro-screen-3-body"),
    };
    theme::text(
        commands,
        panel,
        i18n.t(title_key),
        theme::font(theme::DISPLAY, theme::SIZE_XL * scale, FontWeight::BOLD),
        palette::INK_SRGB,
    );

    // Body copy, centred, in a width-limited column so it wraps.
    let body_box = theme::container(
        commands,
        panel,
        Node {
            width: Val::Percent(62.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            ..default()
        },
    );
    commands.spawn((
        Text::new(i18n.t(body_key)),
        theme::font(theme::EDITORIAL, theme::SIZE_MD * scale, FontWeight::NORMAL),
        TextColor(palette::SLATE_SRGB),
        TextLayout::justify(bevy::text::Justify::Center),
        ChildOf(body_box),
    ));

    // Quiet caption rule line.
    theme::text(
        commands,
        panel,
        i18n.t("caption-rule"),
        theme::font(theme::EDITORIAL, theme::SIZE_XS * scale, FontWeight::NORMAL),
        palette::STONE_SRGB,
    );

    // Navigation row: secondary action on one side, primary on the other.
    let nav = theme::container(
        commands,
        panel,
        Node {
            width: Val::Percent(62.0),
            flex_direction: theme::row(rtl),
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            margin: UiRect::top(Val::Px(16.0)),
            ..default()
        },
    );

    // Left (reading-start): Skip on the first screen, Back afterwards.
    if screen == 0 {
        theme::button(
            commands,
            nav,
            IntroAction::Skip,
            i18n.t("intro-skip"),
            theme::SIZE_SM * scale,
            theme::BtnSpec::outline(),
        );
    } else {
        theme::button(
            commands,
            nav,
            IntroAction::Back,
            i18n.t("intro-back"),
            theme::SIZE_SM * scale,
            theme::BtnSpec::outline(),
        );
    }

    // Right (reading-end): Next, or Play on the last screen.
    if screen + 1 < SCREENS {
        theme::button(
            commands,
            nav,
            IntroAction::Next,
            i18n.t("intro-next"),
            theme::SIZE_BASE * scale,
            theme::BtnSpec::filled(),
        );
    } else {
        theme::button(
            commands,
            nav,
            IntroAction::Play,
            i18n.t("intro-play"),
            theme::SIZE_BASE * scale,
            theme::BtnSpec::filled(),
        );
    }
}
