use crate::core::*;
use bevy::prelude::*;

pub struct SimPlugin;
impl Plugin for SimPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PlannedMoves>();
        app.add_observer(on_place_item);
        app.add_observer(on_place_belt);
        app.add_systems(
            Update,
            (
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

#[derive(Component, Default, Clone)]
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
}

fn on_place_item(trigger: On<PlaceItem>, mut belts: Query<&mut BeltInventory, With<Belt>>) {
    let mut inv = belts.get_mut(trigger.belt).unwrap();
    inv.add(trigger.pos, trigger.entity);
    inv.sort();
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

#[derive(Component, Clone)]
struct BeltConnection {
    next_belt: Entity,
    num_positions: u16,
}

fn calculate_belt_connections(
    mut cmd: Commands,
    belt_coords: Res<BeltCoords>,
    changed_belts: Res<BeltChanges>,
    query: Query<(Entity, Option<&BeltConnection>, &BeltInventory)>,
) {
    // TODO: Double connection gets made when two belts are placed next to each other in the same frame
    debug!("Updating belts: {:?}", changed_belts.0);
    for change in &changed_belts.0 {
        match change {
            BeltChange::New(new) => {
                place_belt(cmd.reborrow(), &belt_coords, *new);
            }
            BeltChange::Removed(removed) => {
                remove_belt(cmd.reborrow(), *removed, &belt_coords, &query);
            }
            BeltChange::Replaced(replaced) => {
                let _items = remove_belt(
                    cmd.reborrow(),
                    RemovedBelt::from(*replaced),
                    &belt_coords,
                    &query,
                );
                place_belt(cmd.reborrow(), &belt_coords, NewBelt::from(*replaced));
            }
        }
    }
}

fn place_belt(
    mut cmd: Commands,
    belt_coords: &BeltCoords,
    NewBelt {
        entity,
        belt,
        coords,
    }: NewBelt,
) -> Option<()> {
    let ahead_coords = coords.step(belt.output());
    if let Some(tile) = belt_coords.get(ahead_coords) {
        if tile.1.input() == belt.output() {
            cmd.entity(entity).insert(BeltConnection {
                next_belt: tile.0,
                num_positions: tile.1.num_positions(),
            });
            debug!(
                "Making new connection ahead from {:?} into {:?}",
                entity, tile.0
            );
        } else if tile.1.input().left() == belt.output() || tile.1.input().right() == belt.output()
        {
            sideload_into(cmd.reborrow(), tile.0, ahead_coords, tile.1, entity, belt);
        }
    }
    let input = belt.input();
    for dir in [input.opposite(), input.left(), input.right()] {
        let stepped_coords = coords.step(dir);
        let Some((other_entity, other_belt)) = belt_coords.get(stepped_coords) else {
            continue;
        };
        if stepped_coords.step(other_belt.output()) == coords {
            if other_belt.output() == input {
                cmd.entity(other_entity).insert(BeltConnection {
                    next_belt: entity,
                    num_positions: belt.num_positions(),
                });
                debug!(
                    "Making new connection from {:?} into {:?}",
                    other_entity, entity
                );
            } else {
                sideload_into(
                    cmd.reborrow(),
                    entity,
                    coords,
                    belt,
                    other_entity,
                    other_belt,
                );
            }
        }
    }
    Some(())
}

fn sideload_into(
    mut cmd: Commands,
    main_entity: Entity,
    main_coords: WorldCoords,
    _main_belt: Belt,
    side_entity: Entity,
    side_belt: Belt,
) {
    let frag = BeltFragment {
        dir: side_belt.output(),
    };
    let frag_entity = cmd
        .spawn((
            frag,
            BeltInventory::default(),
            main_coords,
            BeltConnection {
                next_belt: main_entity,
                num_positions: 128,
            },
        ))
        .id();
    cmd.entity(side_entity).insert(BeltConnection {
        next_belt: frag_entity,
        num_positions: frag.num_positions(),
    });
    debug!(
        "Making new fragment {:?} connecting {:?} into {:?}",
        frag_entity, side_entity, main_entity
    );
}

fn remove_belt(
    mut cmd: Commands,
    RemovedBelt {
        entity,
        old_belt,
        coords,
    }: RemovedBelt,
    belt_coords: &BeltCoords,
    query: &Query<(Entity, Option<&BeltConnection>, &BeltInventory)>,
) -> Option<BeltInventory> {
    let input = old_belt.input();
    for dir in [input.opposite(), input.left(), input.right()] {
        let stepped_coords = coords.step(dir);
        let Some((c_entity, _)) = belt_coords.get(stepped_coords) else {
            continue;
        };
        let Ok((entity, Some(connection), _)) = query.get(c_entity) else {
            continue;
        };
        // TODO: check where its a belt or blet fragment
        if connection.next_belt == entity {
            cmd.entity(c_entity).remove::<BeltConnection>();
        }
    }
    let (_, _, inventory) = query.get(entity).ok()?;
    Some(inventory.clone())
}

#[derive(Component, Clone, Copy)]
struct BeltFragment {
    dir: Dir,
}

impl BeltFragment {
    fn input(&self) -> Dir {
        self.dir
    }
    fn num_positions(&self) -> u16 {
        POSITIONS_PER_TILE as u16 / 2
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

impl From<(Option<&'_ Belt>, Option<&'_ BeltFragment>)> for BeltLike {
    fn from((belt, fragment): (Option<&'_ Belt>, Option<&'_ BeltFragment>)) -> Self {
        match (belt, fragment) {
            (None, None) => panic!("Both belt and fragment are None"),
            (Some(_), Some(_)) => panic!("Both belt and fragment are Some(...)"),
            (Some(belt), _) => BeltLike::Belt(belt.clone()),
            (_, Some(fragment)) => BeltLike::Fragment(fragment.clone()),
        }
    }
}

fn plan_moves(
    invs: Query<(Entity, &BeltInventory, Option<&BeltConnection>)>,

    mut planned_moves: ResMut<PlannedMoves>,
) {
    planned_moves.clear();
    for (ent, inv, conn) in invs.iter() {
        let Some(item) = inv.item_at_head() else {
            continue;
        };
        debug!("Checking belt {:?} with item at position {}", ent, item.0);
        if item.0 >= 8 {
            debug!("  Item too far back (pos >= 8)");
            continue;
        }
        let Some(connection) = conn else {
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
    mut belts: Query<(
        &mut BeltInventory,
        AnyOf<(&Belt, &BeltFragment)>,
        &WorldCoords,
    )>,
) {
    for (mut inv, belt, coords) in belts.iter_mut() {
        let belt = BeltLike::from(belt);
        for (i, (pos, entity)) in &mut inv.item.iter_mut().enumerate() {
            debug!("Moving items on belt, items at {:?}", coords);
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
    fn item_moves_onto_next_belt_other_order() {
        let mut app = test_app();
        app.add_belt((1, 0), Dir::East);
        app.update();
        let belt1 = app.add_belt((0, 0), Dir::East);
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

    #[test]
    fn item_moves_towards_side_loading_belt_other_order() {
        let mut app = test_app();
        app.add_belt((1, 0), Dir::North);
        app.add_belt((1, -1), Dir::North);
        app.update();
        let belt1 = app.add_belt((0, 0), Dir::East);
        app.update();

        let item = app.add_item(belt1, 0);
        app.update();
        let (_, actual) = app.find_item(item).unwrap();
        let expected = Transform::from_xyz(TILE_SIZE / 2.0 + 1.0, 0.0, 2.0);
        assert_eq!(actual, expected);
    }

    #[test]
    fn item_moves_onto_side_loaded_belt() {
        let mut app = test_app();
        let belt1 = app.add_belt((0, 0), Dir::East);
        app.add_belt((1, 0), Dir::North);
        app.add_belt((1, -1), Dir::North);
        app.update();

        let item = app.add_item(belt1, 0);
        for _ in 0..(16 + 1) {
            app.update();
        }
        let (_, actual) = app.find_item(item).unwrap();
        let expected = Transform::from_xyz(TILE_SIZE, 1.0, 2.0);
        assert_eq!(actual, expected);
    }

    #[test]
    #[ignore = "todo"]
    fn item_moves_onto_side_loaded_belt_unless_full() {
        let mut app = test_app();
        let belt1 = app.add_belt((0, 0), Dir::East);
        let belt2 = app.add_belt((1, 0), Dir::North);
        app.add_belt((1, -1), Dir::North);
        app.update();

        let item = app.add_item(belt1, 0);
        app.add_item(belt2, 0);
        app.add_item(belt2, 64);
        for _ in 0..(8 + 1) {
            app.update();
        }
        let (_, actual) = app.find_item(item).unwrap();
        let expected = Transform::from_xyz(TILE_SIZE * 3.0 / 4.0, 0.0, 2.0);
        assert_eq!(actual, expected);
    }
}
