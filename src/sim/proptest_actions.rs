use crate::core::*;
use crate::sim::{BeltLane, InLane};
use bevy::prelude::*;
use proptest::prelude::*;
use std::collections::{HashMap, HashSet};

/// Actions that can be performed during property testing
#[derive(Debug, Clone)]
pub enum Action {
    PlaceBelt { coords: WorldCoords, dir: Dir },
    PlaceItem { belt_coords: WorldCoords, pos: u16 },
    Update,
}

/// Test state for tracking belt/item positions across frames
pub struct TestState {
    /// Set of coordinates where belts exist
    pub belt_coords: HashSet<WorldCoords>,
    /// Map from coordinates to belt entities
    pub belt_entities: HashMap<WorldCoords, Entity>,
    /// Previous frame's item positions (for movement bound checking)
    pub previous_item_positions: HashMap<Entity, Vec3>,
    /// Items that should skip movement bounds check this frame (on replaced belts)
    pub skip_movement_check: HashSet<Entity>,
    /// Current frame number
    pub frame_count: usize,
}

impl TestState {
    pub fn new() -> Self {
        Self {
            belt_coords: HashSet::new(),
            belt_entities: HashMap::new(),
            previous_item_positions: HashMap::new(),
            skip_movement_check: HashSet::new(),
            frame_count: 0,
        }
    }

    /// Update state after placing a belt
    pub fn place_belt(&mut self, coords: WorldCoords, entity: Entity) {
        self.belt_coords.insert(coords);
        self.belt_entities.insert(coords, entity);
    }

    /// Check if a belt exists at the given coordinates
    pub fn has_belt(&self, coords: WorldCoords) -> bool {
        self.belt_coords.contains(&coords)
    }

    /// Get the belt entity at the given coordinates
    pub fn get_belt(&self, coords: WorldCoords) -> Option<Entity> {
        self.belt_entities.get(&coords).copied()
    }

    /// Capture current item positions before an update
    pub fn capture_item_positions(&mut self, app: &mut App) {
        self.previous_item_positions.clear();

        let mut query = app.world_mut().query_filtered::<(Entity, &Transform), With<Item>>();
        for (entity, transform) in query.iter(app.world()) {
            self.previous_item_positions.insert(entity, transform.translation);
        }

        // Capture items that will skip movement bounds check (on replaced belts)
        // This must happen BEFORE app.update() clears BeltChanges
        self.skip_movement_check = self.get_items_on_replaced_belts(app);
    }

    /// Check that items haven't moved more than the maximum distance
    /// Returns an error message if the movement bound is violated
    pub fn check_movement_bounds(&self, app: &mut App) -> Result<(), String> {
        const MAX_MOVEMENT: f32 = BASE_ITEM_MOVEMENT * 1.5; // pixels per frame (with safety margin)

        let mut query = app.world_mut().query_filtered::<(Entity, &Transform), With<Item>>();
        for (entity, transform) in query.iter(app.world()) {
            // Skip items that were on replaced belts (they teleport during replacement)
            // This was captured in capture_item_positions() before app.update()
            if self.skip_movement_check.contains(&entity) {
                continue;
            }

            if let Some(prev_pos) = self.previous_item_positions.get(&entity) {
                let distance = prev_pos.distance(transform.translation);
                if distance > MAX_MOVEMENT {
                    return Err(format!(
                        "Item {:?} moved {} pixels (> {} max) from {:?} to {:?}",
                        entity, distance, MAX_MOVEMENT, prev_pos, transform.translation
                    ));
                }
            }
        }

        Ok(())
    }

    /// Increment frame counter
    pub fn next_frame(&mut self) {
        self.frame_count += 1;
        // Clear skip set for next frame
        self.skip_movement_check.clear();
    }

    /// Get items that were on replaced belts this frame (should skip movement bounds check)
    ///
    /// Works by:
    /// 1. Reading BeltChanges resource to find all Replaced events
    /// 2. For each replacement, querying the OLD entity's lane to find items
    /// 3. Collecting all item entities that will be transferred
    fn get_items_on_replaced_belts(&self, app: &mut App) -> HashSet<Entity> {
        // Clone the BeltChanges vector to avoid holding a reference
        let changes = app.world().resource::<BeltChanges>().0.clone();
        let mut skip_items = HashSet::new();

        // Find all belt replacements in this frame
        for change in &changes {
            if let BeltChange::Replaced(replaced) = change {
                if let Some(old_entity) = replaced.old_entity {
                    // Items are stored in the lane, not the belt entity
                    // Query the old belt's InLane component to find its lane
                    let mut in_lane_query = app.world_mut().query::<&InLane>();
                    if let Ok(in_lane) = in_lane_query.get(app.world(), old_entity) {
                        let lane_entity = in_lane.lane;

                        // Query the lane to get all items
                        let mut lane_query = app.world_mut().query::<&BeltLane>();
                        if let Ok(lane) = lane_query.get(app.world(), lane_entity) {
                            for (_, item_entity) in &lane.items.items {
                                skip_items.insert(*item_entity);
                            }
                        }
                    }
                }
            }
        }
        skip_items
    }
}

// Proptest strategies

/// Generate random WorldCoords in the range -10..=10 for both x and y
pub fn arb_coords() -> impl Strategy<Value = WorldCoords> {
    (-10..=10i32, -10..=10i32).prop_map(|(x, y)| WorldCoords::new(x, y))
}

/// Generate random Dir
pub fn arb_dir() -> impl Strategy<Value = Dir> {
    prop_oneof![
        Just(Dir::North),
        Just(Dir::East),
        Just(Dir::South),
        Just(Dir::West),
    ]
}

/// Generate random belt position (0..256)
pub fn arb_belt_position() -> impl Strategy<Value = u16> {
    0u16..POSITIONS_PER_TILE
}

/// Generate a random Action with weighted distribution:
/// - 50% Update
/// - 30% PlaceBelt
/// - 20% PlaceItem
pub fn arb_action() -> impl Strategy<Value = Action> {
    prop_oneof![
        5 => Just(Action::Update),
        3 => (arb_coords(), arb_dir()).prop_map(|(coords, dir)| Action::PlaceBelt { coords, dir }),
        2 => (arb_coords(), arb_belt_position()).prop_map(|(belt_coords, pos)| Action::PlaceItem { belt_coords, pos }),
    ]
}

/// Generate a variable-length sequence of actions (0 to 100 actions)
pub fn arb_action_sequence() -> impl Strategy<Value = Vec<Action>> {
    proptest::collection::vec(arb_action(), 0..100)
}
