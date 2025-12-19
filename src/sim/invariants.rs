use crate::core::*;
use crate::sim::{BeltConnection, BeltFragment, BeltInventory};
use bevy::prelude::*;

pub struct InvariantsPlugin;

impl Plugin for InvariantsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PostUpdate,
            (
                check_no_belt_and_fragment,
                check_belt_connections_valid,
                check_items_in_inventories,
                check_belt_inventory_has_belt_or_fragment,
                check_belts_have_coords,
                check_belt_coords_sync,
                check_no_duplicate_coords,
                check_inventory_items_exist,
                check_inventory_sorted,
                check_item_positions_in_bounds,
                check_item_spacing,
                check_inventory_items_have_components,
                check_fragment_coordinates,
            )
                .chain(),
        );
    }
}

/// Invariant 1: No entity should have both Belt and BeltFragment
fn check_no_belt_and_fragment(query: Query<Entity, (With<Belt>, With<BeltFragment>)>) {
    for entity in query.iter() {
        panic!(
            "INVARIANT VIOLATION: Entity {:?} has both Belt and BeltFragment components",
            entity
        );
    }
}

/// Invariant 2: All BeltConnection references must point to valid entities with Belt or BeltFragment
fn check_belt_connections_valid(
    connections: Query<(Entity, &BeltConnection)>,
    belts: Query<(), Or<(With<Belt>, With<BeltFragment>)>>,
) {
    for (entity, connection) in connections.iter() {
        if belts.get(connection.next_belt).is_err() {
            panic!(
                "INVARIANT VIOLATION: Entity {:?} has BeltConnection pointing to invalid entity {:?}",
                entity, connection.next_belt
            );
        }
    }
}

/// Invariant 3: Every Item must be referenced in exactly one BeltInventory
fn check_items_in_inventories(
    items: Query<Entity, With<Item>>,
    inventories: Query<&BeltInventory>,
) {
    for item_entity in items.iter() {
        let mut found_count = 0;
        for inventory in inventories.iter() {
            if inventory.item.iter().any(|(_, e)| *e == item_entity) {
                found_count += 1;
            }
        }

        if found_count == 0 {
            panic!(
                "INVARIANT VIOLATION: Item {:?} is not in any BeltInventory",
                item_entity
            );
        }
        if found_count > 1 {
            panic!(
                "INVARIANT VIOLATION: Item {:?} is in {} BeltInventories (expected exactly 1)",
                item_entity, found_count
            );
        }
    }
}

/// Invariant 4: Entities with BeltInventory must have exactly one of Belt or BeltFragment
fn check_belt_inventory_has_belt_or_fragment(
    inventories: Query<Entity, With<BeltInventory>>,
    belts: Query<(), With<Belt>>,
    fragments: Query<(), With<BeltFragment>>,
) {
    for entity in inventories.iter() {
        let has_belt = belts.get(entity).is_ok();
        let has_fragment = fragments.get(entity).is_ok();

        if !has_belt && !has_fragment {
            panic!(
                "INVARIANT VIOLATION: Entity {:?} has BeltInventory but neither Belt nor BeltFragment",
                entity
            );
        }
        if has_belt && has_fragment {
            panic!(
                "INVARIANT VIOLATION: Entity {:?} has BeltInventory with both Belt and BeltFragment",
                entity
            );
        }
    }
}

/// Invariant 5: All Belt entities must have WorldCoords
fn check_belts_have_coords(query: Query<Entity, (With<Belt>, Without<WorldCoords>)>) {
    for entity in query.iter() {
        panic!(
            "INVARIANT VIOLATION: Belt entity {:?} does not have WorldCoords component",
            entity
        );
    }
}

/// Invariant 6: BeltCoords resource must match Belt+WorldCoords entities
fn check_belt_coords_sync(
    belt_coords: Res<BeltCoords>,
    belts: Query<(Entity, &Belt, &WorldCoords)>,
) {
    for (entity, belt, coords) in belts.iter() {
        match belt_coords.get(*coords) {
            Some((res_entity, res_belt)) => {
                if res_entity != entity {
                    panic!(
                        "INVARIANT VIOLATION: BeltCoords at {:?} points to entity {:?} but entity {:?} exists there",
                        coords, res_entity, entity
                    );
                }
                if res_belt != *belt {
                    panic!(
                        "INVARIANT VIOLATION: BeltCoords at {:?} has belt type {:?} but entity {:?} has {:?}",
                        coords, res_belt, entity, belt
                    );
                }
            }
            None => {
                panic!(
                    "INVARIANT VIOLATION: Belt entity {:?} at {:?} is not in BeltCoords resource",
                    entity, coords
                );
            }
        }
    }
}

