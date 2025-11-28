use bevy::prelude::*;

pub struct GamePlugin;
impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (startup, startup_camera));
    }
}

fn startup_camera(mut cmd: Commands) {
    cmd.spawn(Camera2d);
}

fn startup(mut cmd: Commands, asset: Res<AssetServer>) {
    let tile = asset.load("sprites/tile.png");
    for x in -5..=5 {
        for y in -5..=5 {
            cmd.spawn((
                Sprite::from_image(tile.clone()),
                Transform::from_xyz(x as f32 * 32.0, y as f32 * 32.0, 0.0),
            ));
        }
    }
    cmd.spawn((
        Sprite::from_image(asset.load("sprites/player.png")),
        Transform::default(),
    ));
}

#[derive(Component, PartialEq, Eq, Debug)]
pub struct WorldItem;

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn place_ore(mut cmd: Commands) {
        cmd.spawn((WorldItem, Transform::default()));
    }

    #[test]
    fn item() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_systems(Startup, place_ore);

        app.update();
        let mut query = app.world_mut().query::<(&WorldItem, &Transform)>();
        let items = query.iter(app.world()).collect::<Vec<_>>();
        assert_eq!(items, vec![(&WorldItem, &Transform::default())]);
    }
}
