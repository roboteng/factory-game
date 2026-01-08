pub use crate::core::lane::*;
use bevy::{math::ops::sin_cos, prelude::*};
use std::{f32::consts::PI, ops::Range};

mod lane;

pub const BLOCK_SIZE: f32 = 2.0;
pub const HALF_BLOCK_SIZE: f32 = BLOCK_SIZE / 2.0;
pub const ITEM_SIZE: f32 = BLOCK_SIZE / 4.0;
pub const HALF_ITEM_SIZE: f32 = ITEM_SIZE / 2.0;
/// How far from the bottom of the voxel the belt surface is.
pub const BELT_HEIGHT: f32 = 0.25 * BLOCK_SIZE;
pub const BELT_HEIGHT_FROM_CENTER: f32 = -HALF_BLOCK_SIZE + BELT_HEIGHT;
/// Amount of a unit voxel of how far a lane is offset from center.
pub const LANE_OFFSET_FACTOR: f32 = 0.25;
/// How far from center each lane is.
pub const LANE_OFFSET: f32 = LANE_OFFSET_FACTOR * BLOCK_SIZE;

pub const POSITIONS_PER_BELT: i32 = 256;
pub const ITEM_SPACING: i32 = POSITIONS_PER_BELT / 4;
pub const POSITIONS_PER_INNER_CURVE: i32 =
    ((0.5 - LANE_OFFSET_FACTOR) * POSITIONS_PER_BELT as f32 * PI / 2.0).round() as i32;
pub const POSITIONS_PER_OUTER_CURVE: i32 =
    ((0.5 + LANE_OFFSET_FACTOR) * POSITIONS_PER_BELT as f32 * PI / 2.0).round() as i32;

pub struct CorePlugin;
impl Plugin for CorePlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(on_place_belt);
        app.add_observer(on_place_item);

        app.add_systems(Update, replace_items);
    }
}

// ------
// Models
// ------

#[derive(EntityEvent)]
pub struct PlaceBelt {
    pub entity: Entity,
    pub coords: WorldCoords,
    pub dir: HDir,
}

