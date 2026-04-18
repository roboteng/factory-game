mod placement;
pub(crate) use placement::NeedsGhostTint;
use placement::{
    commit_belt_placement, compute_placement_target, handle_click_to_place, sync_belt_ghosts,
    update_belt_placement, update_single_ghost,
};

use crate::{FlyMode, InteractionMode, ScreenMode, WorldMode};
use factory_core::*;

use avian3d::prelude::*;
use bevy::{
    input::mouse::AccumulatedMouseMotion,
    prelude::*,
    window::{CursorGrabMode, CursorOptions},
};
use rand::Rng;

pub(super) struct PlayerControllerPlugin;
impl Plugin for PlayerControllerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PlacementDirection>();
        app.add_systems(Startup, setup);
        app.add_systems(Startup, setup_reticle);
        app.add_systems(Startup, setup_delete_preview);
        app.add_systems(Startup, setup_incline_preview);

        app.add_systems(
            FixedUpdate,
            player_movement.run_if(|fly: Res<FlyMode>| !fly.0),
        );
        app.add_systems(PreUpdate, fly_movement.run_if(|fly: Res<FlyMode>| fly.0));
        app.add_systems(
            PreUpdate,
            (
                handle_mode_inputs,
                (
                    handle_right_click_block_ui::<Furnace>,
                    handle_right_click_block_ui::<Assembler>,
                    handle_right_click_block_ui::<Source>,
                    handle_right_click_block_ui::<Miner>,
                    update_delete_preview,
                    handle_change_incline,
                    (
                        compute_placement_target,
                        handle_click_to_place,
                        commit_belt_placement,
                        update_belt_placement,
                        sync_belt_ghosts,
                        update_single_ghost,
                    )
                        .chain(),
                )
                    .after(handle_mode_inputs),
            ),
        );

        app.add_systems(Update, draw_crosshair_gizmo);
        app.add_systems(Update, camera_look);
        app.add_systems(Update, cursor_grab.after(handle_click_to_place));
    }
}

trait BlockUIScreen: Component {
    fn screen(entity: Entity) -> ScreenMode;
}

impl BlockUIScreen for Assembler {
    fn screen(entity: Entity) -> ScreenMode {
        ScreenMode::Assembler(entity)
    }
}

impl BlockUIScreen for Furnace {
    fn screen(entity: Entity) -> ScreenMode {
        ScreenMode::Furnace(entity)
    }
}

impl BlockUIScreen for Source {
    fn screen(entity: Entity) -> ScreenMode {
        ScreenMode::Source(entity)
    }
}

impl BlockUIScreen for Miner {
    fn screen(entity: Entity) -> ScreenMode {
        ScreenMode::Miner(entity)
    }
}

#[derive(Resource)]
pub(super) struct PlacementDirection(pub(super) HDir);

impl Default for PlacementDirection {
    fn default() -> Self {
        Self(HDir::North)
    }
}

#[derive(Component)]
struct DeletePreview;

#[derive(Component)]
struct InclinePreview;

/// The physics body for the player. The camera is a child entity.
#[derive(Component)]
struct PlayerBody {
    speed: f32,
    jump_impulse: f32,
    jump_cooldown: f32,
    /// Seconds remaining in which the player can still jump after walking off a ledge.
    coyote_timer: f32,
    /// Seconds remaining on a buffered jump input (pressed slightly before landing).
    jump_buffer: f32,
}

#[derive(Component)]
struct FirstPersonCamera {
    pitch: f32,
    yaw: f32,
    sensitivity: f32,
}

