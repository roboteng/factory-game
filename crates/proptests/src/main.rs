mod actions;

use actions::*;
use bevy::prelude::*;
use factory_core::*;
use proptest::test_runner::{Config, TestRunner};

fn main() {
    init_tracing();

    println!("Running regression tests...");
    empty_sequence();
    updates_only_no_belts();
    println!("Regression tests passed.");

    println!("Running property tests...");
    let mut runner = TestRunner::new(Config::with_cases(100));
    runner
        .run(&arb_action_sequence(), |actions| {
            execute_action_sequence(actions)
        })
        .expect("Property test failed");
    println!("Property tests passed.");
}

// ── Test helpers ──────────────────────────────────────────────────────────────

pub fn init_tracing() {
    use tracing_subscriber::{fmt, EnvFilter};
    let _ = fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("warn,factory_core=debug")),
        )
        .with_target(false)
        .without_time()
        .try_init();
}

#[derive(Resource, Default)]
pub struct PlacementErrors {
    pub errors: Vec<ItemPlacementError>,
}

pub fn test_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.init_resource::<PlacementErrors>();
    app
}

pub trait AppExt {
    fn add_belt(&mut self, coords: impl Into<WorldCoords>, dir: HDir) -> Entity;
    fn add_item(&mut self, belt: Entity, pos: i32, lane: Side) -> Entity;
    fn has_placement_errors(&self) -> bool;
    fn take_placement_errors(&mut self) -> Vec<ItemPlacementError>;
}

impl AppExt for App {
    fn add_belt(&mut self, coords: impl Into<WorldCoords>, dir: HDir) -> Entity {
        let entity = self.world_mut().spawn_empty().id();
        let flb: WorldCoords = coords.into();
        let brt = WorldBlock::Belt.brt_for(flb, Some(dir));
        self.world_mut().trigger(PlaceBlock {
            entity,
            block: WorldBlock::Belt,
            flb,
            brt,
        });
        entity
    }

    fn add_item(&mut self, belt: Entity, pos: i32, lane: Side) -> Entity {
        let entity = self.world_mut().spawn(OnBelt).id();
        if let Some(mut lanes) = self.world_mut().get_mut::<ItemLanes>(belt) {
            lanes.0[lane].push((pos, entity));
        }
        self.world_mut().trigger(PlaceItem {
            entity,
            item: Item::Belt,
        });
        entity
    }

    fn has_placement_errors(&self) -> bool {
        self.world()
            .get_resource::<PlacementErrors>()
            .map(|e| !e.errors.is_empty())
            .unwrap_or(false)
    }

    fn take_placement_errors(&mut self) -> Vec<ItemPlacementError> {
        self.world_mut()
            .get_resource_mut::<PlacementErrors>()
            .map(|mut e| std::mem::take(&mut e.errors))
            .unwrap_or_default()
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

fn execute_action_sequence(
    actions: Vec<Action>,
) -> Result<(), proptest::test_runner::TestCaseError> {
    use proptest::prop_assume;

    let mut app = test_app();
    let mut state = TestState::default();

    for action in &actions {
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

    if app.has_placement_errors() {
        let errors = app.take_placement_errors();
        return Err(proptest::test_runner::TestCaseError::Reject(
            format!("{errors:?}").into(),
        ));
    }

    Ok(())
}

fn empty_sequence() {
    execute_action_sequence(vec![]).unwrap();
}

fn updates_only_no_belts() {
    execute_action_sequence(vec![
        Action::Update,
        Action::Update,
        Action::Update,
        Action::Update,
        Action::Update,
    ])
    .unwrap();
}
