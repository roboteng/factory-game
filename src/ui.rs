use crate::{
    core::{inventory::Inventory, *},
    ui::hotbar::{Hotbar, PlacementItem},
};

use bevy::{
    input::mouse::AccumulatedMouseMotion,
    prelude::*,
    window::{CursorGrabMode, CursorOptions},
};
use rand::Rng;

mod hotbar;

pub struct UiPlugin;
impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(hotbar::HotbarPlugin);
        app.init_resource::<InteractionMode>();
        app.insert_resource(ClearColor(Color::srgb(0.01, 0.01, 0.05))); // Dark night sky
        app.add_systems(Startup, setup);
        app.add_systems(Startup, setup_reticle);
        app.add_systems(Startup, setup_models);
        app.add_systems(Startup, setup_delete_preview);

        // Systems that trigger events Must run in PreUpdate
        app.add_systems(PreUpdate, camera_movement);
        app.add_systems(
            PreUpdate,
            (
                handle_mode_inputs,
                update_delete_preview.after(handle_mode_inputs),
                handle_click_to_place.after(handle_mode_inputs),
            ),
        );

        app.add_systems(Update, attach_models);
        app.add_systems(Update, camera_look);
        app.add_systems(Update, cursor_grab.after(handle_click_to_place));

        app.add_observer(on_place_item);
    }
}

#[derive(Resource, Default, PartialEq, Eq)]
pub(super) enum InteractionMode {
    #[default]
    None,
    Placing,
    Deleting,
}

#[derive(Component)]
struct DeletePreview;

enum ModelDef {
    Scene(Handle<Scene>),
    Mesh(Handle<Mesh>, Handle<StandardMaterial>),
}

#[derive(Resource)]
struct AllModels {
    belt_straight: ModelDef,
    belt_curve: ModelDef,
    belt_ramp_up: ModelDef,
    belt_ramp_down: ModelDef,
    source: ModelDef,
    sink: ModelDef,
    rock: ModelDef,
    dirt: ModelDef,
}

/// Creates a scene asset that renders `inner` with `transform` applied.
/// When instantiated, the scene root contains one entity (the transformed inner scene),
/// so the ramp shape is entirely self-contained in the asset.
fn ramp_scene(
    inner: Handle<Scene>,
    transform: Transform,
    scenes: &mut Assets<Scene>,
) -> Handle<Scene> {
    let mut world = World::new();
    world.spawn((SceneRoot(inner), transform));
    scenes.add(Scene::new(world))
}

fn setup_models(
    mut cmd: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut scenes: ResMut<Assets<Scene>>,
) {
    let cuboid = meshes.add(Cuboid::new(BLOCK_SIZE, BLOCK_SIZE, BLOCK_SIZE));
    let straight_scene =
        asset_server.load(GltfAssetLabel::Scene(1).from_asset("models/Untitled.glb"));

    let ramp_angle = (HALF_BLOCK_SIZE / BLOCK_SIZE).atan();
    let ramp_scale =
        (BLOCK_SIZE * BLOCK_SIZE + HALF_BLOCK_SIZE * HALF_BLOCK_SIZE).sqrt() / BLOCK_SIZE;

    cmd.insert_resource(AllModels {
        belt_straight: ModelDef::Scene(straight_scene.clone()),
        belt_curve: ModelDef::Scene(
            asset_server.load(GltfAssetLabel::Scene(0).from_asset("models/Untitled.glb")),
        ),
        belt_ramp_up: ModelDef::Scene(ramp_scene(
            straight_scene.clone(),
            Transform::from_translation(Vec3::new(0.0, HALF_BLOCK_SIZE / 2.0, 0.0))
                .with_rotation(Quat::from_rotation_z(ramp_angle))
                .with_scale(Vec3::new(ramp_scale, 1.0, 1.0)),
            &mut scenes,
        )),
        belt_ramp_down: ModelDef::Scene(ramp_scene(
            straight_scene,
            Transform::from_translation(Vec3::new(0.0, -HALF_BLOCK_SIZE / 2.0, 0.0))
                .with_rotation(Quat::from_rotation_z(-ramp_angle))
                .with_scale(Vec3::new(ramp_scale, 1.0, 1.0)),
            &mut scenes,
        )),
        source: ModelDef::Mesh(cuboid.clone(), materials.add(Color::srgb(0.2, 0.8, 0.2))),
        sink: ModelDef::Mesh(cuboid.clone(), materials.add(Color::srgb(0.8, 0.2, 0.2))),
        rock: ModelDef::Mesh(cuboid.clone(), materials.add(Color::srgb(0.55, 0.55, 0.55))),
        dirt: ModelDef::Mesh(cuboid.clone(), materials.add(Color::srgb(0.55, 0.35, 0.15))),
    });
}