fn setup(
    mut cmd: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    fly_mode: Res<FlyMode>,
) {
    let ambient = AmbientLight {
        color: Color::WHITE,
        brightness: 500.0,
        ..Default::default()
    };

    if fly_mode.0 {
        // Free-fly (noclip) mode: camera is a standalone entity, no physics body.
        cmd.spawn((
            Camera3d::default(),
            Transform::from_xyz(1.5, 2.0, 1.5),
            FirstPersonCamera {
                pitch: 0.0,
                yaw: 0.0,
                sensitivity: 0.002,
            },
            ambient,
        ));
    } else {
        // Physics mode: kinematic body with camera as child at eye height.
        // Cylinder: radius 0.3, height 1.8. Flat bottom/top for stable edge contact.
        // CustomPositionIntegration: we own all position updates via MoveAndSlide.
        let body = cmd
            .spawn((
                PlayerBody {
                    speed: 5.0,
                    jump_impulse: 8.0,
                    jump_cooldown: 0.0,
                    coyote_timer: 0.0,
                    jump_buffer: 0.0,
                },
                RigidBody::Kinematic,
                CustomPositionIntegration,
                Collider::cylinder(0.3, 1.8),
                LockedAxes::ROTATION_LOCKED,
                Transform::from_xyz(1.5, 2.0, 1.5),
                Visibility::Inherited,
                // Ground sensor: small sphere cast downward from inside the bottom of the capsule.
                ShapeCaster::new(
                    Collider::sphere(0.25),
                    Vec3::new(0.0, -0.7, 0.0),
                    Quat::IDENTITY,
                    Dir3::NEG_Y,
                )
                .with_max_distance(0.25),
            ))
            .id();

        cmd.spawn((
            Camera3d::default(),
            Transform::from_xyz(0.0, 0.6, 0.0),
            FirstPersonCamera {
                pitch: 0.0,
                yaw: 0.0,
                sensitivity: 0.002,
            },
            ambient,
            ChildOf(body),
        ));
    }

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

pub(super) fn cursor_grab(
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

fn camera_look(
    accumulated_mouse_motion: Res<AccumulatedMouseMotion>,
    mut query: Query<(&mut Transform, &mut FirstPersonCamera)>,
    mode: Res<InteractionMode>,
) {
    if matches!(*mode, InteractionMode::InScreen(_)) {
        return;
    }
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

fn player_movement(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    camera_q: Query<&FirstPersonCamera>,
    mut body_q: Query<(
        Entity,
        &mut PlayerBody,
        &mut LinearVelocity,
        &ShapeHits,
        &Collider,
        &mut Transform,
    )>,
    mode: Res<InteractionMode>,
    move_and_slide: MoveAndSlide,
) {
    if matches!(*mode, InteractionMode::InScreen(_)) {
        return;
    }
    let Ok(camera) = camera_q.single() else {
        return;
    };
    let Ok((entity, mut body, mut vel, hits, collider, mut transform)) = body_q.single_mut() else {
        return;
    };

    let dt = time.delta_secs();

    body.jump_cooldown = (body.jump_cooldown - dt).max(0.0);
    body.jump_buffer = (body.jump_buffer - dt).max(0.0);

    let grounded = hits.iter().next().is_some();

    // Coyote time: reset while grounded, count down while airborne.
    if grounded {
        body.coyote_timer = 0.15;
    } else {
        body.coyote_timer = (body.coyote_timer - dt).max(0.0);
    }

    // Buffer a jump input so it executes on the next landing if pressed slightly early.
    if keys.just_pressed(KeyCode::Space) {
        body.jump_buffer = 0.15;
    }

    // Apply gravity manually (kinematic bodies receive no automatic gravity).
    const GRAVITY: f32 = 9.81 * 2.5; // matches the old GravityScale(2.5)
    const TERMINAL_VELOCITY: f32 = -50.0;
    vel.y = (vel.y - GRAVITY * dt).max(TERMINAL_VELOCITY);

    // Horizontal velocity from WASD input.
    let forward = Vec3::new(-camera.yaw.sin(), 0.0, -camera.yaw.cos());
    let right = Vec3::new(camera.yaw.cos(), 0.0, -camera.yaw.sin());

    let mut dir = Vec3::ZERO;
    if keys.pressed(KeyCode::KeyW) {
        dir += forward;
    }
    if keys.pressed(KeyCode::KeyS) {
        dir -= forward;
    }
    if keys.pressed(KeyCode::KeyA) {
        dir -= right;
    }
    if keys.pressed(KeyCode::KeyD) {
        dir += right;
    }
    if dir.length_squared() > 0.0 {
        dir = dir.normalize();
    }

    vel.x = dir.x * body.speed;
    vel.z = dir.z * body.speed;

    // Jump.
    let can_jump = body.coyote_timer > 0.0 && body.jump_cooldown <= 0.0;
    if can_jump && body.jump_buffer > 0.0 {
        vel.y = body.jump_impulse;
        body.jump_cooldown = 0.25;
        body.coyote_timer = 0.0;
        body.jump_buffer = 0.0;
    }

    // Move and slide: sweeps the shape, resolves contacts, and projects velocity along surfaces.
    // This replaces the dynamic rigid body's constraint solver for player movement.
    let output = move_and_slide.move_and_slide(
        collider,
        transform.translation,
        transform.rotation,
        vel.0,
        time.delta(),
        &MoveAndSlideConfig::default(),
        &SpatialQueryFilter::from_excluded_entities([entity]),
        |_| MoveAndSlideHitResponse::Accept,
    );

    transform.translation = output.position;
    // Preserve the projected vertical velocity (floor/ceiling contacts zero it naturally).
    // Horizontal velocity is always fresh from input, so projected_velocity.x/z are ignored.
    vel.y = output.projected_velocity.y;
}

/// Free-fly (noclip) movement: WASD moves horizontally, Space/Shift moves vertically.
fn fly_movement(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut query: Query<(&mut Transform, &FirstPersonCamera)>,
    mode: Res<InteractionMode>,
) {
    if matches!(*mode, InteractionMode::InScreen(_)) {
        return;
    }
    let speed = 5.0;
    for (mut transform, camera) in query.iter_mut() {
        let forward = Vec3::new(-camera.yaw.sin(), 0.0, -camera.yaw.cos());
        let right = Vec3::new(camera.yaw.cos(), 0.0, -camera.yaw.sin());

        let mut direction = Vec3::ZERO;
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
        transform.translation += direction * speed * time.delta_secs();

        if keys.pressed(KeyCode::Space) {
            transform.translation.y += speed * time.delta_secs();
        }
        if keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight) {
            transform.translation.y -= speed * time.delta_secs();
        }
    }
}

struct RayTarget {
    coords: WorldCoords,
    center: Vec3,
    half_extents: Vec3,
}

struct RayHit {
    hit_coords: WorldCoords,
    place_coords: WorldCoords,
}

/// Cast a ray from the camera and look up the hit entity in the coord map.
struct ResolvedHit {
    /// World-space bottom-centre of the block (entity Transform translation).
    center: Vec3,
    entity: Entity,
    half_extents: Vec3,
}

fn raycast_and_resolve(
    origin: Vec3,
    forward: Vec3,
    targets: impl Iterator<Item = RayTarget>,
    coord_map: &CoordsMap,
    find_entity: impl Fn(Entity) -> Option<(Vec3, Vec3)>,
) -> Option<ResolvedHit> {
    let hit = cast_ray(origin, forward, targets)?;
    let &entity = coord_map.0.get(&hit.hit_coords)?;
    let (center, half_extents) = find_entity(entity)?;
    Some(ResolvedHit {
        center,
        entity,
        half_extents,
    })
}

fn cast_ray(origin: Vec3, dir: Vec3, targets: impl Iterator<Item = RayTarget>) -> Option<RayHit> {
    // (t_enter, hit_coords, enter_axis, y_slots)
    let mut best: Option<(f32, WorldCoords, usize, i32)> = None;

    'outer: for RayTarget {
        coords,
        center,
        half_extents,
    } in targets
    {
        // Entity transform is at the center-bottom; shift up to AABB center.
        let center = Vec3::new(center.x, center.y + half_extents.y, center.z);
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

        if best.map_or(true, |(best_t, _, _, _)| t_enter < best_t) {
            let y_slots = (2.0 * half_extents.y / 0.5).round() as i32;
            best = Some((t_enter, coords, enter_axis, y_slots));
        }
    }

    best.map(|(t_enter, hit_coords, enter_axis, y_slots)| {
        let hit = origin + t_enter * dir;
        let base_y = hit_coords.height();

        let place_coords = if enter_axis == 1 {
            // Top/bottom face: use the hit x/z to pick the target cell, then step
            // y past all slots the block occupies.
            let px = hit.x.round() as i32;
            let pz = hit.z.round() as i32;
            let py = if dir.y > 0.0 {
                base_y - y_slots
            } else {
                base_y + y_slots
            };
            WorldCoords::from((px, py, pz))
        } else {
            // Side face: step one cell outward from the actual face position on
            // the hit axis, and snap y to the block's occupied half-block range.
            let raw_y = (hit.y / 0.5).round() as i32;
            let py = raw_y.clamp(base_y, base_y + y_slots - 1);
            let mut place = [hit.x.round() as i32, py, hit.z.round() as i32];
            // On the entry axis, offset outward by half a cell so the rounded
            // result lands in the adjacent cell rather than on the face itself.
            place[enter_axis] = (hit[enter_axis] - dir[enter_axis].signum() * 0.5).floor() as i32;
            WorldCoords::from((place[0], place[1], place[2]))
        };

        RayHit {
            hit_coords,
            place_coords,
        }
    })
}

fn handle_mode_inputs(
    keys: Res<ButtonInput<KeyCode>>,
    mut mode: ResMut<InteractionMode>,
    mut placement_dir: ResMut<PlacementDirection>,
) {
    if keys.just_pressed(KeyCode::KeyE) {
        match *mode {
            InteractionMode::InScreen(ScreenMode::Inventory) => {
                *mode = InteractionMode::InWorld(WorldMode::None);
            }
            InteractionMode::InWorld(_) => {
                *mode = InteractionMode::InScreen(ScreenMode::Inventory);
            }
            _ => {}
        }
    }

    // World-mode keys only apply when not in a screen
    if matches!(*mode, InteractionMode::InScreen(_)) {
        return;
    }

    if keys.just_pressed(KeyCode::KeyX) {
        if *mode == InteractionMode::InWorld(WorldMode::Deleting) {
            *mode = InteractionMode::InWorld(WorldMode::None);
        } else {
            *mode = InteractionMode::InWorld(WorldMode::Deleting);
        }
    }
    if keys.just_pressed(KeyCode::KeyC) {
        if *mode == InteractionMode::InWorld(WorldMode::ChangingIncline) {
            *mode = InteractionMode::InWorld(WorldMode::None);
        } else {
            *mode = InteractionMode::InWorld(WorldMode::ChangingIncline);
        }
    }
    if keys.just_pressed(KeyCode::KeyR) {
        placement_dir.0 = placement_dir.0.right();
    }
}

fn update_delete_preview(
    mode: Res<InteractionMode>,
    mouse: Res<ButtonInput<MouseButton>>,
    cursor_options: Single<&CursorOptions>,
    camera_q: Single<(&Transform, &GlobalTransform), With<FirstPersonCamera>>,
    coord_map: Res<CoordsMap>,
    mut preview_q: Single<
        (&mut Transform, &mut Visibility),
        (With<DeletePreview>, Without<FirstPersonCamera>),
    >,
    mut cmd: Commands,
    targets: Query<(&WorldCoords, &Transform, &RaycastTarget), Without<DeletePreview>>,
) {
    let (ref mut t, ref mut vis) = *preview_q;
    let cursor_locked = cursor_options.grab_mode == CursorGrabMode::Locked;

    if *mode != InteractionMode::InWorld(WorldMode::Deleting) || !cursor_locked {
        **vis = Visibility::Hidden;
        return;
    }

    let (cam_local, cam_global) = camera_q.into_inner();
    let Some(resolved) = raycast_and_resolve(
        cam_global.translation(),
        *cam_local.forward(),
        targets.iter().map(|(c, tr, rt)| RayTarget {
            coords: *c,
            center: tr.translation,
            half_extents: rt.half_extents,
        }),
        &coord_map,
        |e| {
            targets
                .get(e)
                .ok()
                .map(|(_, tr, rt)| (tr.translation, rt.half_extents))
        },
    ) else {
        **vis = Visibility::Hidden;
        return;
    };

    **vis = Visibility::Visible;
    let mut pos = resolved.center;
    pos.y += resolved.half_extents.y;
    t.translation = pos;
    t.scale = resolved.half_extents * 2.0 * 1.05;

    if mouse.just_pressed(MouseButton::Left) {
        cmd.trigger(RemoveBlock {
            entity: resolved.entity,
        });
    }
}

fn handle_change_incline(
    mode: Res<InteractionMode>,
    mouse: Res<ButtonInput<MouseButton>>,
    cursor_options: Single<&CursorOptions>,
    camera_q: Single<(&Transform, &GlobalTransform), With<FirstPersonCamera>>,
    coord_map: Res<CoordsMap>,
    mut preview_q: Single<
        (&mut Transform, &mut Visibility),
        (With<InclinePreview>, Without<FirstPersonCamera>),
    >,
    mut cmd: Commands,
    targets: Query<
        (&WorldCoords, &Transform, &RaycastTarget),
        (Without<InclinePreview>, With<Belt>),
    >,
) {
    let (ref mut t, ref mut vis) = *preview_q;
    let cursor_locked = cursor_options.grab_mode == CursorGrabMode::Locked;

    if *mode != InteractionMode::InWorld(WorldMode::ChangingIncline) || !cursor_locked {
        **vis = Visibility::Hidden;
        return;
    }

    let (cam_local, cam_global) = camera_q.into_inner();
    let Some(resolved) = raycast_and_resolve(
        cam_global.translation(),
        *cam_local.forward(),
        targets.iter().map(|(c, tr, rt)| RayTarget {
            coords: *c,
            center: tr.translation,
            half_extents: rt.half_extents,
        }),
        &coord_map,
        |e| {
            targets
                .get(e)
                .ok()
                .map(|(_, tr, rt)| (tr.translation, rt.half_extents))
        },
    ) else {
        **vis = Visibility::Hidden;
        return;
    };

    **vis = Visibility::Visible;
    let mut pos = resolved.center;
    pos.y += resolved.half_extents.y;
    t.translation = pos;
    t.scale = resolved.half_extents * 2.0 * 1.05;

    if mouse.just_pressed(MouseButton::Left) {
        cmd.trigger(Incline {
            entity: resolved.entity,
        });
    }
}

fn handle_right_click_block_ui<T: BlockUIScreen>(
    mouse: Res<ButtonInput<MouseButton>>,
    cursor_options: Single<&CursorOptions>,
    camera_q: Single<(&Transform, &GlobalTransform), With<FirstPersonCamera>>,
    coord_map: Res<CoordsMap>,
    targets: Query<(&WorldCoords, &Transform, &RaycastTarget)>,
    blocks: Query<(), With<T>>,
    mut mode: ResMut<InteractionMode>,
) {
    if cursor_options.grab_mode != CursorGrabMode::Locked {
        return;
    }
    if !mouse.just_pressed(MouseButton::Right) {
        return;
    }
    if !matches!(*mode, InteractionMode::InWorld(_)) {
        return;
    }

    let (cam_local, cam_global) = camera_q.into_inner();
    let origin = cam_global.translation();
    let ray_dir = *cam_local.forward();

    let Some(hit) = cast_ray(
        origin,
        ray_dir,
        targets.iter().map(|(c, t, rt)| RayTarget {
            coords: *c,
            center: t.translation,
            half_extents: rt.half_extents,
        }),
    ) else {
        return;
    };

    let Some(&entity) = coord_map.0.get(&hit.hit_coords) else {
        return;
    };

    if blocks.get(entity).is_ok() {
        *mode = InteractionMode::InScreen(T::screen(entity));
    }
}

fn draw_crosshair_gizmo(
    mut gizmos: Gizmos,
    cursor_options: Single<&CursorOptions>,
    camera_q: Single<(&Transform, &GlobalTransform), With<FirstPersonCamera>>,
    targets: Query<(&WorldCoords, &Transform, &RaycastTarget)>,
    coord_map: Res<CoordsMap>,
) {
    if cursor_options.grab_mode != CursorGrabMode::Locked {
        return;
    }

    let (cam_local, cam_global) = camera_q.into_inner();
    let Some(resolved) = raycast_and_resolve(
        cam_global.translation(),
        *cam_local.forward(),
        targets.iter().map(|(c, t, rt)| RayTarget {
            coords: *c,
            center: t.translation,
            half_extents: rt.half_extents,
        }),
        &coord_map,
        |e| {
            targets
                .get(e)
                .ok()
                .map(|(_, t, rt)| (t.translation, rt.half_extents))
        },
    ) else {
        return;
    };

    let mut pos = resolved.center;
    pos.y += resolved.half_extents.y;
    let size = resolved.half_extents * 2.0;

    gizmos.cube(
        Transform::from_translation(pos).with_scale(size),
        Color::srgba(1.0, 1.0, 1.0, 0.6),
    );
}
