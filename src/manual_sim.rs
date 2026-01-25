use bevy::prelude::*;

use crate::core::{Item, block_changes};
use crate::sim::{determine_sideload_blocks, do_moves, plan_moves, transfers};

#[derive(Resource, Default)]
pub struct ManualSimState {
    pub playing: bool,
    pub force_single_tick: bool,
    pub current_frame: usize,
    pub history: Vec<Vec<(Entity, Transform)>>,
}

impl ManualSimState {
    fn should_run_sim(&self) -> bool {
        self.current_frame >= self.history.len() && (self.playing || self.force_single_tick)
    }
    fn toggle_playing(&mut self) {
        self.playing = !self.playing;
        info!(
            "Manual sim: {}",
            if self.playing { "playing" } else { "paused" }
        );
    }
    fn go_back_frame(&mut self, items: &mut Query<&mut Transform, With<Item>>) {
        if self.current_frame > 0 {
            self.playing = false; // Pause to prevent sim from overwriting
            self.current_frame -= 1;
            restore_transforms(&self.history[self.current_frame], items);
            info!("Manual sim: stepped back to frame {}", self.current_frame);
        } else {
            info!("Manual sim: already at first frame");
        }
    }
    fn advance_frame(&mut self, items: &mut Query<&mut Transform, With<Item>>) {
        self.playing = false; // Pause to prevent sim from overwriting
        if self.current_frame + 1 < self.history.len() {
            self.current_frame += 1;
            restore_transforms(&self.history[self.current_frame], items);
            info!(
                "Manual sim: stepped forward to frame {}",
                self.current_frame
            );
        } else {
            // At end of history, advance simulation by one tick
            self.force_single_tick = true;
            info!("Manual sim: advancing simulation by one tick");
        }
    }
}

fn simulation_should_run(state: Res<ManualSimState>) -> bool {
    state.should_run_sim()
}

fn manual_sim_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<ManualSimState>,
    mut items: Query<&mut Transform, With<Item>>,
) {
    // K: toggle play/pause
    if keyboard.just_pressed(KeyCode::KeyK) {
        state.toggle_playing();
    }

    // J: step backwards one frame (pauses if playing)
    if keyboard.just_pressed(KeyCode::KeyJ) {
        state.go_back_frame(&mut items);
    }

    // L: step forwards one frame
    if keyboard.just_pressed(KeyCode::KeyL) {
        state.advance_frame(&mut items);
    }
}

fn restore_transforms(
    frame: &[(Entity, Transform)],
    items: &mut Query<&mut Transform, With<Item>>,
) {
    for (entity, transform) in frame {
        if let Ok(mut t) = items.get_mut(*entity) {
            *t = *transform;
            info!("Put {entity} at {:?}", transform.translation);
        } else {
            error!("Entity {entity} not found or other error");
        }
    }
}

fn playback_history(
    mut state: ResMut<ManualSimState>,
    mut items: Query<&mut Transform, With<Item>>,
) {
    // When playing but still in recorded history, advance through it
    if state.playing && state.current_frame < state.history.len() {
        state.current_frame += 1;
        if state.current_frame < state.history.len() {
            restore_transforms(&state.history[state.current_frame], &mut items);
            info!("Playback: frame {}", state.current_frame);
        } else {
            info!("Playback: reached live edge, simulation will take over");
        }
    }
}

fn record_transforms(
    mut state: ResMut<ManualSimState>,
    items: Query<(Entity, &Transform), With<Item>>,
) {
    let frame: Vec<(Entity, Transform)> = items.iter().map(|(e, t)| (e, *t)).collect();
    state.history.push(frame);
    info!(
        "Recorded frame {} at index {}",
        state.current_frame,
        state.history.len() - 1
    );
    state.force_single_tick = false;
    state.current_frame += 1;
}

pub struct ManualSimPlugin;
impl Plugin for ManualSimPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ManualSimState>();

        // Add sim systems with run condition
        app.add_systems(
            Update,
            (
                determine_sideload_blocks,
                transfers,
                plan_moves,
                do_moves,
                record_transforms,
            )
                .chain()
                .run_if(simulation_should_run)
                .after(block_changes),
        );

        // Add debug control systems
        app.add_systems(
            Update,
            (manual_sim_input, playback_history)
                .chain()
                .before(determine_sideload_blocks),
        );
    }
}
