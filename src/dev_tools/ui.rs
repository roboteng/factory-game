use bevy::prelude::*;
use crate::core::Item;
use std::collections::HashMap;

pub struct DevUiPlugin;

impl Plugin for DevUiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ItemGraphData>();
        app.add_systems(Startup, setup_graph_labels);
        app.add_systems(Update, collect_item_data);
        app.add_systems(Update, render_debug_graphs);
        app.add_systems(Update, update_graph_labels);
    }
}

// Marker components for graph labels
#[derive(Component)]
struct PositionGraphTitle;

#[derive(Component)]
struct PositionGraphScale;

#[derive(Component)]
struct SpeedGraphTitle;

#[derive(Component)]
struct SpeedGraphScale;

const MAX_SAMPLES: usize = 500;  // ~8 seconds at 60 fps
const ASSUMED_TIMESTEP: f32 = 1.0 / 60.0;  // Assume 60 FPS for smooth graphs

// Resource to store historical data
#[derive(Resource)]
struct ItemGraphData {
    // Per-item position history (circular buffer)
    positions: HashMap<Entity, Vec<Vec2>>,
    // Per-item speed history (circular buffer)
    speeds: HashMap<Entity, Vec<f32>>,
    // Last frame positions for speed calculation
    previous_positions: HashMap<Entity, Vec2>,
    // Current write index in circular buffer
    current_index: usize,
    // Maximum number of samples
    max_samples: usize,
}

impl Default for ItemGraphData {
    fn default() -> Self {
        Self {
            positions: HashMap::new(),
            speeds: HashMap::new(),
            previous_positions: HashMap::new(),
            current_index: 0,
            max_samples: MAX_SAMPLES,
        }
    }
}

fn collect_item_data(
    mut graph_data: ResMut<ItemGraphData>,
    items: Query<(Entity, &Transform), With<Item>>,
) {
    let item_count = items.iter().count();

    // If no items, don't advance the graph and clear previous positions
    if item_count == 0 {
        graph_data.previous_positions.clear();
        return;
    }

    // If we just got our first item(s) after having none, reset to beginning
    let had_no_items_before = graph_data.previous_positions.is_empty();
    if had_no_items_before {
        graph_data.current_index = 0;
    }

    // Store current_index locally to avoid borrow checker issues
    let current_index = graph_data.current_index;
    let max_samples = graph_data.max_samples;

    for (entity, transform) in items.iter() {
        let pos = transform.translation.xy();

        // Calculate speed from position delta using fixed timestep for smoothness
        let speed = if let Some(prev_pos) = graph_data.previous_positions.get(&entity) {
            (pos - *prev_pos).length() / ASSUMED_TIMESTEP
        } else {
            0.0
        };

        // Ensure buffers exist for this entity
        if !graph_data.positions.contains_key(&entity) {
            graph_data.positions.insert(entity, vec![Vec2::ZERO; max_samples]);
        }
        if !graph_data.speeds.contains_key(&entity) {
            graph_data.speeds.insert(entity, vec![0.0; max_samples]);
        }

        // Store current data in circular buffer
        if let Some(positions) = graph_data.positions.get_mut(&entity) {
            positions[current_index] = pos;
        }
        if let Some(speeds) = graph_data.speeds.get_mut(&entity) {
            speeds[current_index] = speed;
        }

        // Update previous position
        graph_data.previous_positions.insert(entity, pos);
    }

    // Remove data for despawned items
    graph_data.positions.retain(|entity, _| items.get(*entity).is_ok());
    graph_data.speeds.retain(|entity, _| items.get(*entity).is_ok());
    graph_data.previous_positions.retain(|entity, _| items.get(*entity).is_ok());

    // Increment wrap index (only when we have items)
    graph_data.current_index = (graph_data.current_index + 1) % graph_data.max_samples;
}

