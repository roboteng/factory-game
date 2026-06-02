use avian3d;
use bevy::{
    ecs::relationship::RelatedSpawnerCommands,
    prelude::*,
    reflect::VariantType::Tuple,
    window::{CursorGrabMode, CursorOptions},
};
use common::{
    Belt, Item, Player, RaycastTarget,
    inventory::{Inventory, Stack},
};
use rand::Rng;

pub use visuals::ItemExt;

pub mod hotbar;
pub mod player_controller;
pub mod visuals;

use hotbar::PlacementItem;
pub use hotbar::{FreeHotbar, SurvivalHotbar};

use crate::hotbar::Hotbar;

pub struct UiPlugin;
impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(avian3d::PhysicsPlugins::default());
        app.add_systems(Update, player_controller::add_block_colliders);
        app.add_plugins(hotbar::HotbarPlugin);
        app.add_plugins(player_controller::PlayerControllerPlugin);
        app.add_plugins(visuals::VisualsPlugin);
        app.init_resource::<InteractionMode>();
        app.init_resource::<LookTarget>();

        app.add_systems(Startup, spawn_stars);
        app.add_systems(Startup, setup_reticle);
        app.add_systems(Startup, setup_delete_preview);
        app.add_systems(Startup, setup_incline_preview);

        app.add_systems(Update, cursor_grab);
        app.add_systems(Update, draw_crosshair_gizmo);
        app.add_systems(Update, update_delete_preview_visual);
        app.add_systems(Update, update_incline_preview_visual);

        app.add_systems(Update, hotbar_view);
    }
}

/// When present and `true`, the player uses a free-flying noclip camera
/// instead of the physics-based controller. Set via the `--fly` CLI flag.
#[derive(Resource)]
pub struct FlyMode(pub bool);

#[derive(Default, PartialEq, Eq)]
pub enum WorldMode {
    #[default]
    None,
    Placing(PlacementItem),
    Deleting,
    ChangingIncline,
}

#[derive(PartialEq, Eq)]
pub enum ScreenMode {
    Inventory,
    Menu,
    Furnace(Entity),
    Assembler(Entity),
    Source(Entity),
    Miner(Entity),
}

#[derive(Resource, PartialEq, Eq)]
pub enum InteractionMode {
    InWorld(WorldMode),
    InScreen(ScreenMode),
}

impl Default for InteractionMode {
    fn default() -> Self {
        InteractionMode::InWorld(WorldMode::None)
    }
}

/// The entity the player is currently looking at, updated each PreUpdate.
/// `None` when the cursor is unlocked or no block is in range.
#[derive(Resource, Default)]
pub struct LookTarget(pub Option<Entity>);

/// Emitted when the player right-clicks an entity in world mode.
/// UI observers react to open the appropriate machine screen.
#[derive(Event)]
pub struct Interact(pub Entity);

#[derive(Component)]
struct DeletePreview;

#[derive(Component)]
struct InclinePreview;

fn spawn_stars(
    mut cmd: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut rng = rand::thread_rng();
    let star_count = 500;
    let sky_radius = 500.0;

    let star_mesh = meshes.add(Sphere::new(0.5).mesh().ico(2).unwrap());

    for _ in 0..star_count {
        let theta = rng.gen_range(0.0..std::f32::consts::TAU);
        let phi = rng.gen_range(0.0..std::f32::consts::PI);

        let x = sky_radius * phi.sin() * theta.cos();
        let y = sky_radius * phi.cos();
        let z = sky_radius * phi.sin() * theta.sin();

        let brightness = rng.gen_range(0.5..1.5);
        let star_color = Color::srgb(brightness, brightness, brightness * 0.95);

        let star_material = materials.add(StandardMaterial {
            base_color: star_color,
            emissive: LinearRgba::new(brightness * 2.0, brightness * 2.0, brightness * 1.9, 1.0),
            ..default()
        });

        cmd.spawn((
            Mesh3d(star_mesh.clone()),
            MeshMaterial3d(star_material),
            Transform::from_translation(Vec3::new(x, y, z)),
        ));
    }
}

