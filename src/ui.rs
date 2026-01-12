use crate::core::*;

use bevy::{
    asset::RenderAssetUsages,
    input::mouse::AccumulatedMouseMotion,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
    window::{CursorGrabMode, CursorOptions},
};
use rand::Rng;

pub struct UiPlugin;
impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ClearColor(Color::srgb(0.01, 0.01, 0.05))); // Dark night sky
        app.add_systems(Startup, setup);

        // Systems that trigger events Must run in PreUpdate
        app.add_systems(PreUpdate, camera_movement);
        app.add_systems(PreUpdate, handle_click_to_place);
        app.add_systems(PreUpdate, handle_place_item_on_belt);

        app.add_systems(Update, camera_look);
        app.add_systems(Update, cursor_grab.after(handle_click_to_place));

        app.add_observer(on_place_belt);
        app.add_observer(on_place_item);
    }
}

#[derive(Component)]
struct FirstPersonCamera {
    pitch: f32,
    yaw: f32,
    sensitivity: f32,
    speed: f32,
    fixed_y: f32,
}

#[derive(Resource)]
struct RenderStuff {
    cube_mesh_handle: Handle<Mesh>,
    material_mesh_handle: Handle<StandardMaterial>,
}

fn setup(
    mut cmd: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    asset_server: Res<AssetServer>,
) {
    // Create and save a handle to the mesh.
    let cube_mesh_handle: Handle<Mesh> = meshes.add(create_cube_mesh());
    let material_mesh_handle: Handle<StandardMaterial> = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        ..default()
    });
    cmd.insert_resource(RenderStuff {
        cube_mesh_handle,
        material_mesh_handle,
    });

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
            fixed_y: camera_transform.translation.y,
        },
        AmbientLight {
            color: Color::WHITE,
            brightness: 100.0,
            affects_lightmapped_meshes: true,
        },
    ));

    // Light up the scene.
    cmd.spawn((PointLight::default(), light_transform));

    cmd.spawn((
        SceneRoot(asset_server.load(GltfAssetLabel::Scene(0).from_asset("models/item.glb"))),
        item_position(
            BeltShape::Straight(HDir::North),
            (0, 0, 0),
            LaneSide::Left,
            0,
        ),
    ));

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

fn on_place_belt(event: On<PlaceBelt>, mut cmd: Commands, asset_server: Res<AssetServer>) {
    cmd.entity(event.entity).insert(SceneRoot(
        asset_server.load(GltfAssetLabel::Scene(0).from_asset("models/box.glb")),
    ));
}

fn on_place_item(event: On<PlaceItem>, mut cmd: Commands, asset_server: Res<AssetServer>) {
    cmd.entity(event.entity).insert(SceneRoot(
        asset_server.load(GltfAssetLabel::Scene(0).from_asset("models/item.glb")),
    ));
}

