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
    let o = WorldCoords::ORIGIN;

    #[cfg(feature = "ui")]
    {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let ground_height = WorldCoordsDelta::ZERO.height(-2);
        for ns in -5..=5 {
            for ew in -5..=5 {
                let block = if rng.gen_bool(0.5) {
                    WorldBlock::Rock
                } else {
                    WorldBlock::Dirt
                };
                let entity = cmd.spawn_empty().id();
                cmd.trigger(PlaceBlock {
                    entity,
                    block,
                    coords: o + ground_height + WorldCoordsDelta::ZERO.north(ns).east(ew),
                    dir: HDir::North,
                });
            }
        }

        // Ore deposits for mining — placed outside the rock/dirt grid.
        for (ns, ew, block) in [
            (6i32, 0i32, WorldBlock::IronOreDeposit),
            (6, 1, WorldBlock::IronOreDeposit),
            (6, -1, WorldBlock::IronOreDeposit),
            (-6, 0, WorldBlock::CopperOreDeposit),
            (-6, 1, WorldBlock::CopperOreDeposit),
        ] {
            let entity = cmd.spawn_empty().id();
            cmd.trigger(crate::core::PlaceBlock {
                entity,
                block,
                coords: o + WorldCoordsDelta::ZERO.north(ns).east(ew),
                dir: HDir::North,
            });
        }
    }
    let entity = cmd.spawn_empty().id();
    cmd.trigger(PlaceBlock {
        entity,
        block: WorldBlock::Belt,
        coords: o.step(HDir::North).step(Dir::Up),
        dir: HDir::North,
    });
    let entity = cmd.spawn_empty().id();
    cmd.trigger(PlaceBlock {
        entity,
        block: WorldBlock::Belt,
        coords: o,
        dir: HDir::North,
    });
    let belt_entity = cmd.spawn_empty().id();
    cmd.trigger(PlaceBlock {
        entity: belt_entity,
        block: WorldBlock::Belt,
        coords: o.step(HDir::South),
        dir: HDir::North,
    });
}
