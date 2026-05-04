---
title: Event Request Contract
done: true
---

# Overview

Events should be **requests**, not **commands**. When a player clicks to move an item, the event says "player requests to move item from slot 3 into the furnace input." The handler decides whether that request is valid and, if so, performs the entire state transition atomically. If the request is invalid — wrong item type, full inventory, missing entity — nothing happens.

This is a contract change. Currently some events are more like commands where callers do work (remove items, allocate entities) before firing the event and assume success. That creates error cases with no recovery path and item loss bugs.

# The Contract

**Callers:**
- Gather the information needed to describe the request (which entity, which slot, which item)
- Fire the event
- Do nothing else — no inventory changes, no entity spawning, no state mutation

**Handlers:**
- Validate the full request before touching any state
- If invalid at any point: return early, nothing has changed
- If valid: perform the complete state transition

This means every event handler must be **all-or-nothing**. Partial execution that leaves state dirty is a bug.

# Event-by-Event Changes

## `LoadMachineInput { player: Entity, player_inventory_slot, machine, machine_input_slot: Option<usize> }`

**Current:** Handler takes the item from inventory unconditionally, then inserts it. Filter is queried but never checked — player can bypass machine input filters. Player entity resolved via `Res<Player>` rather than carried on the event.

**New contract:** Add `player: Entity` to the event; remove `Res<Player>` from the handler. Handler peeks at the slot (without removing), checks the filter, and only then takes and inserts. If the filter rejects the item, the slot is untouched.

`machine_input_slot` identifies which input slot on the machine to target. If `None`, the handler tries each input slot in order and uses the first one whose filter accepts the item. If no slot accepts it, return early.

Update all callers (furnace, assembler UI) to pass the player entity.

---

## `UnloadMachineOutput { player: Entity, machine, output_slot }`

**Current:** Handler removes the stack from the machine's output buffer, then attempts to insert into player inventory. If inventory is full, `insert()` returns an error that is silently ignored with `let _` — the item is gone. Player entity resolved via `Res<Player>` rather than carried on the event.

**New contract:** Add `player: Entity` to the event; remove `Res<Player>` from the handler. Handler attempts to insert the full stack into the player inventory. If the inventory is completely full, return early and nothing changes. Partial transfer (moving less than the full stack) is out of scope here — see `planned-features/partial-inventory-transfer.md`.

Update all callers (furnace, assembler, miner UI) to pass the player entity.

---

## `PlaceStructure { entity, item, flb, brt, player: Entity }`

**Current:** The UI caller removes the item from the player's inventory, spawns an empty entity, then fires `PlaceStructure`. If placement fails (collision with existing block), the spawned entity is despawned silently but the item has already been taken from inventory. Items can be lost on failed placements.

**New contract:** Callers pass a pre-allocated entity but do **not** remove items from inventory. The handler validates the placement (collision check, valid coordinates) first. Only on success does it insert components — and it also consumes the item from that `player`'s inventory. If validation fails, the pre-allocated entity is despawned and inventory is untouched.

The event field changes from `structure: Structure` to `item: Item`. The handler resolves the structure via `Item::can_place()` (already exists). If `can_place()` returns `None` (item is not placeable), the pre-allocated entity is despawned and the handler returns early — inventory is untouched.

### Shared placement function: `Structure::attach_bundle`

Extract the per-structure component insertion from `on_place_structure` into a method on `Structure`:

```rust
pub fn attach_bundle(
    self,
    cmd: &mut EntityCommands,
    coord_map: &mut CoordsMap,
    flb: WorldCoords,
    facing: Option<HDir>,
)
```

This method:
- Computes `RaycastTarget` from `self.size().into_raycast_target(...)`
- Inserts the structure-specific components (the current `match` arm)
- Inserts the spatial bundle (transform, structure, flb coords)
- Registers all occupied voxels in `CoordsMap` via `size.iter_coords(flb)` — uses `cmd.id()` to get the entity

The event handler calls `structure.attach_bundle(...)` for the happy path, replacing the big `match` block. The belt-on-belt replacement case stays as a special case in the handler (it needs `ItemLanes(transferred)` not `ItemLanes::default()`).

World gen bypasses the event entirely and calls `Structure::attach_bundle` directly — no inventory is checked and no player is involved. World gen systems will need `ResMut<CoordsMap>` added to their params. This is a large call-site migration: world gen alone has 6+ `PlaceStructure` trigger sites.

The `AppWorldExt` test helper methods in `mod.rs` (around lines 1926, 2219, etc.) also trigger `PlaceStructure` but do not involve a player — they follow the world gen pattern. These should also be migrated to call `Structure::attach_bundle` directly rather than being given a dummy player entity.

---

## `RemoveBlock { entity, player: Option<Entity> }`