#[rustfmt::skip]
fn create_cube_mesh() -> Mesh {
    // Keep the mesh data accessible in future frames to be able to mutate it in toggle_texture.
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD)
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_POSITION,
        // Each array is an [x, y, z] coordinate in local space.
        // The camera coordinate space is right-handed x-right, y-up, z-back. This means "forward" is -Z.
        // Meshes always rotate around their local [0, 0, 0] when a rotation is applied to their Transform.
        // By centering our mesh around the origin, rotating the mesh preserves its center of mass.
        vec![
            // top (facing towards +y)
            [-0.5, 0.5, -0.5], // vertex with index 0
            [0.5, 0.5, -0.5], // vertex with index 1
            [0.5, 0.5, 0.5], // etc. until 23
            [-0.5, 0.5, 0.5],
            // bottom   (-y)
            [-0.5, -0.5, -0.5],
            [0.5, -0.5, -0.5],
            [0.5, -0.5, 0.5],
            [-0.5, -0.5, 0.5],
            // right    (+x)
            [0.5, -0.5, -0.5],
            [0.5, -0.5, 0.5],
            [0.5, 0.5, 0.5], // This vertex is at the same position as vertex with index 2, but they'll have different UV and normal
            [0.5, 0.5, -0.5],
            // left     (-x)
            [-0.5, -0.5, -0.5],
            [-0.5, -0.5, 0.5],
            [-0.5, 0.5, 0.5],
            [-0.5, 0.5, -0.5],
            // back     (+z)
            [-0.5, -0.5, 0.5],
            [-0.5, 0.5, 0.5],
            [0.5, 0.5, 0.5],
            [0.5, -0.5, 0.5],
            // forward  (-z)
            [-0.5, -0.5, -0.5],
            [-0.5, 0.5, -0.5],
            [0.5, 0.5, -0.5],
            [0.5, -0.5, -0.5],
        ],
    )
    // Set-up UV coordinates to point to the upper (V < 0.5), "dirt+grass" part of the texture.
    // Take a look at the custom image (assets/textures/array_texture.png)
    // so the UV coords will make more sense
    // Note: (0.0, 0.0) = Top-Left in UV mapping, (1.0, 1.0) = Bottom-Right in UV mapping
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_UV_0,
        vec![
            // Assigning the UV coords for the top side.
            [0.0, 0.2], [0.0, 0.0], [1.0, 0.0], [1.0, 0.2],
            // Assigning the UV coords for the bottom side.
            [0.0, 0.45], [0.0, 0.25], [1.0, 0.25], [1.0, 0.45],
            // Assigning the UV coords for the right side.
            [1.0, 0.45], [0.0, 0.45], [0.0, 0.2], [1.0, 0.2],
            // Assigning the UV coords for the left side.
            [1.0, 0.45], [0.0, 0.45], [0.0, 0.2], [1.0, 0.2],
            // Assigning the UV coords for the back side.
            [0.0, 0.45], [0.0, 0.2], [1.0, 0.2], [1.0, 0.45],
            // Assigning the UV coords for the forward side.
            [0.0, 0.45], [0.0, 0.2], [1.0, 0.2], [1.0, 0.45],
        ],
    )
    // For meshes with flat shading, normals are orthogonal (pointing out) from the direction of
    // the surface.
    // Normals are required for correct lighting calculations.
    // Each array represents a normalized vector, which length should be equal to 1.0.
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_NORMAL,
        vec![
            // Normals for the top side (towards +y)
            [0.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            // Normals for the bottom side (towards -y)
            [0.0, -1.0, 0.0],
            [0.0, -1.0, 0.0],
            [0.0, -1.0, 0.0],
            [0.0, -1.0, 0.0],
            // Normals for the right side (towards +x)
            [1.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            // Normals for the left side (towards -x)
            [-1.0, 0.0, 0.0],
            [-1.0, 0.0, 0.0],
            [-1.0, 0.0, 0.0],
            [-1.0, 0.0, 0.0],
            // Normals for the back side (towards +z)
            [0.0, 0.0, 1.0],
            [0.0, 0.0, 1.0],
            [0.0, 0.0, 1.0],
            [0.0, 0.0, 1.0],
            // Normals for the forward side (towards -z)
            [0.0, 0.0, -1.0],
            [0.0, 0.0, -1.0],
            [0.0, 0.0, -1.0],
            [0.0, 0.0, -1.0],
        ],
    )
    // Create the triangles out of the 24 vertices we created.
    // To construct a square, we need 2 triangles, therefore 12 triangles in total.
    // To construct a triangle, we need the indices of its 3 defined vertices, adding them one
    // by one, in a counter-clockwise order (relative to the position of the viewer, the order
    // should appear counter-clockwise from the front of the triangle, in this case from outside the cube).
    // Read more about how to correctly build a mesh manually in the Bevy documentation of a Mesh,
    // further examples and the implementation of the built-in shapes.
    //
    // The first two defined triangles look like this (marked with the vertex indices,
    // and the axis), when looking down at the top (+y) of the cube:
    //   -Z
    //   ^
    // 0---1
    // |  /|
    // | / | -> +X
    // |/  |
    // 3---2
    //
    // The right face's (+x) triangles look like this, seen from the outside of the cube.
    //   +Y
    //   ^
    // 10--11
    // |  /|
    // | / | -> -Z
    // |/  |
    // 9---8
    //
    // The back face's (+z) triangles look like this, seen from the outside of the cube.
    //   +Y
    //   ^
    // 17--18
    // |\  |
    // | \ | -> +X
    // |  \|
    // 16--19
    .with_inserted_indices(Indices::U32(vec![
        0,3,1 , 1,3,2, // triangles making up the top (+y) facing side.
        4,5,7 , 5,6,7, // bottom (-y)
        8,11,9 , 9,11,10, // right (+x)
        12,13,15 , 13,14,15, // left (-x)
        16,19,17 , 17,19,18, // back (+z)
        20,21,23 , 21,22,23, // forward (-z)
    ]))
}

