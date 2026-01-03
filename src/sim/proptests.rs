use crate::core::*;
use crate::sim::SimPlugin;
use crate::sim::proptest_actions::*;
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
        Action::Update,
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
        Action::Update,
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
        Action::Update,
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
        Action::Update,
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

#[cfg(test)]
mod shuffle_order_tests {
    use super::*;
    use crate::sim::proptest_actions::Action;
    use rand::rngs::StdRng;
    use rand::{SeedableRng, seq::SliceRandom};
    use std::collections::HashMap;

    /// Create a shuffled copy using the seed
    fn shuffle_vec<T: Clone>(vec: &[T], seed: u64) -> Vec<T> {
        let mut shuffled = vec.to_vec();
        let mut rng = StdRng::seed_from_u64(seed);
        shuffled.shuffle(&mut rng);
        shuffled
    }

    /// Execute action sequence and return (app, map of belt coords to entities)
    fn execute_actions(actions: &[Option<Action>]) -> (App, HashMap<WorldCoords, Entity>) {
        let mut app = test_app_with_sim();
        let mut belt_map = HashMap::new();

        // Execute actions
        for action in actions {
            match action {
                Some(Action::PlaceBelt { coords, dir }) => {
                    let belt_entity = app.add_belt(*coords, *dir);
                    belt_map.insert(*coords, belt_entity);
                }
                Some(Action::Update) => {
                    app.update();
                }
                Some(Action::PlaceItem { .. }) => {
                    // Should never happen with our strategy, but skip if it does
                }
                None => {
                    // Skip
                }
            }
        }

        (app, belt_map)
    }

    /// Compare transforms for all item pairs at current step
    fn compare_transforms_at_step(
        app_a: &mut App,
        app_b: &mut App,
        item_pairs: &[(Entity, Entity)],
        step: usize,
        tolerance: f32,
    ) -> Result<(), String> {
        for (i, (item_a, item_b)) in item_pairs.iter().enumerate() {
            let transform_a = app_a
                .find_item(*item_a)
                .ok_or_else(|| format!("Step {}: Item {} not found in sim A", step, i))?
                .1;
            let transform_b = app_b
                .find_item(*item_b)
                .ok_or_else(|| format!("Step {}: Item {} not found in sim B", step, i))?
                .1;

            let distance = transform_a.translation.distance(transform_b.translation);
            if distance > tolerance {
                return Err(format!(
                    "Step {}, Item {}:\n  A: {:?}\n  B: {:?}\n  Distance: {:.6} > {:.6}",
                    step, i, transform_a.translation, transform_b.translation, distance, tolerance
                ));
            }
        }

        Ok(())
    }

    /// Format actions for display, filtering out None values
    fn format_actions(actions: &[Option<Action>]) -> String {
        let non_none: Vec<_> = actions
            .iter()
            .enumerate()
            .filter_map(|(i, a)| a.as_ref().map(|action| (i, action)))
            .collect();

        if non_none.is_empty() {
            return "[]".to_string();
        }

        let mut result = String::from("[\n");
        for (i, action) in non_none {
            result.push_str(&format!("  [{}] {:?}\n", i, action));
        }
        result.push(']');
        result
    }

