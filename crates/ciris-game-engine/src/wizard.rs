//! The setup wizard (DESIGN_BRIEF §5.4 stewards drawer, §6.3 defaults, §7
//! BoardView, §6.7/§7.7 accessibility). Three steps over the contained live hero:
//!
//! 1. **Players** — the four steward seats, each Human / Computer / Agent, with a
//!    difficulty sub-control for Computer seats. Defaults come from the launch
//!    [`AppMode`]: 1 Human (or Agent) + 3 Computer.
//! 2. **View / Accessibility** — *one* [`ViewConfig`] resource presented two ways.
//!    In agent mode it reads as §7 BoardView delivery (graphics, video, format,
//!    framerate, size); in human mode it reads as accessibility settings (reduced
//!    motion, effects quality, flat top-down view, screen-reader, captions,
//!    contrast, colorblind, text size, mute). Same machinery, two labelings.
//! 3. **Language** — pick one of the 29 CIRIS languages; the wizard re-renders in
//!    it immediately.
//!
//! The roster and view config are mutated live as the user clicks, so "finish"
//! is just the transition to [`AppScreen::Playing`]. Like the intro, the whole
//! overlay is rebuilt whenever any input changes — selection state is therefore
//! always read straight from the resources at build time.

use bevy::prelude::*;
use ciris_game_engine_core::Difficulty;

use crate::i18n::{Localization, LANGS};
use crate::palette;
use crate::state::{
    AppMode, AppScreen, PlayerKind, Quality, RosterConfig, TextScale, ViewConfig, ViewFormat,
    FRAMERATES, SIZES,
};
use crate::ui_theme as theme;

/// Number of wizard steps.
const STEPS: usize = 3;

/// Steward pigment names, in slot order. Locked brand constants — never
/// translated (CLAUDE.md), so they live here rather than in the `.ftl` files.
const STEWARD_NAMES: [&str; 4] = ["Sienna", "Lapis", "Verdigris", "Kaolin"];

/// Which wizard step is showing (`0..STEPS`). Mutating it triggers a rebuild.
#[derive(Resource)]
struct WizardStep(usize);

/// Marks the wizard UI root so the rebuild can despawn the whole tree.
#[derive(Component)]
struct WizardRoot;

/// What a clicked wizard control does.
#[derive(Component, Clone, Copy)]
enum WizAction {
    Back,
    Next,
    /// Finish setup and start the game.
    Start,
    // Step 1 — players.
    SetKind(usize, PlayerKind),
    SetDifficulty(usize, Difficulty),
    // Step 2 — agent framing.
    ToggleGraphics,
    ToggleAnimation,
    SetFormat(ViewFormat),
    CycleFramerate,
    CycleSize,
    // Step 2 — human / accessibility framing (same underlying ViewConfig).
    ToggleReducedMotion,
    SetQuality(Quality),
    ToggleFlatView,
    ToggleScreenReader,
    ToggleCaptions,
    ToggleHighContrast,
    ToggleColorblind,
    SetTextScale(TextScale),
    ToggleAudioMute,
    // Step 3 — language.
    SetLang(usize),
}

/// Register the wizard's resources and systems.
pub fn plugin(app: &mut App) {
    app.insert_resource(WizardStep(0))
        .add_systems(OnEnter(AppScreen::Setup), enter_setup)
        .add_systems(
            Update,
            (wizard_actions, rebuild_wizard)
                .chain()
                .run_if(in_state(AppScreen::Setup)),
        );
}

/// Reset to the first step on entry (marks the resource changed → first rebuild).
fn enter_setup(mut step: ResMut<WizardStep>) {
    step.0 = 0;
}

