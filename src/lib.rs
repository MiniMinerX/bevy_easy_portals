#![doc = include_str!("../README.md")]

#[cfg(feature = "gizmos")]
pub mod gizmos;
#[cfg(feature = "picking")]
pub mod picking;

use bevy::{
    asset::load_internal_asset,
    core_pipeline::tonemapping::{DebandDither, Tonemapping},
    ecs::system::SystemParam,
    image::{TextureFormatPixelInfo, Volume},
    pbr::{MaterialPipeline, MaterialPipelineKey},
    prelude::*,
    render::{
        camera::{Exposure, RenderTarget},
        mesh::MeshVertexBufferLayoutRef,
        primitives::{Frustum, HalfSpace},
        render_resource::{
            AsBindGroup, Extent3d, Face, RenderPipelineDescriptor, ShaderRef,
            SpecializedMeshPipelineError, TextureDescriptor, TextureDimension, TextureFormat,
            TextureUsages,
        },
        view::{ColorGrading, VisibilitySystems},
    },
    window::{PrimaryWindow, WindowRef, WindowResized},
};

const PORTAL_SHADER_HANDLE: Handle<Shader> =
    Handle::weak_from_u128(115090128739399034051596692516865947112);

/// A plugin that provides the required systems to make a [`Portal`] work.
#[derive(Default)]
pub struct PortalPlugin;

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

impl Plugin for PortalPlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(
            app,
            PORTAL_SHADER_HANDLE,
            concat!(env!("CARGO_MANIFEST_DIR"), "/assets/portal.wgsl"),
            Shader::from_wgsl
        );

        app.add_plugins(MaterialPlugin::<PortalMaterial>::default())
            .add_systems(
                PreUpdate,
                resize_portal_images.in_set(PortalCameraSystems::ResizeImage),
            )
            .add_systems(
                PostUpdate,
                (
                    update_portal_camera_transform.in_set(PortalCameraSystems::UpdateTransform),
                    update_portal_camera_frusta.in_set(PortalCameraSystems::UpdateFrusta),
                )
                    .after(TransformSystem::TransformPropagate)
                    .before(VisibilitySystems::UpdateFrusta)
                    .chain(),
            )
            .add_observer(setup_portal)
            .register_type::<(Portal, PortalCamera)>();
    }
}

/// Component used to create a portal.
///
/// Adding this to an entity causes a camera (marked with [`PortalCamera`], and with
/// [`RenderTarget::Image`]) to be spawned, inheriting the primary camera's properties.
///
/// A [`PortalMaterial`] is also inserted on the entity, inherting [`Portal::cull_mode`].
#[derive(Component, Reflect, Debug)]
#[reflect(Component)]
#[require(Transform)]
pub struct Portal {
    /// The entity with the primary render [`Camera`].
    ///
    /// In other words, the [`Camera`] used to look at this portal.
    pub primary_camera: Entity,
    /// The target entity that should be used to decide the camera's position.
    ///
    /// This entity should contain a [`Transform`] component.
    pub target: Entity,
    /// Specifies which side of the portal to cull: "front", "back", or neither.
    ///
    /// If set to `None`, both sides of the portal’s mesh will be rendered.
    ///
    /// Defaults to `Some(Face::Back)`, similar to [`StandardMaterial::cull_mode`].
    // TODO: Can this be remotely reflected upstream now that #6042 has landed?
    #[reflect(ignore)]
    pub cull_mode: Option<Face>,
    /// The [`Entity`] that has this portal's [`PortalCamera`].
    ///
    /// This is set internally and should not be manually assigned.
    pub linked_camera: Option<Entity>,
}

impl Portal {
    /// Creates a new [`Portal`] from a given `primary_camera` and `target`.
    ///
    /// # See Also
    ///
    /// * [`Portal::primary_camera`]
    /// * [`Portal::target`]
    #[inline]
    #[must_use]
    pub fn new(primary_camera: Entity, target: Entity) -> Self {
        Self {
            primary_camera,
            target,
            cull_mode: Some(Face::Back),
            linked_camera: None,
        }
    }

    #[inline]
    #[must_use]
    pub fn with_cull_mode(mut self, cull_mode: Option<Face>) -> Self {
        self.cull_mode = cull_mode;
        self
    }
}

/// Component used to mark a [`Portal`]'s associated camera.
#[derive(Component, Reflect, Debug)]
#[reflect(Component)]
#[require(Camera3d)]
pub struct PortalCamera(pub Entity);

/// Material used for a [`Portal`]'s mesh.
#[derive(Asset, AsBindGroup, Clone, TypePath)]
#[bind_group_data(PortalMaterialKey)]
pub struct PortalMaterial {
    #[texture(0)]
    #[sampler(1)]
    base_color_texture: Option<Handle<Image>>,
    /// Specifies which side of the portal to cull: "front", "back", or neither.
    ///
    /// If set to `None`, both sides of the portal’s mesh will be rendered.
    ///
    /// This field's value is inherited from what is set on [`Portal`], but not kept in sync.
    ///
    /// Defaults to `Some(Face::Back)`, similar to [`StandardMaterial::cull_mode`] and [`Portal`].
    pub cull_mode: Option<Face>,
}

