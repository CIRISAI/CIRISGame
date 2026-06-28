//! DBS-style tournament cube enclosing the play area, plus a live **tuning
//! panel** (top-right) organised into collapsible families so the (many) knobs
//! stay manageable: Cube, Spheres, Glass, Layout, Post. Each knob drives its
//! target material / resource live.

use bevy::asset::Asset;
use bevy::pbr::{Material, MaterialPipeline, MaterialPipelineKey, MaterialPlugin};
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::reflect::TypePath;
use bevy::render::mesh::MeshVertexBufferLayoutRef;
use bevy::render::render_resource::{
    AsBindGroup, RenderPipelineDescriptor, SpecializedMeshPipelineError,
};
use bevy::shader::ShaderRef;

use crate::effects::CoreScale;
use crate::orb::{OrbHandles, OrbMaterial};
use crate::render::{GlassHandle, MainCam};
use crate::topology::{PeerDistance, TubeWidth};
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

// ── tuning state ────────────────────────────────────────────────────────────

#[derive(Resource, Clone, Copy)]
pub(crate) struct Tuning {
    faces_hue: f32,
    accent_hue: f32,
    cube_opacity: f32,
    gas_luma: f32,
    gas_sat: f32,
    prism: f32,
    core_size: f32,
    tube_width: f32,
    glass_ior: f32,
    glass_thick: f32,
    glass_refl: f32,
    glass_rough: f32,
    bloom: f32,
    peer_dist: f32,
}

