use bevy::app::MainScheduleOrder;
use bevy::ecs::schedule::{ExecutorKind, Schedule, ScheduleLabel};

use crate::core::*;
use std::panic::Location;

#[derive(ScheduleLabel, Debug, Hash, PartialEq, Eq, Clone)]
struct InvariantChecks;

pub struct InvariantsPlugin;

impl Plugin for InvariantsPlugin {
    fn build(&self, app: &mut App) {
        let mut invariant_schedule = Schedule::new(InvariantChecks);
        invariant_schedule.set_executor_kind(ExecutorKind::SingleThreaded);
        app.add_schedule(invariant_schedule);

        let mut main_schedule_order = app.world_mut().resource_mut::<MainScheduleOrder>();
        main_schedule_order.insert_after(PostUpdate, InvariantChecks);

        app.init_resource::<BrokenInvariants>();

        app.add_systems(InvariantChecks, |mut b: ResMut<BrokenInvariants>| {
            b.check();
            b.clear();
        });
    }
}

#[derive(Resource, Default)]
struct BrokenInvariants {
    failures: Vec<String>,
}

impl BrokenInvariants {
    #[track_caller]
    #[expect(dead_code)]
    fn add(&mut self, message: impl AsRef<str>) {
        let loc = Location::caller();
        self.failures.push(format!("{} at {loc}", message.as_ref()));
    }
    fn check(&self) {
        if !self.failures.is_empty() {
            eprintln!("Broken Invariants:");
            for f in self.failures.iter() {
                eprintln!("{f}");
            }
            panic!("Found {} failures", self.failures.len());
        }
    }
    fn clear(&mut self) {
        self.failures.clear();
    }
}
