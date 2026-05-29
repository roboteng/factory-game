use bevy::prelude::*;
use common::inventory::{Inventory, Stack};
use common::{Item, Player};

use super::{InteractionMode, WorldMode};

pub struct HotbarPlugin;
impl Plugin for HotbarPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreUpdate, handle_tool_selection);
    }
}

pub struct SurvivalHotbar;
impl Plugin for SurvivalHotbar {
    fn build(&self, app: &mut App) {
        let mut inv = {
            let world = app.world_mut();
            let player = world.resource::<Player>().0;
            let mut a = world.query::<&mut Inventory>();
            a.get_mut(world, player).unwrap()
        };
        inv.insert(Stack::new(Item::PickAxe, 1)).unwrap();

        let mut hotbar = [None; 10];
        hotbar[0] = Some(Item::PickAxe);
        app.insert_resource(Hotbar(hotbar));
    }
}

pub struct FreeHotbar;
impl Plugin for FreeHotbar {
    fn build(&self, app: &mut App) {
        let mut inv = {
            let world = app.world_mut();
            let player = world.resource::<Player>().0;
            let mut a = world.query::<&mut Inventory>();
            a.get_mut(world, player).unwrap()
        };
        inv.insert(Stack::new(Item::Belt, 15)).unwrap();
        inv.insert(Stack::new(Item::Source, 5)).unwrap();
        inv.insert(Stack::new(Item::Sink, 5)).unwrap();
        inv.insert(Stack::new(Item::Rock, 5)).unwrap();
        inv.insert(Stack::new(Item::Dirt, 5)).unwrap();
        inv.insert(Stack::new(Item::Furnace, 5)).unwrap();
        inv.insert(Stack::new(Item::Assembler, 5)).unwrap();
        inv.insert(Stack::new(Item::Miner, 5)).unwrap();
        inv.insert(Stack::new(Item::Collector, 5)).unwrap();
        inv.insert(Stack::new(Item::CornKernels, 10)).unwrap();
        inv.insert(Stack::new(Item::IronOre, 5)).unwrap();

        let mut hotbar = [None; 10];
        hotbar[0] = Some(Item::Belt);
        hotbar[1] = Some(Item::Source);
        hotbar[2] = Some(Item::Sink);
        hotbar[3] = Some(Item::Rock);
        hotbar[4] = Some(Item::Dirt);
        hotbar[5] = Some(Item::Furnace);
        hotbar[6] = Some(Item::Assembler);
        hotbar[7] = Some(Item::Miner);
        hotbar[8] = Some(Item::Collector);
        hotbar[9] = Some(Item::CornKernels);
        app.insert_resource(Hotbar(hotbar));
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlacementItem {
    HotbarSlot(u16),
    #[expect(dead_code)]
    Custom(Item),
}

#[derive(Resource)]
pub struct Hotbar(pub [Option<Item>; 10]);

const DIGITS: [KeyCode; 10] = [
    KeyCode::Digit1,
    KeyCode::Digit2,
    KeyCode::Digit3,
    KeyCode::Digit4,
    KeyCode::Digit5,
    KeyCode::Digit6,
    KeyCode::Digit7,
    KeyCode::Digit8,
    KeyCode::Digit9,
    KeyCode::Digit0,
];

fn handle_tool_selection(keys: Res<ButtonInput<KeyCode>>, mut mode: ResMut<InteractionMode>) {
    if matches!(*mode, InteractionMode::InScreen(_)) {
        return;
    }
    for (index, key) in DIGITS.iter().enumerate() {
        if keys.just_pressed(*key) {
            match mode.as_ref() {
                InteractionMode::InWorld(WorldMode::Placing(PlacementItem::HotbarSlot(s)))
                    if (*s as usize) == index =>
                {
                    *mode = InteractionMode::InWorld(WorldMode::None);
                }
                _ => {
                    *mode = InteractionMode::InWorld(WorldMode::Placing(
                        PlacementItem::HotbarSlot(index as u16),
                    ));
                }
            }
        }
    }
}
