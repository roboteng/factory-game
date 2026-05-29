# Chests

## Summary

Add a storage chest structure: a placeable block where the player deposits and withdraws items from slot-positional storage that *feels* like Minecraft (same item can sit in multiple slots, slot order is preserved). Chests have no recipe processing — they are bulk storage that also passively passes items between belts and collectors via their `Inventory` directly.

## Design

- Full-block (2 voxels tall, 1×1 footprint) with a tinted Block model, treated as a normal `Structure` (no new block kind).
- Chest carries only **`Inventory`** (canonical storage, slot-positional) + **`Filter`** + **`SlotLimit`**. No buffers.
- A `SlotLimit(usize)` component is the **sole capacity enforcer** for chests. The chest's `Inventory` is created with `Inventory::new()` (internally unbounded); every insertion path (belt, collector, load handler) checks `SlotLimit` before calling `insert_capped`. The `SlotLimit` value also drives the UI slot count. Reusable by future bounded-storage structures.
- Clicking a player inventory slot → `LoadMachineInput{player, player_inventory_slot, machine: chest, machine_input_slot: None}` → inserts directly into chest `Inventory`, gated by `Filter`. (Requires a generalization of the load handler — see §2.)
- Clicking a chest inventory slot → `UnloadMachineOutput{player, machine: chest, output_slot}` → removes directly from chest `Inventory`. (Requires a generalization of the unload handler — see §2.)
- A `recalculate_chest_filters` system mutates the chest's `Filter` in place based on `Inventory` occupancy.
- No `process_chest` system — all reads and writes go directly to `Inventory`.
- On block break, `on_remove_block` drops `Inventory` contents. No buffer spill needed.
- Recipe placeholder: iron plates + iron rod.

### Minecraft-feel storage without sprawling special cases

`Buffer` is type-grouped — it merges by item. That's right for machines (recipes ask "do I have ≥N IronOre?") but wrong for player-facing storage (Minecraft chests let one item occupy multiple slots). `Inventory` is already slot-positional, so the chest reuses it. The previous "no `Inventory` on world blocks" constraint is lifted; that's the only conceptual cost.

Belts and collectors interact with the chest's `Inventory` directly (gated by `Filter`). There are no intermediate buffers. Each belt/collector system gains a chest-aware path that queries `(Entity, &mut Inventory, &Filter, &SlotLimit)` with `With<Chest>`.

### `Filter` change (one-time refactor)

To eliminate a Commands-flush race against `pull_from_belt`/`tick_collectors`, the chest *always* carries a `Filter` component and mutates it in place. That requires `Filter` to express "accept everything" intrinsically:

- Add `Filter::all()` constructor that contains every `Item` variant.
- Derive `strum::EnumIter` on `Item` (Trevor notes an existing enumeration exists under a different name; consolidate during implementation) so `Filter::all()` can iterate variants without a hand-maintained list.
- `Filter::none()` still means reject-all (empty set). No call-site changes needed elsewhere.

### Slot gating (in-place filter recalc)

| Inventory state | Filter | Meaning |
|---|---|---|
| Any empty slot | `Filter::all()` | Accept everything |
| All slots filled, some below `stack_size` | `Filter(present_items)` | Top up existing types only |
| All slots filled at `stack_size` | `Filter::none()` | Reject everything |

All entry points (`on_load_machine_input`, `pull_from_belt`, collector `ReadyToDropOff`) already handle `Option<&Filter>` — since the chest always carries one, they take the `Some` branch and gate by `accepts(item)`.

### Belt & Collector interaction

Chests expose `Inventory` directly — no buffers:

| Direction | Mechanism | How |
|---|---|---|
| Belt → Chest | `pull_from_belt` | Chest path: queries `(&mut Inventory, &Filter, &SlotLimit, &WorldCoords)` with `With<Chest>`. Inserts directly into `Inventory`, gated by `Filter` + `SlotLimit`. |
| Collector ← Chest | `tick_collectors` | Chest path: reads and removes directly from chest `Inventory`. |
| Collector → Chest | `tick_collectors` | Chest path: inserts directly into chest `Inventory`, gated by `Filter` + `SlotLimit`. |
| Chest → Belt | N/A | No `OutputsToBelt`. |

