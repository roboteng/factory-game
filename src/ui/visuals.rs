use bevy::prelude::*;

use crate::common::{BeltShape, Corn, Item, PlaceItem, Structure, ITEM_SIZE};

use crate::ui::player_controller::NeedsGhostTint;

pub struct VisualsPlugin;
impl Plugin for VisualsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ClearColor(Color::srgb(0.01, 0.01, 0.05))); // Dark night sky
        app.add_systems(Startup, setup_models);
        app.add_systems(Update, (attach_models, attach_corn_models));
        app.add_systems(Update, tint_ore_meshes);
        app.add_systems(Update, tint_ghost_children);
        app.add_observer(on_place_item);
    }
}

enum ModelDef {
    Scene(Handle<Scene>),
    TintedScene(Handle<Scene>, Color),
    Random(Vec<ModelDef>),
}

enum ItemModelDef {
    Color(Color, f32),
    Mesh(Handle<Scene>, f32),
    TintedMesh {
        scene: Handle<Scene>,
        color: Color,
        scale: f32,
        metallic: f32,
        roughness: f32,
    },
}

impl Item {
    fn model(self, asset_server: &AssetServer) -> ItemModelDef {
        use Item::*;
        match self {
            Belt => ItemModelDef::Mesh(
                asset_server.load(GltfAssetLabel::Scene(2).from_asset("models/belt.glb")),
                ITEM_SIZE,
            ),
            Source => ItemModelDef::Color(Color::srgb(0.2, 0.8, 0.2), ITEM_SIZE),
            Sink => ItemModelDef::Color(Color::srgb(0.8, 0.2, 0.2), ITEM_SIZE),
            Rock => ItemModelDef::Color(Color::srgb(0.55, 0.55, 0.55), ITEM_SIZE),
            Dirt => ItemModelDef::Color(Color::srgb(0.55, 0.35, 0.15), ITEM_SIZE),
            IronOre => ItemModelDef::TintedMesh {
                scene: asset_server.load(GltfAssetLabel::Scene(0).from_asset("models/Rubble.glb")),
                color: Color::srgb(0.5, 0.4, 0.35),
                scale: 1.0,
                metallic: 0.0,
                roughness: 0.9,
            },
            CopperOre => ItemModelDef::TintedMesh {
                scene: asset_server.load(GltfAssetLabel::Scene(0).from_asset("models/Rubble.glb")),
                color: Color::srgb(0.65, 0.35, 0.1),
                scale: 1.0,
                metallic: 0.0,
                roughness: 0.9,
            },
            IronIngot => ItemModelDef::TintedMesh {
                scene: asset_server.load(GltfAssetLabel::Scene(0).from_asset("models/Ingot.glb")),
                color: Color::srgb(0.7, 0.7, 0.75),
                scale: 1.0,
                metallic: 0.9,
                roughness: 0.2,
            },
            CopperIngot => ItemModelDef::TintedMesh {
                scene: asset_server.load(GltfAssetLabel::Scene(0).from_asset("models/Ingot.glb")),
                color: Color::srgb(0.8, 0.5, 0.2),
                scale: 1.0,
                metallic: 0.9,
                roughness: 0.2,
            },
            IronPlate => ItemModelDef::Mesh(
                asset_server.load(GltfAssetLabel::Scene(0).from_asset("models/IronPlate.glb")),
                ITEM_SIZE,
            ),
            IronRod => ItemModelDef::Mesh(
                asset_server.load(GltfAssetLabel::Scene(0).from_asset("models/IronRod.glb")),
                ITEM_SIZE,
            ),
            CopperWire => ItemModelDef::Mesh(
                asset_server.load(GltfAssetLabel::Scene(0).from_asset("models/CopperWire.glb")),
                ITEM_SIZE,
            ),
            Circuit => ItemModelDef::Mesh(
                asset_server.load(GltfAssetLabel::Scene(0).from_asset("models/Circuit.glb")),
                ITEM_SIZE,
            ),
            Miner => ItemModelDef::Mesh(
                asset_server.load(GltfAssetLabel::Scene(0).from_asset("models/Miner.glb")),
                ITEM_SIZE,
            ),
            Furnace => ItemModelDef::Mesh(
                asset_server.load(GltfAssetLabel::Scene(0).from_asset("models/Furnace.glb")),
                ITEM_SIZE * 0.5,
            ),
            Assembler => ItemModelDef::Mesh(
                asset_server.load(GltfAssetLabel::Scene(0).from_asset("models/Assembler.glb")),
                ITEM_SIZE * 0.4,
            ),
            Collector => ItemModelDef::Mesh(
                asset_server.load(GltfAssetLabel::Scene(0).from_asset("models/Collector.glb")),
                ITEM_SIZE,
            ),
            CornKernels => ItemModelDef::Color(Color::srgb(0.95, 0.85, 0.2), ITEM_SIZE),
            CornStalk => ItemModelDef::Color(Color::srgb(0.3, 0.7, 0.2), ITEM_SIZE),
            Biomass => ItemModelDef::Color(Color::srgb(0.3, 0.5, 0.15), ITEM_SIZE),
            Gear => ItemModelDef::Mesh(
                asset_server.load(GltfAssetLabel::Scene(0).from_asset("models/Gear.glb")),
                1.0,
            ),
        }
    }
}

