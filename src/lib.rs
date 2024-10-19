#[cfg(feature = "gizmos")]
pub mod gizmos;
mod projection;

use bevy::{
    core_pipeline::{
        core_3d::graph::Core3d,
        tonemapping::{DebandDither, Tonemapping},
    },
    ecs::system::SystemParam,
    pbr::{MaterialPipeline, MaterialPipelineKey},
    prelude::*,
    render::{
        camera::{
            CameraMainTextureUsages, CameraProjection, CameraRenderGraph, Exposure, RenderTarget,
        },
        mesh::MeshVertexBufferLayoutRef,
        primitives::{Frustum, HalfSpace},
        render_resource::{
            AsBindGroup, Extent3d, Face, RenderPipelineDescriptor, ShaderRef,
            SpecializedMeshPipelineError, TextureDescriptor, TextureDimension, TextureFormat,
            TextureUsages,
        },
        texture::{TextureFormatPixelInfo, Volume},
        view::{ColorGrading, VisibleEntities},
    },
    window::{PrimaryWindow, WindowRef},
};
use projection::PortalProjection;

/// A plugin that provides the required systems to make a [`Portal`] work.
#[derive(Default)]
pub struct PortalPlugin;

/// Label for systems that update [`Portal`] related [`Camera`]s.
#[derive(Debug, PartialEq, Eq, Clone, Hash, SystemSet)]
pub enum PortalCameraSystem {
    UpdateTransform,
    UpdateFrusta,
}

impl Plugin for PortalPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            projection::plugin,
            MaterialPlugin::<PortalMaterial>::default(),
        ))
        .add_systems(
            PostUpdate,
            (
                update_portal_camera_transform
                    .after(TransformSystem::TransformPropagate)
                    .in_set(PortalCameraSystem::UpdateTransform),
                update_portal_camera_frusta
                    .in_set(PortalCameraSystem::UpdateFrusta)
                    .ambiguous_with(PortalCameraSystem::UpdateFrusta),
            )
                .chain(),
        )
        .observe(setup_portal)
        .register_type::<(Portal, PortalCamera)>();
    }
}

/// Component used to create a portal.
///
/// Adding this to an entity triggers [`setup_portal`] to be ran.
#[derive(Component, Reflect, Debug)]
#[reflect(Component)]
pub struct Portal {
    /// The entity with the primary render camera.
    pub primary_camera: Entity,
    /// The [`Transform`] of this portal's camera.
    pub target_transform: Transform,
    /// The [`Entity`] that has this portal's camera.
    ///
    /// This is set internally by [`setup_portal`] and should not be manually assigned.
    target_camera: Option<Entity>,
    /// Specifies which side of the portal to cull: "front", "back", or neither.
    ///
    /// If set to `None`, both sides of the portal’s mesh will be rendered.
    ///
    /// Defaults to `Some(Face::Back)`, similar to [`StandardMaterial::cull_mode`].
    // TODO: Can this be remotely reflected upstream now that #6042 has landed?
    #[reflect(ignore)]
    pub cull_mode: Option<Face>,
}

impl Portal {
    /// Creates a new [`Portal`] from a given `primary_camera` and `target_transform`.
    ///
    /// # See Also
    ///
    /// * [`Portal::target_transform`]
    #[inline]
    #[must_use]
    pub fn new(primary_camera: Entity, target_transform: Transform) -> Self {
        Self {
            primary_camera,
            target_transform,
            cull_mode: Some(Face::Back),
            target_camera: None,
        }
    }
}

/// Component used to mark a [`Portal`]'s associated camera.
#[derive(Component, Reflect, Debug)]
#[reflect(Component)]
pub struct PortalCamera;

/// Material used for a [`Portal`]'s mesh.
#[derive(Asset, AsBindGroup, Clone, TypePath)]
#[bind_group_data(PortalMaterialKey)]
struct PortalMaterial {
    #[texture(0)]
    #[sampler(1)]
    color_texture: Option<Handle<Image>>,
    cull_mode: Option<Face>,
}

impl Material for PortalMaterial {
    fn fragment_shader() -> ShaderRef {
        "portal.wgsl".into()
    }

