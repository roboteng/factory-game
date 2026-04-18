use bevy::prelude::*;

use crate::{HDir, PlaceBlock, WorldBlock, WorldCoords, WorldCoordsDelta};

/// Describes one type of resource patch in world generation.
pub struct ResourcePatch {
    pub block: WorldBlock,
    /// Perlin seed for this resource's independent noise layer.
    pub seed: u32,
    /// Noise frequency — lower = larger patches, higher = smaller/spottier.
    pub scale: f64,
    /// Noise threshold in [0, 1] — higher = rarer (fewer tiles placed).
    pub threshold: f64,
}

/// Tunable world generation parameters for the Perlin noise world.
#[derive(Resource)]
pub struct WorldGenConfig {
    pub terrain_seed: u32,
    pub terrain_scale: f64,
    pub terrain_amplitude: f64,
    pub resources: Vec<ResourcePatch>,
}

impl Default for WorldGenConfig {
    fn default() -> Self {
        Self {
            terrain_seed: 42,
            terrain_scale: 0.05,
            terrain_amplitude: 5.0,
            resources: vec![
                ResourcePatch {
                    block: WorldBlock::IronOreDeposit,
                    seed: 100,
                    scale: 0.08,
                    threshold: 0.95,
                },
                ResourcePatch {
                    block: WorldBlock::CopperOreDeposit,
                    seed: 200,
                    scale: 0.08,
                    threshold: 0.95,
                },
                ResourcePatch {
                    block: WorldBlock::Corn,
                    seed: 300,
                    scale: 0.12,
                    threshold: 0.95,
                },
            ],
        }
    }
}

/// Spawns a small, flat 11×11 test world with hardcoded ore deposits.
/// Load this plugin instead of [`PerlinWorldPlugin`] when `--flat` is passed.
pub struct FlatWorldPlugin;

impl Plugin for FlatWorldPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (spawn_flat_terrain, spawn_flat_belts));
    }
}

/// Spawns a large Perlin-noise terrain with procedural resource patches.
/// This is the default world plugin.
pub struct PerlinWorldPlugin;

impl Plugin for PerlinWorldPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WorldGenConfig>();
        app.add_systems(Startup, spawn_perlin_terrain);
    }
}

fn spawn_flat_terrain(mut cmd: Commands) {
    use rand::Rng;
    let o = WorldCoords::ORIGIN;
    let mut rng = rand::thread_rng();
    let ground_height = WorldCoordsDelta::ZERO.height(-2);

    for ns in -5..=5 {
        for ew in -5..=5 {
            let block = if rng.gen_bool(0.5) {
                WorldBlock::Rock
            } else {
                WorldBlock::Dirt
            };
            let entity = cmd.spawn_empty().id();
            cmd.trigger(PlaceBlock {
                entity,
                block,
                coords: o + ground_height + WorldCoordsDelta::ZERO.north(ns).east(ew),
                dir: HDir::North,
            });
        }
    }

    for (ns, ew, block) in [
        (6i32, 0i32, WorldBlock::IronOreDeposit),
        (6, 1, WorldBlock::IronOreDeposit),
        (6, -1, WorldBlock::IronOreDeposit),
        (-6, 0, WorldBlock::CopperOreDeposit),
        (-6, 1, WorldBlock::CopperOreDeposit),
    ] {
        let entity = cmd.spawn_empty().id();
        cmd.trigger(PlaceBlock {
            entity,
            block,
            coords: o + WorldCoordsDelta::ZERO.north(ns).east(ew),
            dir: HDir::North,
        });
    }
}

fn spawn_flat_belts(mut cmd: Commands) {
    use crate::Dir;
    let o = WorldCoords::ORIGIN;

    let entity = cmd.spawn_empty().id();
    cmd.trigger(PlaceBlock {
        entity,
        block: WorldBlock::Belt,
        coords: o.step(HDir::North).step(Dir::Up),
        dir: HDir::North,
    });
    let entity = cmd.spawn_empty().id();
    cmd.trigger(PlaceBlock {
        entity,
        block: WorldBlock::Belt,
        coords: o,
        dir: HDir::North,
    });
    let entity = cmd.spawn_empty().id();
    cmd.trigger(PlaceBlock {
        entity,
        block: WorldBlock::Belt,
        coords: o.step(HDir::South),
        dir: HDir::North,
    });
    let entity = cmd.spawn_empty().id();
    cmd.trigger(PlaceBlock {
        entity,
        block: WorldBlock::Furnace,
        coords: o.step(WorldCoordsDelta::ZERO.west(3)),
        dir: HDir::North,
    });
}

fn spawn_perlin_terrain(mut cmd: Commands, config: Res<WorldGenConfig>) {
    use noise::{NoiseFn, Perlin};
    let o = WorldCoords::ORIGIN;
    let terrain_noise = Perlin::new(config.terrain_seed);
    let resource_noises: Vec<(&ResourcePatch, Perlin)> = config
        .resources
        .iter()
        .map(|r| (r, Perlin::new(r.seed)))
        .collect();

    for ns in -50_i32..=50 {
        for ew in -50_i32..=50 {
            let noise_val = terrain_noise.get([
                ns as f64 * config.terrain_scale,
                ew as f64 * config.terrain_scale,
            ]);
            let height_full = (noise_val * config.terrain_amplitude).round() as i32;
            let height_half = height_full * 2;

            // Always place the base terrain block.
            let terrain_block = if height_half <= 0 {
                WorldBlock::Rock
            } else {
                WorldBlock::Dirt
            };
            let entity = cmd.spawn_empty().id();
            cmd.trigger(PlaceBlock {
                entity,
                block: terrain_block,
                coords: o + WorldCoordsDelta::ZERO
                    .height(height_half)
                    .north(ns)
                    .east(ew),
                dir: HDir::North,
            });

            // If a resource patch covers this tile, place it on top of the terrain.
            let resource = resource_noises
                .iter()
                .filter_map(|(patch, noise)| {
                    let v = noise.get([ns as f64 * patch.scale, ew as f64 * patch.scale]);
                    let normalized = (v + 1.0) / 2.0;
                    if normalized >= patch.threshold {
                        Some((normalized, patch.block))
                    } else {
                        None
                    }
                })
                .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap())
                .map(|(_, b)| b);

            if let Some(block) = resource {
                let entity = cmd.spawn_empty().id();
                cmd.trigger(PlaceBlock {
                    entity,
                    block,
                    // One full block (2 half-steps) above the terrain surface.
                    coords: o + WorldCoordsDelta::ZERO
                        .height(height_half + 2)
                        .north(ns)
                        .east(ew),
                    dir: HDir::North,
                });
            }
        }
    }
}