#[derive(Component)]
pub struct SceneTint {
    pub color: Color,
    pub metallic: f32,
    pub roughness: f32,
}

#[derive(Resource)]
pub struct BlockModels {
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
    collector: ModelDef,
    corn_stage_a: ModelDef,
    corn_stage_b: ModelDef,
    corn_stage_c: ModelDef,
    corn_stage_d: ModelDef,
}

fn setup_models(mut cmd: Commands, asset_server: Res<AssetServer>) {
    let block = asset_server.load(GltfAssetLabel::Scene(0).from_asset("models/Block.glb"));
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
        source: ModelDef::TintedScene(block.clone(), Color::srgb(0.2, 0.8, 0.2)),
        sink: ModelDef::TintedScene(block.clone(), Color::srgb(0.8, 0.2, 0.2)),
        rock: ModelDef::Random(
            (0..4)
                .map(|i| {
                    ModelDef::Scene(
                        asset_server.load(GltfAssetLabel::Scene(i).from_asset("models/Rock.glb")),
                    )
                })
                .collect(),
        ),
        dirt: ModelDef::Random(
            (0..4)
                .map(|i| {
                    ModelDef::Scene(
                        asset_server.load(GltfAssetLabel::Scene(i).from_asset("models/Dirt.glb")),
                    )
                })
                .collect(),
        ),
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
            asset_server.load(GltfAssetLabel::Scene(0).from_asset("models/Assembler.glb")),
        ),
        collector: ModelDef::Scene(
            asset_server.load(GltfAssetLabel::Scene(0).from_asset("models/Collector.glb")),
        ),
        corn_stage_a: ModelDef::Scene(asset_server.load(
            GltfAssetLabel::Scene(0).from_asset("models/kenney_nature_kit/crops_cornStageA.glb"),
        )),
        corn_stage_b: ModelDef::Scene(asset_server.load(
            GltfAssetLabel::Scene(0).from_asset("models/kenney_nature_kit/crops_cornStageB.glb"),
        )),
        corn_stage_c: ModelDef::Scene(asset_server.load(
            GltfAssetLabel::Scene(0).from_asset("models/kenney_nature_kit/crops_cornStageC.glb"),
        )),
        corn_stage_d: ModelDef::Scene(asset_server.load(
            GltfAssetLabel::Scene(0).from_asset("models/kenney_nature_kit/crops_cornStageD.glb"),
        )),
    });
}

impl BlockModels {
    pub fn ghost_scene(&self, item: Item) -> Option<Handle<Scene>> {
        let model = match item.can_place()? {
            Structure::Belt => &self.belt_straight,
            Structure::Source => &self.source,
            Structure::Sink => &self.sink,
            Structure::Rock => &self.rock,
            Structure::Dirt => &self.dirt,
            Structure::Miner => &self.miner,
            Structure::Furnace => &self.furnace,
            Structure::Assembler => &self.assembler,
            Structure::Collector => &self.collector,
            _ => return None,
        };
        fn mdl(model: &ModelDef) -> Option<Handle<Scene>> {
            match model {
                ModelDef::Scene(handle) => Some(handle.clone()),
                ModelDef::TintedScene(handle, _) => Some(handle.clone()),
                ModelDef::Random(model_defs) => model_defs.get(0).and_then(mdl),
            }
        }
        mdl(model)
    }
}