fn render_debug_graphs(
    graph_data: Res<ItemGraphData>,
    mut gizmos: Gizmos,
    windows: Query<&Window>,
    camera: Query<(&Camera, &GlobalTransform)>,
) {
    if graph_data.positions.is_empty() {
        return;
    }

    // Get window dimensions
    let Ok(window) = windows.single() else {
        return;
    };
    let Ok((cam, cam_transform)) = camera.single() else {
        return;
    };

    // Graph dimensions
    let graph_width = 400.0;
    let graph_height = 100.0;
    let margin = 20.0;
    let spacing = 20.0;

    // Calculate viewport bounds in world space
    let window_width = window.resolution.width();
    let window_height = window.resolution.height();

    // Get the top-right corner of the viewport in world space
    let viewport_top_right = cam
        .viewport_to_world_2d(cam_transform, Vec2::new(window_width, 0.0))
        .unwrap_or(Vec2::new(window_width / 2.0, window_height / 2.0));

    let viewport_top_left = cam
        .viewport_to_world_2d(cam_transform, Vec2::new(0.0, 0.0))
        .unwrap_or(Vec2::new(-window_width / 2.0, window_height / 2.0));

    // Position graphs in top-right corner relative to viewport
    let right_edge = viewport_top_right.x - margin;
    let top_edge = viewport_top_right.y - margin;
    let graph_left = right_edge - graph_width;

    // Make sure graph doesn't go off the left side
    let graph_left = graph_left.max(viewport_top_left.x + margin);
    let right_edge = graph_left + graph_width;

    let position_graph_top = top_edge;
    let position_graph_bottom = position_graph_top - graph_height;
    let speed_graph_top = position_graph_bottom - spacing;
    let speed_graph_bottom = speed_graph_top - graph_height;

    // Draw position graph
    draw_graph_background(&mut gizmos, graph_left, right_edge, position_graph_bottom, position_graph_top);
    draw_position_graph(&mut gizmos, &graph_data, graph_left, right_edge, position_graph_bottom, position_graph_top);
    draw_wrap_indicator(&mut gizmos, &graph_data, graph_left, right_edge, position_graph_bottom, position_graph_top);

    // Draw speed graph
    draw_graph_background(&mut gizmos, graph_left, right_edge, speed_graph_bottom, speed_graph_top);
    draw_speed_graph(&mut gizmos, &graph_data, graph_left, right_edge, speed_graph_bottom, speed_graph_top);
    draw_wrap_indicator(&mut gizmos, &graph_data, graph_left, right_edge, speed_graph_bottom, speed_graph_top);
}

fn draw_graph_background(gizmos: &mut Gizmos, left: f32, right: f32, bottom: f32, top: f32) {
    // Draw border
    let corners = [
        Vec2::new(left, bottom),
        Vec2::new(right, bottom),
        Vec2::new(right, top),
        Vec2::new(left, top),
    ];

    for i in 0..4 {
        let start = corners[i];
        let end = corners[(i + 1) % 4];
        gizmos.line_2d(start, end, Color::srgb(0.3, 0.3, 0.3));
    }

    // Draw grid lines
    let mid_h = (bottom + top) / 2.0;
    let mid_v = (left + right) / 2.0;

    gizmos.line_2d(
        Vec2::new(left, mid_h),
        Vec2::new(right, mid_h),
        Color::srgb(0.2, 0.2, 0.2),
    );
    gizmos.line_2d(
        Vec2::new(mid_v, bottom),
        Vec2::new(mid_v, top),
        Color::srgb(0.2, 0.2, 0.2),
    );
}

fn draw_position_graph(
    gizmos: &mut Gizmos,
    data: &ItemGraphData,
    left: f32,
    right: f32,
    bottom: f32,
    top: f32,
) {
    if data.positions.is_empty() {
        return;
    }

    // Find min/max X position for scaling
    let (mut min_x, mut max_x) = (f32::INFINITY, f32::NEG_INFINITY);

    for positions in data.positions.values() {
        for pos in positions {
            if *pos == Vec2::ZERO {
                continue;  // Skip uninitialized data
            }
            min_x = min_x.min(pos.x);
            max_x = max_x.max(pos.x);
        }
    }

    // Add padding
    let padding = 10.0;
    min_x -= padding;
    max_x += padding;

    let position_range = (max_x - min_x).max(1.0);
    let width = right - left;
    let height = top - bottom;

    // Draw each item's X position over time
    let colors = [
        Color::srgb(0.4, 0.8, 0.4),
        Color::srgb(0.8, 0.4, 0.4),
        Color::srgb(0.4, 0.4, 0.8),
        Color::srgb(0.8, 0.8, 0.4),
        Color::srgb(0.8, 0.4, 0.8),
        Color::srgb(0.4, 0.8, 0.8),
    ];

    for (idx, positions) in data.positions.values().enumerate() {
        let color = colors[idx % colors.len()];

        // Draw at fixed buffer positions so graph stays stationary
        for buffer_idx in 1..data.max_samples {
            let prev_buffer_idx = buffer_idx - 1;

            let pos = positions[buffer_idx];
            let prev_pos = positions[prev_buffer_idx];

            // Skip if positions are zero (not yet initialized)
            if pos == Vec2::ZERO || prev_pos == Vec2::ZERO {
                continue;
            }

            // Map to screen coordinates (buffer index = x, position.x = y)
            let x = left + (buffer_idx as f32 / data.max_samples as f32) * width;
            let y = bottom + ((pos.x - min_x) / position_range) * height;
            let prev_x = left + (prev_buffer_idx as f32 / data.max_samples as f32) * width;
            let prev_y = bottom + ((prev_pos.x - min_x) / position_range) * height;

            gizmos.line_2d(Vec2::new(prev_x, prev_y), Vec2::new(x, y), color);
        }
    }
}

