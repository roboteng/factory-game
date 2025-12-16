use crate::core::*;
use bevy::prelude::*;

pub struct SimPlugin;
impl Plugin for SimPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(on_place_item);
        app.add_observer(on_place_belt);
        app.add_systems(Update, move_items);
    }
}

#[derive(Component, Default)]
pub struct BeltInventory {
    item: Vec<(u16, Entity)>,
}

impl BeltInventory {
    pub fn add(&mut self, pos: u16, entity: Entity) {
        self.item.push((pos, entity));
    }
}

fn on_place_item(trigger: On<PlaceItem>, mut belts: Query<&mut BeltInventory, With<Belt>>) {
    let mut inv = belts.get_mut(trigger.belt).unwrap();
    inv.add(trigger.pos, trigger.entity);
}

fn on_place_belt(trigger: On<PlaceBelt>, mut cmd: Commands) {
    cmd.entity(trigger.entity).insert(BeltInventory::default());
}

fn move_items(
    mut items: Query<&mut Transform, With<Item>>,
    mut belts: Query<(&mut BeltInventory, &Belt, &WorldCoords)>,
) {
    for (mut inv, belt, coords) in belts.iter_mut() {
        for (pos, entity) in &mut inv.item {
            *pos = pos.saturating_sub(8);
            let mut transform = items.get_mut(*entity).unwrap();
            *transform = belt.item_transform(*pos, *coords);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[allow(unused_imports)]
    use pretty_assertions::{assert_eq, assert_ne, assert_str_eq};

    fn test_app() -> App {
        let mut app = crate::core::test_app();
        app.add_plugins(SimPlugin);
        app
    }

    #[test]
    fn item_moves_on_belt() {
        let mut app = test_app();
        let belt = app.add_belt((0, 0), Dir::East);
        app.update();
        let item = app.add_item(belt, POSITIONS_PER_TILE / 2);
        app.update();
        let (_, actual) = app.find_item(item).unwrap();
        let expected = Transform::from_xyz(1.0, 0.0, 2.0);
        assert_eq!(actual, expected);
    }

    #[test]
    fn item_moves_on_belt_north() {
        let mut app = test_app();
        let belt = app.add_belt((0, 0), Dir::North);
        app.update();
        let item = app.add_item(belt, POSITIONS_PER_TILE / 2);
        app.update();
        let (_, actual) = app.find_item(item).unwrap();
        let expected = Transform::from_xyz(0.0, 1.0, 2.0);
        assert_eq!(actual, expected);
    }

    #[test]
    fn item_doesnt_move_on_belt_end() {
        let mut app = test_app();
        let belt = app.add_belt((0, 0), Dir::East);
        app.update();
        let item = app.add_item(belt, 0);
        app.update();
        let (_, actual) = app.find_item(item).unwrap();
        let expected = Transform::from_xyz(TILE_SIZE / 2.0, 0.0, 2.0);
        assert_eq!(actual, expected);
    }
}
