use bevy::{
    asset::RenderAssetUsages,
    camera::{CameraUpdateSystems, ImageRenderTarget, RenderTarget},
    ecs::system::SystemParam,
    log::error,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages},
    window::{PrimaryWindow, WindowRef, WindowResized},
};

use crate::Portal;

/// Plugin that provides [`PortalCamera`] spawning/despawning, transform and frusta updates, and
/// resizing rendered portal images.
pub struct PortalCameraPlugin;

/// Label for systems that update [`Portal`] related cameras.
#[derive(Debug, PartialEq, Eq, Clone, Hash, SystemSet)]
pub enum PortalCameraSystems {
    /// Resizes [`Portal::linked_camera`]'s rendered image if any [`WindowResized`] messages are
    /// read.
    ResizeImage,
    /// Updates the [`GlobalTransform`] and [`Transform`] components for [`Portal::linked_camera`]
    /// based on the [`Portal::primary_camera`]s [`GlobalTransform`].
    UpdateTransform,
    /// Updates the [`Frustum`] for [`Portal::linked_camera`].
    UpdateProjection,
}

impl Plugin for PortalCameraPlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(
            PostUpdate,
            (
                PortalCameraSystems::UpdateTransform,
                PortalCameraSystems::UpdateProjection,
            )
                .before(CameraUpdateSystems)
                .chain(),
        )
        .configure_sets(
            PreUpdate,
            PortalCameraSystems::ResizeImage.run_if(on_message::<WindowResized>),
        )
        .add_systems(
            PreUpdate,
            resize_portal_images.in_set(PortalCameraSystems::ResizeImage),
        )
        .add_systems(
            PostUpdate,
            (
                update_portal_camera_transform.in_set(PortalCameraSystems::UpdateTransform),
                update_portal_camera_projection.in_set(PortalCameraSystems::UpdateProjection),
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

/// System that despawns a [`Portal::linked_camera`] when the [`Portal`] component is removed from
/// a triggered entity.
fn despawn_portal_camera(
    trigger: On<Remove, Portal>,
    portal_query: Query<&Portal>,
    mut commands: Commands,
) {
    let portal = portal_query.get(trigger.event_target()).unwrap();
    commands.entity(portal.linked_camera).despawn();
}

/// System that is triggered whenever a [`Portal`] component is added to an entity.
///
/// An image is created based on the primary camera's viewport size. Then, a [`PortalCamera`] is
/// created, with [`Camera::target`] set to render the [`PortalCamera`]'s view to the image.
///
/// # Notes
///
/// * The [`PortalCamera`] will inherit any properties currently present on the primary camera.
fn setup_portal_camera(
    trigger: On<Add, Portal>,
    mut commands: Commands,
    mut portal_query: Query<&mut Portal>,
    primary_camera_query: Query<&Camera>,
    global_transform_query: Query<&GlobalTransform>,
    mut portal_images: PortalImages,
) {
    let entity = trigger.event_target();

    let mut portal = portal_query.get_mut(entity).unwrap();

    let Ok(primary_cam) = primary_camera_query.get(portal.primary_camera) else {
        error!(
            "could not setup portal camera {entity}: primary_camera does not contain a Camera component"
        );
        return;
    };

    let Some(image_handle) = portal_images.with_camera(primary_cam) else {
        error!("could not create portal image for {entity}");
        return;
    };

    let Ok(global_transform) = global_transform_query.get(portal.target).copied() else {
        error!("portal target is missing a GlobalTransform");
        return;
    };

    commands
        .entity(entity)
        .insert(PortalImage(image_handle.clone()));

    let mut linked_cam_commands = commands.spawn((
        Camera {
            order: -1,
            ..Default::default()
        },
        Camera3d::default(),
        global_transform.compute_transform(),
        global_transform,
        PortalCamera(entity),
    ));
    if let Some(camera_spawn_fn) = &mut portal.camera_spawn {
        (camera_spawn_fn)(&mut linked_cam_commands);
    }
    let linked_cam = linked_cam_commands.id();
    commands
        .entity(linked_cam)
        .entry::<Camera>()
        .and_modify(move |mut camera| {
            camera.target = RenderTarget::Image(ImageRenderTarget {
                handle: image_handle.clone(),
                scale_factor: 1.0,
            });
        });
    portal.linked_camera = linked_cam;
}

/// System that updates a [`PortalCamera`]s [`Transform`] and [`GlobalTransform`] based on the
/// primary camera.
pub fn update_portal_camera_transform(
    portal_query: Query<(&Portal, Entity), (Without<Camera3d>, Without<PortalCamera>)>,
    mut params: ParamSet<(
        TransformHelper,
        Query<(&mut GlobalTransform, &mut Transform), With<PortalCamera>>,
        Query<&mut GlobalTransform, (Without<PortalCamera>, Without<Portal>)>,
    )>,
) {
    for (portal, portal_entity) in portal_query.iter() {
        let transform_helper = params.p0();

        let Ok(portal_transform) = transform_helper.compute_global_transform(portal_entity) else {
            continue;
        };
        let Ok(primary_camera_transform) =
            transform_helper.compute_global_transform(portal.primary_camera)
        else {
            continue;
        };
        let Ok(target_transform) = transform_helper.compute_global_transform(portal.target) else {
            continue;
        };

        let relative_translation = portal_transform
            .affine()
            .inverse()
            .transform_point3(primary_camera_transform.translation());
        let translation = target_transform.transform_point(relative_translation);

        let relative_rotation =
            portal_transform.rotation().inverse() * primary_camera_transform.rotation();
        let rotation = target_transform.rotation() * relative_rotation;

        // Update portal camera transform
        let mut portal_camera_query = params.p1();
        let Ok((mut portal_camera_global_transform, mut portal_camera_transform)) =
            portal_camera_query.get_mut(portal.linked_camera)
        else {
            continue;
        };

        portal_camera_transform.translation = translation;
        portal_camera_transform.rotation = rotation;
        *portal_camera_global_transform = GlobalTransform::from(Transform {
            translation,
            rotation,
            ..default()
        });

        let mut target_query = params.p2();
        if let Ok(mut global_transform) = target_query.get_mut(portal.target) {
            *global_transform = target_transform;
        }
    }
}

/// System that updates [`Projection`]s for [`PortalCamera`]s.
fn update_portal_camera_projection(
    portal_query: Query<(&Portal, &GlobalTransform)>,
    mut projections: Query<&mut Projection, With<PortalCamera>>,
    global_transform_query: Query<&GlobalTransform>,
) {
    for (portal, portal_transform) in &portal_query {
        let Ok(mut projection) = projections.get_mut(portal.linked_camera) else {
            continue;
        };
        let Projection::Perspective(projection) = &mut *projection else {
            continue;
        };

        let Ok(
            [
                portal_camera_transform,
                primary_camera_transform,
                target_transform,
            ],
        ) = global_transform_query.get_many([
            portal.linked_camera,
            portal.primary_camera,
            portal.target,
        ])
        else {
            continue;
        };

        let mut world_normal = target_transform.forward().normalize();

        if portal.flip_near_plane_normal {
            let primary_to_portal =
                portal_transform.translation() - primary_camera_transform.translation();
            let dot = primary_to_portal.dot(*portal_transform.forward());
            if dot <= 0.0 {
                world_normal = -world_normal;
            }
        }

        let view_from_world = portal_camera_transform.affine().matrix3.inverse();
        let view_space_normal = (view_from_world * world_normal).normalize();
        let view_space_target = portal_camera_transform
            .affine()
            .inverse()
            .transform_point3(target_transform.translation());
        let distance = -view_space_normal.dot(view_space_target);

        projection.near_clip_plane = view_space_normal.extend(distance);
    }
}

/// System that resizes [`PortalImage`]s when the [`WindowResized`] message is fired.
fn resize_portal_images(
    primary_cameras: Query<&Camera, Without<PortalCamera>>,
    mut portal_cameras: Query<&mut Camera, With<PortalCamera>>,
    mut portals: Query<(&Portal, &mut PortalImage)>,
    mut portal_images: PortalImages,
) {
    for (portal, mut portal_image) in &mut portals {
        let Ok(primary_camera) = primary_cameras.get(portal.primary_camera) else {
            continue;
        };

        let Ok(mut camera) = portal_cameras.get_mut(portal.linked_camera) else {
            continue;
        };

        let Some(id) = portal_images.replace(&portal_image.0, &mut camera, primary_camera) else {
            continue;
        };
        portal_image.0 = id;
    }
}

#[derive(SystemParam)]
struct PortalImages<'w, 's> {
    primary_window_query: Query<'w, 's, &'static Window, With<PrimaryWindow>>,
    window_query: Query<'w, 's, &'static Window>,
    images: ResMut<'w, Assets<Image>>,
    manual_texture_views: Res<'w, ManualTextureViews>,
}

impl PortalImages<'_, '_> {
    fn replace(
        &mut self,
        id: &Handle<Image>,
        camera: &mut Camera,
        primary_camera: &Camera,
    ) -> Option<Handle<Image>> {
        self.images.remove(id);
        let image = self.with_camera(primary_camera)?;
        camera.target = image.clone().into();
        Some(image)
    }

    /// Creates a new [`Image`] with size matching the given `camera`.
    ///
    /// Returns `None` if no viewport size could be obtained.
    fn with_camera(&mut self, camera: &Camera) -> Option<Handle<Image>> {
        let size = self.get_viewport_size(camera)?;
        let mut image = Image::new_uninit(
            size,
            TextureDimension::D2,
            TextureFormat::Bgra8UnormSrgb,
            RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
        );
        image.texture_descriptor.usage |= TextureUsages::TEXTURE_BINDING
            | TextureUsages::COPY_DST
            | TextureUsages::RENDER_ATTACHMENT;
        let handle = self.images.add(image);
        Some(handle)
    }

    /// Retrieves the size of the viewport of a given `camera`.
    ///
    /// Returns `None` if no sizing could be obtained.
    fn get_viewport_size(&self, camera: &Camera) -> Option<Extent3d> {
        match camera.viewport.as_ref() {
            Some(viewport) => Some(viewport.physical_size),
            None => match &camera.target {
                RenderTarget::Window(window_ref) => (match window_ref {
                    WindowRef::Primary => Some(self.primary_window_query.single().unwrap()),
                    WindowRef::Entity(entity) => Some(self.window_query.get(*entity).unwrap()),
                })
                .map(Window::physical_size),
                RenderTarget::Image(img_render_handle) => {
                    self.images.get(&img_render_handle.handle).map(Image::size)
                }
                RenderTarget::TextureView(handle) => self
                    .manual_texture_views
                    .get(handle)
                    .map(|texture| texture.size),
                RenderTarget::None { size } => Some(*size),
            },
        }
        .map(|size| Extent3d {
            width: size.x,
            height: size.y,
            ..default()
        })
    }
}
