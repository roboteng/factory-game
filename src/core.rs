use bevy::prelude::*;
use std::{f32::consts::PI, ops::Range};

pub const BLOCK_SIZE: f32 = 2.0;
pub const HALF_BLOCK_SIZE: f32 = BLOCK_SIZE / 2.0;
pub const ITEM_SIZE: f32 = BLOCK_SIZE / 4.0;
pub const HALF_ITEM_SIZE: f32 = ITEM_SIZE / 2.0;
/// How far from the bottom of the voxel the belt surface is.
pub const BELT_HEIGHT: f32 = 0.25 * BLOCK_SIZE;
pub const BELT_HEIGHT_FROM_CENTER: f32 = -HALF_BLOCK_SIZE + BELT_HEIGHT;
/// Ratio of a unit voxel of how far a lane is offset from center.
pub const LANE_OFFSET_FACTOR: f32 = 0.25;
/// How far from center each lane is.
pub const LANE_OFFSET: f32 = LANE_OFFSET_FACTOR * BLOCK_SIZE;

pub const POSITIONS_PER_BELT: i32 = 256;
pub const ITEM_SPACING: i32 = POSITIONS_PER_BELT / 4;

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
        Vec3::new(coords.x as f32, coords.y as f32, coords.z as f32) * BLOCK_SIZE
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

pub enum Lane {
    Left,
    Right,
}

impl HorizontalDir {
    pub fn angle(&self) -> f32 {
        match self {
            HorizontalDir::North => 0.0,
            HorizontalDir::East => -PI / 2.0,
            HorizontalDir::South => PI,
            HorizontalDir::West => PI / 2.0,
        }
    }
}

pub fn item_position(
    belt: BeltShape,
    coords: impl Into<WorldCoords>,
    lane: Lane,
    pos: i32,
) -> Transform {
    let start = Vec3::new(HALF_BLOCK_SIZE, BELT_HEIGHT_FROM_CENTER, -LANE_OFFSET);
    let end = Vec3::new(-HALF_BLOCK_SIZE, BELT_HEIGHT_FROM_CENTER, -LANE_OFFSET);

    let t = (pos + ITEM_SPACING / 2) as f32 / POSITIONS_PER_BELT as f32;
    let angle = match belt {
        BeltShape::Straight(dir) => dir.angle(),
        _ => todo!(),
    };
    Transform::from_translation(start.lerp(end, t).rotate_y(angle))
}

impl Into<Vec3> for HorizontalDir {
    fn into(self) -> Vec3 {
        match self {
            HorizontalDir::North => Vec3::X,
            HorizontalDir::South => Vec3::NEG_X,
            HorizontalDir::East => Vec3::Z,
            HorizontalDir::West => Vec3::NEG_Z,
        }
    }
}

fn assert_close(left: Vec3, right: Vec3) {
    let dist = left.distance(right);
    assert!(
        dist < 0.0001,
        "Left:\n\t{left:?}\nand Right:\n\t{right:?}\nare distance of {dist} away"
    );
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
        let expected = Transform::from_translation(Vec3::new(0.0, 0.0, 0.0) * BLOCK_SIZE);
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
        let expected = Transform::from_translation(Vec3::new(0.0, 0.0, 0.0) * BLOCK_SIZE)
            .with_rotation(Quat::from_rotation_y(-PI / 2.0));
        assert_eq!(actual, expected);
    }

    #[test]
    fn item_positioning_front_boundary() {
        let actual = item_position(
            BeltShape::Straight(HorizontalDir::North),
            (0, 0, 0),
            Lane::Left,
            -ITEM_SPACING / 2,
        );
        let expected = Transform::from_translation(Vec3::new(
            HALF_BLOCK_SIZE,
            -HALF_BLOCK_SIZE + BELT_HEIGHT,
            -LANE_OFFSET,
        ));
        assert_eq!(actual, expected);
    }

    #[test]
    fn item_positioning_start() {
        let actual = item_position(
            BeltShape::Straight(HorizontalDir::North),
            (0, 0, 0),
            Lane::Left,
            0,
        );
        let expected = Transform::from_translation(Vec3::new(
            HALF_BLOCK_SIZE - HALF_ITEM_SIZE,
            -HALF_BLOCK_SIZE + BELT_HEIGHT,
            -LANE_OFFSET,
        ));
        assert_eq!(actual, expected);
    }

    #[test]
    fn item_positioning_start_east() {
        let actual = item_position(
            BeltShape::Straight(HorizontalDir::East),
            (0, 0, 0),
            Lane::Left,
            0,
        );
        let expected = Transform::from_translation(Vec3::new(
            LANE_OFFSET,
            -HALF_BLOCK_SIZE + BELT_HEIGHT,
            HALF_BLOCK_SIZE - HALF_ITEM_SIZE,
        ));
        assert_close(actual.translation, expected.translation);
        assert_eq!(actual.rotation, expected.rotation);
        assert_eq!(actual.scale, expected.scale);
    }
}