**Current:** Handler collects drops and calls `inv.insert()` for each. If inventory is full, a warning is logged but the items are lost. The block is still destroyed. Player entity resolved via `Res<Player>`.

**New contract:** Add `player: Option<Entity>` to the event; remove `Res<Player>` from the handler. `Some(entity)` = player-triggered removal (inventory is checked and drops are returned). `None` = internal removal (drops are skipped, block is still cleaned up and despawned).

Before destroying, when `player` is `Some`, collect all drops first — `Structure::break_drop` plus any stacks in the machine's `InputBuffer` and `OutputBuffer` — then verify the player's inventory has room for all of them. If not, refuse destruction and leave everything unchanged. Only after the capacity check passes does the handler proceed to destroy the block and insert the drops.

**Call site changes:**
- `handle_delete_input` in `player_controller/mod.rs`: add `Res<Player>`, pass `player: Some(player.0)`
- `remove_belt_at` test helper: pass `player: None` (belt removal in tests doesn't involve a player inventory)

---

## `BreakDrop` enum: split `None`

**Current:** `BreakDrop::None` is used for two distinct cases: blocks that cannot be broken at all (ore deposits) and blocks that can be broken but drop nothing. This conflation forces callers to know by convention which applies.

**New variants:**
- `BreakDrop::Unbreakable` — player cannot break this block; `RemoveBlock` handler returns early and leaves everything unchanged. Used by `IronOreDeposit`, `CopperOreDeposit`.
- `BreakDrop::NoDrop` — block can be broken and despawns, but nothing is returned to inventory. No current examples, but the semantic is now expressible.

**Existing bug this fixes:** The current `BreakDrop::None => return` at `on_remove_block:918` fires *after* coord_map cleanup and belt item despawn (lines 893–913). Ore deposits therefore get their coord_map entries removed but the entity survives — partial execution. The `Unbreakable` guard must be the *first* check in the new handler, before any state mutation.

`BreakDrop::Item` and `BreakDrop::Custom` are unchanged.

---

## `Incline { entity }`

**Current:** Handler validates the entity is a belt and returns silently if not. No pre-work by caller. Already close to the request contract.

**New contract:** No structural change needed. Behavior is already atomic.

---

## `PlaceItem { entity, item }`

**Current:** Used internally by simulation systems to assign an `Item` component to a world entity. Not player-facing.

**New contract:** No change needed. This is an internal simulation event, not a player request.

---

## `SetSourceItem { source, item }`

**Current:** Handler sets `source.configured_item`. No pre-work by caller. Already atomic.

**New contract:** No structural change needed.

---

## `SetAssemblerRecipe { assembler, recipe }`

**Current:** Handler sets `assembler.configured_recipe`. No pre-work by caller. Already atomic.

**New contract:** No structural change needed.

---

## `Interact(entity)`

**Current:** Multiple handlers observe this and each checks whether the entity matches their machine type, opening the appropriate screen. No state mutation beyond UI mode change. Already safe.

**New contract:** No structural change needed.

---

# Deferred

**Belt `break_drop` for riding items.** `break_drop` is a `self: Structure` method returning a static `BreakDrop`. Belt lane items are separate `Entity` instances — their `Stack` info lives in components, not in the `Structure` value. Returning riding items requires runtime world access, which would require the `BreakDrop::Custom` + `ReflectWorldDrop` path. Currently belt items are simply despawned on `RemoveBlock` without being returned. Fixing this is tracked separately.

---

# Summary of Work

| Event | Problem | Change Required |
|---|---|---|
| `LoadMachineInput` | Filter bypassed; item taken before check; player via `Res<Player>` | Add `player: Entity`; peek first, check filter, then take; add `Option<usize>` slot targeting with first-available fallback |
| `UnloadMachineOutput` | Item lost if inventory full; player via `Res<Player>` | Add `player: Entity`; if inventory full return early and leave output buffer untouched (all-or-nothing; partial transfer deferred) |
| `PlaceStructure` | Item consumed before validation, lost on collision; world gen and player share one path | Validate first, consume item in handler on success; swap `structure: Structure` for `item: Item`; add `player: Entity`; extract `Structure::attach_bundle` for shared logic; world gen calls `attach_bundle` directly (large migration) |
| `RemoveBlock` | Drops silently lost if inventory full; no player field; internal callers would break with required player | Add `player: Option<Entity>`; check capacity first when `Some`; refuse removal if no room; `remove_belt_at` passes `None` |
| `BreakDrop::None` | Conflates "unbreakable" with "drops nothing" | Split into `Unbreakable` (refuse removal) and `NoDrop` (allow removal, drop nothing) |
| `Incline` | Already correct | No change |
| `PlaceItem` | Internal, not player-facing | No change |
| `SetSourceItem` | Already correct | No change |
| `SetAssemblerRecipe` | Already correct | No change |
| `Interact` | Already correct | No change |