The full passthrough flow per tick: **Belt → `pull_from_belt` → `Inventory` → collector picks up directly → next destination**.

### `recalculate_chest_filters` — the only per-tick chest system

Because there are no buffers to reconcile, the only per-tick chest work is filter recalculation. `recalculate_chest_filters` runs *before* `pull_from_belt` and `tick_collectors`, so the filter it computes reflects the inventory state from the *previous* tick — the same one-tick lag that furnace/assembler filters have. This is intentional: the filter updated mid-tick would require an ordering dependency inside the tick, adding complexity for negligible practical difference.

Concurrency note: if two collectors pick from the same chest in the same tick, both succeed and remove items independently — `Inventory` mutations are in-place and handled by Bevy's exclusive query access. No chest-specific handling needed.

Stall behavior: when a collector is `ReadyToDropOff` and the chest destination is full (`Filter::none()`), `deposited` is `false` and the collector stays in `ReadyToDropOff`, holding its item mid-air. This is identical to the full-`InputBuffer` stall on other machines — correct and expected. Because `recalculate_chest_filters` runs before `tick_collectors`, a collector will not pick up from a belt if the chest was already full at tick start, which prevents most stalls. A stall can still happen if the chest fills during the same tick the collector picks up (e.g. belt and collector both insert in the same tick).

### UI layout (left 40% panel)

```
┌──────────────────────┐
│ Chest                [X]│
├──────────────────────┬──┤
│ Contents             │  │
│ [slot] [slot] [slot] │ P│  ← Inventory slots, fixed positions
│ [slot] [slot] [slot] │ l│
│ [slot] [slot] ...    │ a│
│                      │ y│
│                      │ e│
│                      │ r│
└──────────────────────┴──┘
```

- "Contents" section binds slots directly to `Inventory[i]` via the existing `InventorySlot(usize)` widget — no chest-specific slot type, no mapping logic.
- Slot count = `SlotLimit` on the chest entity.
- Right panel: standard player inventory via `spawn_inventory_panel`.

## Checklist

### 1. `src/common/machine.rs` — Filter refactor + SlotLimit

