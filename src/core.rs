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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
    debug!(
        "Placing belt at {:?} facing {:?}",
        trigger.coords, trigger.dir
    );
    cmd.entity(trigger.entity)
        .insert((Belt::Straight(trigger.dir), WorldCoords::new(0, 0)));
}

#[cfg(test)]
mod tests {
    use super::*;
    #[allow(unused_imports)]
    use pretty_assertions::{assert_eq, assert_ne, assert_str_eq};
    use tracing_subscriber::{EnvFilter, fmt};

    fn init_tracing() {
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

    fn test_app() -> App {
        init_tracing();
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(CorePlugin);
        app
    }

    trait AppExtension {
        fn add_belt(&mut self, coords: impl Into<WorldCoords>, dir: Direction) -> Entity;
        fn find_belt(&mut self, entity: Entity) -> Option<(Belt, WorldCoords)>;
    }

    impl AppExtension for App {
        fn add_belt(&mut self, coords: impl Into<WorldCoords>, dir: Direction) -> Entity {
            let entity = self.world_mut().spawn_empty().id();
            self.world_mut().trigger(PlaceBelt {
                entity,
                dir,
                coords: coords.into(),
            });
            entity
        }
        fn find_belt(&mut self, entity: Entity) -> Option<(Belt, WorldCoords)> {
            self.world_mut()
                .query::<(&Belt, &WorldCoords)>()
                .get(self.world_mut(), entity)
                .map(|(belt, coords)| (belt.clone(), coords.clone()))
                .ok()
        }
    }

    #[test]
    fn place_single_belt_east() {
        let mut app = test_app();
        let entity = app.add_belt((0, 0), Direction::East);

        app.update();
        let actual = app.find_belt(entity).unwrap();
        let expected = (Belt::Straight(Direction::East), (0, 0).into());
        assert_eq!(actual, expected);
    }

    #[test]
    fn place_single_belt_north() {
        let mut app = test_app();
        let entity = app.add_belt((0, 0), Direction::North);

        app.update();
        let actual = app.find_belt(entity).unwrap();
        let expected = (Belt::Straight(Direction::North), (0, 0).into());
        assert_eq!(actual, expected);
    }
}
