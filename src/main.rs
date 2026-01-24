use std::env;

use crate::core::*;

use bevy::prelude::*;

mod core;
#[cfg(feature = "ui")]
mod manual_sim;
mod sim;
#[cfg(feature = "ui")]
mod ui;

fn main() {
    let mut app = App::new();

    app.add_plugins((DefaultPlugins, core::CorePlugin));

    let args = env::args().collect::<Vec<String>>();
    if args.get(1).map(|s| s.as_str()).unwrap_or_default() == "--debug" {
        #[cfg(feature = "ui")]
        app.add_plugins(manual_sim::ManualSimPlugin);
    } else {
        app.add_plugins(sim::SimPlugin);
    }

    #[cfg(feature = "dev")]
    app.add_plugins((
        bevy::diagnostic::FrameTimeDiagnosticsPlugin::default(),
        bevy::diagnostic::SystemInformationDiagnosticsPlugin,
        bevy::diagnostic::LogDiagnosticsPlugin::default(),
    ));

    #[cfg(feature = "ui")]
    app.add_plugins(ui::UiPlugin);

    app.add_systems(Startup, setup);

    #[cfg(all(feature = "ui", feature = "dev"))]
    app.add_systems(Startup, max_framerate);

    app.run();
}

#[cfg(all(feature = "ui", feature = "dev"))]
fn max_framerate(mut windows: Query<&mut Window, With<bevy::window::PrimaryWindow>>) {
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
    let entity = cmd.spawn_empty().id();
    cmd.trigger(crate::core::PlaceBelt {
        entity,
        coords: (0, 0, 0).into(),
        dir: HDir::North,
    });
    let entity = cmd.spawn_empty().id();
    cmd.trigger(crate::core::PlaceBelt {
        entity,
        coords: (-1, 0, 0).into(),
        dir: HDir::North,
    });
}
