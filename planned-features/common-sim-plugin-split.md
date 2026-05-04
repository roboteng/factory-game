# Overview

Split `CorePlugin` into two plugins: `CorePlugin` (structural/reactive behavior) and `SimPlugin` (tick-driven simulation). This makes it possible to write tests that spin up a Bevy app and trigger events without fighting systems that unconditionally overwrite component state every frame.

The immediate motivating case is input filter tests: `recalculate_filters` runs every `Update` and overwrites the `Filter` component on machines, making it impossible to set a specific filter state and then test an event handler against it in the same `app.update()` cycle without relying on subtle timing of synchronous observers.

# The Split

## `CorePlugin` (keep in `src/common/mod.rs`)

Structural and reactive behavior — things that respond to events or set up the world:

- Type registration (Furnace, Assembler reflect data)
- Resource initialization (CoordsMap, Recipes, MinerTicksPerExtract, CollectorMoveTicks, CornGrowthTicks)
- All observers: `on_place_structure`, `on_place_item`, `on_remove_block`, `on_incline`, `on_load_machine_input`, `on_unload_machine_output`, `on_set_assembler_recipe`, `on_set_source_item`
- `spawn_player`

## `SimPlugin` (new, in `src/sim/mod.rs` or `src/common/sim.rs`)

Tick-driven simulation — systems that advance world state each frame:

- `determine_belt_shape` (+ ApplyDeferred chain)
- `move_items_on_belts`
- `transfer_items`
- `set_item_transforms`
- `fill_sources`
- `fill_miners`
- `push_to_belt`
- `recalculate_filters`
- `pull_from_belt`
- `tick_collectors`
- `process_furnace`
- `process_assembler`
- `player::process_hand_crafter`
- `consume_sink_buffer`
- `side_loading`
- `grow_corn`
- `despawn_old_entities` (PostUpdate)

# Changes Required

## Production (`src/main.rs`)

Add both plugins where `CorePlugin` was previously used alone:

```rust
app.add_plugins((CorePlugin, SimPlugin));
```

## Test helpers (`src/common/mod.rs`)

`test_app()` uses `CorePlugin` only — no simulation runs, component state set in tests stays set:

```rust
pub fn test_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.init_resource::<PlacementErrors>();
    app
}
```

Add a new `sim_test_app()` for tests that need tick-driven behavior:

```rust
pub fn sim_test_app() -> App {
    let mut app = test_app();
    app.add_plugins(SimPlugin);
    app
}
```

## Existing tests

Of the 25 tests in `src/common/mod.rs`:

- **22 tests need `SimPlugin`** — all belt shape tests (18) and miner/collector tests (4). Change `test_app()` → `sim_test_app()` in each.
- **1 test is observers only** — `load_machine_input_moves_ore_to_furnace_input_buffer`. Stays on `test_app()`.
- **2 tests are pure unit tests** — `into_raycast`, `place_block_facing`. No app involved, no change needed.

# Benefit

Once this split is in place, tests for event handlers (player inventory transfers, recipe changes, placement logic) can freely set component state — `Filter`, `InputBuffer`, etc. — without needing to account for which simulation systems might overwrite it on the next `app.update()`. The test reads directly as the behavior it describes.
