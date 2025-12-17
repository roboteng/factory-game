use crate::core::*;
use bevy::prelude::*;

pub struct SimPlugin;
impl Plugin for SimPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PlannedMoves>();
        app.init_resource::<BeltConnections>();
        app.add_observer(on_place_item);
        app.add_observer(on_place_belt);
        app.add_observer(on_remove_belt);
        app.add_systems(
            Update,
            (
                belt_placed,
                calculate_belt_connections,
                ApplyDeferred,
                plan_moves,
                execute_moves,
                move_items,
            )
                .chain(),
        );
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

    pub fn has_space_at_tail(&self, n_pos: u16) -> bool {
        self.item
            .last()
            .is_none_or(|&(pos, _)| pos < n_pos - ITEM_SPACING)
    }

    pub fn remove_first(&mut self) {
        self.item.remove(0);
    }

    pub fn sort(&mut self) {
        self.item.sort();
    }

    pub fn items(&self) -> &Vec<(u16, Entity)> {
        &self.item
    }
}

fn on_place_item(trigger: On<PlaceItem>, mut belts: Query<&mut BeltInventory, With<Belt>>) {
    let mut inv = belts.get_mut(trigger.belt).unwrap();
    inv.add(trigger.pos, trigger.entity);
    inv.sort();
}

fn on_place_belt(trigger: On<PlaceBelt>, mut cmd: Commands) {
    cmd.entity(trigger.entity).insert(BeltInventory::default());
}

fn belt_placed(mut cmd: Commands, belts: Query<(Entity, &Belt), Added<Belt>>) {
    for (entity, belt) in belts.iter() {
        cmd.entity(entity).insert(BeltLike::Belt(*belt));
    }
}

