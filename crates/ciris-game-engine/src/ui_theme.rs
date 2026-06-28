//! Shared in-engine UI theme + spawn helpers for the intro and the setup wizard
//! (DESIGN_BRIEF §5 typography, §2.1 palette). Everything is built imperatively
//! with the [`ChildOf`] relationship rather than `with_children` closures, so the
//! data-driven rebuild-on-change in `intro.rs` / `wizard.rs` stays flat.
//!
//! Both screens overlay opaque Bone panels over the live screensaver, leaving a
//! single transparent rectangle that frames the hero — no camera viewport
//! juggling, one render path, webgl2-safe (see `intro.rs`).

use bevy::prelude::*;

use crate::palette;

// ── type stack (DESIGN_BRIEF §5.1) ──────────────────────────────────────
/// Inter — HUD labels, steward names, button text.
pub const DISPLAY: &str = "Inter";
/// Source Serif 4 — rulebook lines, taglines, caption strip.
pub const EDITORIAL: &str = "Source Serif 4";
/// JetBrains Mono — counters, endpoint URLs, numeric readouts.
pub const MONO: &str = "JetBrains Mono";

// ── size scale, 1× = 16 px (DESIGN_BRIEF §5.1) ──────────────────────────
pub const SIZE_XS: f32 = 12.0;
pub const SIZE_SM: f32 = 14.0;
pub const SIZE_BASE: f32 = 16.0;
pub const SIZE_MD: f32 = 18.0;
pub const SIZE_LG: f32 = 22.0;
pub const SIZE_XL: f32 = 32.0;
/// Kept for completeness with the §5.1 scale (end-screen WILD line); not yet used
/// by the front-of-house screens.
#[allow(dead_code)]
pub const SIZE_2XL: f32 = 48.0;

/// A [`TextFont`] for `family` at `size_px`. Font families resolve through
/// Parley's system font database; when none of Inter / Source Serif 4 / JetBrains
/// Mono is installed (typical on a bare wasm host) Parley falls back to the
/// embedded default font, so text always renders.
pub fn font(family: &str, size_px: f32, weight: FontWeight) -> TextFont {
    TextFont {
        font: family.into(),
        font_size: size_px.into(),
        weight,
        ..default()
    }
}

/// Per-interaction background colours for a button. The active/inactive selection
/// state is baked into `normal` at build time (the UI is rebuilt on every change),
/// so `hover`/`pressed` are just nudges of that base.
#[derive(Component, Clone, Copy)]
pub struct ButtonColors {
    pub normal: Color,
    pub hover: Color,
    pub pressed: Color,
}

/// Background + text colours for a button in one of its two visual roles.
pub struct BtnSpec {
    pub colors: ButtonColors,
    pub text: Color,
}

impl BtnSpec {
    /// Clay fill, Bone text — primary actions (Play, Start, Next) and the
    /// currently-selected option in a segmented control.
    pub fn filled() -> Self {
        BtnSpec {
            colors: ButtonColors {
                normal: palette::CLAY_SRGB,
                hover: Color::srgb(0.89, 0.52, 0.40),
                pressed: Color::srgb(0.78, 0.41, 0.30),
            },
            text: palette::BONE_SRGB,
        }
    }

    /// Linen fill, Ink text — secondary actions (Back, Skip) and unselected
    /// options in a segmented control.
    pub fn outline() -> Self {
        BtnSpec {
            colors: ButtonColors {
                normal: palette::LINEN_SRGB,
                hover: Color::srgb(0.86, 0.85, 0.81),
                pressed: Color::srgb(0.80, 0.79, 0.75),
            },
            text: palette::INK_SRGB,
        }
    }
}

/// Spawn a child node under `parent` with a background fill, returning its id.
pub fn node(commands: &mut Commands, parent: Entity, node: Node, background: Color) -> Entity {
    commands
        .spawn((node, BackgroundColor(background), ChildOf(parent)))
        .id()
}

/// Spawn a transparent child container under `parent`, returning its id.
pub fn container(commands: &mut Commands, parent: Entity, node: Node) -> Entity {
    commands.spawn((node, ChildOf(parent))).id()
}

