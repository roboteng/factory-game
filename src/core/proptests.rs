use crate::core::proptest_actions::*;
use crate::core::*;
use bevy::prelude::*;
use proptest::prelude::*;
use proptest::test_runner::Reason;

fn execute_action_sequence(actions: Vec<Action>) -> Result<(), TestCaseError> {
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

                // Check for invalid placements and reject test case if found
                if app.has_placement_errors() {
                    let errors = app.take_placement_errors();
                    prop_assume!(
                        false,
                        "Invalid placement occurred (rejecting test case): {:?}",
                        errors
                    );
                }

                state.next_frame();
            }
        }
    }

    state.capture_item_positions(&mut app);
    app.update();

    // Final check for placement errors
    if app.has_placement_errors() {
        let errors = app.take_placement_errors();
        return Err(TestCaseError::Reject(format!("{errors:?}").into()));
    }

    Ok(())
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
