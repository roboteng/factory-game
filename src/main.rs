use bevy::prelude::*;

mod core;
fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins);
    app.add_plugins(core::CorePlugin);
    app.run();
}
