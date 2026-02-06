use super::*;

pub struct HotbarPlugin;
impl Plugin for HotbarPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PlacementIndex>();
        let mut inv = Inventory::default();
        inv.items[0] = Some(Item(1));
        inv.items[1] = Some(Item(3));
        inv.items[2] = Some(Item(4));
        app.insert_resource(inv);

        app.add_systems(Startup, setup_hotbar);
        app.add_systems(PreUpdate, hotbar::handle_tool_selection);
        app.add_systems(Update, hotbar::update_hotbar_selection);
    }
}

#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq, Component)]
pub struct PlacementIndex {
    pub slot: usize,
}

#[derive(Resource, Default, Debug, Clone, PartialEq, Eq, Component)]
pub struct Inventory {
    pub items: [Option<Item>; 10],
}

const HOTBAR_SLOT_SIZE: f32 = 64.0;
const HOTBAR_SLOT_GAP: f32 = 8.0;
const HOTBAR_BORDER_NORMAL: Color = Color::srgba(0.3, 0.3, 0.3, 0.8);
const HOTBAR_BORDER_SELECTED: Color = Color::srgba(1.0, 0.8, 0.2, 1.0);
const HOTBAR_BG: Color = Color::srgba(0.1, 0.1, 0.1, 0.8);

fn setup_hotbar(mut cmd: Commands, registry: Res<ItemRegistry>, inv: Res<Inventory>) {
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
                left: Val::Px(
                    -((inv.items.len() as f32 * (HOTBAR_SLOT_SIZE + HOTBAR_SLOT_GAP)) / 2.0),
                ),
                column_gap: Val::Px(HOTBAR_SLOT_GAP),
                ..default()
            })
            .with_children(|parent| {
                for (index, &tool) in inv.items.iter().enumerate() {
                    let is_selected = index == 0; // Belt is default

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
                            BorderColor::all(if is_selected {
                                HOTBAR_BORDER_SELECTED
                            } else {
                                HOTBAR_BORDER_NORMAL
                            }),
                            PlacementIndex { slot: index },
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

fn handle_tool_selection(keys: Res<ButtonInput<KeyCode>>, mut tool: ResMut<PlacementIndex>) {
    for (index, key) in DIGITS.iter().enumerate() {
        if keys.just_pressed(*key) {
            *tool = PlacementIndex { slot: index };
        }
    }
}

fn update_hotbar_selection(
    tool: Res<PlacementIndex>,
    mut slots: Query<(&PlacementIndex, &mut BorderColor)>,
) {
    for (slot, mut border) in slots.iter_mut() {
        let target = BorderColor::all(if tool.slot == slot.slot {
            HOTBAR_BORDER_SELECTED
        } else {
            HOTBAR_BORDER_NORMAL
        });
        if *border != target {
            *border = target;
        }
    }
}
