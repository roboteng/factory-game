# Glossary

## Voxel

The primitive grid unit. One `WorldCoords` slot: 1 unit wide (X), 0.5 units tall (Y), 1 unit deep (Z) in world space. Belts occupy one voxel of vertical space. The Y axis has voxel granularity, so moving up one full Block is `y + 2`.

## Block

A 1×1×1 world-unit spatial unit — two voxels tall. Dirt, rock, source, sink, and similar things each occupy one Block. Full-block structures sit at an even Y coordinate.

## Structure

A placeable entity with a defined size — a Miner occupies 1×1×1 Blocks, a Furnace occupies 2×3×2 Blocks. Structures are described by a `StructureSize` (dimensions in voxels) and placed at an origin `WorldCoords`; all voxels they occupy are registered in the `CoordsMap`.

## Footprint

The 2D horizontal extent of a structure: width × depth in Blocks. Does not include height.

## Slot

A single position in an `Inventory` or `Buffer` that can hold a `Stack`. The term applies equally to player inventory and machine buffers.

## Lane

One of the two physical channels on a belt — left or right — along which items travel. `Side` is the address of a lane (`Side::Left`, `Side::Right`); a Lane is the channel itself.

## ItemPos

An `i32` representing an item's progress along a belt lane. 0 is the output end; the maximum varies — curved belts have more positions in the outer lane than the inner, and future belt chains may span many more. "Position" in code refers only to this concept, never to a world-space coordinate (use `WorldCoords` for that).