fn draw_speed_graph(
    gizmos: &mut Gizmos,
    data: &ItemGraphData,
    left: f32,
    right: f32,
    bottom: f32,
    top: f32,
) {
    if data.speeds.is_empty() {
        return;
    }

    // Find max speed for scaling
    let mut max_speed = 0.0f32;
    for speeds in data.speeds.values() {
        for &speed in speeds {
            max_speed = max_speed.max(speed);
        }
    }
    max_speed = max_speed.max(1.0);  // Avoid division by zero

    let width = right - left;
    let height = top - bottom;

    // Draw each item's speed over time
    let colors = [
        Color::srgb(0.4, 0.8, 0.4),
        Color::srgb(0.8, 0.4, 0.4),
        Color::srgb(0.4, 0.4, 0.8),
        Color::srgb(0.8, 0.8, 0.4),
        Color::srgb(0.8, 0.4, 0.8),
        Color::srgb(0.4, 0.8, 0.8),
    ];

    for (idx, speeds) in data.speeds.values().enumerate() {
        let color = colors[idx % colors.len()];

        // Draw at fixed buffer positions so graph stays stationary
        for buffer_idx in 1..data.max_samples {
            let prev_buffer_idx = buffer_idx - 1;

            let speed = speeds[buffer_idx];
            let prev_speed = speeds[prev_buffer_idx];

            // Map to screen coordinates (buffer index = x, speed = y)
            let x = left + (buffer_idx as f32 / data.max_samples as f32) * width;
            let y = bottom + (speed / max_speed) * height;
            let prev_x = left + (prev_buffer_idx as f32 / data.max_samples as f32) * width;
            let prev_y = bottom + (prev_speed / max_speed) * height;

            gizmos.line_2d(Vec2::new(prev_x, prev_y), Vec2::new(x, y), color);
        }
    }
}

fn draw_wrap_indicator(
    gizmos: &mut Gizmos,
    data: &ItemGraphData,
    left: f32,
    right: f32,
    bottom: f32,
    top: f32,
) {
    // Draw vertical line at current position (wrap indicator)
    let wrap_x = left + (data.current_index as f32 / data.max_samples as f32) * (right - left);
    gizmos.line_2d(
        Vec2::new(wrap_x, bottom),
        Vec2::new(wrap_x, top),
        Color::srgb(1.0, 0.0, 0.0),
    );
}

fn setup_graph_labels(mut commands: Commands) {
    // Graph dimensions (must match render_debug_graphs)
    let graph_width = 400.0;
    let graph_height = 100.0;
    let spacing = 20.0;
    let right_edge = 600.0;
    let top_edge = 400.0;
    let graph_left = right_edge - graph_width;
    let position_graph_top = top_edge;
    let position_graph_bottom = position_graph_top - graph_height;
    let speed_graph_top = position_graph_bottom - spacing;

    // Position graph title
    commands.spawn((
        Text2d::new("Item X Position Over Time"),
        TextFont {
            font_size: 16.0,
            ..default()
        },
        TextColor(Color::srgb(0.9, 0.9, 0.9)),
        Transform::from_translation(Vec3::new(
            graph_left + 5.0,
            position_graph_top - 15.0,
            100.0,
        )),
        PositionGraphTitle,
    ));

    // Position graph scale (will be updated)
    commands.spawn((
        Text2d::new(""),
        TextFont {
            font_size: 12.0,
            ..default()
        },
        TextColor(Color::srgb(0.7, 0.7, 0.7)),
        Transform::from_translation(Vec3::new(
            graph_left + 5.0,
            position_graph_bottom + 5.0,
            100.0,
        )),
        PositionGraphScale,
    ));

    // Speed graph title
    commands.spawn((
        Text2d::new("Item Speeds"),
        TextFont {
            font_size: 16.0,
            ..default()
        },
        TextColor(Color::srgb(0.9, 0.9, 0.9)),
        Transform::from_translation(Vec3::new(
            graph_left + 5.0,
            speed_graph_top - 15.0,
            100.0,
        )),
        SpeedGraphTitle,
    ));

    // Speed graph scale (will be updated)
    commands.spawn((
        Text2d::new(""),
        TextFont {
            font_size: 12.0,
            ..default()
        },
        TextColor(Color::srgb(0.7, 0.7, 0.7)),
        Transform::from_translation(Vec3::new(
            graph_left + 5.0,
            speed_graph_top - graph_height + 5.0,
            100.0,
        )),
        SpeedGraphScale,
    ));
}

