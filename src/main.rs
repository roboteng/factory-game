mod ui;

use bevy::prelude::*;
use common::{CorePlugin, FlatWorldPlugin, PerlinWorldPlugin, SimPlugin};
use ui::{FlyMode, FreeHotbar, SurvivalHotbar, UiPlugin};

fn main() {
    let fly_mode = std::env::args().any(|a| a == "--fly");
    let flat_mode = std::env::args().any(|a| a == "--flat");
    let max_framerate_mode = std::env::args().any(|a| a == "--max");
    let creative_mode = std::env::args().any(|a| a == "--creative");

    let mut app = App::new();

    app.insert_resource(FlyMode(fly_mode));
    app.add_plugins((DefaultPlugins, CorePlugin, SimPlugin));

    if flat_mode {
        app.add_plugins(FlatWorldPlugin);
    } else {
        app.add_plugins(PerlinWorldPlugin);
    }

    if creative_mode {
        app.add_plugins(FreeHotbar);
    } else {
        app.add_plugins(SurvivalHotbar);
    }

    app.add_plugins(UiPlugin);
    app.add_systems(Update, screenshot_on_f10);
    #[cfg(feature = "dev")]
    {
        if max_framerate_mode {
            app.add_systems(Startup, max_framerate);
        }
        app.add_plugins((
            bevy::diagnostic::FrameTimeDiagnosticsPlugin::default(),
            bevy::diagnostic::SystemInformationDiagnosticsPlugin,
            bevy::diagnostic::LogDiagnosticsPlugin::default(),
        ));
        app.add_plugins(bevy::dev_tools::fps_overlay::FpsOverlayPlugin::default());
    }

    app.run();
}

#[cfg(feature = "dev")]
fn max_framerate(mut windows: Query<&mut Window, With<bevy::window::PrimaryWindow>>) {
    for mut window in windows.iter_mut() {
        window.present_mode = bevy::window::PresentMode::AutoNoVsync
    }
}

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
