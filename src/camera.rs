use bevy::{
    core_pipeline::tonemapping::{DebandDither, Tonemapping},
    ecs::system::SystemParam,
    image::{TextureFormatPixelInfo, Volume},
    prelude::*,
    render::{
        camera::{Exposure, RenderTarget},
        primitives::{Frustum, HalfSpace},
        render_resource::{
            Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
        },
        view::{ColorGrading, VisibilitySystems},
    },
    window::{PrimaryWindow, WindowRef, WindowResized},
};

use crate::Portal;

/// Plugin that provides [`PortalCamera`] spawning/despawning, transform and frusta updates, and
/// resizing rendered portal images.
pub struct PortalCameraPlugin;

/// Label for systems that update [`Portal`] related cameras.
#[derive(Debug, PartialEq, Eq, Clone, Hash, SystemSet)]
pub enum PortalCameraSystems {
    /// Resizes [`Portal::linked_camera`]'s rendered image if any [`WindowResized`] events are read.
    ResizeImage,
    /// Updates the [`GlobalTransform`] and [`Transform`] components for [`Portal::linked_camera`]
    /// based on the [`Portal::primary_camera`]s [`GlobalTransform`].
    UpdateTransform,
    /// Updates the [`Frustum`] for [`Portal::linked_camera`].
    UpdateFrusta,
}

impl Plugin for PortalCameraPlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(
            PostUpdate,
            (
                PortalCameraSystems::UpdateTransform.after(TransformSystem::TransformPropagate),
                PortalCameraSystems::UpdateFrusta.after(VisibilitySystems::UpdateFrusta),
            )
                .chain(),
        )
        .add_systems(
            PreUpdate,
            resize_portal_images.in_set(PortalCameraSystems::ResizeImage),
        )
        .add_systems(
            PostUpdate,
            (
                update_portal_camera_transform.in_set(PortalCameraSystems::UpdateTransform),
                update_portal_camera_frusta.in_set(PortalCameraSystems::UpdateFrusta),
            ),
        )
        .add_observer(setup_portal_camera)
        .add_observer(despawn_portal_camera)
        .register_type::<(PortalCamera, PortalImage)>();
    }
}

/// Component used to mark a [`Portal`]'s associated camera.
#[derive(Component, Reflect, Debug)]
#[reflect(Component)]
pub struct PortalCamera(pub Entity);

/// Component used to store a weak reference to a [`PortalCamera`]'s rendered image.
#[derive(Component, Reflect, Debug, Deref, DerefMut)]
#[reflect(Component)]
pub struct PortalImage(pub Handle<Image>);

/// System that is triggered whenever a [`Portal`] component is added to an entity.
///
/// An image is created based on the primary camera's viewport size. Then, a [`PortalCamera`] is
/// created, with [`Camera::target`] set to render the [`PortalCamera`]'s view to the image.
///
/// # Notes
///
/// * The [`PortalCamera`] will inherit any properties currently present on the primary camera.
fn setup_portal_camera(
    trigger: Trigger<OnAdd, Portal>,
    mut commands: Commands,
    mut portal_query: Query<&mut Portal>,
    primary_camera_query: Query<(
        &Camera,
        Option<&Camera3d>,
        Option<&DebandDither>,
        Option<&Tonemapping>,
        Option<&ColorGrading>,
        Option<&Exposure>,
    )>,
    mut images: ResMut<Assets<Image>>,
    global_transform_query: Query<&GlobalTransform>,
    viewport_size: ViewportSize,
    manual_views: Res<ManualTextureViews>,
) {
    let entity = trigger.entity();

    let mut portal = portal_query.get_mut(entity).unwrap();

    let Ok((primary_camera, camera_3d, tonemapping, deband_dither, color_grading, exposure)) =
        primary_camera_query.get(portal.primary_camera)
    else {
        error!(
            "could not setup portal camera {entity}: primary_camera does not contain a Camera component"
        );
        return;
    };

    let image_handle = {
        let Some(size) = viewport_size.get_viewport_size(primary_camera) else {
            if let RenderTarget::TextureView(handle) = &primary_camera.target {
                if let Some(manual_view) = manual_views.get(handle) {
                    let size = UVec2::new(manual_view.size.x, manual_view.size.y);    
                }
            }
            
            error!("could not compute viewport size for portal {entity}");
            return;
        };
        let format = TextureFormat::Bgra8UnormSrgb;
        let image = Image {
            data: vec![0; size.volume() * format.pixel_size()],
            texture_descriptor: TextureDescriptor {
                label: None,
                size,
                dimension: TextureDimension::D2,
                format,
                mip_level_count: 1,
                sample_count: 1,
                usage: TextureUsages::TEXTURE_BINDING
                    | TextureUsages::COPY_DST
                    | TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            },
            ..default()
        };
        images.add(image)
    };

    let Ok(global_transform) = global_transform_query.get(portal.target).copied() else {
        error!("portal target is missing a GlobalTransform");
        return;
    };
    portal.linked_camera = Some(
        commands
            .spawn((
                Name::new("Portal Camera"),
                Camera {
                    order: -1,
                    target: RenderTarget::Image(image_handle.clone()),
                    ..primary_camera.clone()
                },
                global_transform.compute_transform(),
                global_transform,
                camera_3d.cloned().unwrap_or_default(),
                tonemapping.copied().unwrap_or_default(),
                deband_dither.copied().unwrap_or_default(),
                color_grading.cloned().unwrap_or_default(),
                exposure.copied().unwrap_or_default(),
                PortalCamera(entity),
            ))
            .id(),
    );

    commands
        .entity(entity)
        .insert(PortalImage(image_handle.clone_weak()));
}

