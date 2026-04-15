use avian3d::prelude::*;
use bevy::prelude::*;

use crate::core::RaycastTarget;

pub struct PhysicsPlugin;
impl Plugin for PhysicsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(PhysicsPlugins::default());

        app.add_systems(Update, add_block_colliders);
    }
}

/// Gives static physics colliders to every world block as it is placed.
fn add_block_colliders(
    mut cmd: Commands,
    blocks: Query<(Entity, &RaycastTarget), Added<RaycastTarget>>,
) {
    for (entity, rt) in &blocks {
        let half = rt.half_extents;
        // The block's Transform is at its bottom corner. Bake the Y offset
        // directly into a compound collider on the block entity itself so no
        // child entity is needed. This keeps all block colliders out of
        // Bevy's transform hierarchy, avoiding per-frame propagation cost
        // across thousands of static blocks.
        cmd.entity(entity).insert((
            RigidBody::Static,
            Collider::compound(vec![(
                Vec3::new(0.0, half.y, 0.0),
                Quat::IDENTITY,
                Collider::cuboid(half.x * 2.0, half.y * 2.0, half.z * 2.0),
            )]),
        ));
    }
}
