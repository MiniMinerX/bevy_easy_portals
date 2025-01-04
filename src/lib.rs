#![doc = include_str!("../README.md")]

pub mod camera;
#[cfg(feature = "gizmos")]
pub mod gizmos;
pub mod material;
#[cfg(feature = "picking")]
pub mod picking;

use bevy::{app::PluginGroupBuilder, prelude::*, render::render_resource::Face};

/// A group of plugins that provides the required systems to make a [`Portal`] work.
pub struct PortalPlugins;

impl PluginGroup for PortalPlugins {
    fn build(self) -> PluginGroupBuilder {
        PluginGroupBuilder::start::<Self>()
            .add(camera::PortalCameraPlugin)
            .add(material::PortalMaterialPlugin)
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
