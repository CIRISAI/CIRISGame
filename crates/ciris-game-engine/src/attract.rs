//! Attract screen (the default landing): the full-screen live screensaver with a
//! single "Play Now" button at the bottom centre. Clicking it opens the
//! front-of-house overlay ([`AppScreen::Intro`] → Setup → Playing). A
//! `--screensaver` launch (`state::detect_screensaver`) skips this entirely and
//! drops straight onto the clean ambient board.

use bevy::prelude::*;

use crate::i18n::Localization;
use crate::state::AppScreen;
use crate::ui_theme as theme;

/// Root of the attract overlay (just the Play Now button), despawned on exit.
#[derive(Component)]
struct AttractRoot;

/// The Play Now button marker.
#[derive(Component, Clone, Copy)]
struct PlayNow;

pub(crate) fn plugin(app: &mut App) {
    app.add_systems(OnEnter(AppScreen::Attract), enter_attract)
        .add_systems(Update, play_now_action.run_if(in_state(AppScreen::Attract)));
}

/// Spawn the bottom-centre Play Now button over the live screensaver.
fn enter_attract(mut commands: Commands, i18n: Res<Localization>) {
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
                // Column so the main axis is vertical: FlexEnd pins to the
                // bottom, AlignItems::Center centres horizontally → bottom-centre.
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::FlexEnd,
                padding: UiRect::bottom(Val::Px(52.0)),
                ..default()
            },
            GlobalZIndex(100),
        ))
        .id();

    theme::button(
        &mut commands,
        root,
        PlayNow,
        i18n.t("play-now"),
        theme::SIZE_LG,
        theme::BtnSpec::filled(),
    );
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
