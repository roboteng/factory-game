---
title: Load Machine Input Filter Tests
done: true
---

# Overview

Write tests for the `on_load_machine_input` observer that exercise `Filter` gating. These were the motivating case for the `CorePlugin`/`SimPlugin` split (`common-sim-plugin-split.md`): before the split, `recalculate_filters` ran every `Update` and overwrote the `Filter` component, making it impossible to hold a specific filter state through a `trigger()` call when `app.update()` was also needed.

With `test_app()` using `CorePlugin` only, `recalculate_filters` never runs. Tests can set `Filter` directly and have it stick.

# Filter Semantics

Two distinct states:

- **`Option<&Filter>` is `None`** — no `Filter` component; observer skips the check and accepts any item.
- **`Filter::none()`** — empty set; `accepts()` returns `false` for everything. What furnaces/assemblers spawn with by default.
- **`Filter::from_iter([...])`** — explicit allowlist.

# Tests

The observer queries `(&mut InputBuffer, Option<&Filter>)` on the machine and `&mut Inventory` on the player. No other components are needed, so tests spawn bare entities with just those.

## 1. Filter accepts item → transfers

```rust
let mut app = test_app();

let player = app.world_mut().spawn(Inventory::new()).id();
let machine = app.world_mut().spawn((
    InputBuffer::default(),
    Filter::from_iter([Item::IronOre]),
)).id();

let mut inv = app.world_mut().get_mut::<Inventory>(player).unwrap();
inv.insert(Stack::new(Item::IronOre, 1)).unwrap();
let ore_slot = (0..64)
    .find(|&s| inv.get(s).map(|st| st.item == Item::IronOre).unwrap_or(false))
    .unwrap();
drop(inv);

app.world_mut().trigger(LoadMachineInput {
    player,
    player_inventory_slot: ore_slot,
    machine,
    machine_input_slot: None,
});

let buf = app.world().get::<InputBuffer>(machine).unwrap();
assert!(buf.slots.iter().any(|s| s.item == Item::IronOre));
let inv = app.world().get::<Inventory>(player).unwrap();
assert!(inv.get(ore_slot).is_none());
```

## 2. Filter rejects item → no transfer

Same setup, but give the player `Item::Coal` instead and assert nothing moves:

```rust
inv.insert(Stack::new(Item::Coal, 1)).unwrap();
// ...trigger LoadMachineInput with the coal slot...

assert!(buf.slots.is_empty());
assert!(inv.get(coal_slot).is_some());
```

## 3. No Filter component → any item transfers

Spawn the machine with only `InputBuffer::default()` (no `Filter`). Give the player any item. Assert it transfers — the absent component means "accept all."

# Where

Add a `mod load_machine_input_filter` block inside `src/common/mod.rs` (alongside the existing `mod tests`).