fn apply_model(cmd: &mut EntityCommands, model: &ModelDef) {
    match model {
        ModelDef::Scene(handle) => {
            cmd.insert(SceneRoot(handle.clone()));
        }
        ModelDef::Mesh(mesh, material) => {
            cmd.insert((Mesh3d(mesh.clone()), MeshMaterial3d(material.clone())));
        }
    }
}

#[derive(Component)]
struct FirstPersonCamera {
    pitch: f32,
    yaw: f32,
    sensitivity: f32,
    speed: f32,
}

fn setup(
    mut cmd: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    // Transform for the camera and lighting, looking at (0,0,0) (the position of the mesh).
    let camera_transform = Transform::from_xyz(1.8, 1.8, 1.8).looking_at(Vec3::ZERO, Vec3::Y);
    let light_transform = camera_transform;

    // Camera in 3D space with first-person controls.
    cmd.spawn((
        Camera3d::default(),
        camera_transform,
        FirstPersonCamera {
            pitch: 0.0,
            yaw: 0.0,
            sensitivity: 0.002,
            speed: 5.0,
        },
        AmbientLight {
            color: Color::WHITE,
            brightness: 100.0,
            affects_lightmapped_meshes: true,
        },
    ));

    // Light up the scene.
    cmd.spawn((PointLight::default(), light_transform));

    // Generate stars
    spawn_stars(&mut cmd, &mut meshes, &mut materials);
}

