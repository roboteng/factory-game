# Partial Inventory Transfer

## Problem

When a player unloads items from a machine's output buffer, the current contract is all-or-nothing per stack: either the full stack fits in the player's inventory, or the transfer is refused. This doesn't match how physical inventory should feel — if a player has room for 3 items out of a stack of 10, the game silently does nothing instead of transferring what fits.

The same problem exists in reverse for `LoadMachineInput`: if a player tries to load a stack that is too large for the machine's remaining capacity, the whole transfer is refused even if partial loading would make progress.

This also applies to `RemoveBlock` drops: if a block drops multiple stacks and the inventory has partial room, the block is currently not destroyed at all, even though some drops could fit.

In all three cases, the current all-or-nothing behavior is a temporary simplification. Partial transfer semantics — "move as much as fits, leave the rest" — are the expected long-term behavior but require the `Inventory` API to support returning remainders from `insert`, and require callers to handle split stacks correctly.