fn setup_reticle(mut cmd: Commands) {
    let color = Color::srgba(1.0, 1.0, 1.0, 0.8);
    let thickness = 2.0;
    let length = 12.0;
    let gap = 4.0;

    // (width, height, left, top) for each crosshair segment
    let segments = [
        (length, thickness, -length - gap, -thickness / 2.0), // left
        (length, thickness, gap, -thickness / 2.0),           // right
        (thickness, length, -thickness / 2.0, -length - gap), // top
        (thickness, length, -thickness / 2.0, gap),           // bottom
    ];

    cmd.spawn(Node {
        position_type: PositionType::Absolute,
        left: Val::Percent(50.0),
        top: Val::Percent(50.0),
        ..default()
    })
    .with_children(|parent| {
        for (w, h, l, t) in segments {
            parent.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Px(w),
                    height: Val::Px(h),
                    left: Val::Px(l),
                    top: Val::Px(t),
                    ..default()
                },
                BackgroundColor(color),
            ));
        }
    });
}

fn setup_delete_preview(
    mut cmd: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    cmd.spawn((
        Mesh3d(meshes.add(Cuboid::new(1.0, 1.0, 1.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgba(1.0, 0.1, 0.1, 0.4),
            alpha_mode: AlphaMode::Blend,
            ..default()
        })),
        Transform::default(),
        Visibility::Hidden,
        DeletePreview,
    ));
}

fn setup_incline_preview(
    mut cmd: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    cmd.spawn((
        Mesh3d(meshes.add(Cuboid::new(1.0, 1.0, 1.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgba(0.1, 1.0, 0.1, 0.4),
            alpha_mode: AlphaMode::Blend,
            ..default()
        })),
        Transform::default(),
        Visibility::Hidden,
        InclinePreview,
    ));
}

fn cursor_grab(
    mut cursor_options: Single<&mut CursorOptions>,
    key: Res<ButtonInput<KeyCode>>,
    mut mode: ResMut<InteractionMode>,
) {
    if key.just_pressed(KeyCode::Escape) {
        *mode = match *mode {
            InteractionMode::InWorld(_) => InteractionMode::InScreen(ScreenMode::Menu),
            InteractionMode::InScreen(_) => InteractionMode::InWorld(WorldMode::None),
        };
    }

    match *mode {
        InteractionMode::InWorld(_) => {
            cursor_options.visible = false;
            cursor_options.grab_mode = CursorGrabMode::Locked;
        }
        InteractionMode::InScreen(_) => {
            cursor_options.visible = true;
            cursor_options.grab_mode = CursorGrabMode::None;
        }
    }
}

fn draw_crosshair_gizmo(
    mut gizmos: Gizmos,
    look_target: Res<LookTarget>,
    blocks: Query<(&Transform, &RaycastTarget)>,
) {
    let Some(entity) = look_target.0 else { return };
    let Ok((t, rt)) = blocks.get(entity) else {
        return;
    };
    let mut pos = t.translation;
    pos.y += rt.half_extents.y;
    gizmos.cube(
        Transform::from_translation(pos).with_scale(rt.half_extents * 2.0),
        Color::srgba(1.0, 1.0, 1.0, 0.6),
    );
}

fn update_delete_preview_visual(
    mode: Res<InteractionMode>,
    look_target: Res<LookTarget>,
    blocks: Query<(&Transform, &RaycastTarget), Without<DeletePreview>>,
    mut preview_q: Single<(&mut Transform, &mut Visibility), With<DeletePreview>>,
) {
    let (ref mut t, ref mut vis) = *preview_q;

    let Some(entity) = look_target
        .0
        .filter(|_| *mode == InteractionMode::InWorld(WorldMode::Deleting))
    else {
        **vis = Visibility::Hidden;
        return;
    };

    let Ok((block_t, rt)) = blocks.get(entity) else {
        **vis = Visibility::Hidden;
        return;
    };

    **vis = Visibility::Visible;
    let mut pos = block_t.translation;
    pos.y += rt.half_extents.y;
    t.translation = pos;
    t.scale = rt.half_extents * 2.0 * 1.05;
}

fn update_incline_preview_visual(
    mode: Res<InteractionMode>,
    look_target: Res<LookTarget>,
    blocks: Query<(&Transform, &RaycastTarget), (Without<InclinePreview>, With<Belt>)>,
    mut preview_q: Single<(&mut Transform, &mut Visibility), With<InclinePreview>>,
) {
    let (ref mut t, ref mut vis) = *preview_q;

    let Some(entity) = look_target
        .0
        .filter(|_| *mode == InteractionMode::InWorld(WorldMode::ChangingIncline))
    else {
        **vis = Visibility::Hidden;
        return;
    };

    let Ok((block_t, rt)) = blocks.get(entity) else {
        **vis = Visibility::Hidden;
        return;
    };

    **vis = Visibility::Visible;
    let mut pos = block_t.translation;
    pos.y += rt.half_extents.y;
    t.translation = pos;
    t.scale = rt.half_extents * 2.0 * 1.05;
}