fn cursor_grab(
    mut cursor_options: Single<&mut CursorOptions>,
    mouse: Res<ButtonInput<MouseButton>>,
    key: Res<ButtonInput<KeyCode>>,
) {
    // Only grab cursor on left click if not already grabbed
    if mouse.just_pressed(MouseButton::Left) && cursor_options.grab_mode != CursorGrabMode::Locked {
        cursor_options.visible = false;
        cursor_options.grab_mode = CursorGrabMode::Locked;
    }

    if key.just_pressed(KeyCode::Escape) {
        cursor_options.visible = true;
        cursor_options.grab_mode = CursorGrabMode::None;
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

        // Keep Y position fixed
        transform.translation.y = camera.fixed_y;
    }
}

fn handle_click_to_place(
    mouse: Res<ButtonInput<MouseButton>>,
    cursor_options: Single<&CursorOptions>,
    camera_query: Single<(&Transform, &FirstPersonCamera)>,
    mut cmd: Commands,
) {
    // Only handle clicks when cursor is grabbed (in game mode)
    if !mouse.just_pressed(MouseButton::Left) || cursor_options.grab_mode != CursorGrabMode::Locked
    {
        return;
    }

    let (camera_transform, camera) = camera_query.into_inner();

    // Get camera position and forward direction
    let camera_pos = camera_transform.translation;
    let camera_forward = camera_transform.forward();

    // Intersect ray with XZ plane (y = 0)
    // Ray equation: P = camera_pos + t * camera_forward
    // Plane equation: y = 0
    // Solve for t: camera_pos.y + t * camera_forward.y = 0

    if camera_forward.y.abs() < 0.001 {
        // Ray is parallel to the plane, no intersection
        return;
    }

    let t = -camera_pos.y / camera_forward.y;

    if t < 0.0 {
        // Intersection is behind the camera
        return;
    }

    let intersection = camera_pos + camera_forward * t;

    // Convert world position to WorldCoords
    // WorldCoords are discrete grid coordinates, world positions are multiplied by BLOCK_SIZE (2.0)
    let world_x = (intersection.x / BLOCK_SIZE).round() as i32;
    let world_z = (intersection.z / BLOCK_SIZE).round() as i32;

    let coords = WorldCoords {
        x: world_x,
        y: 0,
        z: world_z,
    };

    // Determine belt direction based on camera forward direction (belt faces away from camera)
    // Project camera forward onto XZ plane and calculate angle
    let forward_xz = Vec3::new(camera_forward.x, 0.0, camera_forward.z).normalize();
    let angle = forward_xz.z.atan2(forward_xz.x);

    // HDir angle mapping: North=0, East=-PI/2, South=PI, West=PI/2
    let dir = angle_to_hdir(angle);

    // Create entity and trigger PlaceBelt event
    let entity = cmd.spawn_empty().id();
    let event = PlaceBelt {
        entity,
        coords,
        dir,
    };
    debug!("Triggering: {event:?}");
    cmd.trigger(event);
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

fn handle_place_item_on_belt(
    keys: Res<ButtonInput<KeyCode>>,
    cursor_options: Single<&CursorOptions>,
    camera_query: Single<&Transform, With<FirstPersonCamera>>,
    belt_coords: Res<BeltCoords>,
    mut cmd: Commands,
) {
    // Only handle spacebar when cursor is grabbed (game mode)
    if !keys.just_pressed(KeyCode::Space) || cursor_options.grab_mode != CursorGrabMode::Locked {
        return;
    }

    let camera_transform = camera_query.into_inner();
    let ray_origin = camera_transform.translation;
    let ray_dir = camera_transform.forward().as_vec3();

    // Find the closest belt that the ray intersects
    let mut closest_hit: Option<(f32, Entity, BeltShape, WorldCoords, Vec3)> = None;

    for (coords, (entity, belt_shape)) in belt_coords.iter() {
        // Get belt center in world space
        let belt_center = Vec3::from(*coords);

        // Ray-AABB intersection test
        if let Some((t, hit_point)) =
            ray_box_intersection(ray_origin, ray_dir, belt_center, Vec3::splat(BLOCK_SIZE))
        {
            if closest_hit.is_none() || t < closest_hit.as_ref().unwrap().0 {
                closest_hit = Some((t, *entity, *belt_shape, *coords, hit_point));
            }
        }
    }

    if let Some((_t, belt_entity, belt_shape, coords, hit_point)) = closest_hit {
        // Convert hit point to belt local space
        let belt_center = Vec3::from(coords);
        let local_hit = hit_point - belt_center;

        // Rotate hit point to belt's local coordinate system
        let belt_angle = match belt_shape {
            BeltShape::Straight(dir) | BeltShape::Fragment(dir) => dir.angle(),
            BeltShape::Curve(curve) => curve.input().angle(),
        };
        let rotation = Quat::from_rotation_y(-belt_angle);
        let local_rotated = rotation * local_hit;

        // Determine lane based on z coordinate
        // Left lane is at z = -LANE_OFFSET, Right lane is at z = LANE_OFFSET
        let lane = if local_rotated.z < 0.0 {
            LaneSide::Left
        } else {
            LaneSide::Right
        };

        // Determine position based on x coordinate (for straight belts)
        // Position 0 is at x = HALF_BLOCK_SIZE, position POSITIONS_PER_BELT is at x = -HALF_BLOCK_SIZE
        let position = match belt_shape {
            BeltShape::Straight(_) | BeltShape::Fragment(_) => {
                let t = (HALF_BLOCK_SIZE - local_rotated.x) / BLOCK_SIZE;
                let pos = (t * POSITIONS_PER_BELT as f32).round() as i32;
                pos.clamp(0, POSITIONS_PER_BELT - 1)
            }
            BeltShape::Curve(_) => {
                // For curves, use a simpler approach - just use middle position for now
                let num_pos = belt_shape.num_pos(lane);
                num_pos / 2
            }
        };

        // Create item entity and trigger PlaceItem event
        let item_entity = cmd.spawn_empty().id();
        let event = PlaceItem {
            entity: item_entity,
            item: Item(0),
            belt: belt_entity,
            lane,
            position,
        };
        debug!("triggering: {event:?}");
        cmd.trigger(event);
    }
}

// Ray-AABB intersection test
// Returns Some((t, hit_point)) if ray intersects the box, where t is the distance along the ray
fn ray_box_intersection(
    ray_origin: Vec3,
    ray_dir: Vec3,
    box_center: Vec3,
    box_size: Vec3,
) -> Option<(f32, Vec3)> {
    let box_min = box_center - box_size / 2.0;
    let box_max = box_center + box_size / 2.0;

    let mut tmin = f32::NEG_INFINITY;
    let mut tmax = f32::INFINITY;

    for i in 0..3 {
        let dir_component = ray_dir[i];
        let origin_component = ray_origin[i];
        let min_component = box_min[i];
        let max_component = box_max[i];

        if dir_component.abs() < 0.0001 {
            // Ray is parallel to slab
            if origin_component < min_component || origin_component > max_component {
                return None;
            }
        } else {
            let inv_d = 1.0 / dir_component;
            let mut t1 = (min_component - origin_component) * inv_d;
            let mut t2 = (max_component - origin_component) * inv_d;

            if t1 > t2 {
                std::mem::swap(&mut t1, &mut t2);
            }

            tmin = tmin.max(t1);
            tmax = tmax.min(t2);

            if tmin > tmax {
                return None;
            }
        }
    }

    if tmax < 0.0 {
        return None;
    }

    let t = if tmin >= 0.0 { tmin } else { tmax };
    let hit_point = ray_origin + ray_dir * t;

    Some((t, hit_point))
}
