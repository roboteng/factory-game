use crate::core::proptest_actions::*;
use crate::core::*;
use bevy::prelude::*;
use proptest::prelude::*;

fn execute_action_sequence(actions: Vec<Action>) {
    let mut app = test_app_with_invariants();
    let mut state = TestState::default();

    for (i, action) in actions.iter().enumerate() {
        match action {
            Action::PlaceBelt { coords, dir } => {
                let entity = app.add_belt(*coords, *dir);
                state.place_belt(*coords, entity);
            }
            Action::PlaceItem {
                belt_coords,
                pos,
                lane,
            } => {
                if let Some(belt_entity) = state.get_belt(*belt_coords) {
                    app.add_item(belt_entity, *pos, *lane);
                }
            }
            Action::Update => {
                state.capture_item_positions(&mut app);
                app.update();

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

    state.capture_item_positions(&mut app);
    app.update();
    if let Err(msg) = state.check_movement_bounds(&mut app) {
        panic!("Movement bound violation at final update: {}", msg);
    }
}

fn test_app_with_invariants() -> App {
    let mut app = test_app();
    app
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn arbitrary_action_sequences_maintain_invariants(
        actions in arb_action_sequence()
    ) {
        execute_action_sequence(actions);
    }
}

// Regression tests

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
            coords: WorldCoords { x: 0, y: 0, z: 0 },
            dir: HDir::East,
        },
        Action::Update,
        Action::Update,
    ]);
}

#[test]
fn single_belt_with_item() {
    execute_action_sequence(vec![
        Action::PlaceBelt {
            coords: WorldCoords { x: 0, y: 0, z: 0 },
            dir: HDir::East,
        },
        Action::Update,
        Action::PlaceItem {
            belt_coords: WorldCoords { x: 0, y: 0, z: 0 },
            pos: POSITIONS_PER_BELT / 2,
            lane: LaneSide::Left,
        },
        Action::Update,
        Action::Update,
        Action::Update,
    ]);
}

#[test]
#[ignore = "curved belt merge logic not yet implemented"]
fn circular_loop() {
    execute_action_sequence(vec![
        Action::PlaceBelt {
            coords: WorldCoords { x: 0, y: 0, z: 0 },
            dir: HDir::East,
        },
        Action::PlaceBelt {
            coords: WorldCoords { x: 0, y: 0, z: 1 },
            dir: HDir::North,
        },
        Action::PlaceBelt {
            coords: WorldCoords { x: 1, y: 0, z: 1 },
            dir: HDir::West,
        },
        Action::PlaceBelt {
            coords: WorldCoords { x: 1, y: 0, z: 0 },
            dir: HDir::South,
        },
        Action::Update,
        Action::PlaceItem {
            belt_coords: WorldCoords { x: 0, y: 0, z: 0 },
            pos: 0,
            lane: LaneSide::Left,
        },
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
        Action::PlaceItem {
            belt_coords: WorldCoords { x: 5, y: 5, z: 0 },
            pos: 100,
            lane: LaneSide::Left,
        },
        Action::Update,
    ]);
}

#[test]
#[ignore = "curved belt merge logic not yet implemented"]
fn replace_belt_with_item() {
    execute_action_sequence(vec![
        Action::PlaceBelt {
            coords: WorldCoords { x: 0, y: 0, z: 0 },
            dir: HDir::East,
        },
        Action::Update,
        Action::PlaceItem {
            belt_coords: WorldCoords { x: 0, y: 0, z: 0 },
            pos: ITEM_SPACING,
            lane: LaneSide::Left,
        },
        Action::PlaceBelt {
            coords: WorldCoords { x: 0, y: 0, z: 0 },
            dir: HDir::North,
        },
        Action::Update,
        Action::Update,
    ]);
}

#[test]
#[ignore = "curved belt merge logic not yet implemented"]
fn multiple_belts_multiple_items() {
    execute_action_sequence(vec![
        Action::PlaceBelt {
            coords: WorldCoords { x: 0, y: 0, z: 0 },
            dir: HDir::East,
        },
        Action::PlaceBelt {
            coords: WorldCoords { x: 0, y: 0, z: 1 },
            dir: HDir::East,
        },
        Action::PlaceBelt {
            coords: WorldCoords { x: 0, y: 0, z: 2 },
            dir: HDir::East,
        },
        Action::Update,
        Action::PlaceItem {
            belt_coords: WorldCoords { x: 0, y: 0, z: 0 },
            pos: 0,
            lane: LaneSide::Left,
        },
        Action::PlaceItem {
            belt_coords: WorldCoords { x: 0, y: 0, z: 1 },
            pos: 50,
            lane: LaneSide::Right,
        },
        Action::PlaceItem {
            belt_coords: WorldCoords { x: 0, y: 0, z: 2 },
            pos: 100,
            lane: LaneSide::Left,
        },
        Action::Update,
        Action::Update,
        Action::Update,
        Action::Update,
    ]);
}