/// Spawn a text node under `parent`.
pub fn text(
    commands: &mut Commands,
    parent: Entity,
    content: impl Into<String>,
    text_font: TextFont,
    color: Color,
) -> Entity {
    commands
        .spawn((
            Text::new(content),
            text_font,
            TextColor(color),
            ChildOf(parent),
        ))
        .id()
}

/// Spawn a labelled button carrying the `action` marker component, returning the
/// button entity. The single [`button_visuals`] system animates its background.
pub fn button<M: Component>(
    commands: &mut Commands,
    parent: Entity,
    action: M,
    label: impl Into<String>,
    font_size: f32,
    spec: BtnSpec,
) -> Entity {
    let BtnSpec {
        colors,
        text: text_color,
    } = spec;
    let button = commands
        .spawn((
            Button,
            Node {
                padding: UiRect::axes(Val::Px(16.0), Val::Px(10.0)),
                margin: UiRect::all(Val::Px(4.0)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(colors.normal),
            colors,
            action,
            ChildOf(parent),
        ))
        .id();
    text(
        commands,
        button,
        label,
        font(DISPLAY, font_size, FontWeight::MEDIUM),
        text_color,
    );
    button
}

/// Animate every button's background to match its [`Interaction`]. Registered
/// once, runs in all UI states.
pub fn button_visuals(
    mut query: Query<(&Interaction, &ButtonColors, &mut BackgroundColor), Changed<Interaction>>,
) {
    for (interaction, colors, mut background) in &mut query {
        background.0 = match interaction {
            Interaction::Pressed => colors.pressed,
            Interaction::Hovered => colors.hover,
            Interaction::None => colors.normal,
        };
    }
}

/// Build the standard front-of-house overlay: a transparent full-screen root
/// carrying `root_marker`, a Bone top strip, a hero band whose centre is a framed
/// transparent rectangle (the live screensaver shows through), and an opaque Bone
/// content panel filling the rest. Returns the content-panel entity for the
/// caller to fill. `band_pct` sizes the hero band; `justify` lays out the panel's
/// children along its main (vertical) axis.
pub fn hero_overlay(
    commands: &mut Commands,
    root_marker: impl Bundle,
    band_pct: f32,
    justify: JustifyContent,
) -> Entity {
    let root = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(0.0),
                left: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            // Sit above the screensaver's endgame ceremony UI, which the running
            // game may raise underneath us in any front-of-house state.
            GlobalZIndex(100),
            root_marker,
        ))
        .id();

    // Bone margin above the hero.
    node(
        commands,
        root,
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(6.0),
            ..default()
        },
        palette::BONE_SRGB,
    );

    // Hero band: Bone fillers around a framed transparent rectangle.
    let band = container(
        commands,
        root,
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(band_pct),
            flex_direction: FlexDirection::Row,
            ..default()
        },
    );
    let filler = |commands: &mut Commands| {
        node(
            commands,
            band,
            Node {
                flex_grow: 1.0,
                height: Val::Percent(100.0),
                ..default()
            },
            palette::BONE_SRGB,
        );
    };
    filler(commands);
    commands.spawn((
        Node {
            width: Val::Percent(50.0),
            height: Val::Percent(100.0),
            border: UiRect::all(Val::Px(1.5)),
            ..default()
        },
        BorderColor::all(palette::STONE_SRGB),
        ChildOf(band),
    ));
    filler(commands);

    // Opaque Bone content panel filling the rest.
    node(
        commands,
        root,
        Node {
            width: Val::Percent(100.0),
            flex_grow: 1.0,
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            justify_content: justify,
            padding: UiRect::all(Val::Px(20.0)),
            row_gap: Val::Px(8.0),
            overflow: Overflow::clip(),
            ..default()
        },
        palette::BONE_SRGB,
    )
}

/// Row direction honouring right-to-left scripts (Arabic / Persian / Urdu).
pub fn row(rtl: bool) -> FlexDirection {
    if rtl {
        FlexDirection::RowReverse
    } else {
        FlexDirection::Row
    }
}