fn on_remove_belt(
    trigger: On<RemoveBelt>,
    mut cmd: Commands,
    belts: Query<Option<&BeltInventory>>,
) {
    // Despawn all items on the belt if it has an inventory
    if let Ok(Some(inventory)) = belts.get(trigger.entity) {
        debug!(
            "Despawning {} items from belt {:?}",
            inventory.items().len(),
            trigger.entity
        );
        for (_, item_entity) in inventory.items().iter() {
            cmd.entity(*item_entity).despawn();
        }
    }
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

#[derive(Resource, Default)]
struct BeltConnections {
    connections: std::collections::HashMap<Entity, BeltConnection>,
}

impl BeltConnections {
    fn get(&self, entity: Entity) -> Option<&BeltConnection> {
        self.connections.get(&entity)
    }
    fn insert(&mut self, entity: Entity, connection: BeltConnection) {
        self.connections.insert(entity, connection);
    }
    fn clear(&mut self) {
        self.connections.clear();
    }
}

struct BeltConnection {
    next_belt: Entity,
    num_positions: u16,
}

fn calculate_belt_connections(
    mut cmd: Commands,
    belts: Query<(Entity, &Belt, &WorldCoords)>,
    belt_coords: Res<BeltCoords>,
    mut connections: ResMut<BeltConnections>,
) {
    connections.clear();
    for (ent, belt, coords) in belts.iter() {
        let next_coords = coords.step(belt.output());
        let Some(next) = belt_coords.get(next_coords) else {
            continue;
        };
        if next.1.input() == belt.output() {
            connections.insert(
                ent,
                BeltConnection {
                    next_belt: next.0,
                    num_positions: next.1.num_positions(),
                },
            );
        } else {
            if Vec2::from(next.1.input()).dot(belt.output().into()) == 0.0 {
                debug!(
                    "Making sideloaded connection from {:?} to {:?}",
                    ent, next.0
                );
                let belt_fragment = BeltFragment { dir: belt.output() };
                let entity = cmd
                    .spawn((
                        BeltInventory::default(),
                        belt_fragment,
                        BeltLike::Fragment(belt_fragment),
                        coords.step(belt.output()),
                    ))
                    .id();
                connections.insert(
                    ent,
                    BeltConnection {
                        next_belt: entity,
                        num_positions: 128,
                    },
                );
                // Connect the fragment to the receiving belt
                connections.insert(
                    entity,
                    BeltConnection {
                        next_belt: next.0,
                        num_positions: next.1.num_positions(),
                    },
                );
            }
        }
    }
}

#[derive(Component, Clone, Copy)]
struct BeltFragment {
    dir: Dir,
}

impl BeltFragment {
    fn input(&self) -> Dir {
        self.dir
    }
    fn item_transform(&self, pos: u16, coords: WorldCoords) -> Transform {
        debug!("Transforming fragment at {:?} with pos {}", coords, pos);
        let world_offset = Vec2::from(coords);
        let start = Vec2::default();
        let end = Vec2::from(self.input().opposite()) * TILE_SIZE / 2.0;
        let t = pos as f32 / (POSITIONS_PER_TILE as f32 / 2.0);
        let mid = start.lerp(end, t);
        Item::transform(world_offset + mid)
    }
}

#[derive(Component)]
enum BeltLike {
    Belt(Belt),
    Fragment(BeltFragment),
}

impl BeltLike {
    fn item_transform(&self, pos: u16, coords: WorldCoords) -> Transform {
        match self {
            BeltLike::Belt(belt) => belt.item_transform(pos, coords),
            BeltLike::Fragment(fragment) => fragment.item_transform(pos, coords),
        }
    }
}

fn plan_moves(
    invs: Query<(Entity, &BeltInventory)>,
    connections: Res<BeltConnections>,
    mut planned_moves: ResMut<PlannedMoves>,
) {
    planned_moves.clear();
    for (ent, inv) in invs.iter() {
        let Some(item) = inv.item_at_head() else {
            continue;
        };
        debug!("Checking belt {:?} with item at position {}", ent, item.0);
        if item.0 >= 8 {
            debug!("  Item too far back (pos >= 8)");
            continue;
        }
        let Some(connection) = connections.get(ent) else {
            debug!("  No connection found");
            continue;
        };
        debug!("  Found connection to {:?}", connection.next_belt);
        let next_belt = invs.get(connection.next_belt).unwrap();

        if !next_belt.1.has_space_at_tail(connection.num_positions) {
            debug!("  Next belt has no space");
            continue;
        }
        debug!(
            "  Planning move from {:?} to {:?}, new_pos: {}",
            ent,
            connection.next_belt,
            connection.num_positions + item.0
        );
        planned_moves.push(PlannedMove {
            from: ent,
            to: connection.next_belt,
            new_pos: connection.num_positions + item.0,
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
    mut belts: Query<(&mut BeltInventory, &BeltLike, &WorldCoords)>,
) {
    for (mut inv, belt, coords) in belts.iter_mut() {
        debug!(
            "Moving items on belt with {} items at {:?}",
            inv.items().len(),
            coords
        );
        for (i, (pos, entity)) in &mut inv.item.iter_mut().enumerate() {
            let i = i as u16;
            if *pos < i * ITEM_SPACING {
                warn!("Items are overcompressed");
                continue;
            }
            let next_pos = (*pos - i * ITEM_SPACING).saturating_sub(8) + i * ITEM_SPACING;
            if next_pos < *pos {
                *pos = next_pos;
            }
            let mut transform = items.get_mut(*entity).unwrap();
            let new_transform = belt.item_transform(*pos, *coords);
            debug!(
                "  Item {:?} at pos {} -> {:?}",
                entity, pos, new_transform.translation
            );
            *transform = new_transform;
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
        app.add_belt((1, 0), Dir::East);
        app.update();
        let item = app.add_item(belt1, 0);
        app.update();
        let (_, actual) = app.find_item(item).unwrap();
        let expected = Transform::from_xyz(TILE_SIZE / 2.0 + 1.0, 0.0, 2.0);
        assert_eq!(actual, expected);
    }

    #[test]
    fn item_dont_get_too_close() {
        let mut app = test_app();
        let belt1 = app.add_belt((0, 0), Dir::East);
        app.update();
        app.add_item(belt1, 0);
        let item = app.add_item(belt1, ITEM_SPACING);
        app.update();
        let (_, actual) = app.find_item(item).unwrap();
        let expected = Transform::from_xyz(
            TILE_SIZE / 2.0 - ITEM_SPACING as f32 / POSITIONS_PER_TILE as f32 * TILE_SIZE,
            0.0,
            2.0,
        );
        assert_eq!(actual, expected);
    }

    #[test]
    fn item_moves_to_next_belt_with_item() {
        let mut app = test_app();
        let belt1 = app.add_belt((0, 0), Dir::East);
        let belt2 = app.add_belt((1, 0), Dir::East);
        app.update();
        app.add_item(belt2, 0);
        let item = app.add_item(belt1, 0);
        app.update();
        let (_, actual) = app.find_item(item).unwrap();
        let expected = Transform::from_xyz(TILE_SIZE / 2.0 + 1.0, 0.0, 2.0);
        assert_eq!(actual, expected);
    }

    #[test]
    fn handles_items_too_close_together() {
        let mut app = test_app();
        let belt1 = app.add_belt((0, 0), Dir::East);
        app.update();
        app.add_item(belt1, 0);
        let item = app.add_item(belt1, 1);
        app.update();
        let (_, actual) = app.find_item(item).unwrap();
        let expected = Transform::from_xyz(TILE_SIZE / 2.0 - 4.0 / TILE_SIZE, 0.0, 2.0);
        assert_eq!(actual, expected);
    }

    #[test]
    fn item_moves_towards_side_loading_belt() {
        let mut app = test_app();
        let belt1 = app.add_belt((0, 0), Dir::East);
        app.add_belt((1, 0), Dir::North);
        app.add_belt((1, -1), Dir::North);
        app.update();

        let item = app.add_item(belt1, 0);
        app.update();
        let (_, actual) = app.find_item(item).unwrap();
        let expected = Transform::from_xyz(TILE_SIZE / 2.0 + 1.0, 0.0, 2.0);
        assert_eq!(actual, expected);
    }
}