/// System that despawns a [`Portal::linked_camera`] when the [`Portal`] component is removed from
/// a triggered entity.
fn despawn_portal_camera(
    trigger: Trigger<OnRemove, Portal>,
    portal_query: Query<&Portal>,
    mut commands: Commands,
) {
    let portal = portal_query.get(trigger.entity()).unwrap();

    if let Some(linked_camera) = portal.linked_camera {
        commands.entity(linked_camera).despawn_recursive();
    }
}

/// System that updates a [`PortalCamera`]s [`Transform`] and [`GlobalTransform`] based on the
/// primary camera.
fn update_portal_camera_transform(
    portal_query: Query<(&GlobalTransform, &Portal), (Without<Camera3d>, Without<PortalCamera>)>,
    mut portal_camera_transform_query: Query<
        (&mut GlobalTransform, &mut Transform),
        With<PortalCamera>,
    >,
    global_transform_query: Query<&GlobalTransform, Without<PortalCamera>>,
) {
    for (portal_transform, portal) in &portal_query {
        let Ok([primary_camera_transform, target_transform]) =
            global_transform_query.get_many([portal.primary_camera, portal.target])
        else {
            continue;
        };

        let Some((mut portal_camera_global_transform, mut portal_camera_transform)) = portal
            .linked_camera
            .and_then(|camera| portal_camera_transform_query.get_mut(camera).ok())
        else {
            continue;
        };

        // Transform the camera's translation from world space to the portal's space
        let relative_translation = portal_transform
            .affine()
            .inverse()
            .transform_point3(primary_camera_transform.translation());
        // Now transform it back to world space using the target's transform
        let translation = target_transform.transform_point(relative_translation);

        let relative_rotation =
            portal_transform.rotation().inverse() * primary_camera_transform.rotation();
        let rotation = target_transform.rotation() * relative_rotation;

        portal_camera_transform.translation = translation;
        portal_camera_transform.rotation = rotation;

        *portal_camera_global_transform = GlobalTransform::from(*portal_camera_transform);
    }
}

/// System that updates [`Frustum`] for [`PortalCamera`]s.
fn update_portal_camera_frusta(
    portal_query: Query<(&Portal, &GlobalTransform)>,
    mut frustum_query: Query<&mut Frustum, With<PortalCamera>>,
    global_transform_query: Query<&GlobalTransform>,
) {
    for (portal, portal_transform) in &portal_query {
        let Some(linked_camera) = portal.linked_camera else {
            continue;
        };

        let Ok(mut frustum) = frustum_query.get_mut(linked_camera) else {
            continue;
        };

        let Ok([primary_camera_transform, target_transform]) =
            global_transform_query.get_many([portal.primary_camera, portal.target])
        else {
            continue;
        };

        let mut normal = target_transform.forward();

        if portal.flip_near_plane_normal {
            let camera_to_portal =
                portal_transform.translation() - primary_camera_transform.translation();
            if camera_to_portal.dot(*portal_transform.forward()) <= 0.0 {
                normal = -normal;
            }
        }

        let distance = -target_transform
            .translation()
            .dot(normal.normalize_or_zero());
        frustum.half_spaces[4] = HalfSpace::new(normal.extend(distance));
    }
}

/// System that resizes [`PortalImage`]s when the [`WindowResized`] event is fired.
fn resize_portal_images(
    mut resized_reader: EventReader<WindowResized>,
    window_query: Query<&Window>,
    portal_image_query: Query<&PortalImage>,
    mut images: ResMut<Assets<Image>>,
) {
    for event in resized_reader.read() {
        let window_size = window_query.get(event.window).unwrap().physical_size();
        let size = Extent3d {
            width: window_size.x,
            height: window_size.y,
            ..default()
        };

        for portal_image in &portal_image_query {
            let Some(image) = images.get_mut(&portal_image.0) else {
                continue;
            };

            image.resize(size);
        }
    }
}

#[derive(SystemParam)]
struct ViewportSize<'w, 's> {
    primary_window_query: Query<'w, 's, &'static Window, With<PrimaryWindow>>,
    window_query: Query<'w, 's, &'static Window>,
}

impl ViewportSize<'_, '_> {
    /// Retrieves the size of the viewport of a given `camera`.
    ///
    /// Returns [`None`] if no sizing could be obtained, or for any [`RenderTarget`] variant other
    /// than [`RenderTarget::Window`].
    fn get_viewport_size(&self, camera: &Camera) -> Option<Extent3d> {
        match camera.viewport.as_ref() {
            Some(viewport) => Some(viewport.physical_size),
            None => match &camera.target {
                RenderTarget::Window(window_ref) => (match window_ref {
                    WindowRef::Primary => self.primary_window_query.get_single().ok(),
                    WindowRef::Entity(entity) => self.window_query.get(*entity).ok(),
                })
                .map(Window::physical_size),
                _ => None,
            },
        }
        .map(|size| Extent3d {
            width: size.x,
            height: size.y,
            ..default()
        })
    }
}
