use bevy::prelude::*;

mod core;
#[cfg(feature = "invariant-ckeck")]
mod invariants;
mod sim;
#[cfg(feature = "ui")]
mod ui;

fn main() {
    let mut app = App::new();

    app.add_plugins((DefaultPlugins, core::CorePlugin, sim::SimPlugin));

    #[cfg(feature = "ui")]
    app.add_plugins(ui::UiPlugin);
    #[cfg(feature = "invariant-ckeck")]
    app.add_plugins(invariants::InvariantsPlugin);

    app.run();
}
