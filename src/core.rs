use bevy::prelude::*;

pub struct CorePlugin;
impl Plugin for CorePlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(on_place_belt);
    }
}

#[derive(EntityEvent)]
pub struct PlaceBelt {
    entity: Entity,
    dir: Direction,
    coords: WorldCoords,
}

#[derive(EntityEvent, Clone, Debug, PartialEq, Eq)]
pub struct BeltPlaced {
    entity: Entity,
    belt: Belt,
    coords: WorldCoords,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Direction {
    North,
    East,
    South,
    West,
}

#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub struct WorldCoords {
    x: i32,
    y: i32,
}

impl WorldCoords {
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

impl From<(i32, i32)> for WorldCoords {
    fn from((x, y): (i32, i32)) -> Self {
        Self { x, y }
    }
}

#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub enum Belt {
    Straight(Direction),
    CurvedNorthToEast,
    CurvedNorthToWest,
    CurvedEastToSouth,
    CurvedEastToNorth,
    CurvedSouthToWest,
    CurvedSouthToEast,
    CurvedWestToNorth,
    CurvedWestToSouth,
}

fn on_place_belt(trigger: On<PlaceBelt>, mut cmd: Commands) {
    println!("Placing belt at {:?}", trigger.coords);
    cmd.entity(trigger.entity)
        .insert((Belt::Straight(Direction::East), WorldCoords::new(0, 0)));
}

#[cfg(test)]
mod tests {
    use super::*;
    #[allow(unused_imports)]
    use pretty_assertions::{assert_eq, assert_ne, assert_str_eq};

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(CorePlugin);
        app
    }

    #[test]
    fn place_single_belt() {
        let mut app = test_app();
        let entity = app.world_mut().spawn_empty().id();
        app.world_mut().trigger(PlaceBelt {
            entity,
            dir: Direction::East,
            coords: (0, 0).into(),
        });

        app.update();
        let world = app.world_mut();
        let actual = world
            .query::<(&Belt, &WorldCoords)>()
            .get(world, entity)
            .unwrap();
        let expected = (&Belt::Straight(Direction::East), &(0, 0).into());
        assert_eq!(actual, expected);
    }
}