/// Apply a clicked control to the roster / view / language / step / state.
#[allow(clippy::too_many_arguments)]
fn wizard_actions(
    actions: Query<(&Interaction, &WizAction), Changed<Interaction>>,
    mut step: ResMut<WizardStep>,
    mut roster: ResMut<RosterConfig>,
    mut view: ResMut<ViewConfig>,
    mut i18n: ResMut<Localization>,
    mut next: ResMut<NextState<AppScreen>>,
) {
    for (interaction, action) in &actions {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match *action {
            WizAction::Back => step.0 = step.0.saturating_sub(1),
            WizAction::Next => step.0 = (step.0 + 1).min(STEPS - 1),
            // The roster/view are already live; finishing is just the transition.
            WizAction::Start => next.set(AppScreen::Playing),

            WizAction::SetKind(slot, kind) => roster.slots[slot].kind = kind,
            WizAction::SetDifficulty(slot, difficulty) => {
                roster.slots[slot].difficulty = difficulty
            }

            WizAction::ToggleGraphics => view.graphics = !view.graphics,
            WizAction::ToggleAnimation => view.animation = !view.animation,
            WizAction::SetFormat(format) => view.format = format,
            WizAction::CycleFramerate => {
                let i = FRAMERATES
                    .iter()
                    .position(|&f| f == view.framerate)
                    .unwrap_or(0);
                view.framerate = FRAMERATES[(i + 1) % FRAMERATES.len()];
            }
            WizAction::CycleSize => {
                let i = SIZES.iter().position(|&s| s == view.size).unwrap_or(0);
                view.size = SIZES[(i + 1) % SIZES.len()];
            }

            // Human framing maps onto the same fields, sometimes inverted.
            WizAction::ToggleReducedMotion => view.animation = !view.animation,
            WizAction::SetQuality(quality) => view.quality = quality,
            WizAction::ToggleFlatView => {
                view.format = if view.format == ViewFormat::Ascii {
                    ViewFormat::Png
                } else {
                    ViewFormat::Ascii
                };
            }
            WizAction::ToggleScreenReader => view.screen_reader = !view.screen_reader,
            WizAction::ToggleCaptions => view.captions = !view.captions,
            WizAction::ToggleHighContrast => view.high_contrast = !view.high_contrast,
            WizAction::ToggleColorblind => view.colorblind = !view.colorblind,
            WizAction::SetTextScale(scale) => view.text_scale = scale,
            WizAction::ToggleAudioMute => view.audio_muted = !view.audio_muted,

            WizAction::SetLang(index) => i18n.set_lang_index(index),
        }
    }
}

/// Rebuild the wizard whenever the step or any edited resource changes.
#[allow(clippy::too_many_arguments)]
fn rebuild_wizard(
    step: Res<WizardStep>,
    roster: Res<RosterConfig>,
    view: Res<ViewConfig>,
    i18n: Res<Localization>,
    mode: Res<AppMode>,
    roots: Query<Entity, With<WizardRoot>>,
    mut commands: Commands,
) {
    if !(step.is_changed() || roster.is_changed() || view.is_changed() || i18n.is_changed()) {
        return;
    }
    for root in &roots {
        commands.entity(root).despawn();
    }
    build_wizard(&mut commands, step.0, &roster, &view, &i18n, *mode);
}

/// Shared per-build context to keep helper signatures short.
struct Ctx<'a> {
    i18n: &'a Localization,
    scale: f32,
    rtl: bool,
}

