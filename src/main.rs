use crate::core::*;

use bevy::prelude::*;

mod core;
#[cfg(feature = "ui")]
mod ui;

/// When present and `true`, the player uses a free-flying noclip camera
/// instead of the physics-based controller. Set via the `--fly` CLI flag.
#[derive(Resource)]
pub struct FlyMode(pub bool);

fn main() {
    let fly_mode = std::env::args().any(|a| a == "--fly");

    let mut app = App::new();

    app.insert_resource(FlyMode(fly_mode));
    app.add_plugins((DefaultPlugins, core::CorePlugin));

    #[cfg(feature = "dev")]
    app.add_plugins((
        bevy::diagnostic::FrameTimeDiagnosticsPlugin::default(),
        bevy::diagnostic::SystemInformationDiagnosticsPlugin,
        bevy::diagnostic::LogDiagnosticsPlugin::default(),
    ));

    #[cfg(feature = "ui")]
    app.add_plugins(ui::UiPlugin);

    #[cfg(feature = "ui")]
    app.add_systems(Update, screenshot_on_f10);

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

#[cfg(feature = "ui")]
fn screenshot_on_f10(
    input: Res<ButtonInput<KeyCode>>,
    mut counter: Local<u32>,
    mut commands: Commands,
) {
    use bevy::render::view::screenshot::{Screenshot, save_to_disk};
    if input.just_pressed(KeyCode::F10) {
        let path = format!("./screenshot-{}.png", *counter);
        *counter += 1;
        if let Ok(full_path) = std::path::absolute(&path) {
            info!("Saving screenshot to: {}", full_path.display());
        }
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(path));
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
