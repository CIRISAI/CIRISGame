//! Human-turn input and turn-indicator HUD (BACKLOG Tier 0 items 0b + 0c).
//!
//! `place_on_click` — translates a left-click or tap on a hovered legal cell
//! into `apply_move` when the current steward seat is Human.
//!
//! `TurnHud` — a bottom-center pill showing the current steward's color, name,
//! and whether they should click ("your move") or wait ("thinking…").

use bevy::input::touch::Touches;
use bevy::prelude::*;

use crate::hover::HoveredCell;
use crate::palette;
use crate::render::BoardDirty;
use crate::state::{AppScreen, PlayerKind, RosterConfig};
use crate::BoardResource;

/// Locked pigment names in steward-slot order (CLAUDE.md).
const STEWARD_NAMES: [&str; 4] = ["Sienna", "Lapis", "Verdigris", "Kaolin"];

pub(crate) fn plugin(app: &mut App) {
    app.add_systems(OnEnter(AppScreen::Playing), spawn_turn_hud)
        .add_systems(
            Update,
            (place_on_click, update_turn_hud).run_if(in_state(AppScreen::Playing)),
        );
}

// ── click-to-place (0b) ──────────────────────────────────────────────────────

fn place_on_click(
    buttons: Res<ButtonInput<MouseButton>>,
    touches: Res<Touches>,
    hovered: Res<HoveredCell>,
    roster: Res<RosterConfig>,
    mut board: ResMut<BoardResource>,
    mut dirty: ResMut<BoardDirty>,
) {
    // Accept left-click or any touch lift (tap).
    let clicked = buttons.just_pressed(MouseButton::Left)
        || touches.iter_just_released().next().is_some();
    if !clicked {
        return;
    }
    if board.0.is_over() {
        return;
    }
    let slot = board.0.current_steward().slot() as usize;
    if roster.slots[slot].kind != PlayerKind::Human {
        return;
    }
    let Some(idx) = hovered.0 else { return };
    let coord = board.0.board.coord(idx);
    if !board.0.current_legal_moves().contains(&coord) {
        return;
    }
    if board
        .0
        .apply_move(ciris_game_engine_core::Move::place(coord))
        .is_ok()
    {
        dirty.0 = true;
    }
}

// ── turn indicator HUD (0c) ──────────────────────────────────────────────────

/// Marks the HUD root so `update_turn_hud` can find and update it.
#[derive(Component)]
struct TurnHud;

/// Marks the steward color disc inside the HUD.
#[derive(Component)]
struct TurnDisc;

/// Marks the status text node (name + action) inside the HUD.
#[derive(Component)]
struct TurnText;

fn spawn_turn_hud(mut commands: Commands) {
    // Bottom-center pill. Children added in update_turn_hud on the first frame.
    let root = commands
        .spawn((
            TurnHud,
            DespawnOnExit(AppScreen::Playing),
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(24.0),
                left: Val::Percent(50.0),
                // negative margin pulls the node left by half its own width,
                // centering it; Bevy doesn't yet support `translate(-50%, 0)`.
                margin: UiRect::left(Val::Px(-120.0)),
                width: Val::Px(240.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(10.0),
                padding: UiRect::axes(Val::Px(14.0), Val::Px(8.0)),
                border_radius: BorderRadius::all(Val::Px(20.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.08, 0.08, 0.10, 0.88)),
            GlobalZIndex(50),
        ))
        .id();

    // Color disc — border for Kaolin (slot 3) is added dynamically.
    commands.spawn((
        TurnDisc,
        Node {
            width: Val::Px(16.0),
            height: Val::Px(16.0),
            min_width: Val::Px(16.0),
            border_radius: BorderRadius::all(Val::Px(8.0)),
            ..default()
        },
        BackgroundColor(palette::STEWARD_SRGB[0]),
        ChildOf(root),
    ));

    // Status text — filled each frame.
    commands.spawn((
        TurnText,
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(13.0),
            ..default()
        },
        TextColor(palette::BONE_SRGB),
        ChildOf(root),
    ));
}

fn update_turn_hud(
    board: Res<BoardResource>,
    roster: Res<RosterConfig>,
    mut disc: Query<(&mut BackgroundColor, &mut Node), With<TurnDisc>>,
    mut label: Query<(&mut Text, &mut TextColor), With<TurnText>>,
) {
    if board.0.is_over() {
        if let Ok((mut text, mut color)) = label.single_mut() {
            *text = Text::new("Game over");
            *color = TextColor(palette::STONE_SRGB);
        }
        return;
    }
    let steward = board.0.current_steward();
    let slot = steward.slot() as usize;
    let color = palette::STEWARD_SRGB[slot];
    let name = STEWARD_NAMES[slot];
    let is_human = roster.slots[slot].kind == PlayerKind::Human;
    let status = if is_human { "your move" } else { "thinking…" };

    if let Ok((mut bg, mut node)) = disc.single_mut() {
        bg.0 = color;
        // Kaolin mandatory 2 px Ink ring so it reads against the dark backdrop.
        node.border = if slot == 3 {
            UiRect::all(Val::Px(2.0))
        } else {
            UiRect::all(Val::Px(0.0))
        };
    }
    if let Ok((mut text, mut tc)) = label.single_mut() {
        *text = Text::new(format!("{name} — {status}"));
        tc.0 = if is_human {
            palette::BONE_SRGB
        } else {
            palette::STONE_SRGB
        };
    }
}
