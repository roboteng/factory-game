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
    let flat_mode = std::env::args().any(|a| a == "--flat");

    let mut app = App::new();

    app.insert_resource(FlyMode(fly_mode));
    app.add_plugins((DefaultPlugins, core::CorePlugin));

    if flat_mode {
        app.add_plugins(FlatWorldPlugin);
    } else {
        app.add_plugins(PerlinWorldPlugin);
    }

    #[cfg(feature = "dev")]
    app.add_plugins((
        bevy::diagnostic::FrameTimeDiagnosticsPlugin::default(),
        bevy::diagnostic::SystemInformationDiagnosticsPlugin,
        bevy::diagnostic::LogDiagnosticsPlugin::default(),
    ));

    #[cfg(feature = "ui")]
    {
        app.add_plugins(ui::UiPlugin);
        app.add_systems(Update, screenshot_on_f10);
        #[cfg(feature = "dev")]
        {
            // app.add_systems(Startup, max_framerate);
            app.add_plugins(bevy::dev_tools::fps_overlay::FpsOverlayPlugin::default());
        }
    }

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
