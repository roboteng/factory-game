use bevy::prelude::*;

pub mod hotbar;
pub mod inventory;
pub mod slot;

pub struct ViewsPlugin;
impl Plugin for ViewsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, inventory::view);
        app.add_systems(Update, hotbar::hotbar_view);
    }
}