/// Build the whole wizard overlay for `step`.
fn build_wizard(
    commands: &mut Commands,
    step: usize,
    roster: &RosterConfig,
    view: &ViewConfig,
    i18n: &Localization,
    mode: AppMode,
) {
    let ctx = Ctx {
        i18n,
        scale: view.text_scale.factor(),
        rtl: i18n.current_rtl(),
    };
    let panel = theme::hero_overlay(
        commands,
        (WizardRoot, DespawnOnExit(AppScreen::Setup)),
        theme::HERO_SETUP,
        JustifyContent::FlexStart,
    );

    // Title + "Step n of N".
    theme::text(
        commands,
        panel,
        i18n.t("wizard-title"),
        theme::font(theme::DISPLAY, theme::SIZE_LG * ctx.scale, FontWeight::BOLD),
        palette::INK_SRGB,
    );
    theme::text(
        commands,
        panel,
        i18n.ta(
            "wizard-step-of",
            &[
                ("step", (step + 1).to_string()),
                ("total", STEPS.to_string()),
            ],
        ),
        theme::font(theme::MONO, theme::SIZE_XS * ctx.scale, FontWeight::NORMAL),
        palette::STONE_SRGB,
    );

    // Step body.
    let body = theme::container(
        commands,
        panel,
        Node {
            width: Val::Percent(82.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: Val::Px(4.0),
            margin: UiRect::vertical(Val::Px(8.0)),
            ..default()
        },
    );
    match step {
        0 => build_players(commands, &ctx, body, roster),
        1 => build_view(commands, &ctx, body, view, mode),
        _ => build_language(commands, &ctx, body),
    }

    // Navigation row: Back on the reading-start side, Next/Start on the end.
    let nav = theme::container(
        commands,
        panel,
        Node {
            width: Val::Percent(82.0),
            flex_direction: theme::row(ctx.rtl),
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            margin: UiRect::top(Val::Px(12.0)),
            ..default()
        },
    );
    if step > 0 {
        theme::button(
            commands,
            nav,
            WizAction::Back,
            i18n.t("wizard-back"),
            theme::SIZE_SM * ctx.scale,
            theme::BtnSpec::outline(),
        );
    } else {
        // Zero-size spacer so SpaceBetween still pins the primary action to the end.
        theme::container(commands, nav, Node::default());
    }
    if step + 1 < STEPS {
        theme::button(
            commands,
            nav,
            WizAction::Next,
            i18n.t("wizard-next"),
            theme::SIZE_BASE * ctx.scale,
            theme::BtnSpec::filled(),
        );
    } else {
        theme::button(
            commands,
            nav,
            WizAction::Start,
            i18n.t("wizard-start"),
            theme::SIZE_BASE * ctx.scale,
            theme::BtnSpec::filled(),
        );
    }
}

/// Step 1: the four steward seats.
fn build_players(commands: &mut Commands, ctx: &Ctx, body: Entity, roster: &RosterConfig) {
    theme::text(
        commands,
        body,
        ctx.i18n.t("wizard-step-players-title"),
        theme::font(
            theme::DISPLAY,
            theme::SIZE_MD * ctx.scale,
            FontWeight::MEDIUM,
        ),
        palette::INK_SRGB,
    );

    // `slot` indexes several parallel arrays (names, colours, roster), so a plain
    // range loop is clearer than enumerate over any one of them.
    #[allow(clippy::needless_range_loop)]
    for slot in 0..4 {
        let row = theme::container(
            commands,
            body,
            Node {
                width: Val::Percent(100.0),
                flex_direction: theme::row(ctx.rtl),
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                column_gap: Val::Px(8.0),
                margin: UiRect::vertical(Val::Px(3.0)),
                ..default()
            },
        );

        // Left: pigment disc + steward name. Kaolin gets its mandatory Ink ring.
        let left = theme::container(
            commands,
            row,
            Node {
                flex_direction: theme::row(ctx.rtl),
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                ..default()
            },
        );
        let kaolin = slot == 3;
        commands.spawn((
            Node {
                width: Val::Px(12.0),
                height: Val::Px(12.0),
                border: if kaolin {
                    UiRect::all(Val::Px(1.5))
                } else {
                    UiRect::all(Val::Px(0.0))
                },
                ..default()
            },
            BackgroundColor(palette::STEWARD_SRGB[slot]),
            BorderColor::all(palette::INK_SRGB),
            ChildOf(left),
        ));
        theme::text(
            commands,
            left,
            STEWARD_NAMES[slot],
            theme::font(
                theme::DISPLAY,
                theme::SIZE_SM * ctx.scale,
                FontWeight::MEDIUM,
            ),
            palette::INK_SRGB,
        );

        // Right: kind segmented control, with a difficulty row for Computer seats.
        let right = theme::container(
            commands,
            row,
            Node {
                flex_direction: FlexDirection::Column,
                align_items: if ctx.rtl {
                    AlignItems::FlexStart
                } else {
                    AlignItems::FlexEnd
                },
                row_gap: Val::Px(2.0),
                ..default()
            },
        );
        let kinds = theme::container(
            commands,
            right,
            Node {
                flex_direction: theme::row(ctx.rtl),
                column_gap: Val::Px(4.0),
                ..default()
            },
        );
        for kind in [PlayerKind::Human, PlayerKind::Computer, PlayerKind::Agent] {
            option(
                commands,
                ctx,
                kinds,
                WizAction::SetKind(slot, kind),
                ctx.i18n.t(kind_key(kind)),
                roster.slots[slot].kind == kind,
            );
        }
        if roster.slots[slot].kind == PlayerKind::Computer {
            let diffs = theme::container(
                commands,
                right,
                Node {
                    flex_direction: theme::row(ctx.rtl),
                    column_gap: Val::Px(4.0),
                    ..default()
                },
            );
            for difficulty in [
                Difficulty::Easy,
                Difficulty::Medium,
                Difficulty::Hard,
                Difficulty::Brutal,
            ] {
                option(
                    commands,
                    ctx,
                    diffs,
                    WizAction::SetDifficulty(slot, difficulty),
                    ctx.i18n.t(diff_key(difficulty)),
                    roster.slots[slot].difficulty == difficulty,
                );
            }
        }
        if roster.slots[slot].kind == PlayerKind::Agent {
            // Show endpoint URL as a read-only mono label.
            // Full text editing requires a Bevy TextInput widget (planned).
            let url_row = theme::container(
                commands,
                right,
                Node {
                    flex_direction: theme::row(ctx.rtl),
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(4.0),
                    ..default()
                },
            );
            theme::text(
                commands,
                url_row,
                roster.slots[slot].endpoint_url.clone(),
                theme::font(theme::MONO, theme::SIZE_XS * ctx.scale, FontWeight::NORMAL),
                palette::STONE_SRGB,
            );
            theme::text(
                commands,
                url_row,
                "(--p".to_string()
                    + &(slot + 1).to_string()
                    + " agent:url)",
                theme::font(theme::MONO, 8.0 * ctx.scale, FontWeight::NORMAL),
                palette::STONE_SRGB,
            );
        }
    }
}

/// Step 2: the shared [`ViewConfig`], framed for agents or for humans.
fn build_view(commands: &mut Commands, ctx: &Ctx, body: Entity, view: &ViewConfig, mode: AppMode) {
    let i18n = ctx.i18n;
    let title_key = match mode {
        AppMode::Agent => "wizard-step-view-title-agent",
        AppMode::Human => "wizard-step-view-title-human",
    };
    theme::text(
        commands,
        body,
        i18n.t(title_key),
        theme::font(
            theme::DISPLAY,
            theme::SIZE_MD * ctx.scale,
            FontWeight::MEDIUM,
        ),
        palette::INK_SRGB,
    );

    match mode {
        AppMode::Agent => {
            let c = setting_row(commands, ctx, body, i18n.t("view-graphics"));
            toggle(commands, ctx, c, WizAction::ToggleGraphics, view.graphics);

            let c = setting_row(commands, ctx, body, i18n.t("view-animation"));
            toggle(commands, ctx, c, WizAction::ToggleAnimation, view.animation);

            let c = setting_row(commands, ctx, body, i18n.t("view-format"));
            for format in [
                ViewFormat::Json,
                ViewFormat::Ascii,
                ViewFormat::Png,
                ViewFormat::Animation,
            ] {
                option(
                    commands,
                    ctx,
                    c,
                    WizAction::SetFormat(format),
                    i18n.t(format_key(format)),
                    view.format == format,
                );
            }

            let c = setting_row(commands, ctx, body, i18n.t("view-framerate"));
            theme::button(
                commands,
                c,
                WizAction::CycleFramerate,
                i18n.ta(
                    "view-framerate-value",
                    &[("fps", view.framerate.to_string())],
                ),
                theme::SIZE_XS * ctx.scale,
                theme::BtnSpec::outline(),
            );

            let c = setting_row(commands, ctx, body, i18n.t("view-size"));
            theme::button(
                commands,
                c,
                WizAction::CycleSize,
                i18n.ta("view-size-value", &[("px", view.size.to_string())]),
                theme::SIZE_XS * ctx.scale,
                theme::BtnSpec::outline(),
            );
        }
        AppMode::Human => {
            // Reduced motion is the inverse of delivering animation.
            let c = setting_row(commands, ctx, body, i18n.t("a11y-reduced-motion"));
            toggle(
                commands,
                ctx,
                c,
                WizAction::ToggleReducedMotion,
                !view.animation,
            );

            let c = setting_row(commands, ctx, body, i18n.t("a11y-effects-quality"));
            for quality in [Quality::Low, Quality::Medium, Quality::High] {
                option(
                    commands,
                    ctx,
                    c,
                    WizAction::SetQuality(quality),
                    i18n.t(quality_key(quality)),
                    view.quality == quality,
                );
            }

            // Flat top-down view ⇔ the ASCII / 2D delivery format (§6.7).
            let c = setting_row(commands, ctx, body, i18n.t("a11y-flat-view"));
            toggle(
                commands,
                ctx,
                c,
                WizAction::ToggleFlatView,
                view.format == ViewFormat::Ascii,
            );

            let c = setting_row(commands, ctx, body, i18n.t("a11y-screen-reader"));
            toggle(
                commands,
                ctx,
                c,
                WizAction::ToggleScreenReader,
                view.screen_reader,
            );

            let c = setting_row(commands, ctx, body, i18n.t("a11y-captions"));
            toggle(commands, ctx, c, WizAction::ToggleCaptions, view.captions);

            let c = setting_row(commands, ctx, body, i18n.t("a11y-high-contrast"));
            toggle(
                commands,
                ctx,
                c,
                WizAction::ToggleHighContrast,
                view.high_contrast,
            );

            let c = setting_row(commands, ctx, body, i18n.t("a11y-colorblind"));
            toggle(
                commands,
                ctx,
                c,
                WizAction::ToggleColorblind,
                view.colorblind,
            );

            let c = setting_row(commands, ctx, body, i18n.t("a11y-text-size"));
            for text_scale in [TextScale::Small, TextScale::Normal, TextScale::Large] {
                option(
                    commands,
                    ctx,
                    c,
                    WizAction::SetTextScale(text_scale),
                    i18n.t(text_scale_key(text_scale)),
                    view.text_scale == text_scale,
                );
            }

            let c = setting_row(commands, ctx, body, i18n.t("a11y-audio-mute"));
            toggle(
                commands,
                ctx,
                c,
                WizAction::ToggleAudioMute,
                view.audio_muted,
            );
        }
    }
}

/// Step 3: the 29-language selector as a wrapped grid of endonym buttons.
fn build_language(commands: &mut Commands, ctx: &Ctx, body: Entity) {
    theme::text(
        commands,
        body,
        ctx.i18n.t("wizard-step-language-title"),
        theme::font(
            theme::DISPLAY,
            theme::SIZE_MD * ctx.scale,
            FontWeight::MEDIUM,
        ),
        palette::INK_SRGB,
    );
    let grid = theme::container(
        commands,
        body,
        Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            column_gap: Val::Px(4.0),
            row_gap: Val::Px(4.0),
            ..default()
        },
    );
    let current = ctx.i18n.current_index();
    for (index, (_code, endonym)) in LANGS.iter().enumerate() {
        option(
            commands,
            ctx,
            grid,
            WizAction::SetLang(index),
            (*endonym).to_string(),
            index == current,
        );
    }
}

