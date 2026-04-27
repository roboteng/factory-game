use bevy::prelude::*;
use factory_core::{inventory::Inventory, *};

use super::common::{InventorySlot, StackView, SLOT_BG, SLOT_BORDER, SLOT_FONT_SIZE, SLOT_SIZE};
use super::{InteractionMode, WorldMode};

pub struct HotbarPlugin;
impl Plugin for HotbarPlugin {
    fn build(&self, app: &mut App) {
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

        app.add_systems(Startup, setup_hotbar);
        app.add_systems(PreUpdate, handle_tool_selection);
        app.add_systems(Update, update_hotbar_selection);
        app.add_systems(Update, update_hotbar_counts);
        app.add_systems(Update, handle_hotbar_assignment);
        app.add_systems(Update, update_hotbar_items);
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

#[derive(Component)]
struct HotbarSlot(u16);

#[derive(Component)]
struct HotbarSlotCount(u16);

#[derive(Component)]
struct HotbarSlotText(u16);

const HOTBAR_SLOT_GAP: f32 = 8.0;
const HOTBAR_BORDER_SELECTED: Color = Color::srgba(1.0, 0.8, 0.2, 1.0);
const TEXT_COLOR_NORMAL: Color = Color::WHITE;
const TEXT_COLOR_EMPTY: Color = Color::srgba(0.4, 0.4, 0.4, 1.0);

fn setup_hotbar(mut cmd: Commands, inv: Res<Hotbar>) {
    cmd.spawn(Node {
        position_type: PositionType::Absolute,
        bottom: Val::Px(20.0),
        width: Val::Percent(100.0),
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        column_gap: Val::Px(HOTBAR_SLOT_GAP),
        ..default()
    })
    .with_children(|parent| {
        for (index, &tool) in inv.0.iter().enumerate() {
            spawn_hotbar_slot(parent, index, tool);
        }
    });
}

fn spawn_hotbar_slot(parent: &mut ChildSpawnerCommands, index: usize, tool: Option<Item>) {
    parent
        .spawn((
            Node {
                width: Val::Px(SLOT_SIZE),
                height: Val::Px(SLOT_SIZE),
                border: UiRect::all(Val::Px(3.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(SLOT_BG),
            BorderColor::all(SLOT_BORDER),
            StackView,
            HotbarSlot(index as u16),
        ))
        .with_children(|parent| {
            spawn_slot_children(parent, index, tool);
        });
}

fn spawn_slot_children(parent: &mut ChildSpawnerCommands, index: usize, tool: Option<Item>) {
    let Some(tool) = tool else { return };
    parent.spawn((
        Text::new(format!("{}", index + 1)),
        TextFont {
            font_size: 14.0,
            ..default()
        },
        TextColor(TEXT_COLOR_NORMAL),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(2.0),
            left: Val::Px(4.0),
            ..default()
        },
        HotbarSlotText(index as u16),
    ));

    parent.spawn((
        Text::new(tool.name()),
        TextFont {
            font_size: SLOT_FONT_SIZE,
            ..default()
        },
        TextColor(TEXT_COLOR_NORMAL),
        HotbarSlotText(index as u16),
    ));

    parent.spawn((
        Text::new("0"),
        TextFont {
            font_size: SLOT_FONT_SIZE,
            ..default()
        },
        TextColor(TEXT_COLOR_NORMAL),
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(2.0),
            right: Val::Px(4.0),
            ..default()
        },
        HotbarSlotCount(index as u16),
        HotbarSlotText(index as u16),
    ));
}

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

fn update_hotbar_selection(
    mode: Res<InteractionMode>,
    mut slots: Query<(&HotbarSlot, &mut BorderColor)>,
) {
    let selected_slot = match &*mode {
        InteractionMode::InWorld(WorldMode::Placing(PlacementItem::HotbarSlot(slot))) => Some(slot),
        _ => None,
    };
    for (slot, mut border) in slots.iter_mut() {
        let target = BorderColor::all(if selected_slot == Some(&slot.0) {
            HOTBAR_BORDER_SELECTED
        } else {
            SLOT_BORDER
        });
        *border = target;
    }
}

fn update_hotbar_counts(
    player: Res<Player>,
    hotbar: Res<Hotbar>,
    invs: Query<&Inventory>,
    mut counts: Query<(&HotbarSlotCount, &mut Text)>,
    mut texts: Query<(&HotbarSlotText, &mut TextColor)>,
) {
    let Ok(inv) = invs.get(player.0) else { return };
    for (slot, mut text) in counts.iter_mut() {
        if let Some(Some(item)) = hotbar.0.get(slot.0 as usize) {
            let count = inv.item_count(*item);
            text.0 = count.to_string();
        }
    }
    for (slot, mut color) in texts.iter_mut() {
        let out_of_stock = hotbar
            .0
            .get(slot.0 as usize)
            .is_some_and(|s| s.is_some_and(|item| inv.item_count(item) == 0));
        *color = TextColor(if out_of_stock {
            TEXT_COLOR_EMPTY
        } else {
            TEXT_COLOR_NORMAL
        });
    }
}

fn handle_hotbar_assignment(
    keys: Res<ButtonInput<KeyCode>>,
    mode: Res<InteractionMode>,
    player: Res<Player>,
    inventories: Query<&Inventory>,
    slots: Query<(&Interaction, &InventorySlot)>,
    mut hotbar: ResMut<Hotbar>,
) {
    if !matches!(*mode, InteractionMode::InScreen(_)) {
        return;
    }

    let Some(index) = DIGITS
        .iter()
        .enumerate()
        .find_map(|(i, key)| keys.just_pressed(*key).then_some(i))
    else {
        return;
    };

    let hovered_slot = slots.iter().find_map(|(interaction, slot)| {
        matches!(interaction, Interaction::Hovered | Interaction::Pressed).then_some(slot.0)
    });

    hotbar.0[index] = hovered_slot
        .and_then(|idx| inventories.get(player.0).ok()?.get(idx))
        .map(|s| s.item);
}

fn update_hotbar_items(
    hotbar: Res<Hotbar>,
    mut commands: Commands,
    slots: Query<(Entity, &HotbarSlot)>,
) {
    if !hotbar.is_changed() {
        return;
    }
    for (entity, slot) in &slots {
        commands.entity(entity).despawn_children();
        let item = hotbar.0.get(slot.0 as usize).copied().flatten();
        commands.entity(entity).with_children(|parent| {
            spawn_slot_children(parent, slot.0 as usize, item);
        });
    }
}