/// Invariant 7: No two belts should occupy the same WorldCoords
fn check_no_duplicate_coords(belts: Query<(Entity, &WorldCoords), With<Belt>>) {
    let mut coords_map = std::collections::HashMap::new();

    for (entity, coords) in belts.iter() {
        if let Some(existing_entity) = coords_map.insert(*coords, entity) {
            panic!(
                "INVARIANT VIOLATION: Multiple belts at {:?}: entities {:?} and {:?}",
                coords, existing_entity, entity
            );
        }
    }
}

/// Invariant 8: All items in BeltInventory must reference valid entities
fn check_inventory_items_exist(
    inventories: Query<(Entity, &BeltInventory)>,
    items: Query<(), With<Item>>,
) {
    for (belt_entity, inventory) in inventories.iter() {
        for (pos, item_entity) in &inventory.item {
            if items.get(*item_entity).is_err() {
                panic!(
                    "INVARIANT VIOLATION: BeltInventory on {:?} references non-existent item {:?} at position {}",
                    belt_entity, item_entity, pos
                );
            }
        }
    }
}

/// Invariant 9: BeltInventory items must be sorted by position (ascending)
fn check_inventory_sorted(inventories: Query<(Entity, &BeltInventory)>) {
    for (entity, inventory) in inventories.iter() {
        let positions: Vec<u16> = inventory.item.iter().map(|(pos, _)| *pos).collect();

        for i in 1..positions.len() {
            if positions[i] < positions[i - 1] {
                panic!(
                    "INVARIANT VIOLATION: BeltInventory on {:?} is not sorted: position {} ({}) comes before position {} ({})",
                    entity,
                    i - 1,
                    positions[i - 1],
                    i,
                    positions[i]
                );
            }
        }
    }
}

/// Invariant 10: Item positions must be within valid range for their belt
fn check_item_positions_in_bounds(
    inventories: Query<(Entity, &BeltInventory, Option<&Belt>, Option<&BeltFragment>)>,
) {
    for (entity, inventory, belt, fragment) in inventories.iter() {
        let max_pos = if let Some(b) = belt {
            b.num_positions()
        } else if fragment.is_some() {
            POSITIONS_PER_TILE / 2
        } else {
            // This should be caught by check_belt_inventory_has_belt_or_fragment
            continue;
        };

        for (pos, item_entity) in &inventory.item {
            if *pos >= max_pos {
                panic!(
                    "INVARIANT VIOLATION: Item {:?} on belt/fragment {:?} has position {} >= max {}",
                    item_entity, entity, pos, max_pos
                );
            }
        }
    }
}

/// Invariant 11: Items should maintain minimum spacing (ITEM_SPACING)
/// Note: This is a soft invariant - the system is designed to recover from overcompression
fn check_item_spacing(inventories: Query<(Entity, &BeltInventory)>) {
    for (entity, inventory) in inventories.iter() {
        let items: Vec<(u16, Entity)> = inventory.item.iter().copied().collect();

        for i in 0..items.len() {
            let expected_min_pos = i as u16 * ITEM_SPACING;
            let (actual_pos, item_entity) = items[i];

            if actual_pos < expected_min_pos {
                warn!(
                    "INVARIANT WARNING: Item {:?} on belt {:?} at index {} has position {} < expected minimum {} (overcompressed, will self-correct)",
                    item_entity, entity, i, actual_pos, expected_min_pos
                );
            }
        }
    }
}

/// Invariant 12: Items referenced in inventories must have Item and Transform components
fn check_inventory_items_have_components(
    inventories: Query<(Entity, &BeltInventory)>,
    items_with_transform: Query<(), (With<Item>, With<Transform>)>,
) {
    for (belt_entity, inventory) in inventories.iter() {
        for (pos, item_entity) in &inventory.item {
            if items_with_transform.get(*item_entity).is_err() {
                panic!(
                    "INVARIANT VIOLATION: Item {:?} in inventory of {:?} at position {} does not have Item+Transform components",
                    item_entity, belt_entity, pos
                );
            }
        }
    }
}

/// Invariant 13: Fragment coordinates must match their target belt coordinates
fn check_fragment_coordinates(
    fragments: Query<(Entity, &WorldCoords, &BeltConnection), With<BeltFragment>>,
    belts: Query<&WorldCoords, With<Belt>>,
) {
    for (fragment_entity, fragment_coords, connection) in fragments.iter() {
        if let Ok(belt_coords) = belts.get(connection.next_belt) {
            if fragment_coords != belt_coords {
                panic!(
                    "INVARIANT VIOLATION: Fragment {:?} at {:?} connects to belt {:?} at {:?} (coordinates should match)",
                    fragment_entity, fragment_coords, connection.next_belt, belt_coords
                );
            }
        }
        // Note: If belt doesn't exist, check_belt_connections_valid will catch it
    }
}
