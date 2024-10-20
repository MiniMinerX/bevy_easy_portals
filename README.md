# `bevy_easy_portals`

Easy-to-use portals for Bevy

![screenshot showing a cube being reflected in a mirror using portals](https://github.com/chompaa/bevy_easy_portals/blob/main/assets/mirror.png)

## Getting Started

First, add `PortalPlugin` to your app, then use the `Portal` component, et voila!

See [the examples](https://github.com/chompaa/bevy_easy_portals/tree/main/examples) for more references.

<details>

<summary>Example usage</summary>

```rust
use bevy::prelude::*;
use bevy_easy_portals::{Portal, PortalPlugin}

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, PortalPlugin))
        .add_systems(Startup, setup)
        .run();
}

fn setup(
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
```

</details>

## Compatibility

| `bevy_easy_portals` | `bevy` |
| :--                 | :--    |
| `0.1`               | `0.14` |

## Features

| Feature                | Description                                           |
| :--                    | :--                                                   |
| `gizmos`               | Use gizmos for the portal's aabb and camera transform |

## Contributing

Feel free to open a PR!

If possible, please try to keep it minimal and scoped.

## Alternatives

- [`bevy_basic_portals`](https://github.com/Selene-Amanita/bevy_basic_portals)
