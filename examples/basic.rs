use bevy::{color::palettes::tailwind::ORANGE_600, prelude::*};
#[cfg(feature = "gizmos")]
use bevy_easy_portals::gizmos::PortalGizmosPlugin;
use bevy_easy_portals::{Portal, PortalPlugin};

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins,
            PortalPlugin,
            #[cfg(feature = "gizmos")]
            PortalGizmosPlugin,
        ))
        .add_systems(Startup, setup)
        .run();
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let primary_camera = commands
        .spawn(Camera3dBundle {
            camera: Camera {
                clear_color: ClearColorConfig::Custom(Color::BLACK),
                ..default()
            },
            transform: Transform::from_xyz(-3.5, 0.0, 8.0).looking_at(Vec3::ZERO, Vec3::Y),
            ..default()
        })
        .id();

    commands.insert_resource(AmbientLight {
        brightness: 750.0,
        ..default()
    });

    commands.spawn(PbrBundle {
        mesh: meshes.add(Cuboid::default()),
        material: materials.add(Color::from(ORANGE_600)),
        transform: Transform::from_xyz(1.5, 0.0, 0.0),
        ..default()
    });

    let portal_transform = Transform::from_xyz(-1.5, 0.0, 0.0);
    let target_transform = Transform::from_xyz(1.5, 0.0, 2.0);
    let rectangle = Rectangle::from_size(Vec2::splat(2.5));
    commands.spawn((
        meshes.add(rectangle),
        SpatialBundle::from_transform(portal_transform),
        Portal::new(primary_camera, target_transform),
    ));
    // For the purposes of this example, we'll use a plane with transparency to represent the portal
    commands.spawn(PbrBundle {
        mesh: meshes.add(rectangle),
        material: materials.add(Color::WHITE.with_alpha(0.05)),
        transform: portal_transform,
        ..default()
    });
}
