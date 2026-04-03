use bevy::prelude::*;

// ── Theme ─────────────────────────────────────────────────────────────────────

pub(super) const PANE_BG: Color = Color::srgba(0.08, 0.08, 0.12, 0.95);
pub(super) const CLOSE_BTN_BG: Color = Color::srgba(0.3, 0.1, 0.1, 0.9);
pub(super) const SLOT_BG: Color = Color::srgba(0.1, 0.1, 0.1, 0.8);
pub(super) const SLOT_BORDER: Color = Color::srgba(0.3, 0.3, 0.3, 0.8);
pub(super) const LABEL_COLOR: Color = Color::srgba(0.7, 0.7, 0.7, 1.0);

pub(super) const SLOT_SIZE: f32 = 64.0;
pub(super) const CLOSE_BTN_SIZE: f32 = 32.0;
pub(super) const TITLE_FONT_SIZE: f32 = 20.0;
pub(super) const LABEL_FONT_SIZE: f32 = 14.0;
pub(super) const SLOT_FONT_SIZE: f32 = 12.0;
pub(super) const CLOSE_BTN_FONT_SIZE: f32 = 16.0;

pub(super) const PLAYER_INVENTORY_SLOTS: u16 = 64;

// ── Shared component ──────────────────────────────────────────────────────────

/// Marks a UI button slot that displays a player inventory slot.
/// Shared by the inventory screen and the furnace screen's inventory panel.
#[derive(Component)]
pub(super) struct InventorySlot(pub(super) u16);

// ── Layout helpers ────────────────────────────────────────────────────────────

/// Absolute-positioned column pane, hidden by default.
/// Callers add their own marker component after spawning.
pub(super) fn pane_node(
    left: Val,
    right: Val,
    top: Val,
    bottom: Val,
) -> (Node, BackgroundColor, Visibility) {
    (
        Node {
            position_type: PositionType::Absolute,
            left,
            right,
            top,
            bottom,
            flex_direction: FlexDirection::Column,
            ..default()
        },
        BackgroundColor(PANE_BG),
        Visibility::Hidden,
    )
}

/// Title bar row: screen title on the left, close button on the right.
pub(super) fn spawn_title_bar(
    parent: &mut ChildSpawnerCommands,
    title: &str,
    close_marker: impl Bundle,
) {
    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            padding: UiRect::axes(Val::Px(16.0), Val::Px(12.0)),
            ..default()
        })
        .with_children(|parent| {
            parent.spawn((
                Text::new(title),
                TextFont {
                    font_size: TITLE_FONT_SIZE,
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
            parent
                .spawn((
                    Button,
                    Node {
                        width: Val::Px(CLOSE_BTN_SIZE),
                        height: Val::Px(CLOSE_BTN_SIZE),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(CLOSE_BTN_BG),
                    close_marker,
                ))
                .with_children(|parent| {
                    parent.spawn((
                        Text::new("X"),
                        TextFont {
                            font_size: CLOSE_BTN_FONT_SIZE,
                            ..default()
                        },
                        TextColor(Color::WHITE),
                    ));
                });
        });
}

/// 64×64 button slot with a dark background, border, and an empty text child.
pub(super) fn spawn_slot(parent: &mut ChildSpawnerCommands, marker: impl Bundle) {
    parent
        .spawn((
            Button,
            Node {
                width: Val::Px(SLOT_SIZE),
                height: Val::Px(SLOT_SIZE),
                border: UiRect::all(Val::Px(2.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                margin: UiRect::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(SLOT_BG),
            BorderColor::all(SLOT_BORDER),
            marker,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new(""),
                TextFont {
                    font_size: SLOT_FONT_SIZE,
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
        });
}

/// Standard two-panel screen layout: title bar, then a 40/60 content row.
///
/// `left_content` fills the 40% left panel (use `|_| {}` for an empty placeholder).
/// `inventory_marker` is passed through to `spawn_inventory_panel` for the 60% right panel.
pub(super) fn spawn_screen_layout<M: Bundle + Clone>(
    parent: &mut ChildSpawnerCommands,
    title: &str,
    close_marker: impl Bundle,
    left_content: impl FnOnce(&mut ChildSpawnerCommands),
    inventory_marker: M,
) {
    spawn_title_bar(parent, title, close_marker);
    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            flex_grow: 1.0,
            ..default()
        })
        .with_children(|parent| {
            parent
                .spawn(Node {
                    width: Val::Percent(40.0),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(16.0)),
                    ..default()
                })
                .with_children(left_content);
            parent
                .spawn(Node {
                    width: Val::Percent(60.0),
                    flex_direction: FlexDirection::Column,
                    ..default()
                })
                .with_children(|parent| {
                    spawn_inventory_panel(parent, inventory_marker);
                });
        });
}

/// Inventory panel: "Inventory" section label + wrapped grid of 64 player inventory slots.
///
/// `extra_marker` is cloned onto every slot — use `()` when no extra tagging is needed,
/// or e.g. `FurnaceInventoryPanel` to scope click handlers to a specific screen.
pub(super) fn spawn_inventory_panel<M: Bundle + Clone>(
    parent: &mut ChildSpawnerCommands,
    extra_marker: M,
) {
    section_label(parent, "Inventory");
    parent
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            padding: UiRect::all(Val::Px(16.0)),
            ..default()
        })
        .with_children(|parent| {
            for i in 0..PLAYER_INVENTORY_SLOTS {
                spawn_slot(parent, (InventorySlot(i), extra_marker.clone()));
            }
        });
}

/// Muted section-label text (e.g. "Input", "Inventory").
pub(super) fn section_label(parent: &mut ChildSpawnerCommands, label: &str) {
    parent.spawn((
        Text::new(label),
        TextFont {
            font_size: LABEL_FONT_SIZE,
            ..default()
        },
        TextColor(LABEL_COLOR),
        Node {
            margin: UiRect::axes(Val::Px(4.0), Val::Px(8.0)),
            ..default()
        },
    ));
}