impl Material for PortalMaterial {
    fn fragment_shader() -> ShaderRef {
        PORTAL_SHADER_HANDLE.into()
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
pub struct PortalMaterialKey {
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
        Option<&DebandDither>,
        Option<&Tonemapping>,
        Option<&ColorGrading>,
        Option<&Exposure>,
    )>,
    mut images: ResMut<Assets<Image>>,
    mut portal_materials: ResMut<Assets<PortalMaterial>>,
    global_transform_query: Query<&GlobalTransform>,
    viewport_size: ViewportSize,
) {
    let entity = trigger.entity();

    let mut portal = portal_query
        .get_mut(entity)
        .expect("observer guarantees existence of component");

    let Ok((primary_camera, camera_3d, tonemapping, deband_dither, color_grading, exposure)) =
        primary_camera_query.get(portal.primary_camera)
    else {
        error!(
            "could not setup portal {entity}: primary_camera does not contain a Camera component"
        );
        return;
    };

    let image_handle = {
        let Some(size) = viewport_size.get_viewport_size(primary_camera) else {
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
        .insert(MeshMaterial3d(portal_materials.add(PortalMaterial {
            base_color_texture: Some(image_handle.clone()),
            cull_mode: portal.cull_mode,
        })));
}

/// System that updates a [`PortalCamera`]'s translation and rotation based on the primary camera.
///
/// # Notes
///
/// * Both [`Transform`] and [`GlobalTransform`] are updated.
fn update_portal_camera_transform(
    primary_camera_transform_query: Query<
        &GlobalTransform,
        (With<Camera3d>, Without<PortalCamera>),
    >,
    portal_query: Query<(&GlobalTransform, &Portal), (Without<Camera3d>, Without<PortalCamera>)>,
    mut portal_camera_transform_query: Query<
        (&mut GlobalTransform, &mut Transform),
        With<PortalCamera>,
    >,
    target_global_transform_query: Query<
        &GlobalTransform,
        (Without<Camera3d>, Without<PortalCamera>, Without<Portal>),
    >,
) {
    for (portal_global_transform, portal) in &portal_query {
        let Ok(primary_camera_transform) = primary_camera_transform_query
            .get(portal.primary_camera)
            .map(GlobalTransform::compute_transform)
        else {
            continue;
        };

        let Some(linked_camera) = portal.linked_camera else {
            continue;
        };

        // `PortalCamera` requires `Camera3d`
        let (mut portal_camera_global_transform, mut portal_camera_transform) =
            portal_camera_transform_query
                .get_mut(linked_camera)
                .unwrap();

        let portal_transform = portal_global_transform.compute_transform();
        // If the `Portal` has a valid `linked_camera`, this is guaranteed.
        let target_transform = target_global_transform_query
            .get(portal.target)
            .unwrap()
            .compute_transform();

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
    mut frustum_query: Query<&mut Frustum, With<PortalCamera>>,
    global_transform_query: Query<&GlobalTransform>,
) {
    for portal in &portal_query {
        let Some(linked_camera) = portal.linked_camera else {
            continue;
        };

        // `PortalCamera` requires `Camera3d`.
        let mut frustum = frustum_query.get_mut(linked_camera).unwrap();

        // If the `Portal` has a valid `linked_camera`, this is guaranteed.
        let (target_transform, portal_camera_transform) = global_transform_query
            .get_many([portal.target, linked_camera])
            .map(|[t, c]| (t.compute_transform(), c.compute_transform()))
            .unwrap();

        // Set the near clip plane
        let normal = -target_transform.forward().normalize_or_zero();
        let distance =
            -((target_transform.translation - portal_camera_transform.translation).dot(normal));
        frustum.half_spaces[4] = HalfSpace::new(normal.extend(distance));
    }
}

fn resize_portal_images(
    mut resized_reader: EventReader<WindowResized>,
    window_query: Query<&Window>,
    portal_query: Query<(&Portal, &MeshMaterial3d<PortalMaterial>)>,
    camera_query: Query<&Camera>,
    mut images: ResMut<Assets<Image>>,
    mut portal_materials: ResMut<Assets<PortalMaterial>>,
) {
    for event in resized_reader.read() {
        let window_size = window_query.get(event.window).unwrap().physical_size();
        let size = Extent3d {
            width: window_size.x,
            height: window_size.y,
            ..default()
        };

        for (portal, portal_material_handle) in &portal_query {
            let Some(camera) = portal.linked_camera.and_then(|c| camera_query.get(c).ok()) else {
                continue;
            };

            let RenderTarget::Image(ref image_handle) = camera.target else {
                continue;
            };

            let Some(image) = images.get_mut(image_handle) else {
                continue;
            };

            image.resize(size);
            // Blocked on https://github.com/bevyengine/bevy/issues/5069
            portal_materials.get_mut(portal_material_handle);
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