#[derive(Component)]
pub struct HotbarTag;

fn hotbar_view(
    player: Res<Player>,
    invs: Query<Ref<Inventory>>,
    hotbar: Res<hotbar::Hotbar>,
    mode: Res<InteractionMode>,
    asset_server: Res<AssetServer>,
    prev_hotbar: Query<Entity, With<HotbarTag>>,
    mut cmd: Commands,
) {
    let Ok(inv) = invs.get(player.0) else {
        return;
    };
    if !(hotbar.is_changed() || mode.is_changed() || inv.is_changed()) {
        return;
    }

    for hb in prev_hotbar {
        cmd.entity(hb).despawn();
    }

    let mut cmd = cmd.spawn(HotbarTag);

    spawn_hotbar(&mut cmd, &asset_server, &hotbar, &inv, &mode);
}

pub fn spawn_hotbar(
    cmd: &mut EntityCommands,
    asset_server: &AssetServer,
    hotbar: &Hotbar,
    inv: &Inventory,
    mode: &InteractionMode,
) {
    const UNSELECTED: usize = 11;
    let selected_slot = match mode {
        InteractionMode::InWorld(WorldMode::Placing(PlacementItem::HotbarSlot(slot))) => {
            *slot as usize
        }
        InteractionMode::InWorld(WorldMode::Placing(PlacementItem::Custom(item))) => hotbar
            .0
            .iter()
            .enumerate()
            .find(|(_, a)| **a == Some(*item))
            .map(|(index, _)| index)
            .unwrap_or(UNSELECTED),
        InteractionMode::InWorld(_) => UNSELECTED,
        InteractionMode::InScreen(_) => UNSELECTED,
    };
    cmd.insert(Node {
        width: percent(100),
        height: percent(100),
        align_items: AlignItems::Center,
        justify_content: JustifyContent::FlexEnd,
        display: Display::Flex,
        flex_direction: FlexDirection::Column,
        ..default()
    })
    .with_children(|cmd| {
        cmd.spawn(Node {
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            flex_direction: FlexDirection::Row,
            column_gap: px(10.0),
            padding: UiRect::all(px(5.0)),
            ..default()
        })
        .with_children(|cmd| {
            for i in 0..10 {
                let stack = hotbar.0[i].map(|item| Stack {
                    count: inv.item_count(item),
                    item,
                });
                let selected = match mode {
                    InteractionMode::InWorld(WorldMode::Placing(PlacementItem::HotbarSlot(
                        slot,
                    ))) => *slot as usize == i,
                    InteractionMode::InWorld(WorldMode::Placing(PlacementItem::Custom(item))) => {
                        Some(*item) == hotbar.0[i]
                    }
                    InteractionMode::InWorld(_) => false,
                    InteractionMode::InScreen(_) => false,
                };
                slot(cmd, stack, asset_server, selected);
            }
        });
    });
}

const SLOT_SIZE: f64 = 64.0;

fn slot(
    cmd: &mut ChildSpawnerCommands,
    stack: Option<Stack>,
    asset_server: &AssetServer,
    selected: bool,
) {
    let mut child_cmd = cmd.spawn((
        Node {
            height: px(SLOT_SIZE),
            width: px(SLOT_SIZE),
            border: UiRect::all(px(2)),
            position_type: PositionType::Relative,
            ..default()
        },
        BorderColor::all(if selected { Color::WHITE } else { Color::BLACK }),
    ));
    if let Some(stack) = stack {
        child_cmd.with_children(|cmd| {
            cmd.spawn((
                ImageNode::new(asset_server.load(stack.item.icon())).with_color(
                    if stack.count == 0 {
                        Color::linear_rgb(0.25, 0.25, 0.25)
                    } else {
                        Color::WHITE
                    },
                ),
                Node {
                    width: percent(100),
                    height: percent(100),
                    ..default()
                },
            ));
            if !(stack.count == 1 && stack.item.stack_size() == 1) {
                cmd.spawn((
                    Text::new(format!("{}", stack.count)),
                    Node {
                        position_type: PositionType::Absolute,
                        bottom: px(2.0),
                        right: px(4.0),
                        ..default()
                    },
                ));
            }
        });
    }
}
