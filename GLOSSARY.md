# Glossary

## Voxel

The primitive grid unit. One `WorldCoords` slot: 1 unit wide (X), 0.5 units tall (Y), 1 unit deep (Z) in world space. Belts occupy one voxel of vertical space. The Y axis has voxel granularity, so moving up one full Block is `y + 2`.

## Block

A 1×1×1 world-unit spatial unit — two voxels tall. Dirt, rock, source, sink, and similar things each occupy one Block. Full-block structures sit at an even Y coordinate.

## Structure

A placeable entity with a defined size — a Miner occupies 1×1×1 Blocks, a Furnace occupies 2×3×2 Blocks. Structures are described by a `StructureSize` (dimensions in voxels) and placed at an origin `WorldCoords`; all voxels they occupy are registered in the `CoordsMap`.
