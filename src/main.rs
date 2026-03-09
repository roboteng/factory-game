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
    #[cfg(feature = "ui")]
    {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        for x in -4..=4 {
            for z in -4..=4 {
                let item = if rng.gen_bool(0.5) {
                    crate::core::Item::Rock
                } else {
                    crate::core::Item::Dirt
                };
                let entity = cmd.spawn_empty().id();
                cmd.trigger(crate::core::PlaceBlock {
                    entity,
                    item,
                    coords: (x, -2, z).into(),
                    dir: HDir::North,
                });
            }
        }
    }
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
    let belt_entity = cmd.spawn_empty().id();
    cmd.trigger(crate::core::PlaceBlock {
        entity: belt_entity,
        item: crate::core::Item::Belt,
        coords: (-1, 0, 0).into(),
        dir: HDir::North,
    });
    let item_entity = cmd.spawn_empty().id();
    cmd.trigger(PlaceItem {
        entity: item_entity,
        item: Item::Belt,
        belt: belt_entity,
        lane: Side::Left,
        position: 128,
        on_error: Box::new(|_, _| {}),
    });
}
