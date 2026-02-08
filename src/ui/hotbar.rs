use std::u16;

use super::*;

pub struct HotbarPlugin;
impl Plugin for HotbarPlugin {
    fn build(&self, app: &mut App) {
        let mut hotbar = [None; 10];
        hotbar[0] = Some(Item(1));
        hotbar[1] = Some(Item(3));
        hotbar[2] = Some(Item(4));
        app.insert_resource(Hotbar(hotbar));
        app.insert_resource(PlacementItem::None);

        app.add_systems(Startup, setup_hotbar);
        app.add_systems(PreUpdate, handle_tool_selection);
        app.add_systems(Update, update_hotbar_selection);
    }
}

#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlacementItem {
    HotbarSlot(u16),
    Custom(Item),
    None,
}

#[derive(Resource)]
pub struct Hotbar(pub [Option<Item>; 10]);

#[derive(Component)]
struct HotbarSlot(u16);

const HOTBAR_SLOT_SIZE: f32 = 64.0;
const HOTBAR_SLOT_GAP: f32 = 8.0;
const HOTBAR_BORDER_NORMAL: Color = Color::srgba(0.3, 0.3, 0.3, 0.8);
const HOTBAR_BORDER_SELECTED: Color = Color::srgba(1.0, 0.8, 0.2, 1.0);
const HOTBAR_BG: Color = Color::srgba(0.1, 0.1, 0.1, 0.8);

fn setup_hotbar(mut cmd: Commands, registry: Res<ItemRegistry>, inv: Res<Hotbar>) {
    // Root container at bottom center
    cmd.spawn(Node {
        position_type: PositionType::Absolute,
        bottom: Val::Px(20.0),
        left: Val::Percent(50.0),
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        ..default()
    })
    .with_children(|parent| {
        // Inner container offset to center the hotbar
        parent
            .spawn(Node {
                position_type: PositionType::Relative,
                left: Val::Px(-((10.0 * (HOTBAR_SLOT_SIZE + HOTBAR_SLOT_GAP)) / 2.0)),
                column_gap: Val::Px(HOTBAR_SLOT_GAP),
                ..default()
            })
            .with_children(|parent| {
                for (index, &tool) in inv.0.iter().enumerate() {
                    // Slot container
                    parent
                        .spawn((
                            Node {
                                width: Val::Px(HOTBAR_SLOT_SIZE),
                                height: Val::Px(HOTBAR_SLOT_SIZE),
                                border: UiRect::all(Val::Px(3.0)),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                ..default()
                            },
                            BackgroundColor(HOTBAR_BG),
                            BorderColor::all(HOTBAR_BORDER_NORMAL),
                            HotbarSlot(index as u16),
                        ))
                        .with_children(|parent| {
                            let Some(tool) = tool else { return };
                            // Slot number label
                            parent.spawn((
                                Text::new(format!("{}", index + 1)),
                                TextFont {
                                    font_size: 14.0,
                                    ..default()
                                },
                                TextColor(Color::WHITE),
                                Node {
                                    position_type: PositionType::Absolute,
                                    top: Val::Px(2.0),
                                    left: Val::Px(4.0),
                                    ..default()
                                },
                            ));

                            // Tool name label
                            let name = registry.get(&tool).expect("Item not in registry").name;
                            parent.spawn((
                                Text::new(name),
                                TextFont {
                                    font_size: 12.0,
                                    ..default()
                                },
                                TextColor(Color::WHITE),
                            ));
                        });
                }
            });
    });
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

fn handle_tool_selection(keys: Res<ButtonInput<KeyCode>>, mut tool: ResMut<PlacementItem>) {
    for (index, key) in DIGITS.iter().enumerate() {
        if keys.just_pressed(*key) {
            *tool = PlacementItem::HotbarSlot(index as u16);
        }
    }
}

fn update_hotbar_selection(
    tool: Res<PlacementItem>,
    mut slots: Query<(&HotbarSlot, &mut BorderColor)>,
) {
    let PlacementItem::HotbarSlot(selected_slot) = *tool else {
        for (_, mut border) in slots.iter_mut() {
            let target = HOTBAR_BORDER_NORMAL;
            *border = BorderColor::all(target);
        }
        return;
    };
    for (slot, mut border) in slots.iter_mut() {
        let target = BorderColor::all(if selected_slot == slot.0 {
            HOTBAR_BORDER_SELECTED
        } else {
            HOTBAR_BORDER_NORMAL
        });
        *border = target;
    }
}
