# Collectors

## What
Collectors are placeable entities (similar to Factorio inserters) that move items from an adjacent belt into an adjacent machine's `InputBuffer`. They are placed in the world facing a direction — one side faces the source belt, the other faces the target machine. Currently machines auto-pull from any belt pointing at them; collectors would replace or supplement that with an explicit, placeable transfer step. This makes item routing visible and intentional rather than implicit.

## Implementations

### Option A: Adjacency-based lookup each tick
- **Summary:** `Collector` component stores only a facing direction (`HDir`) and a tick timer. Each tick, the system looks up neighbors at runtime — checks the cell behind for a belt, the cell in front for a machine — and transfers if both are found and the machine's `InputBuffer` has space.
- **Best suited for:** Early-stage iteration. No stored references means no stale pointers. Easy to extend to belt-belt or machine-machine later by adjusting the neighbor checks. Mirrors how `pull_from_belt` already works.
- **Tradeoffs:** Slightly more work per tick (hash map lookups); behavior changes silently if something moves into/out of an adjacent cell.

### Option B: Explicit source/target entity references
- **Summary:** `Collector` stores `Option<Entity>` for the source belt and `Option<Entity>` for the target machine, resolved at placement time and cached. Transfer logic uses the stored references directly.
- **Best suited for:** When collectors need to handle non-adjacent or indirect connections (e.g., longer-range inserters), or when lookup cost matters.
- **Tradeoffs:** References can go stale if the world changes (machine removed, belt replaced). Requires a wiring step at placement. More complexity for no current benefit.

## Chosen: Option A
Reason: Matches existing patterns (`pull_from_belt` does the same adjacency scan), no reference lifecycle to manage, and easiest to change when we add belt-belt or machine-machine modes.

## Enabling Refactors
- [ ] Decide whether collectors *replace* the existing auto-pull behavior on machines, or coexist with it (machines with no collector still auto-pull). For now, coexist — no refactor needed.

## Implementation Notes
- New component: `Collector { dir: HDir, state: CollectorState }`
- New `WorldBlock::Collector` and `Item::Collector`
- Placement: player faces a direction, collector placed facing that direction (output side toward machine, input side toward belt)
- State machine enum (enables animation and future timing changes):
  ```
  enum CollectorState {
      ReadyToPickUp,
      MovingItem { ticks: u32 },
      ReadyToDropOff,
      MovingToStart { ticks: u32 },
  }
  ```
- System `tick_collectors`:
  - `ReadyToPickUp` → look up belt behind, pull head item if available → `MovingItem { ticks: 0 }`
  - `MovingItem` → advance ticks; when done → `ReadyToDropOff` (item held by collector)
  - `ReadyToDropOff` → look up machine in front, call `fill_slot` if space → `MovingToStart { ticks: 0 }`
  - `MovingToStart` → advance ticks; when done → `ReadyToPickUp`
- Tick duration per `Moving*` state: configurable constant (e.g. `COLLECTOR_MOVE_TICKS: u32 = 15`)
- Collector holds at most one item (stored in `CollectorState::MovingItem` variant)
- Filter: collector can optionally carry a `Filter` (initially none — accepts anything)

## Status
[ ] Planned / [ ] Refactoring / [x] Ready to implement / [ ] Done
