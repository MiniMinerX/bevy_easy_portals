//! Gizmos for [`Portal`] debugging.

use bevy::{color::palettes::tailwind::ORANGE_600, prelude::*, render::primitives::Aabb};

use crate::Portal;

#[derive(Reflect, Default, GizmoConfigGroup)]
pub struct PortalGizmos;

/// Gizmo plugin for [`Portal`]s.
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
    portal_query: Query<(&Transform, &Aabb), With<Portal>>,
) {
    for (&transform, aabb) in &portal_query {
        let transform = Transform {
            scale: (aabb.half_extents * 2.0).into(),
            ..transform
        };
        gizmos.cuboid(transform, ORANGE_600);
    }
}
/// System that renders arrows indicating the translation and rotation of [`PortalCamera`]s.
fn debug_portal_cameras(mut gizmos: Gizmos<PortalGizmos>, portal_query: Query<&Portal>) {
    for portal in &portal_query {
        let start = portal.target_transform.translation;
        let end = start + portal.target_transform.forward() * 0.5;
        gizmos.arrow(start, end, ORANGE_600);
    }
}
