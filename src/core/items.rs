use bevy::prelude::*;

use crate::core::{BeltPlaceable, IndependentPlaceable, Item, ItemName, ItemRegistry, NotWorldPlaceable};

pub fn register_all(world: &mut World, registry: &mut ItemRegistry) {
    let e = world
        .spawn((Item(0), ItemName("Item"), NotWorldPlaceable))
        .id();
    registry.0.insert(Item(0), e);

    let e = world
        .spawn((Item(1), ItemName("Belt"), BeltPlaceable))
        .id();
    registry.0.insert(Item(1), e);

    let e = world
        .spawn((Item(3), ItemName("Source"), IndependentPlaceable))
        .id();
    registry.0.insert(Item(3), e);

    let e = world
        .spawn((Item(4), ItemName("Sink"), IndependentPlaceable))
        .id();
    registry.0.insert(Item(4), e);
}
