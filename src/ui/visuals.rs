use crate::core::*;
use bevy::prelude::*;

pub(super) struct VisualsPlugin;
impl Plugin for VisualsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ClearColor(Color::srgb(0.01, 0.01, 0.05))); // Dark night sky
        app.add_systems(Startup, setup_models);
        app.add_systems(Update, attach_models);
        app.add_systems(Update, tint_ore_meshes);
        app.add_observer(on_place_item);
    }
}

enum ModelDef {
    Scene(Handle<Scene>),
    TintedScene(Handle<Scene>, Color),
    Random(Vec<ModelDef>),
}

#[derive(Component)]
pub(super) struct SceneTint(pub(super) Color);

#[derive(Resource)]
struct BlockModels {
    belt_straight: ModelDef,
    belt_curve_cw: ModelDef,
    belt_curve_ccw: ModelDef,
    belt_ramp_up: ModelDef,
    belt_ramp_down: ModelDef,
    source: ModelDef,
    sink: ModelDef,
    rock: ModelDef,
    dirt: ModelDef,
    iron_ore: ModelDef,
    copper_ore: ModelDef,
    miner: ModelDef,
    furnace: ModelDef,
    assembler: ModelDef,
}
fn setup_models(mut cmd: Commands, asset_server: Res<AssetServer>) {
    let voxel = asset_server.load(GltfAssetLabel::Scene(0).from_asset("models/Voxel.glb"));
    cmd.insert_resource(BlockModels {
        belt_straight: ModelDef::Scene(
            asset_server.load(GltfAssetLabel::Scene(2).from_asset("models/belt.glb")),
        ),
        belt_curve_cw: ModelDef::Scene(
            asset_server.load(GltfAssetLabel::Scene(0).from_asset("models/belt-curve-cw.glb")),
        ),
        belt_curve_ccw: ModelDef::Scene(
            asset_server.load(GltfAssetLabel::Scene(0).from_asset("models/belt-curve-ccw.glb")),
        ),
        belt_ramp_up: ModelDef::Scene(
            asset_server.load(GltfAssetLabel::Scene(1).from_asset("models/belt-up.glb")),
        ),
        belt_ramp_down: ModelDef::Scene(
            asset_server.load(GltfAssetLabel::Scene(1).from_asset("models/belt-down.glb")),
        ),
        source: ModelDef::TintedScene(voxel.clone(), Color::srgb(0.2, 0.8, 0.2)),
        sink: ModelDef::TintedScene(voxel.clone(), Color::srgb(0.8, 0.2, 0.2)),
        rock: ModelDef::TintedScene(voxel.clone(), Color::srgb(0.55, 0.55, 0.55)),
        dirt: ModelDef::TintedScene(voxel.clone(), Color::srgb(0.55, 0.35, 0.15)),
        iron_ore: {
            ModelDef::Random(
                [
                    "rock_largeA",
                    "rock_largeB",
                    "rock_largeC",
                    "rock_largeD",
                    "rock_largeE",
                    "rock_largeF",
                ]
                .iter()
                .map(|name| {
                    ModelDef::TintedScene(
                        asset_server.load(
                            GltfAssetLabel::Scene(0)
                                .from_asset(format!("models/kenney_nature_kit/{name}.glb")),
                        ),
                        Color::srgb(0.6, 0.5, 0.45),
                    )
                })
                .collect(),
            )
        },
        copper_ore: ModelDef::Random(
            [
                "rock_largeA",
                "rock_largeB",
                "rock_largeC",
                "rock_largeD",
                "rock_largeE",
                "rock_largeF",
            ]
            .iter()
            .map(|name| {
                ModelDef::TintedScene(
                    asset_server.load(
                        GltfAssetLabel::Scene(0)
                            .from_asset(format!("models/kenney_nature_kit/{name}.glb")),
                    ),
                    Color::srgb(0.7, 0.4, 0.15),
                )
            })
            .collect(),
        ),
        miner: ModelDef::Scene(
            asset_server.load(GltfAssetLabel::Scene(0).from_asset("models/Miner.glb")),
        ),
        furnace: ModelDef::Scene(
            asset_server.load(GltfAssetLabel::Scene(0).from_asset("models/Furnace.glb")),
        ),
        assembler: ModelDef::Scene(
            asset_server.load(GltfAssetLabel::Scene(0).from_asset("models/Voxel.glb")),
        ),
    });
}

