use crate::core::*;
use bevy::prelude::*;

pub struct SimPlugin;
impl Plugin for SimPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PlannedMoves>();
        app.add_observer(on_place_item);
        app.add_observer(on_place_belt);
        app.add_systems(Update, (plan_moves, execute_moves, move_items).chain());
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

    pub fn item_at_head(&self) -> Option<(u16, Entity)> {
        self.item.first().copied()
    }

    pub fn has_space_at_tail(&self, belt: Belt) -> bool {
        self.item
            .last()
            .map_or(true, |&(pos, _)| pos > belt.num_positions() - 64)
    }

    pub fn remove_first(&mut self) {
        self.item.remove(0);
    }
}

fn on_place_item(trigger: On<PlaceItem>, mut belts: Query<&mut BeltInventory, With<Belt>>) {
    let mut inv = belts.get_mut(trigger.belt).unwrap();
    inv.add(trigger.pos, trigger.entity);
}

fn on_place_belt(trigger: On<PlaceBelt>, mut cmd: Commands) {
    cmd.entity(trigger.entity).insert(BeltInventory::default());
}

#[derive(Resource, Default, Clone)]
struct PlannedMoves(Vec<PlannedMove>);

impl PlannedMoves {
    fn push(&mut self, planned_move: PlannedMove) {
        self.0.push(planned_move);
    }
    fn clear(&mut self) {
        self.0.clear();
    }
}
#[derive(Clone)]
struct PlannedMove {
    from: Entity,
    to: Entity,
    new_pos: u16,
    item: Entity,
}

fn plan_moves(
    invs: Query<(Entity, &BeltInventory, &Belt, &WorldCoords)>,
    belt_coords: Res<BeltCoords>,
    mut planned_moves: ResMut<PlannedMoves>,
) {
    planned_moves.clear();
    for (ent, inv, belt, coords) in invs.iter() {
        let Some(item) = inv.item_at_head() else {
            continue;
        };
        if item.0 >= 8 {
            continue;
        }
        let Some(next) = belt_coords.get(coords.step(belt.output())) else {
            continue;
        };
        let next_belt = invs.get(next.0).unwrap();
        if !next_belt.1.has_space_at_tail(*next_belt.2) {
            continue;
        }
        planned_moves.push(PlannedMove {
            from: ent,
            to: next_belt.0,
            new_pos: next_belt.2.num_positions() + item.0,
            item: item.1,
        });
    }
}

fn execute_moves(world: &mut World) {
    let moves = world.resource::<PlannedMoves>().clone();
    for m in moves.0.iter() {
        let mut query = world.query::<&mut BeltInventory>();
        let mut inv = query.get_mut(world, m.from).unwrap();
        inv.remove_first();
        let mut inv = query.get_mut(world, m.to).unwrap();
        inv.add(m.new_pos, m.item);
    }
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

    #[test]
    fn item_moves_onto_next_belt() {
        let mut app = test_app();
        let belt1 = app.add_belt((0, 0), Dir::East);
        let belt2 = app.add_belt((1, 0), Dir::East);
        app.update();
        let item = app.add_item(belt1, 0);
        app.update();
        let (_, actual) = app.find_item(item).unwrap();
        let expected = Transform::from_xyz(TILE_SIZE / 2.0 + 1.0, 0.0, 2.0);
        assert_eq!(actual, expected);
    }
}