fn spawn_stars(
    cmd: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
) {
    let mut rng = rand::thread_rng();
    let star_count = 500;
    let sky_radius = 500.0;

    // Create a small sphere mesh for stars
    let star_mesh = meshes.add(Sphere::new(0.5).mesh().ico(2).unwrap());

    for _ in 0..star_count {
        // Generate random point on sphere using spherical coordinates
        let theta = rng.gen_range(0.0..std::f32::consts::TAU);
        let phi = rng.gen_range(0.0..std::f32::consts::PI);

        let x = sky_radius * phi.sin() * theta.cos();
        let y = sky_radius * phi.cos();
        let z = sky_radius * phi.sin() * theta.sin();

        // Random brightness for stars
        let brightness = rng.gen_range(0.5..1.5);
        let star_color = Color::srgb(brightness, brightness, brightness * 0.95);

        // Create emissive material for star
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

fn attach_models(
    world_items: Query<(Entity, &Item, Option<&BeltShape>), Or<(Added<Item>, Changed<BeltShape>)>>,
    all_models: Res<AllModels>,
    mut cmd: Commands,
) {
    for (entity, item, shape) in &world_items {
        let model = match item {
            Item::Source => &all_models.source,
            Item::Sink => &all_models.sink,
            Item::Rock => &all_models.rock,
            Item::Dirt => &all_models.dirt,
            Item::Belt => match shape {
                Some(BeltShape::Straight(_)) => &all_models.belt_straight,
                Some(BeltShape::Curve(_)) => &all_models.belt_curve,
                Some(BeltShape::RampUp(_)) => &all_models.belt_ramp_up,
                Some(BeltShape::RampDown(_)) => &all_models.belt_ramp_down,
                None => continue,
            },
        };
        apply_model(&mut cmd.entity(entity), model);
    }
}

fn on_place_item(event: On<PlaceItem>, mut cmd: Commands, asset_server: Res<AssetServer>) {
    cmd.entity(event.entity).insert(SceneRoot(
        asset_server.load(GltfAssetLabel::Scene(0).from_asset("models/item.glb")),
    ));
}

fn cursor_grab(
    mut cursor_options: Single<&mut CursorOptions>,
    mouse: Res<ButtonInput<MouseButton>>,
    key: Res<ButtonInput<KeyCode>>,
    mut mode: ResMut<InteractionMode>,
) {
    // Only grab cursor on left click if not already grabbed
    if mouse.just_pressed(MouseButton::Left) && cursor_options.grab_mode != CursorGrabMode::Locked {
        cursor_options.visible = false;
        cursor_options.grab_mode = CursorGrabMode::Locked;
    }

    if key.just_pressed(KeyCode::Escape) {
        if *mode != InteractionMode::None {
            *mode = InteractionMode::None;
        } else {
            cursor_options.visible = true;
            cursor_options.grab_mode = CursorGrabMode::None;
        }
    }
}

fn camera_look(
    accumulated_mouse_motion: Res<AccumulatedMouseMotion>,
    mut query: Query<(&mut Transform, &mut FirstPersonCamera)>,
) {
    for (mut transform, mut camera) in query.iter_mut() {
        let delta = accumulated_mouse_motion.delta;

        camera.yaw -= delta.x * camera.sensitivity;
        camera.pitch -= delta.y * camera.sensitivity;
        camera.pitch = camera.pitch.clamp(-1.54, 1.54); // Limit pitch to avoid flipping

        // Apply rotation: yaw around Y axis, pitch around X axis
        transform.rotation =
            Quat::from_rotation_y(camera.yaw) * Quat::from_rotation_x(camera.pitch);
    }
}

fn camera_movement(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut query: Query<(&mut Transform, &FirstPersonCamera)>,
) {
    for (mut transform, camera) in query.iter_mut() {
        let mut direction = Vec3::ZERO;

        // Get forward and right directions based on yaw only (no pitch)
        let forward = Vec3::new(-camera.yaw.sin(), 0.0, -camera.yaw.cos());
        let right = Vec3::new(camera.yaw.cos(), 0.0, -camera.yaw.sin());

        if keys.pressed(KeyCode::KeyW) {
            direction += forward;
        }
        if keys.pressed(KeyCode::KeyS) {
            direction -= forward;
        }
        if keys.pressed(KeyCode::KeyA) {
            direction -= right;
        }
        if keys.pressed(KeyCode::KeyD) {
            direction += right;
        }

        if direction.length_squared() > 0.0 {
            direction = direction.normalize();
        }

        transform.translation += direction * camera.speed * time.delta_secs();

        if keys.pressed(KeyCode::Space) {
            transform.translation.y += camera.speed * time.delta_secs();
        }
        if keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight) {
            transform.translation.y -= camera.speed * time.delta_secs();
        }
    }
}

struct RayHit {
    hit_coords: WorldCoords,
    place_coords: WorldCoords,
}

fn cast_ray(
    origin: Vec3,
    dir: Vec3,
    targets: impl Iterator<Item = (WorldCoords, Vec3, Vec3)>,
) -> Option<RayHit> {
    let mut best: Option<(f32, WorldCoords, [i32; 3])> = None;

    'outer: for (coords, center, half_extents) in targets {
        let mut t_enter = f32::NEG_INFINITY;
        let mut t_leave = f32::INFINITY;
        let mut enter_axis = 0usize;

        for axis in 0..3usize {
            let half = half_extents[axis];
            let d = dir[axis];
            let o = origin[axis];
            let c = center[axis];

            if d.abs() < 1e-9 {
                if (o - c).abs() > half {
                    continue 'outer; // parallel and outside slab → miss
                }
                continue; // parallel and inside slab → unconstrained on this axis
            }

            let t_a = (c - half - o) / d;
            let t_b = (c + half - o) / d;
            let (t0, t1) = if d > 0.0 { (t_a, t_b) } else { (t_b, t_a) };

            if t0 > t_enter {
                t_enter = t0;
                enter_axis = axis;
            }
            t_leave = t_leave.min(t1);
        }

        if t_enter >= t_leave || t_leave <= 0.0 {
            continue; // miss
        }

        if best.map_or(true, |(best_t, _, _)| t_enter < best_t) {
            let mut offset = [0i32; 3];
            // How many half-block y-slots does this block occupy?
            let y_slots = (2.0 * half_extents.y / HALF_BLOCK_SIZE).round() as i32;
            if enter_axis == 1 {
                // Top/bottom face: step past all occupied y slots.
                offset[1] = if dir[1] > 0.0 { -y_slots } else { y_slots };
            } else {
                // Side face: snap Y to whichever half-block slot the ray actually
                // hit, clamped to the block's own occupied slots.
                let hit_world_y = origin[1] + t_enter * dir[1];
                let raw_y = (hit_world_y / HALF_BLOCK_SIZE).round() as i32;
                let snapped_y = raw_y.clamp(coords.y, coords.y + y_slots - 1);
                offset[1] = snapped_y - coords.y;
                offset[enter_axis] = if dir[enter_axis] > 0.0 { -1 } else { 1 };
            }
            best = Some((t_enter, coords, offset));
        }
    }

    best.map(|(_, hit_coords, offset)| {
        let place_coords = WorldCoords {
            x: hit_coords.x + offset[0],
            y: hit_coords.y + offset[1],
            z: hit_coords.z + offset[2],
        };
        RayHit {
            hit_coords,
            place_coords,
        }
    })
}