fn apply_model(entity: Entity, mut cmd: Commands, model: &ModelDef) {
    match model {
        ModelDef::Scene(handle) => {
            cmd.entity(entity).insert(SceneRoot(handle.clone()));
        }
        ModelDef::TintedScene(handle, color) => {
            cmd.entity(entity).insert((
                SceneRoot(handle.clone()),
                SceneTint(*color),
                Visibility::Hidden,
            ));
        }
        ModelDef::Random(options) => {
            let chosen = &options[rand::random::<usize>() % options.len()];
            apply_model(entity, cmd, chosen);
        }
    }
}

fn tint_ore_meshes(
    mut cmd: Commands,
    tinted: Query<(Entity, &SceneTint)>,
    mut transforms: Query<&mut Transform>,
    children_q: Query<&Children>,
    mesh_mat_q: Query<&MeshMaterial3d<StandardMaterial>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (entity, SceneTint(color)) in &tinted {
        let mut found_any = false;
        for desc in children_q.iter_descendants(entity) {
            let Ok(mat_handle) = mesh_mat_q.get(desc) else {
                continue;
            };
            let Some(mat) = materials.get(mat_handle.id()) else {
                continue;
            };
            let mut new_mat = mat.clone();
            new_mat.base_color = *color;
            let new_handle = materials.add(new_mat);
            cmd.entity(desc).insert(MeshMaterial3d(new_handle));
            found_any = true;
        }
        if found_any {
            let angle = (rand::random::<u8>() % 4) as f32 * std::f32::consts::FRAC_PI_2;
            if let Ok(mut transform) = transforms.get_mut(entity) {
                transform.rotate_y(angle);
            }
            cmd.entity(entity).remove::<SceneTint>();
            cmd.entity(entity).insert(Visibility::Inherited);
        }
    }
}

fn attach_models(
    world_blocks: Query<
        (Entity, &WorldBlock, Option<&BeltShape>),
        Or<(Added<WorldBlock>, Changed<BeltShape>)>,
    >,
    all_models: Res<BlockModels>,
    mut cmd: Commands,
) {
    for (entity, block, shape) in &world_blocks {
        let model = match block {
            WorldBlock::Source => &all_models.source,
            WorldBlock::Sink => &all_models.sink,
            WorldBlock::Rock => &all_models.rock,
            WorldBlock::Dirt => &all_models.dirt,
            WorldBlock::IronOreDeposit => &all_models.iron_ore,
            WorldBlock::CopperOreDeposit => &all_models.copper_ore,
            WorldBlock::Miner => &all_models.miner,
            WorldBlock::Furnace => &all_models.furnace,
            WorldBlock::Assembler => &all_models.assembler,
            WorldBlock::Belt => match shape {
                Some(BeltShape::Straight(_)) => &all_models.belt_straight,
                Some(BeltShape::Curve(c)) => {
                    if c.is_clockwise() {
                        &all_models.belt_curve_cw
                    } else {
                        &all_models.belt_curve_ccw
                    }
                }
                Some(BeltShape::RampUp(_)) => &all_models.belt_ramp_up,
                Some(BeltShape::RampDown(_)) => &all_models.belt_ramp_down,
                None => continue,
            },
        };
        apply_model(entity, cmd.reborrow(), model);
    }
}

fn on_place_item(event: On<PlaceItem>, mut cmd: Commands, asset_server: Res<AssetServer>) {
    let color = match event.item {
        Item::Belt => Color::srgb(0.5, 0.5, 0.5),
        Item::Source => Color::srgb(0.2, 0.8, 0.2),
        Item::Sink => Color::srgb(0.8, 0.2, 0.2),
        Item::Rock => Color::srgb(0.55, 0.55, 0.55),
        Item::Dirt => Color::srgb(0.55, 0.35, 0.15),
        Item::IronOre => Color::srgb(0.6, 0.4, 0.3),
        Item::CopperOre => Color::srgb(0.7, 0.4, 0.15),
        Item::IronIngot => Color::srgb(0.7, 0.7, 0.75),
        Item::CopperIngot => Color::srgb(0.8, 0.5, 0.2),
        Item::Miner => Color::srgb(0.3, 0.3, 0.5),
        Item::Furnace => Color::srgb(0.8, 0.4, 0.1),
        Item::Assembler => Color::srgb(0.6, 0.4, 0.5),
    };
    let visual = cmd
        .spawn((
            SceneRoot(asset_server.load(GltfAssetLabel::Scene(0).from_asset("models/Voxel.glb"))),
            Transform::from_scale(Vec3::splat(ITEM_SIZE * 0.95)),
            SceneTint(color),
            Visibility::Hidden,
        ))
        .id();
    cmd.entity(event.entity).add_child(visual);
}