impl Default for Tuning {
    fn default() -> Self {
        Tuning {
            faces_hue: 0.45,
            accent_hue: 0.66,
            cube_opacity: 0.06,
            gas_luma: 4.0,
            gas_sat: 4.0,
            prism: 0.0,
            core_size: 0.6,
            tube_width: 1.0,
            glass_ior: 1.45,
            glass_thick: 0.18,
            glass_refl: 0.12,
            glass_rough: 0.08,
            bloom: 0.18,
            peer_dist: 1.0,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Knob {
    FacesHue,
    AccentHue,
    CubeOpacity,
    GasLuma,
    GasSat,
    Prism,
    CoreSize,
    TubeWidth,
    GlassIor,
    GlassThick,
    GlassRefl,
    GlassRough,
    Bloom,
    PeerDist,
}

impl Knob {
    fn get(self, t: &Tuning) -> f32 {
        match self {
            Knob::FacesHue => t.faces_hue,
            Knob::AccentHue => t.accent_hue,
            Knob::CubeOpacity => t.cube_opacity,
            Knob::GasLuma => t.gas_luma,
            Knob::GasSat => t.gas_sat,
            Knob::Prism => t.prism,
            Knob::CoreSize => t.core_size,
            Knob::TubeWidth => t.tube_width,
            Knob::GlassIor => t.glass_ior,
            Knob::GlassThick => t.glass_thick,
            Knob::GlassRefl => t.glass_refl,
            Knob::GlassRough => t.glass_rough,
            Knob::Bloom => t.bloom,
            Knob::PeerDist => t.peer_dist,
        }
    }
    fn adjust(self, t: &mut Tuning, d: f32) {
        match self {
            Knob::FacesHue => t.faces_hue = (t.faces_hue + d).rem_euclid(1.0),
            Knob::AccentHue => t.accent_hue = (t.accent_hue + d).rem_euclid(1.0),
            Knob::CubeOpacity => t.cube_opacity = (t.cube_opacity + d).clamp(0.0, 1.0),
            Knob::GasLuma => t.gas_luma = (t.gas_luma + d).clamp(0.2, 8.0),
            Knob::GasSat => t.gas_sat = (t.gas_sat + d).clamp(1.0, 6.0),
            Knob::Prism => t.prism = (t.prism + d).clamp(0.0, 1.0),
            Knob::CoreSize => t.core_size = (t.core_size + d).clamp(0.05, 1.8),
            Knob::TubeWidth => t.tube_width = (t.tube_width + d).clamp(0.3, 2.5),
            Knob::GlassIor => t.glass_ior = (t.glass_ior + d).clamp(1.0, 2.2),
            Knob::GlassThick => t.glass_thick = (t.glass_thick + d).clamp(0.0, 1.5),
            Knob::GlassRefl => t.glass_refl = (t.glass_refl + d).clamp(0.0, 1.0),
            Knob::GlassRough => t.glass_rough = (t.glass_rough + d).clamp(0.0, 0.6),
            Knob::Bloom => t.bloom = (t.bloom + d).clamp(0.0, 0.6),
            Knob::PeerDist => t.peer_dist = (t.peer_dist + d).clamp(0.3, 2.5),
        }
    }
    fn fmt(self, v: f32) -> String {
        match self {
            Knob::FacesHue | Knob::AccentHue => format!("{:.0}\u{b0}", v * 360.0),
            _ => format!("{v:.2}"),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Family {
    Cube,
    Spheres,
    Glass,
    Layout,
    Post,
}

impl Family {
    const ALL: [Family; 5] = [
        Family::Cube,
        Family::Spheres,
        Family::Glass,
        Family::Layout,
        Family::Post,
    ];
    fn idx(self) -> usize {
        match self {
            Family::Cube => 0,
            Family::Spheres => 1,
            Family::Glass => 2,
            Family::Layout => 3,
            Family::Post => 4,
        }
    }
    fn name(self) -> &'static str {
        match self {
            Family::Cube => "Cube",
            Family::Spheres => "Spheres",
            Family::Glass => "Glass",
            Family::Layout => "Layout",
            Family::Post => "Post",
        }
    }
}

/// (knob, family, label, per-press step).
const KNOBS: [(Knob, Family, &str, f32); 14] = [
    (Knob::FacesHue, Family::Cube, "Faces hue", 0.06),
    (Knob::AccentHue, Family::Cube, "Top hue", 0.06),
    (Knob::CubeOpacity, Family::Cube, "Opacity", 0.06),
    (Knob::GasLuma, Family::Spheres, "Gas luma", 0.6),
    (Knob::GasSat, Family::Spheres, "Gas sat", 0.4),
    (Knob::Prism, Family::Spheres, "Prism", 0.1),
    (Knob::CoreSize, Family::Spheres, "Core size", 0.1),
    (Knob::GlassIor, Family::Glass, "IOR", 0.05),
    (Knob::GlassThick, Family::Glass, "Thickness", 0.05),
    (Knob::GlassRefl, Family::Glass, "Reflect", 0.04),
    (Knob::GlassRough, Family::Glass, "Rough", 0.03),
    (Knob::PeerDist, Family::Layout, "Peer dist", 0.08),
    (Knob::TubeWidth, Family::Layout, "Tube width", 0.1),
    (Knob::Bloom, Family::Post, "Bloom", 0.03),
];

/// Which families are expanded. Spheres open by default; the rest collapsed.
#[derive(Resource)]
struct FamilyOpen([bool; 5]);

impl Default for FamilyOpen {
    fn default() -> Self {
        FamilyOpen([false, true, false, false, false])
    }
}

#[derive(Resource)]
struct CubeHandle(Handle<CubeMaterial>);

/// A `[−]`/`[+]` button: carries its own signed step.
#[derive(Component, Clone, Copy)]
struct Tune {
    knob: Knob,
    step: f32,
}

#[derive(Component)]
struct PanelToggle;

#[derive(Component)]
struct PanelRoot;

#[derive(Component)]
struct ValueText(Knob);

/// A collapsible family header button.
#[derive(Component, Clone, Copy)]
struct FamilyHeader(Family);

/// The header's caret+name text (updated on collapse/expand).
#[derive(Component, Clone, Copy)]
struct FamilyHeaderLabel(Family);

/// A knob row, tagged with its family so it can be shown/hidden.
#[derive(Component, Clone, Copy)]
struct RowFamily(Family);

pub(crate) fn plugin(app: &mut App) {
    app.add_plugins(MaterialPlugin::<CubeMaterial>::default())
        .init_resource::<Tuning>()
        .init_resource::<FamilyOpen>()
        .add_systems(Startup, (spawn_cube, spawn_panel))
        .add_systems(
            Update,
            (
                toggle_panel,
                toggle_family,
                tune_control,
                family_visibility,
                apply_tuning,
            ),
        );
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
        bevy::camera::visibility::RenderLayers::layer(crate::render::CUBE_LAYER),
    ));
    commands.insert_resource(CubeHandle(handle));
}

fn header_text(f: Family, open: bool) -> String {
    format!("{} {}", if open { "v" } else { ">" }, f.name())
}

fn spawn_panel(mut commands: Commands, tuning: Res<Tuning>, open: Res<FamilyOpen>) {
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
                width: Val::Px(290.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(10.0)),
                row_gap: Val::Px(4.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.1, 0.1, 0.12, 0.94)),
            GlobalZIndex(60),
            Visibility::Hidden,
        ))
        .id();

