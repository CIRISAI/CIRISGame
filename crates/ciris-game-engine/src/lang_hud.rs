//! Persistent language-selector dropdown in the top-right corner of the HUD,
//! positioned below the Tune button. Available in all game states.
//!
//! A single button shows the current language's English name. Clicking opens an
//! inline list of all 29 languages; clicking a language closes the list and
//! switches the active locale immediately.

use bevy::prelude::*;

use crate::i18n::{Localization, LANGS};
use crate::ui_theme as theme;

/// Whether the language dropdown is currently expanded.
#[derive(Resource, Default)]
struct HudLangOpen(bool);

/// Marks the root container of the language HUD widget.
#[derive(Component)]
struct HudLangRoot;

/// The toggle button that opens/closes the language list.
#[derive(Component, Clone, Copy)]
struct HudLangToggle;

/// One language option in the open list.
#[derive(Component, Clone, Copy)]
struct HudLangItem(usize);

pub(crate) fn plugin(app: &mut App) {
    app.init_resource::<HudLangOpen>()
        .add_systems(Startup, spawn_lang_hud)
        .add_systems(Update, (lang_hud_actions, rebuild_lang_hud));
}

fn spawn_lang_hud(mut commands: Commands, i18n: Res<Localization>) {
    build_lang_hud(&mut commands, &i18n, false);
}

/// Rebuild the widget whenever the active language or open state changes.
fn rebuild_lang_hud(
    i18n: Res<Localization>,
    open: Res<HudLangOpen>,
    roots: Query<Entity, With<HudLangRoot>>,
    mut commands: Commands,
) {
    if !(i18n.is_changed() || open.is_changed()) {
        return;
    }
    for root in &roots {
        commands.entity(root).despawn();
    }
    build_lang_hud(&mut commands, &i18n, open.0);
}

/// Handle toggle button and language-item clicks.
fn lang_hud_actions(
    toggles: Query<&Interaction, (Changed<Interaction>, With<HudLangToggle>)>,
    items: Query<(&Interaction, &HudLangItem), Changed<Interaction>>,
    mut open: ResMut<HudLangOpen>,
    mut i18n: ResMut<Localization>,
) {
    for interaction in &toggles {
        if *interaction == Interaction::Pressed {
            open.0 = !open.0;
        }
    }
    for (interaction, item) in &items {
        if *interaction == Interaction::Pressed {
            i18n.set_lang_index(item.0);
            open.0 = false;
        }
    }
}

fn build_lang_hud(commands: &mut Commands, i18n: &Localization, open: bool) {
    let current = i18n.current_index();
    let (_, _, english_name) = LANGS[current];

    // Outer column: toggle button on top, option list below when open.
    let root = commands
        .spawn((
            HudLangRoot,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(62.0), // below the Tune button (top: 16 + ~42px height + 4px gap)
                right: Val::Px(16.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::FlexEnd,
                row_gap: Val::Px(2.0),
                ..default()
            },
            GlobalZIndex(60),
        ))
        .id();

    // Toggle button: shows current language English name.
    theme::button(
        commands,
        root,
        HudLangToggle,
        english_name.to_string(),
        theme::SIZE_XS,
        if open {
            theme::BtnSpec::filled()
        } else {
            theme::BtnSpec::outline()
        },
    );

    // Option list (visible when open).
    if open {
        let list = commands
            .spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::FlexEnd,
                    row_gap: Val::Px(2.0),
                    max_height: Val::Vh(70.0),
                    ..default()
                },
                ChildOf(root),
            ))
            .id();
        for (index, (_, _, eng)) in LANGS.iter().enumerate() {
            theme::button(
                commands,
                list,
                HudLangItem(index),
                (*eng).to_string(),
                theme::SIZE_XS,
                if index == current {
                    theme::BtnSpec::filled()
                } else {
                    theme::BtnSpec::outline()
                },
            );
        }
    }
}
