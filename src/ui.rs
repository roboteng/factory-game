use crate::core::*;
use bevy::prelude::*;

pub struct UiPlugin;
impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_ui);
        app.add_systems(Update, apply_belt_sprites);
    }
}

fn setup_ui(mut commands: Commands) {
    commands.spawn(Camera2d);
}

fn apply_belt_sprites(
    belts: Query<(Entity, &Belt, &WorldCoords), Changed<Belt>>,
    assets: Res<AssetServer>,
    mut cmd: Commands,
) {
    for (ent, belt, coords) in belts.iter() {
        let (sprite, quat) = match belt {
            Belt::Straight(dir) => {
                let k = Vec2::from(*dir);
                let angle = Vec2::X.angle_to(k);
                (
                    Sprite::from(assets.load("sprites/belt.png")),
                    Quat::from_axis_angle(Vec3::Z, angle),
                )
            }
            _ => {
                info!("making sprite for {:?}", belt);
                let k = Vec2::from(belt.output());
                let angle = Vec2::X.angle_to(k);

                if belt.input().left() == belt.output() {
                    let mut sprite = Sprite::from(assets.load("sprites/belt_curved.png"));
                    sprite.flip_y = true;
                    (sprite, Quat::from_axis_angle(Vec3::Z, angle))
                } else {
                    (
                        Sprite::from(assets.load("sprites/belt_curved.png")),
                        Quat::from_axis_angle(Vec3::Z, angle),
                    )
                }
            }
        };
        cmd.entity(ent).insert((
            sprite,
            Transform::from_xyz(coords.x as f32 * 32.0, coords.y as f32 * 32.0, 1.0)
                .with_rotation(quat),
        ));
    }
}