- [ ] Add `Filter::all()` constructor returning a filter containing every `Item` variant.
- [ ] Derive `strum::EnumIter` on `Item` (or consolidate with existing enumeration named differently per Trevor's note). `cargo add strum --features derive` if not present.
- [ ] Add `#[derive(Component)] pub struct SlotLimit(pub usize);`.

### 2. `src/common/mod.rs` — Enums, components, observers

- [ ] Add `Chest` variant to `Item` enum.
- [ ] `Item::name()` → `"Chest"`.
- [ ] `Item::can_place()` → `Some(Structure::Chest)`.
- [ ] Add `Chest` variant to `Structure` enum.
- [ ] `Structure::name()` → `"Chest"`.
- [ ] `Structure::mine()` → `None`.
- [ ] `Structure::break_drop()` → `BreakDrop::Custom(TypeId::of::<Chest>())` (same pattern as `Furnace`/`Assembler`).
- [ ] `Structure::attach_bundle()` `Chest` arm — inserts `Chest`, `Inventory::new()`, `SlotLimit(20)`, `Filter::all()`. No buffers. (`Inventory::new()` is unbounded; `SlotLimit` is the sole capacity enforcer — see §3.)
- [ ] `#[derive(Component)] pub struct Chest;` marker. Implement `WorldDrop` for `Chest`: `drop_items()` returns `vec![Stack::from(Item::Chest)]` plus all stacks from the chest's `Inventory`. Register `ReflectWorldDrop` so `on_remove_block` can dispatch to it. This ensures the full drop list (structure item + contents) is collected before the `can_fit_all` pre-check — same as `Furnace`/`Assembler`.
- [ ] **Load event handling**: `on_load_machine_input` currently writes to `InputBuffer`. Add a chest-specific query `chest_q: Query<(&mut Inventory, &Filter, &SlotLimit), With<Chest>>` (the `With<Chest>` filter proves disjointness from the player `Inventory` query, avoiding aliasing). Insert into chest `Inventory` only if `SlotLimit` not yet reached and `Filter` accepts the item.
- [ ] **Unload event handling**: `on_unload_machine_output` currently reads `OutputBuffer`. Add a chest path via a separate query `Query<&mut Inventory, With<Chest>>`. Remove from `Inventory` at the given slot index.
- [ ] Delete `PlayerToChest` / `ChestToPlayer` event types — the existing `LoadMachineInput` / `UnloadMachineOutput` events cover both directions once their handlers are generalized.
- [ ] Delete stub import of `PlayerToChest`/`ChestToPlayer` in `src/ui/chest.rs`.

### 3. `src/common/sim.rs` — Systems

- [ ] Add `recalculate_chest_filters`:
  ```rust
  fn recalculate_chest_filters(
      mut chests: Query<(&Inventory, &SlotLimit, &mut Filter), With<Chest>>,
  ) {
      for (inv, limit, mut filter) in &mut chests {
          let any_empty = inv.slot_count() < limit.0 || inv.has_empty_slot();
          if any_empty {
              *filter = Filter::all();
          } else {
              let all_capped = inv.iter().all(|s| s.count >= s.item.stack_size());
              if all_capped {
                  *filter = Filter::none();
              } else {
                  *filter = Filter::from_iter(inv.iter().map(|s| s.item));
              }
          }
      }
  }
  ```
  (Method names like `slot_count` / `has_empty_slot` are illustrative — match whatever `Inventory` actually exposes.)

- [ ] No `process_chest` system. There are no buffers to reconcile.

- [ ] Extend `pull_from_belt` with a chest path:
  ```rust
  // Existing: machines with InputBuffer
  // New: chests with Inventory
  for (chest_entity, mut inv, filter, limit, coords) in &mut chest_q {
      // find adjacent belt, pull item if filter accepts and SlotLimit not reached
      inv.insert_capped(item.into(), limit.0);
  }
  ```

- [ ] Extend `tick_collectors` with chest paths:
  - **Pickup from chest**: query `(&mut Inventory, &Filter)` with `With<Chest>`. Remove item directly from `Inventory` (first matching slot, Minecraft order).
  - **Dropoff to chest**: query `(&mut Inventory, &Filter, &SlotLimit)` with `With<Chest>`. Insert directly into `Inventory`, gated by `Filter` + `SlotLimit`.

- [ ] New `Inventory` helpers needed (`src/common/inventory.rs`):
  - `iter(&self) -> impl Iterator<Item = &Stack>` — yields only occupied slots (skips `None`).
  - `slot_count(&self) -> usize` — number of occupied slots (i.e. `slots.iter().filter(|s| s.is_some()).count()`).
  - `has_empty_slot(&self) -> bool` — true if any slot is `None` or `slots.len() < max_slots` (for unbounded `Inventory::new()`, always true until `SlotLimit` stops insertions).
  - `insert_capped(stack: Stack, slot_limit: usize) -> u32` — like `insert` but enforces an external slot cap instead of `max_slots`; returns the number of items actually placed. Callers (belt, collector, load handler) use this for chests.
  - `remove_first_of(item: Item) -> Option<Stack>` — removes one stack from the first matching slot (Minecraft slot order); returns the removed stack.

- [ ] Wire into sim schedule:
  ```rust
  (recalculate_filters, recalculate_chest_filters, pull_from_belt, tick_collectors).chain()
  ```
  Rationale: `recalculate_chest_filters` runs before `pull_from_belt` and `tick_collectors` so those systems see the filter computed from last tick's inventory state — one tick stale, same lag as furnace/assembler filters. This matches the existing pattern and avoids mid-tick ordering dependencies.

### 4. `src/ui/chest.rs` — Rewrite stub

- [ ] Drop import of `PlayerToChest`/`ChestToPlayer`.
- [ ] Drop `ChestSlot` — use the existing `InventorySlot(usize)` widget against the chest entity's `Inventory`.
- [ ] `setup_chest_pane`: single "Contents" section, spawning `SlotLimit`-many `InventorySlot` widgets bound to the chest entity. Right panel via `spawn_inventory_panel` + `ChestInventoryPanel` marker.
- [ ] `update_chest_pane`: change query from `(&ChestSlot, &Children)` to `(&InventorySlot, &Children)`. Read the chest entity's `Inventory` by index, same shape as the player inventory updater.
- [ ] `handle_player_slot_clicks`: trigger `LoadMachineInput{player, player_inventory_slot, machine: chest_entity, machine_input_slot: None}`.
- [ ] `handle_chest_slot_clicks`: trigger the unified unload event (per §2 option (a)), addressing the chest's `Inventory` slot.

### 5. `src/ui/mod.rs` — Wire chest screen (mirror furnace/assembler exactly)

- [ ] `mod chest;`, imports, register systems.
- [ ] `ScreenMode::Chest(Entity)`.
- [ ] Open trigger — same pattern as furnace/assembler (look at how those wire up `InteractWith` or equivalent; copy that).

### 6. `src/ui/visuals.rs` — Chest model

- [ ] `chest: ModelDef` on `BlockModels`.
- [ ] `setup_models`: `ModelDef::TintedScene(block, Color::srgb(0.6, 0.4, 0.2))` (brown crate).
- [ ] `Structure::Chest` arm in `attach_models` and `BlockModels::ghost_scene`.
- [ ] `Item::Chest` arm in `Item::model()`.

### 7. `src/ui/hotbar.rs` — Starting inventory

- [ ] `Item::Chest` in `FreeHotbar`. Consider for `SurvivalHotbar`.

### 8. Recipe

- [ ] `Recipes::new()`:
  ```rust
  assembler(
      vec![s(Item::IronPlate, 2), s(Item::IronRod, 1)],
      vec![s(Item::Chest, 1)],
      300,
  ),
  ```

### 9. Tests

- [ ] `LoadMachineInput` deposits directly into chest `Inventory`, gated by `Filter` and `SlotLimit`.
- [ ] `LoadMachineInput` respects `SlotLimit`: 20 distinct types fill `Inventory`, 21st-type input is rejected.
- [ ] Unload event withdraws from chest `Inventory` (slot-positional — withdrawing from slot 3 doesn't pull from slot 0).
- [ ] `recalculate_chest_filters`: empty `Inventory` → `Filter::all()`; one empty slot remaining → `Filter::all()`; full but uncapped → `Filter(present_items)`; full and capped → `Filter::none()`.
- [ ] `pull_from_belt` inserts directly into chest `Inventory`, gated by `Filter` + `SlotLimit`.
- [ ] `pull_from_belt` honors chest `Filter` at capacity (no items pulled when `Filter::none()`).
- [ ] Collector dropoff inserts directly into chest `Inventory`, gated by `Filter` + `SlotLimit`.
- [ ] Collector pickup removes directly from chest `Inventory` (first matching slot, Minecraft order).
- [ ] Two collectors picking same item type same tick: both succeed, `Inventory` decremented by 2 total.
- [ ] `insert_capped` fills a partially-full slot first (IronOre at `stack_size - 5`, insert 20 → slot at `stack_size`, 15 remaining placed in next available slot or returned).
- [ ] `on_remove_block` drops include the Chest item + full `Inventory` contents. No buffer spill.
- [ ] Player inventory full → unload from chest leaves chest `Inventory` slot unchanged.

## Open questions

- **Load/unload handler branching.** `on_load_machine_input` and `on_unload_machine_output` currently assume `InputBuffer`/`OutputBuffer`. Adding a chest path means branching inside the handler (check for `With<Chest>` or `Has<Inventory>`). If the branching grows unwieldy, split into separate observers triggered by the same event.
- **Partial-fill general plan.** `Buffer::insert` ignores `stack_size` and the existing furnace/assembler ticks rely on full-stack transfers. A separate plan (`plans/partial-buffer-fills.md`, not yet written) should generalize partial fills across all buffer-bearing machines. The chest's `insert_capped` is a local implementation for `Inventory` only.
- **`pull_from_belt` extensibility.** That system currently queries `&mut InputBuffer`. A chest-aware path requires a second query. If more buffer-less structures appear, consider an abstraction; for now two parallel queries is fine.

## Notes

- `on_remove_block` (mod.rs:910-917) currently collects `InputBuffer` + `OutputBuffer`. The chest variant only needs to spill `Inventory` — add a `With<Chest>` branch that queries `&Inventory` and skips the buffer logic.
- The Filter refactor (Filter::all + strum::EnumIter on Item) is a small one-time investment that also de-risks future structures that want "default accept-all" semantics without absence-of-component tricks.
- The chest has no `OutputBuffer`, so existing systems that query `OutputBuffer` will naturally skip it — no guard needed.
