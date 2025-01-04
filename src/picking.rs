//! Portal picking functionality for `bevy_picking`.
//!
//! Add the [`PortalPickingPlugin`] to propagate picking events from backends "through" portals.
//!
//! This module does *not* provide any backend for you. It provides custom inputs that are
//! compatible with any backend. The entity containing the [`Portal`] will need to be picked via a
//! backend, hits will then be sent "through" the target.

use bevy::{
    picking::{
        focus::HoverMap,
        pointer::{Location, PointerAction, PointerId, PointerInput, PointerLocation},
        PickSet,
    },
    prelude::*,
    render::camera::NormalizedRenderTarget,
    utils::HashSet,
};
use uuid::Uuid;

use crate::{camera::PortalImage, Portal};

/// Enables picking "through" [`Portal`]s.
pub struct PortalPickingPlugin;

impl Plugin for PortalPickingPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<PortalInput>()
            .add_systems(
                PreUpdate,
                (
                    portal_inputs.in_set(PickSet::Input),
                    portal_hover.in_set(PickSet::PostFocus),
                ),
            )
            .add_observer(add_pointer);
    }
}

#[derive(Event, Debug)]
struct PortalInput {
    pointer_id: PointerId,
    location: Location,
    action: PointerAction,
}

fn add_pointer(
    trigger: Trigger<OnAdd, PortalImage>,
    mut commands: Commands,
    query: Query<(Entity, &PortalImage)>,
) {
    let (entity, portal_image) = query.get(trigger.entity()).unwrap();

    let location = Location {
        target: NormalizedRenderTarget::Image(portal_image.0.clone()),
        position: Vec2::ZERO,
    };

    commands.entity(entity).insert((
        PointerId::Custom(Uuid::new_v4()),
        PointerLocation::new(location),
    ));
}

fn portal_inputs(
    mut portal_inputs: EventReader<PortalInput>,
    mut output: EventWriter<PointerInput>,
) {
    for event in portal_inputs.read() {
        output.send(PointerInput {
            pointer_id: event.pointer_id,
            location: event.location.clone(),
            action: event.action,
        });
    }
}

fn portal_hover(
    portal_query: Query<(&Portal, &Transform, &PointerId, &PointerLocation)>,
    camera_global_transform_query: Query<(&Camera, &GlobalTransform)>,
    camera_query: Query<&Camera>,
    hover_map: Res<HoverMap>,
    mut pointer_inputs: EventReader<PointerInput>,
    mut portal_inputs: EventWriter<PortalInput>,
    mut drag_events: EventReader<Pointer<Drag>>,
    mut drag_end_events: EventReader<Pointer<DragEnd>>,
) {
    let mut portals: HashSet<(PointerId, Entity)> = HashSet::new();

    for (hover_pointer_id, hits) in hover_map.iter() {
        for (entity, _hit_data) in hits.iter() {
            if portal_query.contains(*entity) {
                portals.insert((*hover_pointer_id, *entity));
            }
        }
    }

    // Currently, we have only retrieved portal entities if they are being hovered. However, this
    // does not allow dragging in-and-out of portals.
    for event in drag_events
        .read()
        .filter(|event| portal_query.contains(event.target))
    {
        portals.insert((event.pointer_id, event.target));
    }
    for event in drag_end_events
        .read()
        .filter(|event| portal_query.contains(event.target))
    {
        portals.insert((event.pointer_id, event.target));
    }

    for (pointer_id, entity) in portals {
        let (portal, &portal_transform, &portal_pointer_id, portal_pointer_location) =
            portal_query.get(entity).unwrap();

        let portal_camera = camera_query.get(portal.linked_camera.unwrap()).unwrap();
        let Ok((primary_camera, primary_camera_transform)) =
            camera_global_transform_query.get(portal.primary_camera)
        else {
            continue;
        };
        let target = portal_pointer_location.location().cloned().unwrap().target;

        for input in pointer_inputs.read() {
            // We only care about inputs related to the hovering pointer
            if input.pointer_id != pointer_id {
                continue;
            }

            // Manually retrieve the current pointer's position, so that it doesn't lag a frame
            // behind
            let Ok(ray) =
                primary_camera.viewport_to_world(primary_camera_transform, input.location.position)
            else {
                continue;
            };
            let Some(distance) = ray.intersect_plane(
                portal_transform.translation,
                InfinitePlane3d::new(portal_transform.forward()),
            ) else {
                continue;
            };
            let Ok(position) =
                portal_camera.world_to_viewport(primary_camera_transform, ray.get_point(distance))
            else {
                continue;
            };

            portal_inputs.send(PortalInput {
                pointer_id: portal_pointer_id,
                location: Location {
                    target: target.clone(),
                    position,
                },
                action: input.action,
            });
        }
    }
}