#[derive(EntityEvent)]
pub struct PlaceItem {
    pub entity: Entity,
    pub item: Item,
    pub belt: Entity,
    pub lane: LaneSide,
    pub position: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorldCoords {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

/// Horizon direction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HDir {
    North,
    South,
    East,
    West,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BeltShape {
    Straight(HDir),
    Curve(Curve),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Curve {
    NorthToEast,
    EastToSouth,
    SouthToWest,
    WestToNorth,
    NorthToWest,
    WestToSouth,
    SouthToEast,
    EastToNorth,
}

/// Item ID
#[derive(Component, Debug, PartialEq, Eq, Clone, Copy)]
pub struct Item(pub u32);

#[derive(Debug, Component)]
pub struct LaneConnection {
    pub target: Entity,
    pub offset: i32,
}

#[derive(Component)]
pub struct InLane {
    pub lane: Entity,
}

// -------
// Systems
// -------

fn on_place_belt(event: On<PlaceBelt>, mut cmd: Commands) {
    let angle = event.dir.angle();

    let lane_ent = cmd
        .spawn(BeltLane::from_belt(
            BeltShape::Straight(event.dir),
            event.coords,
            event.entity,
        ))
        .id();
    cmd.entity(event.entity).insert((
        Transform::from_translation(Vec3::from(event.coords))
            .with_rotation(Quat::from_rotation_y(angle)),
        InLane { lane: lane_ent },
    ));
}

fn on_place_item(
    event: On<PlaceItem>,
    belts: Query<&InLane>,
    mut lanes: Query<&mut BeltLane>,
    mut cmd: Commands,
) {
    let lane_ent = belts.get(event.belt).unwrap().lane;
    let mut lane = lanes.get_mut(lane_ent).unwrap();
    lane.push_item(
        ItemEntry {
            pos: event.position,
            item: event.item,
            entity: event.entity,
        },
        event.lane,
    );
}

fn replace_items(lanes: Query<&BeltLane>, mut items: Query<(&mut Item, &mut Transform)>) {
    for ((item, pos, belt, lane, coords), mut b) in Iterator::zip(
        lanes.iter().map(|l| l.item_iter()).flatten(),
        items.iter_mut(),
    ) {
        let transform = item_position(belt, coords, lane, pos);
        *b.0 = item;
        *b.1 = transform;
    }
}

// -----------
// Model impls
// -----------

impl HDir {
    pub fn angle(&self) -> f32 {
        match self {
            Self::North => 0.0,
            Self::East => -PI / 2.0,
            Self::South => PI,
            Self::West => PI / 2.0,
        }
    }

    pub fn opposite(&self) -> Self {
        match self {
            Self::North => Self::South,
            Self::East => Self::West,
            Self::South => Self::North,
            Self::West => Self::East,
        }
    }
}

impl BeltShape {
    pub fn output(&self) -> HDir {
        match self {
            Self::Straight(dir) => *dir,
            Self::Curve(curve) => curve.output(),
        }
    }
    pub fn input(&self) -> HDir {
        match self {
            Self::Straight(dir) => *dir,
            Self::Curve(curve) => curve.input(),
        }
    }

    pub fn left_num_pos(&self) -> i32 {
        match self {
            Self::Straight(_) => POSITIONS_PER_BELT,
            Self::Curve(_) => todo!(),
        }
    }
    pub fn right_num_pos(&self) -> i32 {
        match self {
            Self::Straight(_) => POSITIONS_PER_BELT,
            Self::Curve(_) => todo!(),
        }
    }
}

impl Curve {
    pub fn input(&self) -> HDir {
        match self {
            Self::NorthToEast => HDir::North,
            Self::EastToSouth => HDir::East,
            Self::SouthToWest => HDir::South,
            Self::WestToNorth => HDir::West,
            Self::NorthToWest => HDir::North,
            Self::EastToNorth => HDir::East,
            Self::SouthToEast => HDir::South,
            Self::WestToSouth => HDir::West,
        }
    }

    pub fn output(&self) -> HDir {
        match self {
            Self::NorthToEast => HDir::East,
            Self::EastToSouth => HDir::South,
            Self::SouthToWest => HDir::West,
            Self::WestToNorth => HDir::North,
            Self::NorthToWest => HDir::West,
            Self::EastToNorth => HDir::North,
            Self::SouthToEast => HDir::East,
            Self::WestToSouth => HDir::South,
        }
    }

    pub fn is_clockwise(&self) -> bool {
        match self {
            Self::NorthToEast => true,
            Self::EastToSouth => true,
            Self::SouthToWest => true,
            Self::WestToNorth => true,
            Self::NorthToWest => false,
            Self::EastToNorth => false,
            Self::SouthToEast => false,
            Self::WestToSouth => false,
        }
    }

    pub fn inner_lane(&self) -> LaneSide {
        if self.is_clockwise() {
            LaneSide::Right
        } else {
            LaneSide::Left
        }
    }
    pub fn outet_lane(&self) -> LaneSide {
        if self.is_clockwise() {
            LaneSide::Left
        } else {
            LaneSide::Right
        }
    }
}

// -----------
// Trait impls
// -----------

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

impl From<HDir> for Vec3 {
    fn from(value: HDir) -> Vec3 {
        match value {
            HDir::North => Vec3::X,
            HDir::South => Vec3::NEG_X,
            HDir::East => Vec3::Z,
            HDir::West => Vec3::NEG_Z,
        }
    }
}

impl From<HDir> for Vec2 {
    fn from(value: HDir) -> Self {
        Vec3::from(value).zx()
    }
}

// --------
// Functions
// ---------

pub fn item_position(
    belt: BeltShape,
    coords: impl Into<WorldCoords>,
    lane: LaneSide,
    pos: i32,
) -> Transform {
    match belt {
        BeltShape::Straight(dir) => {
            let z = match lane {
                LaneSide::Left => -LANE_OFFSET,
                LaneSide::Right => LANE_OFFSET,
            };
            let start = Vec3::new(HALF_BLOCK_SIZE, BELT_HEIGHT_FROM_CENTER, z);
            let end = Vec3::new(-HALF_BLOCK_SIZE, BELT_HEIGHT_FROM_CENTER, z);

            let t = (pos + ITEM_SPACING / 2) as f32 / POSITIONS_PER_BELT as f32;
            let angle = dir.angle();
            Transform::from_translation(
                start.lerp(end, t).rotate_y(angle) + Vec3::from(coords.into()),
            )
        }
        BeltShape::Curve(curve) => {
            let center_offset =
                (Vec2::from(belt.input().opposite()) + Vec2::from(belt.output())) / 2.0;
            let n_pos = if curve.inner_lane() == lane {
                POSITIONS_PER_INNER_CURVE
            } else {
                POSITIONS_PER_OUTER_CURVE
            };
            let lane_offset = if curve.inner_lane() == lane {
                0.5 - LANE_OFFSET_FACTOR
            } else {
                0.5 + LANE_OFFSET_FACTOR
            };
            let angle_offset = (pos + ITEM_SPACING / 2) as f32 / n_pos as f32 * PI / 2.0;
            let angle_base = curve.input().angle();
            // Positions move the opposite way of items, so this is backwards
            let angle = if curve.is_clockwise() {
                angle_base + angle_offset
            } else {
                angle_base - angle_offset
            };
            debug!(
                "angle: {}*pi, angle_offset: {}*pi, angle_base: {}*pi",
                angle / PI,
                angle_offset / PI,
                angle_base / PI
            );
            let local_offset = center_offset
                + lane_offset * {
                    let (sin, cos) = sin_cos(angle);
                    Vec2 { x: -sin, y: cos }
                };
            debug!(
                "center_offset: {center_offset:?}, lane_offset: {lane_offset}, local_offset: {:?}, ",
                local_offset
            );
            Transform::from_translation(
                Vec3::new(
                    local_offset.y * BLOCK_SIZE,
                    BELT_HEIGHT_FROM_CENTER,
                    local_offset.x * BLOCK_SIZE,
                ) + Vec3::from(coords.into()),
            )
            .with_rotation(Quat::from_rotation_y(angle + PI / 2.0))
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
pub fn init_tracing() {
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
            dir: HDir::North,
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
            dir: HDir::East,
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
            BeltShape::Straight(HDir::North),
            (0, 0, 0),
            LaneSide::Left,
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
            BeltShape::Straight(HDir::North),
            (0, 0, 0),
            LaneSide::Left,
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
            BeltShape::Straight(HDir::East),
            (0, 0, 0),
            LaneSide::Left,
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

    #[test]
    fn item_positioning_start_right() {
        let actual = item_position(
            BeltShape::Straight(HDir::North),
            (0, 0, 0),
            LaneSide::Right,
            0,
        );
        let expected = Transform::from_translation(Vec3::new(
            HALF_BLOCK_SIZE - HALF_ITEM_SIZE,
            -HALF_BLOCK_SIZE + BELT_HEIGHT,
            LANE_OFFSET,
        ));
        assert_eq!(actual, expected);
    }

    #[test]
    fn item_positioning_start_coords() {
        let actual = item_position(
            BeltShape::Straight(HDir::North),
            (1, 1, 1),
            LaneSide::Left,
            0,
        );
        let expected = Transform::from_translation(Vec3::new(
            HALF_BLOCK_SIZE - HALF_ITEM_SIZE + BLOCK_SIZE,
            -HALF_BLOCK_SIZE + BELT_HEIGHT + BLOCK_SIZE,
            -LANE_OFFSET + BLOCK_SIZE,
        ));
        assert_eq!(actual, expected);
    }

    #[test]
    fn item_positioning_front_curved() {
        init_tracing();
        let actual = item_position(
            BeltShape::Curve(Curve::EastToNorth),
            (0, 0, 0),
            LaneSide::Left,
            -ITEM_SPACING / 2,
        );
        let expected = Transform::from_translation(Vec3::new(
            HALF_BLOCK_SIZE,
            -HALF_BLOCK_SIZE + BELT_HEIGHT,
            -LANE_OFFSET,
        ));
        assert_close(actual.translation, expected.translation);
        assert_eq!(actual.rotation, expected.rotation);
    }

    #[test]
    fn item_positioning_end_curved() {
        init_tracing();
        let actual = item_position(
            BeltShape::Curve(Curve::EastToNorth),
            (0, 0, 0),
            LaneSide::Left,
            POSITIONS_PER_INNER_CURVE - ITEM_SPACING / 2,
        );
        let expected = Transform::from_translation(Vec3::new(
            LANE_OFFSET,
            -HALF_BLOCK_SIZE + BELT_HEIGHT,
            -HALF_BLOCK_SIZE,
        ))
        .with_rotation(Quat::from_axis_angle(Vec3::Y, -PI / 2.0));
        assert_close(actual.translation, expected.translation);
        assert_eq!(actual.rotation, expected.rotation);
    }

    #[test]
    fn item_on_belt() {
        let mut app = test_app();
        let entity = app.world_mut().spawn_empty().id();
        app.world_mut().trigger(PlaceBelt {
            entity,
            coords: (0, 0, 0).into(),
            dir: HDir::North,
        });
        app.update();

        let world = app.world_mut();
        let item_ent = world.spawn_empty().id();
        world.trigger(PlaceItem {
            entity: item_ent,
            item: Item(0),
            belt: entity,
            lane: LaneSide::Left,
            position: 0,
        });
        let actual = world.query::<&BeltLane>().single(world).unwrap();
        let expected = BeltLane {
            belts: vec![BeltEntry {
                belt: BeltShape::Straight(HDir::North),
                coords: (0, 0, 0).into(),
                entity,
                left_range: 0..POSITIONS_PER_BELT,
                right_range: 0..POSITIONS_PER_BELT,
            }],
            left_items: vec![ItemEntry {
                pos: 0,
                item: Item(0),
                entity: item_ent,
            }],
            right_items: vec![],
        };
        assert_eq!(*actual, expected);
    }
}