    fn specialize(
        _pipeline: &MaterialPipeline<Self>,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        descriptor.primitive.cull_mode = key.bind_group_data.cull_mode;
        Ok(())
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct PortalMaterialKey {
    cull_mode: Option<Face>,
}

impl From<&PortalMaterial> for PortalMaterialKey {
    fn from(material: &PortalMaterial) -> Self {
        Self {
            cull_mode: material.cull_mode,
        }
    }
}

/// System that is triggered whenever a [`Portal`] component is added to an entity.
///
/// An image is created based on the primary camera's viewport size. Then, a [`PortalCamera`] is
/// created, with [`Camera::target`] set to render the [`PortalCamera`]'s view to the image.
///
/// Finally, a [`PortalMaterial`] is added to the [`Portal`] entity.
///
/// # Notes
///
/// * The [`PortalCamera`] will inherit any properties currently present on the primary camera.
fn setup_portal(
    trigger: Trigger<OnAdd, Portal>,
    mut commands: Commands,
    mut portal_query: Query<&mut Portal>,
    primary_camera_query: Query<(
        &Camera,
        Option<&Camera3d>,
        Option<&Tonemapping>,
        Option<&DebandDither>,
        Option<&ColorGrading>,
        Option<&Exposure>,
    )>,
    mut images: ResMut<Assets<Image>>,
    mut portal_materials: ResMut<Assets<PortalMaterial>>,
    viewport_size: ViewportSize,
) {
    let entity = trigger.entity();

    let mut portal = portal_query
        .get_mut(entity)
        .expect("hook guarantees existence of component");

    let Ok((primary_camera, camera_3d, tonemapping, deband_dither, color_grading, exposure)) =
        primary_camera_query.get(portal.primary_camera)
    else {
        error!(
            "could not setup portal {entity}: primary_camera does not contain a Camera component"
        );
        return;
    };

    let image_handle = {
        let Some(size) = viewport_size
            .get_viewport_size(primary_camera)
            .map(|size| Extent3d {
                width: size.x,
                height: size.y,
                ..default()
            })
        else {
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

    portal.target_camera = Some(
        commands
            .spawn((
                Name::new("Portal Camera"),
                Camera {
                    order: -1,
                    target: RenderTarget::Image(image_handle.clone()),
                    ..primary_camera.clone()
                },
                CameraRenderGraph::new(Core3d),
                PortalProjection::default(),
                VisibleEntities::default(),
                Frustum::default(),
                portal.target_transform,
                GlobalTransform::from(portal.target_transform),
                camera_3d.cloned().unwrap_or_default(),
                tonemapping.copied().unwrap_or_default(),
                deband_dither.copied().unwrap_or_default(),
                color_grading.cloned().unwrap_or_default(),
                exposure.copied().unwrap_or_default(),
                CameraMainTextureUsages::default(),
                PortalCamera,
            ))
            .id(),
    );

    commands
        .entity(entity)
        .insert(portal_materials.add(PortalMaterial {
            color_texture: Some(image_handle.clone()),
            cull_mode: portal.cull_mode,
        }));
}

/// System that updates a [`PortalCamera`]'s translation and rotation based on the primary camera.
///
/// # Notes
///
/// * Both [`Transform`] and [`GlobalTransform`] are updated.
pub fn update_portal_camera_transform(
    primary_camera_transform_query: Query<
        &GlobalTransform,
        (With<Camera3d>, Without<PortalCamera>),
    >,
    portal_query: Query<(&GlobalTransform, &Portal), (Without<Camera3d>, Without<PortalCamera>)>,
    mut portal_camera_transform_query: Query<
        (&mut GlobalTransform, &mut Transform),
        With<PortalCamera>,
    >,
) {
    let Ok(primary_camera_transform) = primary_camera_transform_query
        .get_single()
        .map(GlobalTransform::compute_transform)
    else {
        // A valid camera wasn't entity wasn't provided for the portal
        return;
    };

    for (portal_global_transform, portal) in &portal_query {
        let Some((mut portal_camera_global_transform, mut portal_camera_transform)) = portal
            .target_camera
            .and_then(|camera| portal_camera_transform_query.get_mut(camera).ok())
        else {
            continue;
        };

        let portal_transform = portal_global_transform.compute_transform();
        let target_transform = portal.target_transform;

        let translation = primary_camera_transform.translation - portal_transform.translation
            + target_transform.translation;

        let rotation = portal_transform
            .rotation
            .inverse()
            .mul_quat(target_transform.rotation);

        *portal_camera_transform = primary_camera_transform.with_translation(translation);
        portal_camera_transform.rotate_around(target_transform.translation, rotation);
        *portal_camera_global_transform = GlobalTransform::from(*portal_camera_transform);
    }
}

/// System that updates [`Frustum`] for [`PortalCamera`]s.
///
/// [`update_frusta`]: bevy::render::view::update_frusta
fn update_portal_camera_frusta(
    portal_query: Query<&Portal>,
    mut frustum_query: Query<
        (&GlobalTransform, &PortalProjection, &mut Frustum),
        With<PortalCamera>,
    >,
) {
    for portal in &portal_query {
        let Some((global_transform, projection, mut frustum)) = portal
            .target_camera
            .and_then(|camera| frustum_query.get_mut(camera).ok())
        else {
            continue;
        };

        // Apply `bevy::render::view::update_frusta` as usual
        *frustum = projection.compute_frustum(global_transform);

        let target_transform = portal.target_transform;

        // Compute the normal vector for the near clipping plane of the portal camera's frustum.
        //
        // The near clipping plane is the closest plane to the camera where rendering occurs, which
        // is usually aligned with the forward direction of the camera.
        let near_half_space_normal = target_transform.forward();

        // Compute the distance from the portal's exit point (target) to the near clipping plane.
        //
        // This allows the near plane to be set precisely at the point where the portal exits,
        // controlling how close objects can get before they are clipped (disappear) from the
        // camera's view.
        let near_half_space_distance = -target_transform
            .translation
            .dot(near_half_space_normal.normalize_or_zero());

        // Update the near plane of the frustum with the newly calculated normal vector and
        // distance.
        frustum.half_spaces[4] =
            HalfSpace::new(near_half_space_normal.extend(near_half_space_distance));
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
    fn get_viewport_size(&self, camera: &Camera) -> Option<UVec2> {
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
    }
}
