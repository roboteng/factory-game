use bevy::prelude::*;

use factory_core::{Miner, OutputBuffer, UnloadMachineOutput};

use super::common::{pane_node, section_label, spawn_screen_layout, spawn_slot, stack_label};
use super::{InteractionMode, ScreenMode};

#[derive(Component)]
pub(super) struct MinerPane;

#[derive(Component)]
pub(super) struct CloseMinerButton;

#[derive(Component)]
pub(super) struct MinerOutputSlot(pub(super) usize);

/// Marks the InventorySlot entities that belong to the miner screen's inventory panel.
#[derive(Component, Clone)]
pub(super) struct MinerInventoryPanel;

pub(super) fn setup_miner_pane(mut cmd: Commands) {
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

pub(super) fn update_miner_pane(
    mode: Res<InteractionMode>,
    mut pane: Single<&mut Visibility, With<MinerPane>>,
    miner_q: Query<&OutputBuffer, With<Miner>>,
    output_slots: Query<(&MinerOutputSlot, &Children)>,
    mut texts: Query<&mut Text>,
) {
    let InteractionMode::InScreen(ScreenMode::Miner(miner_entity)) = *mode else {
        **pane = Visibility::Hidden;
        return;
    };
    **pane = Visibility::Visible;

    let Ok(output_buf) = miner_q.get(miner_entity) else {
        return;
    };

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

pub(super) fn handle_miner_output_slot_clicks(
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