fn update_graph_labels(
    graph_data: Res<ItemGraphData>,
    windows: Query<&Window>,
    camera: Query<(&Camera, &GlobalTransform)>,
    mut position_title: Query<(&mut Text2d, &mut Transform), (With<PositionGraphTitle>, Without<PositionGraphScale>, Without<SpeedGraphTitle>, Without<SpeedGraphScale>)>,
    mut position_scale: Query<(&mut Text2d, &mut Transform), (With<PositionGraphScale>, Without<PositionGraphTitle>, Without<SpeedGraphTitle>, Without<SpeedGraphScale>)>,
    mut speed_title: Query<(&mut Text2d, &mut Transform), (With<SpeedGraphTitle>, Without<PositionGraphTitle>, Without<PositionGraphScale>, Without<SpeedGraphScale>)>,
    mut speed_scale: Query<(&mut Text2d, &mut Transform), (With<SpeedGraphScale>, Without<PositionGraphTitle>, Without<PositionGraphScale>, Without<SpeedGraphTitle>)>,
) {
    // Get viewport bounds for positioning
    let Ok(window) = windows.single() else {
        return;
    };
    let Ok((cam, cam_transform)) = camera.single() else {
        return;
    };

    let window_width = window.resolution.width();
    let window_height = window.resolution.height();

    let viewport_top_right = cam
        .viewport_to_world_2d(cam_transform, Vec2::new(window_width, 0.0))
        .unwrap_or(Vec2::new(window_width / 2.0, window_height / 2.0));

    let viewport_top_left = cam
        .viewport_to_world_2d(cam_transform, Vec2::new(0.0, 0.0))
        .unwrap_or(Vec2::new(-window_width / 2.0, window_height / 2.0));

    // Graph dimensions (must match render_debug_graphs)
    let graph_width = 400.0;
    let graph_height = 100.0;
    let margin = 20.0;
    let spacing = 20.0;

    let right_edge = viewport_top_right.x - margin;
    let top_edge = viewport_top_right.y - margin;
    let mut graph_left = right_edge - graph_width;
    graph_left = graph_left.max(viewport_top_left.x + margin);

    let position_graph_top = top_edge;
    let position_graph_bottom = position_graph_top - graph_height;
    let speed_graph_top = position_graph_bottom - spacing;

    // Update position graph title position
    if let Ok((_, mut transform)) = position_title.single_mut() {
        transform.translation = Vec3::new(graph_left + 5.0, position_graph_top - 15.0, 100.0);
    }

    // Update position graph scale text and position
    if let Ok((mut text, mut transform)) = position_scale.single_mut() {
        transform.translation = Vec3::new(graph_left + 5.0, position_graph_bottom + 5.0, 100.0);

        if !graph_data.positions.is_empty() {
            let (mut min_x, mut max_x) = (f32::INFINITY, f32::NEG_INFINITY);

            for positions in graph_data.positions.values() {
                for pos in positions {
                    if *pos == Vec2::ZERO {
                        continue;
                    }
                    min_x = min_x.min(pos.x);
                    max_x = max_x.max(pos.x);
                }
            }

            **text = format!("Range: [{:.0}, {:.0}]", min_x, max_x);
        } else {
            **text = "No items".to_string();
        }
    }

    // Update speed graph title position
    if let Ok((_, mut transform)) = speed_title.single_mut() {
        transform.translation = Vec3::new(graph_left + 5.0, speed_graph_top - 15.0, 100.0);
    }

    // Update speed graph scale text and position
    if let Ok((mut text, mut transform)) = speed_scale.single_mut() {
        transform.translation = Vec3::new(graph_left + 5.0, speed_graph_top - graph_height + 5.0, 100.0);

        if !graph_data.speeds.is_empty() {
            let mut max_speed = 0.0f32;
            for speeds in graph_data.speeds.values() {
                for &speed in speeds {
                    max_speed = max_speed.max(speed);
                }
            }
            **text = format!("Max: {:.1} units/sec", max_speed);
        } else {
            **text = "No items".to_string();
        }
    }
}