// ── small control helpers ───────────────────────────────────────────────

/// A `label … controls` row; returns the (right-hand) controls container.
fn setting_row(commands: &mut Commands, ctx: &Ctx, parent: Entity, label: String) -> Entity {
    let row = theme::container(
        commands,
        parent,
        Node {
            width: Val::Percent(100.0),
            flex_direction: theme::row(ctx.rtl),
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            column_gap: Val::Px(8.0),
            margin: UiRect::vertical(Val::Px(2.0)),
            ..default()
        },
    );
    theme::text(
        commands,
        row,
        label,
        theme::font(
            theme::DISPLAY,
            theme::SIZE_SM * ctx.scale,
            FontWeight::MEDIUM,
        ),
        palette::INK_SRGB,
    );
    theme::container(
        commands,
        row,
        Node {
            flex_direction: theme::row(ctx.rtl),
            column_gap: Val::Px(4.0),
            align_items: AlignItems::Center,
            ..default()
        },
    )
}

/// An On/Off toggle button reflecting `on`.
fn toggle(commands: &mut Commands, ctx: &Ctx, controls: Entity, action: WizAction, on: bool) {
    let (label, spec) = if on {
        (ctx.i18n.t("toggle-on"), theme::BtnSpec::filled())
    } else {
        (ctx.i18n.t("toggle-off"), theme::BtnSpec::outline())
    };
    theme::button(
        commands,
        controls,
        action,
        label,
        theme::SIZE_XS * ctx.scale,
        spec,
    );
}

