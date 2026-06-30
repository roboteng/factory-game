//! PNG snapshot tests for the screen UIs in `views`.
//!
//! Each `#[test]` renders one screen variant headlessly and compares it against a committed
//! reference under `tests/snapshots/`. To add coverage:
//!   - **new screen**: add a module with its own tests calling `snapshot("screen", ...)`.
//!   - **new variant**: add a `#[test]` with a `screen__variant` name and a different fixture.
//! See `tests/support/mod.rs` for the harness and the accept (`UPDATE_SNAPSHOTS=1`) workflow.

mod support;

use bevy::prelude::*; // brings `Commands::spawn_scene` (SpawnSceneExt) into scope
use common::{
    inventory::{Inventory, Stack},
    Item,
};
use support::snapshot;

mod hotbar {
    use super::*;
    use gui::{
        hotbar::{Hotbar, PlacementItem},
        InteractionMode, WorldMode,
    };
    use views::hotbar::hotbar_scene;

    /// Mirrors the fixture in `views/examples/hotbar.rs`.
    #[test]
    fn placing_rock() {
        let mut hotbar = Hotbar(Default::default());
        hotbar.0[0] = Some(Item::PickAxe);
        hotbar.0[1] = Some(Item::IronOre);
        hotbar.0[2] = Some(Item::Rock);
        hotbar.0[3] = Some(Item::Dirt);
        hotbar.0[4] = Some(Item::Rock);

        let mut inventory = Inventory::new();
        inventory.insert(Item::PickAxe.into()).unwrap();
        inventory
            .insert(Stack {
                item: Item::Dirt,
                count: 3,
            })
            .unwrap();

        let mode = InteractionMode::InWorld(WorldMode::Placing(PlacementItem::Custom(Item::Rock)));

        snapshot("hotbar__placing_rock", move |cmd| {
            cmd.spawn_scene(hotbar_scene(&hotbar, &inventory, &mode));
        });
    }
}

mod inventory {
    use super::*;
    use views::inventory::inventory_scene;

    /// Mirrors the fixture in `views/examples/inventory.rs`.
    #[test]
    fn two_stacks() {
        let mut inv = Inventory::new();
        inv.insert(Stack {
            item: Item::Furnace,
            count: 3,
        })
        .unwrap();
        inv.insert(Stack {
            item: Item::Belt,
            count: 4,
        })
        .unwrap();

        snapshot("inventory__two_stacks", move |cmd| {
            cmd.spawn_scene(inventory_scene(&inv));
        });
    }
}
