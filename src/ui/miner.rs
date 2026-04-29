use bevy::prelude::*;

use crate::common::{Miner, MinerTicksPerExtract, OutputBuffer, UnloadMachineOutput};

use super::common::{pane_node, section_label, spawn_screen_layout, spawn_slot, stack_label};
use super::{InteractionMode, ScreenMode};

#[derive(Component)]
pub struct MinerPane;

#[derive(Component)]
pub struct CloseMinerButton;

#[derive(Component)]
pub struct MinerOutputSlot(pub usize);

#[derive(Component)]
pub struct MinerProgressFill;

/// Marks the InventorySlot entities that belong to the miner screen's inventory panel.
#[derive(Component, Clone)]
pub struct MinerInventoryPanel;

pub fn setup_miner_pane(mut cmd: Commands) {
    cmd.spawn((
        pane_node(
            Val::Percent(5.0),
            Val::Percent(5.0),
            Val::Percent(5.0),
            Val::Percent(5.0),
        ),
        MinerPane,
    ))
    .with_children(|parent| {
        spawn_screen_layout(
            parent,
            "Miner",
            CloseMinerButton,
            |parent| {
                section_label(parent, "Progress");
                parent
                    .spawn(Node {
                        width: Val::Percent(100.0),
                        height: Val::Px(24.0),
                        margin: UiRect::axes(Val::Px(4.0), Val::Px(4.0)),
                        ..default()
                    })
                    .insert(BackgroundColor(super::common::SLOT_BG))
                    .with_children(|parent| {
                        parent.spawn((
                            Node {
                                width: Val::Percent(0.0),
                                height: Val::Percent(100.0),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.3, 0.7, 1.0, 1.0)),
                            MinerProgressFill,
                        ));
                    });

                section_label(parent, "Output Buffer");
                parent
                    .spawn(Node {
                        flex_direction: FlexDirection::Row,
                        flex_wrap: FlexWrap::Wrap,
                        ..default()
                    })
                    .with_children(|parent| {
                        for i in 0..4 {
                            spawn_slot(parent, MinerOutputSlot(i));
                        }
                    });
            },
            MinerInventoryPanel,
        );
    });
}

pub fn update_miner_pane(
    mode: Res<InteractionMode>,
    mut pane: Single<&mut Visibility, With<MinerPane>>,
    miner_q: Query<(&Miner, &OutputBuffer)>,
    output_slots: Query<(&MinerOutputSlot, &Children)>,
    mut fill_node: Single<&mut Node, With<MinerProgressFill>>,
    mut texts: Query<&mut Text>,
    miner_ticks: Res<MinerTicksPerExtract>,
) {
    let InteractionMode::InScreen(ScreenMode::Miner(miner_entity)) = *mode else {
        **pane = Visibility::Hidden;
        return;
    };
    **pane = Visibility::Visible;

    let Ok((miner, output_buf)) = miner_q.get(miner_entity) else {
        return;
    };

    let progress_fraction = if miner_ticks.0 > 0 {
        miner.ticks as f32 / miner_ticks.0 as f32
    } else {
        0.0
    };
    fill_node.width = Val::Percent(progress_fraction * 100.0);

    let view = output_buf.view();
    for (slot_marker, children) in &output_slots {
        let label = stack_label(view.get(slot_marker.0).copied());
        if let Some(&child) = children.first() {
            if let Ok(mut text) = texts.get_mut(child) {
                **text = label.into();
            }
        }
    }
}

pub fn handle_miner_output_slot_clicks(
    interactions: Query<(&Interaction, &MinerOutputSlot), Changed<Interaction>>,
    mode: Res<InteractionMode>,
    mut cmd: Commands,
) {
    let InteractionMode::InScreen(ScreenMode::Miner(miner_entity)) = *mode else {
        return;
    };
    for (&interaction, slot_marker) in &interactions {
        if interaction == Interaction::Pressed {
            cmd.trigger(UnloadMachineOutput {
                machine: miner_entity,
                output_slot: slot_marker.0,
            });
        }
    }
}
