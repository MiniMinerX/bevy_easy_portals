//! Gizmos for [`Portal`] debugging.

use bevy::{color::palettes::tailwind::ORANGE_600, prelude::*, render::primitives::Aabb};

use crate::Portal;

#[derive(Reflect, Default, GizmoConfigGroup)]
pub struct PortalGizmos;

/// Gizmo plugin for [`Portal`] debugging.
///
/// These gizmos help visualize aspects like [`Portal`] meshes and where the
/// [`Portal::target_transform`] is located (along with its facing direction).
pub struct PortalGizmosPlugin;

impl Plugin for PortalGizmosPlugin {
    fn build(&self, app: &mut App) {
        app.init_gizmo_group::<PortalGizmos>()
            .add_systems(Update, (debug_portal_meshes, debug_portal_cameras));
    }
}

/// System that renders the [`Aabb`]s of a [`Portal`]'s mesh.
fn debug_portal_meshes(
    mut gizmos: Gizmos<PortalGizmos>,
    // Query GlobalTransform instead of local Transform
    portal_query: Query<(&GlobalTransform, &Aabb), With<Portal>>,
) {
    for (&global_transform, aabb) in &portal_query {
        // Create a new Transform from the GlobalTransform, but keep the Aabb's scale
        // The Aabb's half_extents are in local space, so we want to apply them
        // relative to the global orientation.
        let gizmo_transform = Transform {
            translation: global_transform.translation(),
            rotation: global_transform.rotation(),
            scale: (aabb.half_extents * 2.0).into(), // Aabb half_extents are already in local space of the mesh
        };
        gizmos.cuboid(gizmo_transform, ORANGE_600);
    }
}

/// System that renders arrows indicating the translation and rotation of [`PortalCamera`]s.
fn debug_portal_cameras(
    mut gizmos: Gizmos<PortalGizmos>,
    portal_query: Query<(&Portal, &GlobalTransform)>, // Query GlobalTransform for the portal itself
    global_transform_query: Query<&GlobalTransform>,
) {
    for (portal, portal_global_transform) in &portal_query {
        let target_transform = global_transform_query
            .get(portal.target)
            .map(GlobalTransform::compute_transform)
            .expect("target should have GlobalTransform");

        // Gizmo for the portal's target (already correct)
        let start_target = target_transform.translation;
        let end_target = start_target + target_transform.forward() * 0.5;
        gizmos.arrow(start_target, end_target, ORANGE_600);
    }
}
