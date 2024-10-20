//! A wrapper around [`Projection`].

use bevy::{
    math::Vec3A,
    pbr::PbrProjectionPlugin,
    prelude::*,
    render::camera::{camera_system, CameraProjection, CameraUpdateSystem},
};

/// Duplicate of [`CameraProjectionPlugin`] without [`update_frusta`]. This plugin thus allows for
/// fine-grained control of a [`Camera`]'s [`Frustum`].
///
/// [`CameraProjectionPlugin`]: bevy::render::camera::CameraProjectionPlugin
/// [`update_frusta`]: bevy::render::view::update_frusta
/// [`Frustum`]: bevy::render::primitives::Frustum
pub(super) fn plugin(app: &mut App) {
    app.add_plugins(PbrProjectionPlugin::<PortalProjection>::default())
        .add_systems(
            PostStartup,
            camera_system::<PortalProjection>
                .in_set(CameraUpdateSystem)
                .ambiguous_with(CameraUpdateSystem),
        )
        .add_systems(
            PostUpdate,
            camera_system::<PortalProjection>
                .in_set(CameraUpdateSystem)
                .ambiguous_with(CameraUpdateSystem),
        )
        .register_type::<PortalProjection>();
}

/// Wrapper around [`Projection`], which is a  configurable [`CameraProjection`] that can select
/// its projection type at runtime.
///
/// # See Also
///
/// * [`plugin`]
#[derive(Component, Debug, Clone, Reflect, Default, Deref, DerefMut)]
#[reflect(Component, Default)]
pub(super) struct PortalProjection(Projection);

impl CameraProjection for PortalProjection {
    fn get_clip_from_view(&self) -> Mat4 {
        self.0.get_clip_from_view()
    }

    fn update(&mut self, width: f32, height: f32) {
        self.0.update(width, height);
    }

    fn far(&self) -> f32 {
        self.0.far()
    }

    fn get_frustum_corners(&self, z_near: f32, z_far: f32) -> [Vec3A; 8] {
        self.0.get_frustum_corners(z_near, z_far)
    }
}
