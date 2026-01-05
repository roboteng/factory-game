use bevy::prelude::*;
use std::{f32::consts::PI, ops::Range};

const TILE_SIZE: f32 = 1.0;
const BELT_HEIGHT: f32 = 0.25;

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
    let angle = match event.dir {
        HorizontalDir::North => 0.0,
        HorizontalDir::East => -PI / 2.0,
        HorizontalDir::South => PI,
        HorizontalDir::West => PI / 2.0,
    };
    cmd.entity(event.entity).insert(
        Transform::from_translation(Vec3::from(event.coords))
            .with_rotation(Quat::from_rotation_y(angle)),
    );
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
fn init_tracing() {
    use tracing_subscriber::{EnvFilter, fmt};
    let _ = fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("warn,factory_game=debug")),
        )
        .with_target(false)
        .with_test_writer()
        .without_time()
        .try_init();
}

#[cfg(test)]
pub fn test_app() -> App {
    init_tracing();
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app
}

#[cfg(test)]
mod tests {
    use std::f32::consts::PI;

    use super::*;
    #[allow(unused_imports)]
    use pretty_assertions::{assert_eq, assert_ne};

    #[test]
    fn north_betl_placement() {
        let mut app = test_app();

        let entity = app.world_mut().spawn_empty().id();
        app.world_mut().trigger(PlaceBelt {
            entity,
            coords: (0, 0, 0).into(),
            dir: HorizontalDir::North,
        });
        app.update();

        let world = app.world_mut();
        let &actual = world.query::<&Transform>().get(world, entity).unwrap();
        let expected = Transform::from_translation(Vec3::new(0.0, 0.0, 0.0) * TILE_SIZE);
        assert_eq!(actual, expected);
    }

    #[test]
    fn east_betl_placement() {
        let mut app = test_app();

        let entity = app.world_mut().spawn_empty().id();
        app.world_mut().trigger(PlaceBelt {
            entity,
            coords: (0, 0, 0).into(),
            dir: HorizontalDir::East,
        });
        app.update();

        let world = app.world_mut();
        let &actual = world.query::<&Transform>().get(world, entity).unwrap();
        let expected = Transform::from_translation(Vec3::new(0.0, 0.0, 0.0) * TILE_SIZE)
            .with_rotation(Quat::from_rotation_y(-PI / 2.0));
        assert_eq!(actual, expected);
    }
}
