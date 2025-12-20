use crate::core::*;
use crate::sim::proptest_actions::*;
use crate::sim::SimPlugin;
use bevy::prelude::*;
use proptest::prelude::*;

/// Execute a sequence of actions and verify all invariants hold
fn execute_action_sequence(actions: Vec<Action>) {
    // Create test app with SimPlugin (includes invariants)
    let mut app = test_app_with_sim();
    let mut state = TestState::new();

    for (i, action) in actions.iter().enumerate() {
        match action {
            Action::PlaceBelt { coords, dir } => {
                // Place the belt
                let entity = app.add_belt(*coords, *dir);
                state.place_belt(*coords, entity);
            }
            Action::PlaceItem { belt_coords, pos } => {
                // Only place item if belt exists
                if let Some(belt_entity) = state.get_belt(*belt_coords) {
                    app.add_item(belt_entity, *pos);
                }
                // Otherwise skip - this tests robustness
            }
            Action::Update => {
                // Capture positions before update
                state.capture_item_positions(&mut app);

                // Run update
                app.update();

                // Check movement bounds
                if let Err(msg) = state.check_movement_bounds(&mut app) {
                    panic!(
                        "Movement bound violation after action {}/{}: {}",
                        i + 1,
                        actions.len(),
                        msg
                    );
                }

                state.next_frame();
            }
        }
    }

    // Final update to ensure invariants hold at the end
    state.capture_item_positions(&mut app);
    app.update();
    if let Err(msg) = state.check_movement_bounds(&mut app) {
        panic!("Movement bound violation at final update: {}", msg);
    }
}

/// Create a test app with both CorePlugin and SimPlugin
fn test_app_with_sim() -> App {
    let mut app = crate::core::test_app();
    app.add_plugins(SimPlugin);
    app
}

// Property tests

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Main property test: arbitrary sequences of actions should maintain all invariants
    #[test]
    fn arbitrary_action_sequences_maintain_invariants(
        actions in arb_action_sequence()
    ) {
        execute_action_sequence(actions);
    }
}

// Regression tests for specific scenarios

#[test]
fn empty_sequence() {
    execute_action_sequence(vec![]);
}

#[test]
fn updates_only_no_belts() {
    execute_action_sequence(vec![
        Action::Update,
        Action::Update,
        Action::Update,
        Action::Update,
        Action::Update,
    ]);
}

#[test]
fn single_belt_no_items() {
    execute_action_sequence(vec![
        Action::PlaceBelt {
            coords: WorldCoords::new(0, 0),
            dir: Dir::East,
        },
        Action::Update,
        Action::Update,
    ]);
}

#[test]
fn single_belt_with_item() {
    execute_action_sequence(vec![
        Action::PlaceBelt {
            coords: WorldCoords::new(0, 0),
            dir: Dir::East,
        },
        Action::PlaceItem {
            belt_coords: WorldCoords::new(0, 0),
            pos: POSITIONS_PER_TILE / 2,
        },
        Action::Update,
        Action::Update,
        Action::Update,
    ]);
}

#[test]
fn circular_loop() {
    execute_action_sequence(vec![
        // Create a 2x2 loop
        Action::PlaceBelt {
            coords: WorldCoords::new(0, 0),
            dir: Dir::East,
        },
        Action::PlaceBelt {
            coords: WorldCoords::new(1, 0),
            dir: Dir::North,
        },
        Action::PlaceBelt {
            coords: WorldCoords::new(1, 1),
            dir: Dir::West,
        },
        Action::PlaceBelt {
            coords: WorldCoords::new(0, 1),
            dir: Dir::South,
        },
        // Add an item
        Action::PlaceItem {
            belt_coords: WorldCoords::new(0, 0),
            pos: 0,
        },
        // Run many updates - item should loop forever without violating movement bounds
        Action::Update,
        Action::Update,
        Action::Update,
        Action::Update,
        Action::Update,
        Action::Update,
        Action::Update,
        Action::Update,
        Action::Update,
        Action::Update,
    ]);
}

#[test]
fn place_item_on_nonexistent_belt() {
    execute_action_sequence(vec![
        // Try to place item on belt that doesn't exist
        Action::PlaceItem {
            belt_coords: WorldCoords::new(5, 5),
            pos: 100,
        },
        Action::Update,
    ]);
}

#[test]
fn replace_belt_with_item() {
    execute_action_sequence(vec![
        // Create belt with item
        Action::PlaceBelt {
            coords: WorldCoords::new(0, 0),
            dir: Dir::East,
        },
        Action::PlaceItem {
            belt_coords: WorldCoords::new(0, 0),
            pos: ITEM_SPACING,
        },
        // Replace the belt
        Action::PlaceBelt {
            coords: WorldCoords::new(0, 0),
            dir: Dir::North,
        },
        Action::Update,
        Action::Update,
    ]);
}

#[test]
fn multiple_belts_multiple_items() {
    execute_action_sequence(vec![
        // Create several belts
        Action::PlaceBelt {
            coords: WorldCoords::new(0, 0),
            dir: Dir::East,
        },
        Action::PlaceBelt {
            coords: WorldCoords::new(1, 0),
            dir: Dir::East,
        },
        Action::PlaceBelt {
            coords: WorldCoords::new(2, 0),
            dir: Dir::East,
        },
        // Add items to different belts
        Action::PlaceItem {
            belt_coords: WorldCoords::new(0, 0),
            pos: 0,
        },
        Action::PlaceItem {
            belt_coords: WorldCoords::new(1, 0),
            pos: 50,
        },
        Action::PlaceItem {
            belt_coords: WorldCoords::new(2, 0),
            pos: 100,
        },
        // Multiple updates
        Action::Update,
        Action::Update,
        Action::Update,
        Action::Update,
    ]);
}