/// One option in a segmented control; filled when `active`.
fn option(
    commands: &mut Commands,
    ctx: &Ctx,
    controls: Entity,
    action: WizAction,
    label: String,
    active: bool,
) {
    let spec = if active {
        theme::BtnSpec::filled()
    } else {
        theme::BtnSpec::outline()
    };
    theme::button(
        commands,
        controls,
        action,
        label,
        theme::SIZE_XS * ctx.scale,
        spec,
    );
}

// ── i18n key lookups ────────────────────────────────────────────────────

fn kind_key(kind: PlayerKind) -> &'static str {
    match kind {
        PlayerKind::Human => "player-human",
        PlayerKind::Computer => "player-computer",
        PlayerKind::Agent => "player-agent",
    }
}

fn diff_key(difficulty: Difficulty) -> &'static str {
    match difficulty {
        Difficulty::Easy => "diff-easy",
        Difficulty::Medium => "diff-medium",
        Difficulty::Hard => "diff-hard",
        Difficulty::Brutal => "diff-brutal",
    }
}

fn format_key(format: ViewFormat) -> &'static str {
    match format {
        ViewFormat::Json => "view-format-json",
        ViewFormat::Ascii => "view-format-ascii",
        ViewFormat::Png => "view-format-png",
        ViewFormat::Animation => "view-format-animation",
    }
}

fn quality_key(quality: Quality) -> &'static str {
    match quality {
        Quality::Low => "quality-low",
        Quality::Medium => "quality-medium",
        Quality::High => "quality-high",
    }
}

fn text_scale_key(text_scale: TextScale) -> &'static str {
    match text_scale {
        TextScale::Small => "text-size-small",
        TextScale::Normal => "text-size-normal",
        TextScale::Large => "text-size-large",
    }
}
