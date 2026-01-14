#![allow(unused)]
use crate::core::*;

use bevy::prelude::*;
#[cfg(feature = "ui")]
use bevy::window::PrimaryWindow;

mod core;
mod sim;
#[cfg(feature = "ui")]
mod ui;

fn main() {
    let mut app = App::new();

    app.add_plugins((DefaultPlugins, core::CorePlugin, sim::SimPlugin));

    #[cfg(feature = "dev")]
    app.add_plugins((
        bevy::diagnostic::FrameTimeDiagnosticsPlugin::default(),
        bevy::diagnostic::SystemInformationDiagnosticsPlugin,
        bevy::diagnostic::LogDiagnosticsPlugin::default(),
    ));

    #[cfg(feature = "ui")]
    app.add_plugins(ui::UiPlugin);

    app.add_systems(Startup, setup);
    app.add_systems(Update, update);

    #[cfg(all(feature = "ui", feature = "dev"))]
    app.add_systems(Startup, max_framerate);

    app.init_resource::<ShouldRun>();

    app.run();
}

#[cfg(all(feature = "ui", feature = "dev"))]
fn max_framerate(mut windows: Query<&mut Window, With<PrimaryWindow>>) {
    for mut window in windows.iter_mut() {
        window.present_mode = bevy::window::PresentMode::AutoNoVsync
    }
}

fn setup(mut cmd: Commands) {
    let entity = cmd.spawn_empty().id();
    cmd.trigger(crate::core::PlaceBelt {
        entity,
        coords: (1, 0, 0).into(),
        dir: HDir::North,
    });
    // let item = cmd.spawn_empty().id();
    // cmd.trigger(PlaceItem {
    //     entity: item,
    //     item: Item(0),
    //     belt: entity,
    //     lane: LaneSide::Left,
    //     position: 0,
    // });
    // let entity = cmd.spawn_empty().id();
    // cmd.trigger(crate::core::PlaceBelt {
    //     entity,
    //     coords: (0, 0, 1).into(),
    //     dir: HDir::East,
    // });
    // let item = cmd.spawn_empty().id();
    // cmd.trigger(PlaceItem {
    //     entity: item,
    //     item: Item(0),
    //     belt: entity,
    //     lane: LaneSide::Left,
    //     position: 0,
    // });
    let entity = cmd.spawn_empty().id();
    cmd.trigger(crate::core::PlaceBelt {
        entity,
        coords: (0, 0, 0).into(),
        dir: HDir::North,
    });
    // let item = cmd.spawn_empty().id();
    // cmd.trigger(PlaceItem {
    //     entity: item,
    //     item: Item(0),
    //     belt: entity,
    //     lane: LaneSide::Left,
    //     position: 0,
    // });
    let entity = cmd.spawn_empty().id();
    cmd.trigger(crate::core::PlaceBelt {
        entity,
        coords: (-1, 0, 0).into(),
        dir: HDir::North,
    });
    // let item = cmd.spawn_empty().id();
    // cmd.trigger(PlaceItem {
    //     entity: item,
    //     item: Item(0),
    //     belt: entity,
    //     lane: LaneSide::Left,
    //     position: 0,
    // });
}

#[derive(Resource, Default)]
struct ShouldRun(bool);

fn update(mut should_run: ResMut<ShouldRun>, belts: Query<(Entity, &InLane)>, mut cmd: Commands) {
    if !should_run.0 {
        info!("Ran once");
        for (entity, _) in belts.iter() {
            let item = cmd.spawn_empty().id();
            cmd.trigger(PlaceItem {
                entity: item,
                item: Item(0),
                belt: entity,
                lane: LaneSide::Left,
                position: 0,
                on_error: Box::new(|mut commands, error| {
                    warn!("Failed to place item in main: {:?}", error);
                }),
            });
        }
    }
    should_run.0 = true;
}
