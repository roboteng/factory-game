# Item Models: IronPlate, IronRod, CopperWire, Circuit

Four items currently use `ItemModelDef::Color` (solid-colored cube fallback).
Each gets a unique .blend + .glb and uses `ItemModelDef::Mesh` (no tinting needed
since each model is not shared with any other item).

The models should generally look low-poly.

## Models

| Item | Shape | File |
|---|---|---|
| `iron_plate` | A few stacked flat rectangular slabs | `IronPlate.blend` / `IronPlate.glb` |
| `iron_rod` | Thin cylinders - like rebar | `IronRod.blend` / `IronRod.glb` |
| `copper_wire` | A short, squat coill of wire. A square swept around a central axis | `CopperWire.blend` / `CopperWire.glb` |
| `circuit` | Flat board with wire tracesa and a small number of raised component bumps | `Circuit.blend` / `Circuit.glb` |

## Files Changed

- `blender/IronPlate.blend`, `blender/IronRod.blend`, `blender/CopperWire.blend`, `blender/Circuit.blend`
- `assets/models/IronPlate.glb`, `assets/models/IronRod.glb`, `assets/models/CopperWire.glb`, `assets/models/Circuit.glb`
- `blender/render_icons.py` — add 4 entries (no tint, unique model per item)
- `src/ui/visuals.rs` — change 4 `Color(...)` entries to `Mesh(...)` with correct GLB paths

## visuals.rs changes

```rust
iron_plate: ItemModelDef::Mesh(
    asset_server.load(GltfAssetLabel::Scene(0).from_asset("models/IronPlate.glb")),
    ITEM_SIZE,
),
iron_rod: ItemModelDef::Mesh(
    asset_server.load(GltfAssetLabel::Scene(0).from_asset("models/IronRod.glb")),
    ITEM_SIZE,
),
copper_wire: ItemModelDef::Mesh(
    asset_server.load(GltfAssetLabel::Scene(0).from_asset("models/CopperWire.glb")),
    ITEM_SIZE,
),
circuit: ItemModelDef::Mesh(
    asset_server.load(GltfAssetLabel::Scene(0).from_asset("models/Circuit.glb")),
    ITEM_SIZE,
),
```

## render_icons.py additions

```python
{"blend": "IronPlate.blend", "out": "iron_plate.png"},
{"blend": "IronRod.blend",   "out": "iron_rod.png"},
{"blend": "CopperWire.blend","out": "copper_wire.png"},
{"blend": "Circuit.blend",   "out": "circuit.png"},
```

## Verification

Run the the blender/render_icons.py sciprt, and ensure the icons look reasonable and centered.