    for fam in Family::ALL {
        let is_open = open.0[fam.idx()];
        // Family header (click to expand/collapse).
        let spec = theme::BtnSpec::outline();
        let header = commands
            .spawn((
                Button,
                Node {
                    width: Val::Percent(100.0),
                    padding: UiRect::axes(Val::Px(8.0), Val::Px(5.0)),
                    margin: UiRect::top(Val::Px(4.0)),
                    ..default()
                },
                BackgroundColor(spec.colors.normal),
                spec.colors,
                FamilyHeader(fam),
                ChildOf(panel),
            ))
            .id();
        let htext = theme::text(
            &mut commands,
            header,
            header_text(fam, is_open),
            theme::font(theme::DISPLAY, theme::SIZE_SM, FontWeight::MEDIUM),
            crate::palette::INK_SRGB,
        );
        commands.entity(htext).insert(FamilyHeaderLabel(fam));

        // Rows for this family.
        for (knob, kfam, label, step) in KNOBS {
            if kfam != fam {
                continue;
            }
            let row = commands
                .spawn((
                    Node {
                        display: if is_open {
                            Display::Flex
                        } else {
                            Display::None
                        },
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::SpaceBetween,
                        column_gap: Val::Px(6.0),
                        padding: UiRect::left(Val::Px(8.0)),
                        ..default()
                    },
                    RowFamily(fam),
                    ChildOf(panel),
                ))
                .id();
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
                knob.fmt(knob.get(&tuning)),
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
}

/// Show/hide the whole panel on the Tune press.
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

/// Expand/collapse a family on its header press.
fn toggle_family(
    q: Query<(&Interaction, &FamilyHeader), Changed<Interaction>>,
    mut open: ResMut<FamilyOpen>,
) {
    for (interaction, fh) in &q {
        if *interaction == Interaction::Pressed {
            let i = fh.0.idx();
            open.0[i] = !open.0[i];
        }
    }
}

/// Reflect family open-state into the rows' display + the header carets.
fn family_visibility(
    open: Res<FamilyOpen>,
    mut rows: Query<(&RowFamily, &mut Node)>,
    mut headers: Query<(&FamilyHeaderLabel, &mut Text)>,
) {
    if !open.is_changed() {
        return;
    }
    for (rf, mut node) in &mut rows {
        node.display = if open.0[rf.0.idx()] {
            Display::Flex
        } else {
            Display::None
        };
    }
    for (hl, mut text) in &mut headers {
        *text = Text::new(header_text(hl.0, open.0[hl.0.idx()]));
    }
}

/// Apply a knob press to [`Tuning`].
fn tune_control(q: Query<(&Interaction, &Tune), Changed<Interaction>>, mut tuning: ResMut<Tuning>) {
    for (interaction, tune) in &q {
        if *interaction == Interaction::Pressed {
            tune.knob.adjust(&mut tuning, tune.step);
        }
    }
}

/// Push [`Tuning`] into every target (cube, orbs, glass, pipes, bloom, core
/// scale, peer distance) + the value readouts whenever it changes.
#[allow(clippy::too_many_arguments)]
fn apply_tuning(
    tuning: Res<Tuning>,
    cube: Option<Res<CubeHandle>>,
    orbs: Option<Res<OrbHandles>>,
    glass: Option<Res<GlassHandle>>,
    mut cube_mats: ResMut<Assets<CubeMaterial>>,
    mut orb_mats: ResMut<Assets<OrbMaterial>>,
    mut std_mats: ResMut<Assets<StandardMaterial>>,
    mut core_scale: ResMut<CoreScale>,
    mut peer: ResMut<PeerDistance>,
    mut tube: ResMut<TubeWidth>,
    mut bloom: Query<&mut Bloom, With<MainCam>>,
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

    if let Some(h) = orbs {
        for handle in h.0.iter().skip(1) {
            if let Some(mut m) = orb_mats.get_mut(handle) {
                m.params.z = tuning.gas_luma;
                m.params2.x = tuning.gas_sat;
                m.params2.y = tuning.prism;
            }
        }
    }

    if let Some(h) = glass {
        if let Some(mut m) = std_mats.get_mut(&h.0) {
            m.ior = tuning.glass_ior;
            m.thickness = tuning.glass_thick;
            m.reflectance = tuning.glass_refl;
            m.perceptual_roughness = tuning.glass_rough;
        }
    }

    core_scale.0 = tuning.core_size;
    peer.0 = tuning.peer_dist;
    tube.0 = tuning.tube_width;
    if let Ok(mut b) = bloom.single_mut() {
        b.intensity = tuning.bloom;
    }

    for (vt, mut text) in &mut values {
        *text = Text::new(vt.0.fmt(vt.0.get(&tuning)));
    }
}
