use crate::core::*;

use bevy::prelude::*;

mod core;
#[cfg(feature = "ui")]
mod ui;

fn main() {
    let mut app = App::new();

    app.add_plugins((DefaultPlugins, core::CorePlugin));

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
    cmd.trigger(crate::core::PlaceBlock {
        entity,
        item: crate::core::Item::Belt,
        coords: (1, 0, 0).into(),
        dir: HDir::East,
    });
    let entity = cmd.spawn_empty().id();
    cmd.trigger(crate::core::PlaceBlock {
        entity,
        item: crate::core::Item::Belt,
        coords: (0, 0, 0).into(),
        dir: HDir::North,
    });
    let entity = cmd.spawn_empty().id();
    cmd.trigger(crate::core::PlaceBlock {
        entity,
        item: crate::core::Item::Belt,
        coords: (-1, 0, 0).into(),
        dir: HDir::North,
    });
}