fn apply_model(entity: Entity, mut cmd: Commands, model: &ModelDef) {
    match model {
        ModelDef::Scene(handle) => {
            cmd.entity(entity).insert(SceneRoot(handle.clone()));
        }
        ModelDef::TintedScene(handle, color) => {
            cmd.entity(entity).insert((
                SceneRoot(handle.clone()),
                SceneTint {
                    color: *color,
                    metallic: 0.0,
                    roughness: 0.8,
                },
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
    for (entity, tint) in &tinted {
        let mut found_any = false;
        for desc in children_q.iter_descendants(entity) {
            let Ok(mat_handle) = mesh_mat_q.get(desc) else {
                continue;
            };
            let Some(mat) = materials.get(mat_handle.id()) else {
                continue;
            };
            let mut new_mat = mat.clone();
            new_mat.base_color = tint.color;
            new_mat.metallic = tint.metallic;
            new_mat.perceptual_roughness = tint.roughness;
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
        (Entity, &Structure, Option<&BeltShape>),
        Or<(Added<Structure>, Changed<BeltShape>)>,
    >,
    all_models: Res<BlockModels>,
    mut cmd: Commands,
) {
    for (entity, block, shape) in &world_blocks {
        let model = match block {
            Structure::Corn => continue, // handled by attach_corn_models
            Structure::Source => &all_models.source,
            Structure::Sink => &all_models.sink,
            Structure::Rock => &all_models.rock,
            Structure::Dirt => &all_models.dirt,
            Structure::IronOreDeposit => &all_models.iron_ore,
            Structure::CopperOreDeposit => &all_models.copper_ore,
            Structure::Miner => &all_models.miner,
            Structure::Furnace => &all_models.furnace,
            Structure::Assembler => &all_models.assembler,
            Structure::Collector => &all_models.collector,
            Structure::Belt => match shape {
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

/// Applies semi-transparent tinting to all mesh children of a ghost entity.
/// Keeps retrying (by not removing `NeedsGhostTint`) until the scene children are ready.
fn tint_ghost_children(
    mut cmd: Commands,
    ghosts: Query<(Entity, &NeedsGhostTint)>,
    children_q: Query<&Children>,
    mesh_mat_q: Query<&MeshMaterial3d<StandardMaterial>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (entity, NeedsGhostTint(color)) in &ghosts {
        let mut found_any = false;
        for desc in children_q.iter_descendants(entity) {
            let Ok(mat_handle) = mesh_mat_q.get(desc) else {
                continue;
            };
            let Some(mat) = materials.get(mat_handle.id()) else {
                continue;
            };
            let lin = LinearRgba::from(*color);
            let mut new_mat = mat.clone();
            // Keep base_color white so the texture (e.g. belt direction arrow) stays visible.
            // Use emissive for the blue/red indication so it shows regardless of texture darkness.
            new_mat.base_color = Color::srgba(1.0, 1.0, 1.0, lin.alpha);
            new_mat.emissive = LinearRgba::new(lin.red * 0.7, lin.green * 0.7, lin.blue * 0.7, 1.0);
            new_mat.alpha_mode = AlphaMode::Blend;
            new_mat.depth_bias = 100.0;
            let new_handle = materials.add(new_mat);
            cmd.entity(desc).insert(MeshMaterial3d(new_handle));
            found_any = true;
        }
        if found_any {
            cmd.entity(entity).remove::<NeedsGhostTint>();
        }
    }
}

fn on_place_item(event: On<PlaceItem>, mut cmd: Commands, asset_server: Res<AssetServer>) {
    let model = event.item.model(&asset_server);

    let visual = match &model {
        ItemModelDef::Color(color, scale) => cmd
            .spawn((
                SceneRoot(
                    asset_server.load(GltfAssetLabel::Scene(0).from_asset("models/Block.glb")),
                ),
                Transform::from_scale(Vec3::splat(scale * 0.95)),
                SceneTint {
                    color: *color,
                    metallic: 0.0,
                    roughness: 0.8,
                },
                Visibility::Hidden,
            ))
            .id(),
        ItemModelDef::Mesh(handle, scale) => cmd
            .spawn((
                SceneRoot(handle.clone()),
                Transform::from_scale(Vec3::splat(scale * 0.95)),
            ))
            .id(),
        ItemModelDef::TintedMesh {
            scene,
            color,
            scale,
            metallic,
            roughness,
        } => cmd
            .spawn((
                SceneRoot(scene.clone()),
                Transform::from_scale(Vec3::splat(scale * 0.95)),
                SceneTint {
                    color: *color,
                    metallic: *metallic,
                    roughness: *roughness,
                },
                Visibility::Hidden,
            ))
            .id(),
    };

    cmd.entity(event.entity)
        .insert(Visibility::Inherited)
        .add_child(visual);
}

fn attach_corn_models(
    corns: Query<(Entity, &Corn), Changed<Corn>>,
    all_models: Res<BlockModels>,
    corn_ticks: Res<crate::common::CornGrowthTicks>,
    mut cmd: Commands,
) {
    for (entity, corn) in &corns {
        let model = match corn.visual_stage(corn_ticks.0) {
            0 => &all_models.corn_stage_a,
            1 => &all_models.corn_stage_b,
            2 => &all_models.corn_stage_c,
            _ => &all_models.corn_stage_d,
        };
        apply_model(entity, cmd.reborrow(), model);
    }
}