    /// Main test function
    fn test_shuffle_invariance(
        _vec_size: usize,
        actions: Vec<Option<Action>>,
        seed: u64,
    ) -> Result<(), TestCaseError> {
        // Early exit if empty
        if actions.iter().all(|a| a.is_none()) {
            return Ok(());
        }

        const STEPS: usize = (POSITIONS_PER_TILE / BASE_BELT_SPEED) as usize; // 32
        const TOLERANCE: f32 = 0.01;

        // Execute actions in both sims
        let shuffled = shuffle_vec(&actions, seed);
        let (mut app_a, belts_a) = execute_actions(&actions);
        let (mut app_b, belts_b) = execute_actions(&shuffled);

        // Final update to ensure lanes are initialized
        app_a.update();
        app_b.update();

        // Check that all matching belts have the same type
        // If any differ, reject this test case (not a valid comparison)
        for (coords, belt_a) in &belts_a {
            if let Some(belt_b) = belts_b.get(coords) {
                let belt_type_a = app_a.world().get::<Belt>(*belt_a);
                let belt_type_b = app_b.world().get::<Belt>(*belt_b);

                if belt_type_a != belt_type_b {
                    // Reject test: final belt configurations differ
                    return Err(TestCaseError::reject(
                        "Belt configurations differ after shuffle",
                    ));
                }
            }
        }

        // Place items on all belts in both sims
        let mut item_pairs = Vec::new();
        for (coords, belt_a) in &belts_a {
            if let Some(belt_b) = belts_b.get(coords) {
                let item_a = app_a.add_item(*belt_a, 0);
                let item_b = app_b.add_item(*belt_b, 0);
                item_pairs.push((item_a, item_b));
            }
        }

        if item_pairs.is_empty() {
            return Ok(()); // No belts to test
        }

        // Run simulation and compare at each step
        for step in 0..STEPS {
            // Update both
            app_a.update();
            app_b.update();

            // Compare transforms
            if let Err(msg) =
                compare_transforms_at_step(&mut app_a, &mut app_b, &item_pairs, step + 1, TOLERANCE)
            {
                panic!(
                    "Transform mismatch:\n{}\n\nOriginal order (seed {}):\n{}\n\nShuffled order:\n{}",
                    msg,
                    seed,
                    format_actions(&actions),
                    format_actions(&shuffled)
                );
            }
        }

        Ok(())
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(50))]

        #[test]
        #[ignore = "finish implementation"]
        fn belt_placement_order_invariant(
            (vec_size, actions, seed) in crate::sim::proptest_actions::arb_action_vec_for_shuffle()
        ) {
            test_shuffle_invariance(vec_size, actions, seed)?;
        }
    }

    // Edge case regression tests
    #[test]
    fn shuffle_empty_vec() {
        test_shuffle_invariance(0, vec![], 42).unwrap();
    }

    #[test]
    fn shuffle_single_belt() {
        use crate::core::{Dir, WorldCoords};
        test_shuffle_invariance(
            1,
            vec![Some(Action::PlaceBelt {
                coords: WorldCoords::new(0, 0),
                dir: Dir::East,
            })],
            42,
        )
        .unwrap();
    }

    #[test]
    fn shuffle_belt_chain() {
        use crate::core::{Dir, WorldCoords};
        test_shuffle_invariance(
            5,
            vec![
                Some(Action::PlaceBelt {
                    coords: WorldCoords::new(0, 0),
                    dir: Dir::East,
                }),
                Some(Action::PlaceBelt {
                    coords: WorldCoords::new(1, 0),
                    dir: Dir::East,
                }),
                Some(Action::PlaceBelt {
                    coords: WorldCoords::new(2, 0),
                    dir: Dir::East,
                }),
                Some(Action::PlaceBelt {
                    coords: WorldCoords::new(3, 0),
                    dir: Dir::East,
                }),
                Some(Action::PlaceBelt {
                    coords: WorldCoords::new(4, 0),
                    dir: Dir::East,
                }),
            ],
            42,
        )
        .unwrap();
    }

    #[test]
    fn shuffle_with_updates() {
        use crate::core::{Dir, WorldCoords};
        test_shuffle_invariance(
            7,
            vec![
                Some(Action::PlaceBelt {
                    coords: WorldCoords::new(0, 0),
                    dir: Dir::East,
                }),
                Some(Action::Update),
                Some(Action::PlaceBelt {
                    coords: WorldCoords::new(1, 0),
                    dir: Dir::East,
                }),
                Some(Action::Update),
                Some(Action::PlaceBelt {
                    coords: WorldCoords::new(2, 0),
                    dir: Dir::East,
                }),
                Some(Action::Update),
                Some(Action::Update),
            ],
            42,
        )
        .unwrap();
    }
}
