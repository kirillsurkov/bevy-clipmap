use bevy::{
    color::palettes::css::FUCHSIA,
    pbr::wireframe::{WireframeConfig, WireframePlugin},
    prelude::*,
};
use bevy_inspector_egui::{bevy_egui::EguiPlugin, quick::WorldInspectorPlugin};

use crate::clipmap::{Clipmap, ClipmapPlugin};

mod clipmap;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(EguiPlugin::default())
        .add_plugins(WorldInspectorPlugin::default())
        .add_plugins(WireframePlugin::default())
        .insert_resource(WireframeConfig {
            global: true,
            ..Default::default()
        })
        .add_plugins(ClipmapPlugin { square_side: 9 })
        .add_systems(Startup, setup)
        .add_systems(Update, update)
        .run();
}

#[derive(Component)]
struct Player;

/// set up a simple 3D scene
fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let target = commands
        .spawn((
            Player,
            Mesh3d(meshes.add(Sphere::new(0.5))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: FUCHSIA.into(),
                unlit: true,
                ..Default::default()
            })),
        ))
        .id();

    commands.spawn(Clipmap {
        levels: 6,
        base_scale: 1.0,
        target,
    });

    // camera
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 150.0, 1.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

fn update(
    player: Single<&mut Transform, With<Player>>,
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
) {
    let mut transform = player.into_inner();
    let mut move_vec = Vec3::ZERO;
    if keys.pressed(KeyCode::KeyW) {
        move_vec -= Vec3::Z
    }
    if keys.pressed(KeyCode::KeyS) {
        move_vec += Vec3::Z
    }
    if keys.pressed(KeyCode::KeyA) {
        move_vec -= Vec3::X
    }
    if keys.pressed(KeyCode::KeyD) {
        move_vec += Vec3::X
    }
    transform.translation += move_vec.normalize_or_zero() * time.delta_secs() * 2.0;
}
