use bevy::prelude::*;

mod hotbar;
mod inventory;

pub use hotbar::spawn_hotbar;
pub use inventory::spawn_inventory;

pub struct ViewsPlugin;
impl Plugin for ViewsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, inventory::view);
        app.add_systems(Update, hotbar::hotbar_view);
    }
}
