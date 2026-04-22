use bevy::input::mouse::{AccumulatedMouseScroll, MouseScrollUnit};
use bevy::prelude::*;
use bevy::ui::ComputedNode;

use factory_core::{Item, SetSourceItem};

use super::common::{
    LABEL_FONT_SIZE, PANE_BG, SLOT_BG, SLOT_BORDER, section_label, spawn_title_bar,
};
use super::{InteractionMode, ScreenMode};

#[derive(Component)]
pub(super) struct SourcePane;

#[derive(Component)]
pub(super) struct SourceScrollList;

#[derive(Component)]
pub(super) struct CloseSourceButton;

/// Button for selecting a specific item to produce.
#[derive(Component, Clone, Copy)]
pub(super) struct SourceItemButton(pub(super) Item);

/// All items a source can be configured to produce.
const ALL_ITEMS: &[Item] = &[
    Item::Belt,
    Item::Source,
    Item::Sink,
    Item::Rock,
    Item::Dirt,
    Item::IronOre,
    Item::CopperOre,
    Item::IronIngot,
    Item::CopperIngot,
    Item::Miner,
    Item::Furnace,
    Item::Assembler,
    Item::Collector,
    Item::Gear,
];

pub(super) fn setup_source_pane(mut cmd: Commands) {
    cmd.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(5.0),
            top: Val::Percent(5.0),
            bottom: Val::Percent(5.0),
            width: Val::Px(320.0),
            flex_direction: FlexDirection::Column,
            ..default()
        },
        BackgroundColor(PANE_BG),
        Visibility::Hidden,
        SourcePane,
    ))
    .with_children(|parent| {
        spawn_title_bar(parent, "Source", CloseSourceButton);

        parent
            .spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(16.0)),
                    overflow: Overflow::scroll_y(),
                    flex_grow: 1.0,
                    ..default()
                },
                ScrollPosition::default(),
                SourceScrollList,
            ))
            .with_children(|parent| {
                section_label(parent, "Select Item to Produce");
                for &item in ALL_ITEMS {
                    parent
                        .spawn((
                            Button,
                            Node {
                                width: Val::Percent(100.0),
                                padding: UiRect::axes(Val::Px(16.0), Val::Px(12.0)),
                                margin: UiRect::bottom(Val::Px(6.0)),
                                border: UiRect::all(Val::Px(2.0)),
                                ..default()
                            },
                            BackgroundColor(SLOT_BG),
                            BorderColor::all(SLOT_BORDER),
                            SourceItemButton(item),
                        ))
                        .with_children(|parent| {
                            parent.spawn((
                                Text::new(item.name()),
                                TextFont {
                                    font_size: LABEL_FONT_SIZE,
                                    ..default()
                                },
                                TextColor(Color::WHITE),
                            ));
                        });
                }
            });
    });
}

pub(super) fn update_source_pane(
    mode: Res<InteractionMode>,
    mut pane: Single<&mut Visibility, With<SourcePane>>,
) {
    if matches!(*mode, InteractionMode::InScreen(ScreenMode::Source(_))) {
        **pane = Visibility::Visible;
    } else {
        **pane = Visibility::Hidden;
    }
}

const SCROLL_SPEED_LINE: f32 = 20.0;

pub(super) fn handle_source_scroll(
    scroll_input: Res<AccumulatedMouseScroll>,
    mut scroll_q: Query<(&mut ScrollPosition, &ComputedNode), With<SourceScrollList>>,
    mode: Res<InteractionMode>,
) {
    if !matches!(*mode, InteractionMode::InScreen(ScreenMode::Source(_))) {
        return;
    }
    if scroll_input.delta == Vec2::ZERO {
        return;
    }
    let Ok((mut scroll_pos, computed)) = scroll_q.single_mut() else {
        return;
    };
    let dy = match scroll_input.unit {
        MouseScrollUnit::Line => -scroll_input.delta.y * SCROLL_SPEED_LINE,
        MouseScrollUnit::Pixel => -scroll_input.delta.y,
    };
    let max_offset =
        (computed.content_size().y - computed.size().y) * computed.inverse_scale_factor();
    scroll_pos.y = (scroll_pos.y + dy).clamp(0.0, max_offset.max(0.0));
}

pub(super) fn handle_source_item_button(
    interactions: Query<(&Interaction, &SourceItemButton), Changed<Interaction>>,
    mode: Res<InteractionMode>,
    mut cmd: Commands,
) {
    let InteractionMode::InScreen(ScreenMode::Source(source_entity)) = *mode else {
        return;
    };
    for (&interaction, btn) in &interactions {
        if interaction == Interaction::Pressed {
            cmd.trigger(SetSourceItem {
                source: source_entity,
                item: Some(btn.0),
            });
        }
    }
}
