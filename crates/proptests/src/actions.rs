use bevy::prelude::*;
use factory_core::*;
use proptest::prelude::*;
use std::collections::{HashMap, HashSet};

/// Actions that can be performed during property testing
#[derive(Debug, Clone)]
pub enum Action {
    PlaceBelt {
        coords: WorldCoords,
        dir: HDir,
    },
    PlaceItem {
        belt_coords: WorldCoords,
        pos: i32,
        lane: Side,
    },
    Update,
}

/// Test state for tracking belt/item positions across frames
#[derive(Default)]
pub struct TestState {
    pub belt_coords: HashSet<WorldCoords>,
    pub belt_entities: HashMap<WorldCoords, Entity>,
    pub previous_item_positions: HashMap<Entity, Vec3>,
    pub frame_count: usize,
}

impl TestState {
    pub fn place_belt(&mut self, coords: WorldCoords, entity: Entity) {
        self.belt_coords.insert(coords);
        self.belt_entities.insert(coords, entity);
    }

    pub fn get_belt(&self, coords: WorldCoords) -> Option<Entity> {
        self.belt_entities.get(&coords).copied()
    }

    pub fn capture_item_positions(&mut self, app: &mut App) {
        self.previous_item_positions.clear();

        let mut query = app
            .world_mut()
            .query_filtered::<(Entity, &Transform), With<Item>>();
        for (entity, transform) in query.iter(app.world()) {
            self.previous_item_positions
                .insert(entity, transform.translation);
        }
    }

    pub fn next_frame(&mut self) {
        self.frame_count += 1;
    }
}

// Proptest strategies

pub fn arb_coords() -> impl Strategy<Value = WorldCoords> {
    (-3..=3i32, -3..=3i32, 0..=0i32).prop_map(|(x, y, z)| WorldCoords::from((x, y, z)))
}

pub fn arb_dir() -> impl Strategy<Value = HDir> {
    prop_oneof![
        Just(HDir::North),
        Just(HDir::East),
        Just(HDir::South),
        Just(HDir::West),
    ]
}

pub fn arb_lane() -> impl Strategy<Value = Side> {
    prop_oneof![Just(Side::Left), Just(Side::Right),]
}

pub fn arb_belt_position() -> impl Strategy<Value = i32> {
    0i32..POSITIONS_PER_BELT
}

pub fn arb_action() -> impl Strategy<Value = Action> {
    prop_oneof![
        5 => Just(Action::Update),
        3 => (arb_coords(), arb_dir()).prop_map(|(coords, dir)| Action::PlaceBelt { coords, dir }),
        2 => (arb_coords(), arb_belt_position(), arb_lane()).prop_map(|(belt_coords, pos, lane)| Action::PlaceItem { belt_coords, pos, lane }),
    ]
}

fn is_valid_action_sequence(actions: &[Action]) -> bool {
    let mut newly_placed_belts = HashSet::new();

    for action in actions {
        match action {
            Action::PlaceBelt { coords, .. } => {
                newly_placed_belts.insert(*coords);
            }
            Action::PlaceItem { belt_coords, .. } => {
                if newly_placed_belts.contains(belt_coords) {
                    return false;
                }
            }
            Action::Update => {
                newly_placed_belts.clear();
            }
        }
    }

    true
}

pub fn arb_action_sequence() -> impl Strategy<Value = Vec<Action>> {
    proptest::collection::vec(arb_action(), 0..100).prop_filter(
        "Cannot place item on belt without Update between PlaceBelt and PlaceItem",
        |actions| is_valid_action_sequence(actions),
    )
}
