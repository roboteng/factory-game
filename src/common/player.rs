use bevy::prelude::*;

use crate::common::{MachineStatus, Recipe, inventory::Inventory};

#[derive(Resource)]
pub struct Player(pub Entity);

#[derive(Component, Default, Reflect)]
#[reflect(Component)]
pub struct HandCrafter {
    #[reflect(ignore)]
    pub status: MachineStatus<Recipe>,
}

pub fn spawn_player(world: &mut World) {
    let entity = world.spawn((Inventory::new(), HandCrafter::default())).id();
    world.insert_resource(Player(entity));
}

pub fn process_hand_crafter(
    player: Res<Player>,
    mut q: Query<(&mut HandCrafter, &mut Inventory)>,
) {
    let Ok((mut crafter, mut inventory)) = q.get_mut(player.0) else {
        return;
    };
    let MachineStatus::Processing {
        recipe,
        elapsed_ticks,
    } = &mut crafter.status
    else {
        return;
    };
    *elapsed_ticks += 1;
    if *elapsed_ticks >= recipe.ticks() {
        let outputs = recipe.outputs();
        crafter.status = MachineStatus::Idle;
        for s in outputs {
            inventory.insert(s).unwrap();
        }
    }
}
