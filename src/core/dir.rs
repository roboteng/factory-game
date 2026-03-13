use super::{BLOCK_SIZE, HALF_BLOCK_SIZE, Side};
use bevy::prelude::*;
use std::f32::consts::PI;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WorldCoords {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

/// Horizontal direction
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum HDir {
    North,
    South,
    East,
    West,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dir {
    Up,
    Down,
    North,
    South,
    East,
    West,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorldCoordsDelta {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BeltOutput {
    Up(HDir),
    Level(HDir),
    Down(HDir),
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum BeltShape {
    Straight(HDir),
    Curve(Curve),
    RampUp(HDir),
    RampDown(HDir),
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

// -----------
// Model impls
// -----------

impl WorldCoords {
    pub fn step(&self, dir: impl Into<WorldCoordsDelta>) -> Self {
        let d: WorldCoordsDelta = dir.into();
        Self {
            x: self.x + d.x,
            y: self.y + d.y,
            z: self.z + d.z,
        }
    }
}

impl std::ops::Add<WorldCoordsDelta> for WorldCoords {
    type Output = Self;
    fn add(self, rhs: WorldCoordsDelta) -> Self {
        Self {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
            z: self.z + rhs.z,
        }
    }
}

impl From<Dir> for WorldCoordsDelta {
    fn from(dir: Dir) -> Self {
        match dir {
            Dir::Up => Self { x: 0, y: 1, z: 0 },
            Dir::Down => Self { x: 0, y: -1, z: 0 },
            Dir::North => Self { x: 1, y: 0, z: 0 },
            Dir::South => Self { x: -1, y: 0, z: 0 },
            Dir::East => Self { x: 0, y: 0, z: 1 },
            Dir::West => Self { x: 0, y: 0, z: -1 },
        }
    }
}

impl From<HDir> for WorldCoordsDelta {
    fn from(dir: HDir) -> Self {
        Dir::from(dir).into()
    }
}

impl From<HDir> for Dir {
    fn from(dir: HDir) -> Self {
        match dir {
            HDir::North => Dir::North,
            HDir::South => Dir::South,
            HDir::East => Dir::East,
            HDir::West => Dir::West,
        }
    }
}

impl From<BeltOutput> for WorldCoordsDelta {
    fn from(bo: BeltOutput) -> Self {
        let (hdir, dy) = match bo {
            BeltOutput::Up(d) => (d, 1),
            BeltOutput::Level(d) => (d, 0),
            BeltOutput::Down(d) => (d, -1),
        };
        let mut delta = WorldCoordsDelta::from(Dir::from(hdir));
        delta.y = dy;
        delta
    }
}

impl HDir {
    pub const fn angle(&self) -> f32 {
        match self {
            Self::North => 0.0,
            Self::East => -PI / 2.0,
            Self::South => PI,
            Self::West => PI / 2.0,
        }
    }

    pub const fn opposite(&self) -> Self {
        match self {
            Self::North => Self::South,
            Self::East => Self::West,
            Self::South => Self::North,
            Self::West => Self::East,
        }
    }

    pub const fn left(&self) -> Self {
        match self {
            Self::North => Self::West,
            Self::East => Self::North,
            Self::South => Self::East,
            Self::West => Self::South,
        }
    }

    pub const fn right(&self) -> Self {
        match self {
            Self::North => Self::East,
            Self::East => Self::South,
            Self::South => Self::West,
            Self::West => Self::North,
        }
    }
}

impl BeltShape {
    pub fn belt_output(&self) -> BeltOutput {
        match self {
            Self::Straight(dir) => BeltOutput::Level(*dir),
            Self::Curve(curve) => BeltOutput::Level(curve.output()),
            Self::RampUp(dir) => BeltOutput::Up(*dir),
            Self::RampDown(dir) => BeltOutput::Down(*dir),
        }
    }

    pub const fn output(&self) -> HDir {
        match self {
            Self::Straight(dir) | Self::RampUp(dir) | Self::RampDown(dir) => *dir,
            Self::Curve(curve) => curve.output(),
        }
    }
    pub const fn input(&self) -> HDir {
        match self {
            Self::Straight(dir) | Self::RampUp(dir) | Self::RampDown(dir) => *dir,
            Self::Curve(curve) => curve.input(),
        }
    }

    pub const fn num_pos(&self, side: Side) -> i32 {
        match side {
            Side::Left => self.left_num_pos(),
            Side::Right => self.right_num_pos(),
        }
    }

    pub const fn left_num_pos(&self) -> i32 {
        match self {
            Self::Straight(_) | Self::RampUp(_) | Self::RampDown(_) => super::POSITIONS_PER_BELT,
            Self::Curve(curve) => {
                if curve.is_clockwise() {
                    super::POSITIONS_PER_OUTER_CURVE
                } else {
                    super::POSITIONS_PER_INNER_CURVE
                }
            }
        }
    }
    pub const fn right_num_pos(&self) -> i32 {
        match self {
            Self::Straight(_) | Self::RampUp(_) | Self::RampDown(_) => super::POSITIONS_PER_BELT,
            Self::Curve(curve) => {
                if curve.is_clockwise() {
                    super::POSITIONS_PER_INNER_CURVE
                } else {
                    super::POSITIONS_PER_OUTER_CURVE
                }
            }
        }
    }
}

impl Curve {
    pub const fn from_input_output(input: HDir, output: HDir) -> Option<Self> {
        use HDir::*;
        match (input, output) {
            (North, East) => Some(Self::NorthToEast),
            (North, West) => Some(Self::NorthToWest),
            (South, East) => Some(Self::SouthToEast),
            (South, West) => Some(Self::SouthToWest),
            (East, North) => Some(Self::EastToNorth),
            (East, South) => Some(Self::EastToSouth),
            (West, North) => Some(Self::WestToNorth),
            (West, South) => Some(Self::WestToSouth),
            _ => None,
        }
    }
    pub const fn input(&self) -> HDir {
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

    pub const fn output(&self) -> HDir {
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

    pub const fn is_clockwise(&self) -> bool {
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

    pub const fn inner_lane(&self) -> Side {
        if self.is_clockwise() {
            Side::Right
        } else {
            Side::Left
        }
    }
    #[expect(unused)]
    pub const fn outer_lane(&self) -> Side {
        if self.is_clockwise() {
            Side::Left
        } else {
            Side::Right
        }
    }
}

// -----------
// Trait impls
// -----------

impl From<WorldCoords> for Vec3 {
    fn from(coords: WorldCoords) -> Self {
        Vec3::new(
            coords.x as f32 * BLOCK_SIZE,
            coords.y as f32 * HALF_BLOCK_SIZE,
            coords.z as f32 * BLOCK_SIZE,
        )
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

impl From<HDir> for BeltOutput {
    fn from(value: HDir) -> Self {
        Self::Level(value)
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

#[cfg(test)]
mod tests {
    use super::*;
    #[allow(unused_imports)]
    use pretty_assertions::{assert_eq, assert_ne};

    #[test]
    fn dir_to_world_coords_delta() {
        assert_eq!(WorldCoordsDelta::from(Dir::Up), WorldCoordsDelta { x: 0, y: 1, z: 0 });
        assert_eq!(WorldCoordsDelta::from(Dir::Down), WorldCoordsDelta { x: 0, y: -1, z: 0 });
        assert_eq!(WorldCoordsDelta::from(Dir::North), WorldCoordsDelta { x: 1, y: 0, z: 0 });
        assert_eq!(WorldCoordsDelta::from(Dir::South), WorldCoordsDelta { x: -1, y: 0, z: 0 });
        assert_eq!(WorldCoordsDelta::from(Dir::East), WorldCoordsDelta { x: 0, y: 0, z: 1 });
        assert_eq!(WorldCoordsDelta::from(Dir::West), WorldCoordsDelta { x: 0, y: 0, z: -1 });
    }

    #[test]
    fn hdir_works_with_step() {
        let origin = WorldCoords { x: 0, y: 0, z: 0 };
        assert_eq!(origin.step(HDir::North), WorldCoords { x: 1, y: 0, z: 0 });
        assert_eq!(origin.step(HDir::South), WorldCoords { x: -1, y: 0, z: 0 });
        assert_eq!(origin.step(HDir::East), WorldCoords { x: 0, y: 0, z: 1 });
        assert_eq!(origin.step(HDir::West), WorldCoords { x: 0, y: 0, z: -1 });
    }

    #[test]
    fn belt_output_works_with_step() {
        let origin = WorldCoords { x: 0, y: 0, z: 0 };
        assert_eq!(origin.step(BeltOutput::Up(HDir::North)), WorldCoords { x: 1, y: 1, z: 0 });
        assert_eq!(origin.step(BeltOutput::Level(HDir::East)), WorldCoords { x: 0, y: 0, z: 1 });
        assert_eq!(origin.step(BeltOutput::Down(HDir::South)), WorldCoords { x: -1, y: -1, z: 0 });
    }

    #[test]
    fn dir_up_down_step() {
        let coords = WorldCoords { x: 3, y: 5, z: 7 };
        assert_eq!(coords.step(Dir::Up), WorldCoords { x: 3, y: 6, z: 7 });
        assert_eq!(coords.step(Dir::Down), WorldCoords { x: 3, y: 4, z: 7 });
    }

    #[test]
    fn add_world_coords_delta() {
        let coords = WorldCoords { x: 1, y: 2, z: 3 };
        let delta = WorldCoordsDelta { x: -1, y: 1, z: -1 };
        assert_eq!(coords + delta, WorldCoords { x: 0, y: 3, z: 2 });
    }
}