fn handle_click_to_place(
    mouse: Res<ButtonInput<MouseButton>>,
    cursor_options: Single<&CursorOptions>,
    camera_query: Single<&Transform, With<FirstPersonCamera>>,
    tool: Res<PlacementItem>,
    player: Res<Player>,
    hotbar: Res<Hotbar>,
    mut invs: Query<&mut Inventory>,
    mut cmd: Commands,
    mode: Res<InteractionMode>,
    targets: Query<(&WorldCoords, &Transform, &RaycastTarget)>,
) {
    if *mode != InteractionMode::Placing {
        return;
    }
    let Ok(_) = invs.get_mut(player.0) else {
        error!("Could not find the player");
        return;
    };
    let item = match *tool {
        PlacementItem::HotbarSlot(slot) => match hotbar.0.get(slot as usize) {
            Some(Some(item)) => *item,
            _ => return,
        },
        PlacementItem::Custom(item) => item,
        PlacementItem::None => return,
    };
    // Only handle clicks when cursor is grabbed (in game mode)
    if !mouse.just_pressed(MouseButton::Left) || cursor_options.grab_mode != CursorGrabMode::Locked
    {
        return;
    }

    let camera_transform = camera_query.into_inner();
    let origin = camera_transform.translation;
    let ray_dir = *camera_transform.forward();

    let Some(hit) = cast_ray(
        origin,
        ray_dir,
        targets
            .iter()
            .map(|(c, t, rt)| (*c, t.translation, rt.half_extents)),
    ) else {
        return;
    };

    // Determine belt direction based on camera forward direction (belt faces away from camera)
    // Project camera forward onto XZ plane and calculate angle
    let camera_forward = camera_transform.forward();
    let forward_xz = Vec3::new(camera_forward.x, 0.0, camera_forward.z).normalize();
    let angle = forward_xz.z.atan2(forward_xz.x);

    // HDir angle mapping: North=0, East=-PI/2, South=PI, West=PI/2
    let dir = angle_to_hdir(angle);

    let entity = cmd.spawn_empty().id();

    let event = PlaceBlock {
        entity,
        item,
        coords: hit.place_coords,
        dir,
    };
    debug!("Triggering: {event:?}");
    cmd.trigger(event);
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

fn handle_mode_inputs(keys: Res<ButtonInput<KeyCode>>, mut mode: ResMut<InteractionMode>) {
    if keys.just_pressed(KeyCode::KeyX) {
        *mode = InteractionMode::Deleting;
    }
}

fn update_delete_preview(
    mode: Res<InteractionMode>,
    mouse: Res<ButtonInput<MouseButton>>,
    cursor_options: Single<&CursorOptions>,
    camera_q: Single<&Transform, With<FirstPersonCamera>>,
    coord_map: Res<CoordMap>,
    mut preview_q: Single<
        (&mut Transform, &mut Visibility),
        (With<DeletePreview>, Without<FirstPersonCamera>),
    >,
    mut cmd: Commands,
    targets: Query<(&WorldCoords, &Transform, &RaycastTarget), Without<DeletePreview>>,
) {
    let (ref mut t, ref mut vis) = *preview_q;
    let cursor_locked = cursor_options.grab_mode == CursorGrabMode::Locked;

    if *mode != InteractionMode::Deleting || !cursor_locked {
        **vis = Visibility::Hidden;
        return;
    }

    let camera_transform = camera_q.into_inner();
    let origin = camera_transform.translation;
    let ray_dir = *camera_transform.forward();

    let Some(hit) = cast_ray(
        origin,
        ray_dir,
        targets
            .iter()
            .map(|(c, tr, rt)| (*c, tr.translation, rt.half_extents)),
    ) else {
        **vis = Visibility::Hidden;
        return;
    };

    let Some(&target) = coord_map.0.get(&hit.hit_coords) else {
        **vis = Visibility::Hidden;
        return;
    };

    let Ok((_, _, rt)) = targets.get(target) else {
        **vis = Visibility::Hidden;
        return;
    };

    **vis = Visibility::Visible;
    t.translation = Vec3::from(hit.hit_coords);
    t.scale = rt.half_extents * 2.0 * 1.05;

    if mouse.just_pressed(MouseButton::Left) {
        cmd.trigger(RemoveBlock { entity: target });
    }
}

fn angle_to_hdir(angle: f32) -> HDir {
    use std::f32::consts::PI;

    // angle is from atan2(z, x)
    // We need to map this to HDir angles where:
    // North=0 (facing +X), East=-PI/2 (facing +Z), South=PI (facing -X), West=PI/2 (facing -Z)

    // atan2(z, x) gives:
    // +Z direction (East): atan2(1, 0) = PI/2
    // +X direction (North): atan2(0, 1) = 0
    // -Z direction (West): atan2(-1, 0) = -PI/2
    // -X direction (South): atan2(0, -1) = PI

    // Negate to align with HDir coordinate system
    let hdir_angle = -angle;

    // Normalize to [-PI, PI]
    let mut normalized = hdir_angle;
    if normalized > PI {
        normalized -= 2.0 * PI;
    } else if normalized < -PI {
        normalized += 2.0 * PI;
    }

    // Find closest HDir based on angle
    if normalized.abs() < PI / 4.0 {
        HDir::North
    } else if normalized > 3.0 * PI / 4.0 || normalized < -3.0 * PI / 4.0 {
        HDir::South
    } else if normalized > 0.0 {
        HDir::West
    } else {
        HDir::East
    }
}

fn setup_reticle(mut cmd: Commands) {
    let color = Color::srgba(1.0, 1.0, 1.0, 0.8);
    let thickness = 2.0;
    let length = 12.0;
    let gap = 4.0; // Gap in the center

    // Container centered on screen
    cmd.spawn(Node {
        position_type: PositionType::Absolute,
        left: Val::Percent(50.0),
        top: Val::Percent(50.0),
        ..default()
    })
    .with_children(|parent| {
        // Horizontal left
        parent.spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Px(length),
                height: Val::Px(thickness),
                left: Val::Px(-length - gap),
                top: Val::Px(-thickness / 2.0),
                ..default()
            },
            BackgroundColor(color),
        ));
        // Horizontal right
        parent.spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Px(length),
                height: Val::Px(thickness),
                left: Val::Px(gap),
                top: Val::Px(-thickness / 2.0),
                ..default()
            },
            BackgroundColor(color),
        ));
        // Vertical top
        parent.spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Px(thickness),
                height: Val::Px(length),
                left: Val::Px(-thickness / 2.0),
                top: Val::Px(-length - gap),
                ..default()
            },
            BackgroundColor(color),
        ));
        // Vertical bottom
        parent.spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Px(thickness),
                height: Val::Px(length),
                left: Val::Px(-thickness / 2.0),
                top: Val::Px(gap),
                ..default()
            },
            BackgroundColor(color),
        ));
    });
}
