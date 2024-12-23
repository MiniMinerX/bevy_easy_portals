//! Portal picking functionality for `bevy_picking`.
//!
//! Add the [`PortalPickingPlugin`] to propagate picking events from backends "through" portals.
//!
//! This module does *not* provide any backend for you. It provides custom inputs that are
//! compatible with any backend.

use bevy::{
    picking::{
        backend::PointerHits,
        pointer::{Location, PointerAction, PointerId, PointerInput, PointerLocation},
        PickSet,
    },
    prelude::*,
};
use uuid::Uuid;

use crate::{Portal, PortalCamera};

/// Enables picking "through" [`Portal`]s.
pub struct PortalPickingPlugin;

impl Plugin for PortalPickingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PreUpdate,
            pointer_inputs.pipe(propagate_hits).in_set(PickSet::Backend),
        )
        .add_observer(add_pointer);
    }
}

fn add_pointer(
    trigger: Trigger<OnAdd, PortalCamera>,
    mut commands: Commands,
    query: Query<(&PortalCamera, &Camera)>,
) {
    let (marker, camera) = query.get(trigger.entity()).unwrap();

    let location = Location {
        target: camera.target.normalize(None).unwrap(),
        position: Vec2::ZERO,
    };

    commands.entity(marker.0).insert((
        PointerId::Custom(Uuid::new_v4()),
        PointerLocation::new(location),
    ));
}

fn pointer_inputs(
    mut pointer_inputs: EventReader<PointerInput>,
) -> Vec<(PointerId, PointerAction)> {
    pointer_inputs
        .read()
        .map(|p| (p.pointer_id, p.action))
        .collect()
}

fn propagate_hits(
    In(pointer_inputs): In<Vec<(PointerId, PointerAction)>>,
    portal_query: Query<(&Portal, &PointerId, &PointerLocation)>,
    global_transform_query: Query<&GlobalTransform>,
    camera_query: Query<&Camera>,
    mut pointer_hits: EventReader<PointerHits>,
    mut output: EventWriter<PointerInput>,
) {
    for hit in pointer_hits.read() {
        for (entity, hit_data) in hit.picks.iter() {
            // Check if the entity hit was a pickable portal
            let Ok((portal, &portal_pointer_id, portal_pointer_location)) =
                portal_query.get(*entity)
            else {
                continue;
            };

            // Get the pointer's location based on the raycast hit
            let portal_camera = camera_query.get(portal.linked_camera.unwrap()).unwrap();
            let Ok(primary_camera_transform) = global_transform_query.get(portal.primary_camera)
            else {
                continue;
            };
            let Ok(position) = portal_camera
                .world_to_viewport(primary_camera_transform, hit_data.position.unwrap())
            else {
                continue;
            };
            let target = portal_pointer_location.location().cloned().unwrap().target;
            let location = Location { target, position };

            // Pipe pointer actions
            for &(_pointer_id, action) in pointer_inputs
                .iter()
                .filter(|(pointer_id, _action)| *pointer_id == hit.pointer)
            {
                output.send(PointerInput {
                    pointer_id: portal_pointer_id,
                    location: location.clone(),
                    action,
                });
            }
        }
    }
}
