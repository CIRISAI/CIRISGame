//! DBS-style tournament cube enclosing the play area (a far-off arena boundary),
//! plus a live **tuning panel** (top-right) so the look can be dialled in by hand:
//! the five faces' colour, the sixth (top) face's accent colour, the cube
//! opacity, and the gas luminosity / saturation / core size of the marbles.

use bevy::asset::Asset;
use bevy::pbr::{Material, MaterialPipeline, MaterialPipelineKey, MaterialPlugin};
use bevy::prelude::*;
use bevy::reflect::TypePath;
use bevy::render::mesh::MeshVertexBufferLayoutRef;
use bevy::render::render_resource::{
    AsBindGroup, RenderPipelineDescriptor, SpecializedMeshPipelineError,
};
use bevy::shader::ShaderRef;

use crate::effects::CoreScale;
use crate::orb::{OrbHandles, OrbMaterial};
use crate::ui_theme as theme;
use ciris_game_engine_core::DEFAULT_BOARD_N;

/// The play-area cube material: five faces `color` (a = opacity), the +Y face
/// `accent`.
#[derive(Asset, AsBindGroup, TypePath, Clone)]
pub struct CubeMaterial {
    #[uniform(0)]
    pub color: LinearRgba,
    #[uniform(1)]
    pub accent: LinearRgba,
}

impl Material for CubeMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/cube.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }

    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        descriptor.primitive.cull_mode = None; // double-sided clear box
        Ok(())
    }
}

/// The live-tunable knobs.
#[derive(Resource, Clone, Copy)]
pub(crate) struct Tuning {
    faces_hue: f32,
    accent_hue: f32,
    cube_opacity: f32,
    gas_luma: f32,
    gas_sat: f32,
    core_size: f32,
}

impl Default for Tuning {
    fn default() -> Self {
        Tuning {
            faces_hue: 0.45,
            accent_hue: 0.60,
            cube_opacity: 0.06,
            gas_luma: 2.2,
            gas_sat: 2.0,
            core_size: 1.0,
        }
    }
}

/// Which knob a `[−]`/`[+]` button drives.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Knob {
    FacesHue,
    AccentHue,
    CubeOpacity,
    GasLuma,
    GasSat,
    CoreSize,
}

/// (knob, label, per-press step) — steps are chosen so each click visibly moves.
const KNOBS: [(Knob, &str, f32); 6] = [
    (Knob::FacesHue, "Faces hue", 0.06),
    (Knob::AccentHue, "Top hue", 0.06),
    (Knob::CubeOpacity, "Cube opacity", 0.06),
    (Knob::GasLuma, "Gas luma", 0.6),
    (Knob::GasSat, "Gas sat", 0.4),
    (Knob::CoreSize, "Core size", 0.12),
];

impl Tuning {
    fn get(&self, k: Knob) -> f32 {
        match k {
            Knob::FacesHue => self.faces_hue,
            Knob::AccentHue => self.accent_hue,
            Knob::CubeOpacity => self.cube_opacity,
            Knob::GasLuma => self.gas_luma,
            Knob::GasSat => self.gas_sat,
            Knob::CoreSize => self.core_size,
        }
    }
    fn adjust(&mut self, k: Knob, d: f32) {
        match k {
            Knob::FacesHue => self.faces_hue = (self.faces_hue + d).rem_euclid(1.0),
            Knob::AccentHue => self.accent_hue = (self.accent_hue + d).rem_euclid(1.0),
            Knob::CubeOpacity => self.cube_opacity = (self.cube_opacity + d).clamp(0.0, 1.0),
            Knob::GasLuma => self.gas_luma = (self.gas_luma + d).clamp(0.2, 8.0),
            Knob::GasSat => self.gas_sat = (self.gas_sat + d).clamp(1.0, 4.0),
            Knob::CoreSize => self.core_size = (self.core_size + d).clamp(0.3, 2.5),
        }
    }
}

#[derive(Resource)]
struct CubeHandle(Handle<CubeMaterial>);

/// A `[−]`/`[+]` button: carries its own signed step (no lookup, can't panic).
#[derive(Component, Clone, Copy)]
struct Tune {
    knob: Knob,
    step: f32,
}

/// The button that shows/hides the tuning panel.
#[derive(Component)]
struct PanelToggle;

/// The tuning panel root (toggled visible).
#[derive(Component)]
struct PanelRoot;

/// A knob's value readout text.
#[derive(Component)]
struct ValueText(Knob);

pub(crate) fn plugin(app: &mut App) {
    app.add_plugins(MaterialPlugin::<CubeMaterial>::default())
        .init_resource::<Tuning>()
        .add_systems(Startup, (spawn_cube, spawn_panel))
        .add_systems(Update, (toggle_panel, tune_control, apply_tuning));
}

fn hue(h: f32, l: f32) -> LinearRgba {
    Color::hsl(h * 360.0, 0.7, l).to_linear()
}

