use bevy::{
    asset::load_internal_asset,
    core_pipeline::core_3d::CORE_3D_DEPTH_FORMAT,
    pbr::{MaterialPipeline, MaterialPipelineKey},
    prelude::*,
    render::{
        mesh::MeshVertexBufferLayoutRef,
        render_resource::{
            AsBindGroup, CompareFunction, DepthBiasState, DepthStencilState, Face,
            RenderPipelineDescriptor, ShaderRef, SpecializedMeshPipelineError, StencilFaceState,
            StencilState,
        },
    },
    window::WindowResized,
};

use crate::{
    camera::{PortalCameraSystems, PortalImage},
    Portal,
};

pub const PORTAL_SHADER_HANDLE: Handle<Shader> =
    Handle::weak_from_u128(115090128739399034051596692516865947112);

pub struct PortalMaterialPlugin;

impl Plugin for PortalMaterialPlugin {
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
                update_materials::<PortalMaterial>
                    .run_if(on_event::<WindowResized>)
                    .after(PortalCameraSystems::ResizeImage),
            )
            .add_observer(spawn_material);
    }
}

/// Material used for a [`Portal`]'s mesh.
#[derive(Asset, AsBindGroup, Clone, Reflect)]
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
    #[reflect(ignore)]
    pub cull_mode: Option<Face>,
    /// The effect of draw calls on the depth and stencil aspects of the portal.
    ///
    /// You can make use of this field to resolve z-fighting.
    ///
    /// Defaults to the standard mesh [`DepthStencilState`].
    #[reflect(ignore)]
    pub depth_stencil: Option<DepthStencilState>,
}

impl Default for PortalMaterial {
    fn default() -> Self {
        Self {
            base_color_texture: None,
            cull_mode: Some(Face::Back),
            depth_stencil: Some(DepthStencilState {
                format: CORE_3D_DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: CompareFunction::GreaterEqual,
                stencil: StencilState {
                    front: StencilFaceState::IGNORE,
                    back: StencilFaceState::IGNORE,
                    read_mask: 0,
                    write_mask: 0,
                },
                bias: DepthBiasState::default(),
            }),
        }
    }
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
        descriptor.depth_stencil = key.bind_group_data.depth_stencil;
        Ok(())
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct PortalMaterialKey {
    cull_mode: Option<Face>,
    depth_stencil: Option<DepthStencilState>,
}

impl From<&PortalMaterial> for PortalMaterialKey {
    fn from(material: &PortalMaterial) -> Self {
        Self {
            cull_mode: material.cull_mode,
            depth_stencil: material.depth_stencil.clone(),
        }
    }
}

/// Marks all materials `T` that are on [`Portal`] entities as changed in the asset system.
///
/// See https://github.com/bevyengine/bevy/issues/5069 for context.
pub fn update_materials<T: Material>(
    material_query: Query<&MeshMaterial3d<T>, With<Portal>>,
    mut materials: ResMut<Assets<T>>,
) {
    for material_handle in &material_query {
        materials.get_mut(material_handle);
    }
}

fn spawn_material(
    trigger: Trigger<OnAdd, PortalImage>,
    mut commands: Commands,
    portal_query: Query<(&Portal, &PortalImage)>,
    mut materials: ResMut<Assets<PortalMaterial>>,
) {
    let entity = trigger.entity();
    let Ok((portal, portal_image)) = portal_query.get(entity) else {
        return;
    };
    commands
        .entity(entity)
        .insert_if_new(MeshMaterial3d(materials.add(PortalMaterial {
            base_color_texture: Some(portal_image.0.clone()),
            cull_mode: portal.cull_mode,
            ..default()
        })));
}
