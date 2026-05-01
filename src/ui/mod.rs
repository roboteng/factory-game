use crate::common::{
    inventory::Inventory, Assembler, Belt, Furnace, Miner, Player, RaycastTarget, Source,
};

use avian3d;
use bevy::{
    prelude::*,
    window::{CursorGrabMode, CursorOptions},
};
use rand::Rng;

mod assembler;
mod common;
mod furnace;
mod hotbar;
mod inventory;
mod menu;
mod miner;
mod player_controller;
mod source;
mod visuals;

use assembler::{
    handle_assembler_inventory_slot_clicks, handle_assembler_output_slot_clicks,
    handle_assembler_recipe_button, handle_clear_assembler_recipe, setup_assembler_pane,
    update_assembler_pane, CloseAssemblerButton,
};
use common::{stack_label, InventorySlot};
use furnace::{
    handle_furnace_inventory_slot_clicks, handle_furnace_output_slot_clicks, setup_furnace_pane,
    update_furnace_pane, CloseFurnaceButton,
};
use hotbar::PlacementItem;
pub use hotbar::{FreeHotbar, SurvivalHotbar};
use inventory::{
    handle_inventory_recipe_button, setup_inventory_pane, update_inventory_pane,
    CloseInventoryButton,
};
use menu::{setup_menu_pane, update_menu_pane, ResumeButton};
use miner::{
    handle_miner_output_slot_clicks, setup_miner_pane, update_miner_pane, CloseMinerButton,
};
use source::{
    handle_source_item_button, handle_source_scroll, setup_source_pane, update_source_pane,
    CloseSourceButton,
};

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

        app.add_observer(open_furnace_screen);
        app.add_observer(open_assembler_screen);
        app.add_observer(open_source_screen);
        app.add_observer(open_miner_screen);

        app.add_systems(Startup, spawn_stars);
        app.add_systems(Startup, setup_reticle);
        app.add_systems(Startup, setup_delete_preview);
        app.add_systems(Startup, setup_incline_preview);
        app.add_systems(Startup, setup_inventory_pane);
        app.add_systems(Startup, setup_menu_pane);
        app.add_systems(Startup, setup_furnace_pane);
        app.add_systems(Startup, setup_assembler_pane);

        app.add_systems(Update, cursor_grab);
        app.add_systems(Update, draw_crosshair_gizmo);
        app.add_systems(Update, update_delete_preview_visual);
        app.add_systems(Update, update_incline_preview_visual);

        app.add_systems(Update, update_inventory_pane.after(cursor_grab));
        app.add_systems(Update, handle_close_button::<CloseInventoryButton>);
        app.add_systems(Update, handle_inventory_recipe_button);
        app.add_systems(Update, update_menu_pane.after(cursor_grab));
        app.add_systems(Update, handle_close_button::<ResumeButton>);
        app.add_systems(Update, update_furnace_pane.after(cursor_grab));
        app.add_systems(Update, handle_close_button::<CloseFurnaceButton>);
        app.add_systems(Update, handle_furnace_inventory_slot_clicks);
        app.add_systems(Update, handle_furnace_output_slot_clicks);
        app.add_systems(Update, update_assembler_pane.after(cursor_grab));
        app.add_systems(Update, handle_close_button::<CloseAssemblerButton>);
        app.add_systems(Update, handle_assembler_recipe_button);
        app.add_systems(Update, handle_clear_assembler_recipe);
        app.add_systems(Update, handle_assembler_inventory_slot_clicks);
        app.add_systems(Update, handle_assembler_output_slot_clicks);
        app.add_systems(Startup, setup_source_pane);
        app.add_systems(Update, update_source_pane.after(cursor_grab));
        app.add_systems(Update, handle_close_button::<CloseSourceButton>);
        app.add_systems(Update, handle_source_item_button);
        app.add_systems(Update, handle_source_scroll);
        app.add_systems(Startup, setup_miner_pane);
        app.add_systems(Update, update_miner_pane.after(cursor_grab));
        app.add_systems(Update, handle_close_button::<CloseMinerButton>);
        app.add_systems(Update, handle_miner_output_slot_clicks);
        app.add_systems(Update, update_inventory_slots);
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

fn open_furnace_screen(
    event: On<Interact>,
    furnaces: Query<(), With<Furnace>>,
    mut mode: ResMut<InteractionMode>,
) {
    if furnaces.contains(event.0) {
        *mode = InteractionMode::InScreen(ScreenMode::Furnace(event.0));
    }
}

fn open_assembler_screen(
    event: On<Interact>,
    assemblers: Query<(), With<Assembler>>,
    mut mode: ResMut<InteractionMode>,
) {
    if assemblers.contains(event.0) {
        *mode = InteractionMode::InScreen(ScreenMode::Assembler(event.0));
    }
}

fn open_source_screen(
    event: On<Interact>,
    sources: Query<(), With<Source>>,
    mut mode: ResMut<InteractionMode>,
) {
    if sources.contains(event.0) {
        *mode = InteractionMode::InScreen(ScreenMode::Source(event.0));
    }
}

fn open_miner_screen(
    event: On<Interact>,
    miners: Query<(), With<Miner>>,
    mut mode: ResMut<InteractionMode>,
) {
    if miners.contains(event.0) {
        *mode = InteractionMode::InScreen(ScreenMode::Miner(event.0));
    }
}

/// Generic close button handler for all screen close buttons.
/// Closes the current screen and returns to world mode.
fn handle_close_button<T: Component>(
    interaction: Query<&Interaction, (Changed<Interaction>, With<T>)>,
    mut mode: ResMut<InteractionMode>,
) {
    for &interaction in interaction.iter() {
        if interaction == Interaction::Pressed {
            *mode = InteractionMode::InWorld(WorldMode::None);
        }
    }
}

/// Updates all InventorySlot labels whenever the player's inventory changes.
/// Shared by the inventory screen and the furnace screen's inventory panel.
fn update_inventory_slots(
    player: Res<Player>,
    inventory_q: Query<&Inventory, Changed<Inventory>>,
    inv_slots: Query<(&InventorySlot, &Children)>,
    mut texts: Query<&mut Text>,
) {
    let Ok(inventory) = inventory_q.get(player.0) else {
        return;
    };
    for (slot_marker, children) in &inv_slots {
        let label = stack_label(inventory.get(slot_marker.0));
        if let Some(&child) = children.first() {
            if let Ok(mut text) = texts.get_mut(child) {
                **text = label.into();
            }
        }
    }
}
