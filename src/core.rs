use bevy::prelude::*;
use std::ops::Range;

const TILE_SIZE: f32 = 1.0;

pub struct CorePlugin;
impl Plugin for CorePlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(on_place_belt);
    }
}

#[derive(EntityEvent)]
pub struct PlaceBelt {
    pub entity: Entity,
    pub coords: WorldCoords,
    pub dir: HorizontalDir,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorldCoords {
    x: i32,
    y: i32,
    z: i32,
}

pub enum HorizontalDir {
    North,
    South,
    East,
    West,
}

#[derive(Component)]
pub struct BeltLane {
    pub belts: Belts,
    pub left_items: Vec<(i32, Item)>,
    pub right_items: Vec<(i32, Item)>,
}

pub struct Belts {
    belts: Vec<BeltShape>,
    coords: Vec<WorldCoords>,
    left_range: Vec<Range<i32>>,
    right_range: Vec<Range<i32>>,
}

pub enum BeltShape {
    Straight(HorizontalDir),
    CurvedNorthToEast,
    CurvedEastToSouth,
    CurvedSouthToWest,
    CurvedWestToNorth,
    CurvedNorthToWest,
    CurvedWestToSouth,
    CurvedSouthToEast,
    CurvedEastToNorth,
}

/// Item ID
pub struct Item(u32);

pub struct BeltConnection {
    pub left: LaneConnection,
    pub right: LaneConnection,
}

#[derive(Debug, Component)]
pub struct LaneConnection {
    pub target: Entity,
    pub offset: i32,
}

fn on_place_belt(event: On<PlaceBelt>, mut cmd: Commands) {
    cmd.entity(event.entity)
        .insert(Transform::from_translation(Vec3::from(event.coords)));
}

impl From<WorldCoords> for Vec3 {
    fn from(coords: WorldCoords) -> Self {
        Vec3::new(coords.x as f32, coords.y as f32, coords.z as f32) * TILE_SIZE
    }
}

impl From<(i32, i32, i32)> for WorldCoords {
    fn from(coords: (i32, i32, i32)) -> Self {
        WorldCoords {
            x: coords.0,
            y: coords.1,
            z: coords.2,
        }
    }
}

#[cfg(test)]
pub fn test_app() -> App {
    use bevy::log::{
        LogPlugin,
        tracing_subscriber::{Layer, fmt::layer},
    };

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(LogPlugin {
        filter: "warn,factory_game=debug".to_string(),
        fmt_layer: |_| {
            Some(
                layer()
                    .with_target(false)
                    .with_test_writer()
                    .without_time()
                    .boxed(),
            )
        },
        ..default()
    });
    app.add_plugins(CorePlugin);
    app
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn foobar() {
        let mut app = test_app();
        app.update();
        info!("log message");
    }
}
