use std::f32::consts::PI;

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
        .add_systems(Update, rotate_shape)
        .run();
}

#[derive(Component)]
struct Shape;

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
            transform: Transform::from_xyz(2.5, 0.0, 9.0).looking_at(Vec3::ZERO, Vec3::Y),
            ..default()
        })
        .id();

    let shape_transform = Transform::from_xyz(0.0, 0.0, 4.0);
    commands.spawn((
        PbrBundle {
            mesh: meshes.add(Cuboid::default()),
            material: materials.add(Color::from(ORANGE_600)),
            transform: shape_transform,
            ..default()
        },
        Shape,
    ));
    commands.spawn(PointLightBundle {
        point_light: PointLight {
            intensity: 10_000_000.0,
            ..default()
        },
        transform: Transform::from_xyz(0.0, 10.0, 0.0)
            .looking_at(shape_transform.translation, Vec3::Y),
        ..default()
    });

    let mirror_transform = Transform::default();
    // The target should be the transform of the mirror itself, but flipped
    let target_transform = mirror_transform.with_rotation(Quat::from_rotation_y(PI));
    let rectangle = Rectangle::from_size(Vec2::splat(5.0));
    commands.spawn((
        meshes.add(rectangle),
        SpatialBundle::from_transform(mirror_transform),
        Portal::new(primary_camera, target_transform),
    ));
    // For the purposes of this example, we'll use a plane with transparency to represent the mirror
    commands.spawn(PbrBundle {
        mesh: meshes.add(rectangle),
        material: materials.add(Color::WHITE.with_alpha(0.2)),
        transform: mirror_transform,
        ..default()
    });
}

fn setoop(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let primary_camera = commands
        .spawn(Camera3dBundle {
            transform: Transform::from_xyz(0.0, 0.0, 10.0),
            ..default()
        })
        .id();

    // Where you want the portal to be located
    let portal_transform = Transform::default();

    // Where the portal's target camera should be
    let target_transform = Transform::from_xyz(10.0, 0.0, 10.0);

    // Spawn something for the portal to look at
    commands.spawn(PbrBundle {
        mesh: meshes.add(Cuboid::default()),
        material: materials.add(Color::WHITE),
        transform: Transform::from_xyz(10.0, 0.0, 0.0),
        ..default()
    });

    // Spawn the portal, omit a material since one will be added automatically
    commands.spawn((
        meshes.add(Rectangle::default()),
        SpatialBundle::from_transform(portal_transform),
        Portal::new(primary_camera, target_transform),
    ));
}

fn rotate_shape(mut query: Query<&mut Transform, With<Shape>>, time: Res<Time>) {
    let angle = time.delta_seconds() / 2.0;
    for mut transform in &mut query {
        transform.rotate(Quat::from_axis_angle(Vec3::new(1.0, 1.0, 0.0), angle));
    }
}