fn spawn_cube(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<CubeMaterial>>,
    tuning: Res<Tuning>,
) {
    // Far-off arena boundary: ~5× the board's own extent, so the cube reads as a
    // distant enclosure, not a tight box around the marbles.
    let size = DEFAULT_BOARD_N as f32 * 6.0;
    let mut color = hue(tuning.faces_hue, 0.45);
    color.alpha = tuning.cube_opacity;
    let handle = mats.add(CubeMaterial {
        color,
        accent: hue(tuning.accent_hue, 0.5),
    });
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(size, size, size))),
        MeshMaterial3d(handle.clone()),
        Transform::default(),
    ));
    commands.insert_resource(CubeHandle(handle));
}

/// Build the top-right toggle + the (hidden) tuning panel.
fn spawn_panel(mut commands: Commands, tuning: Res<Tuning>) {
    let toggle_root = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(16.0),
                right: Val::Px(16.0),
                ..default()
            },
            GlobalZIndex(60),
        ))
        .id();
    theme::button(
        &mut commands,
        toggle_root,
        PanelToggle,
        "Tune",
        theme::SIZE_SM,
        theme::BtnSpec::outline(),
    );

    let panel = commands
        .spawn((
            PanelRoot,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(56.0),
                right: Val::Px(16.0),
                width: Val::Px(280.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(12.0)),
                row_gap: Val::Px(6.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.1, 0.1, 0.12, 0.92)),
            GlobalZIndex(60),
            Visibility::Hidden,
        ))
        .id();

    for (knob, label, step) in KNOBS {
        let row = theme::container(
            &mut commands,
            panel,
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                column_gap: Val::Px(6.0),
                ..default()
            },
        );
        theme::text(
            &mut commands,
            row,
            label,
            theme::font(theme::DISPLAY, theme::SIZE_XS, FontWeight::NORMAL),
            crate::palette::BONE_SRGB,
        );
        theme::button(
            &mut commands,
            row,
            Tune { knob, step: -step },
            "-",
            theme::SIZE_SM,
            theme::BtnSpec::outline(),
        );
        let value = theme::text(
            &mut commands,
            row,
            fmt(knob, tuning.get(knob)),
            theme::font(theme::MONO, theme::SIZE_XS, FontWeight::NORMAL),
            crate::palette::BONE_SRGB,
        );
        commands.entity(value).insert(ValueText(knob));
        theme::button(
            &mut commands,
            row,
            Tune { knob, step },
            "+",
            theme::SIZE_SM,
            theme::BtnSpec::outline(),
        );
    }
}

fn fmt(knob: Knob, v: f32) -> String {
    match knob {
        Knob::FacesHue | Knob::AccentHue => format!("{:.0}\u{b0}", v * 360.0),
        Knob::CubeOpacity => format!("{:.2}", v),
        Knob::GasLuma | Knob::GasSat | Knob::CoreSize => format!("{:.1}", v),
    }
}

/// Show/hide the panel on the toggle press.
fn toggle_panel(
    q: Query<&Interaction, (Changed<Interaction>, With<PanelToggle>)>,
    mut panel: Query<&mut Visibility, With<PanelRoot>>,
) {
    for interaction in &q {
        if *interaction == Interaction::Pressed {
            if let Ok(mut vis) = panel.single_mut() {
                *vis = match *vis {
                    Visibility::Hidden => Visibility::Visible,
                    _ => Visibility::Hidden,
                };
            }
        }
    }
}

/// Apply a knob press to [`Tuning`].
fn tune_control(q: Query<(&Interaction, &Tune), Changed<Interaction>>, mut tuning: ResMut<Tuning>) {
    for (interaction, tune) in &q {
        if *interaction == Interaction::Pressed {
            tuning.adjust(tune.knob, tune.step);
        }
    }
}

/// Push [`Tuning`] into the cube + orb materials, the core scale, and the value
/// readouts whenever it changes (and once at startup).
#[allow(clippy::too_many_arguments)]
fn apply_tuning(
    tuning: Res<Tuning>,
    cube: Option<Res<CubeHandle>>,
    orbs: Option<Res<OrbHandles>>,
    mut cube_mats: ResMut<Assets<CubeMaterial>>,
    mut orb_mats: ResMut<Assets<OrbMaterial>>,
    mut core_scale: ResMut<CoreScale>,
    mut values: Query<(&ValueText, &mut Text)>,
) {
    if !tuning.is_changed() {
        return;
    }

    if let Some(h) = cube {
        if let Some(mut m) = cube_mats.get_mut(&h.0) {
            let mut c = hue(tuning.faces_hue, 0.45);
            c.alpha = tuning.cube_opacity;
            m.color = c;
            m.accent = hue(tuning.accent_hue, 0.5);
        }
    }

    // Gas luma + saturation drive the live steward cores (orb handles 1..; index
    // 0 is the empty-position marker, left alone).
    if let Some(h) = orbs {
        for handle in h.0.iter().skip(1) {
            if let Some(mut m) = orb_mats.get_mut(handle) {
                m.params.z = tuning.gas_luma;
                m.params2.x = tuning.gas_sat;
            }
        }
    }

    core_scale.0 = tuning.core_size;

    for (vt, mut text) in &mut values {
        *text = Text::new(fmt(vt.0, tuning.get(vt.0)));
    }
}
