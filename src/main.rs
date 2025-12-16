use bevy::{
    color::palettes::css::{FUCHSIA, WHITE},
    image::{ImageFilterMode, ImageLoaderSettings, ImageSampler, ImageSamplerDescriptor},
    pbr::wireframe::{WireframeConfig, WireframePlugin},
    prelude::*,
    render::{
        RenderPlugin,
        settings::{RenderCreation, WgpuFeatures, WgpuSettings},
    },
};
use bevy_flycam::{FlyCam, MovementSettings, NoCameraPlayerPlugin};
use bevy_inspector_egui::{bevy_egui::EguiPlugin, quick::WorldInspectorPlugin};

use crate::clipmap::{Clipmap, ClipmapPlugin};

mod clipmap;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(EguiPlugin::default())
        .add_plugins(WorldInspectorPlugin::default())
        .add_plugins(NoCameraPlayerPlugin)
        .insert_resource(MovementSettings {
            speed: 100.0,
            ..Default::default()
        })
        .add_plugins(ClipmapPlugin)
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
    asset_server: Res<AssetServer>,
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

    let target = commands
        .spawn((
            Camera3d::default(),
            Transform::from_xyz(0.0, 150.0, 0.0).looking_at(Vec3::ZERO, Vec3::Y),
            FlyCam,
        ))
        .id();

    commands.spawn((
        DirectionalLight {
            shadows_enabled: true,
            ..Default::default()
        },
        Transform::from_xyz(0.5, 1.0, 0.0).looking_at(Vec3::ZERO, Vec3::Y),
        // CascadeShadowConfigBuilder {
        //     first_cascade_far_bound: 0.3,
        //     maximum_distance: 3.0,
        //     ..default()
        // }
        // .build(),
    ));

    commands.spawn(Clipmap {
        square_side: 64,
        levels: 6,
        base_scale: 2.0,
        target,
        heightmap: asset_server.load_with_settings(
            "heightmap_1024x1024.ktx2",
            |settings: &mut ImageLoaderSettings| {
                settings.is_srgb = false;
            },
        ),
        color: asset_server.load("color_2048x2048.png"),
        min: -1312.5,
        max: 1312.5,
    });

    // commands.spawn();
}

fn update(
    player: Single<&mut Transform, With<Player>>,
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
) {
    let mut transform = player.into_inner();
    let mut move_vec = Vec3::ZERO;
    if keys.pressed(KeyCode::ArrowUp) {
        move_vec -= Vec3::Z
    }
    if keys.pressed(KeyCode::ArrowDown) {
        move_vec += Vec3::Z
    }
    if keys.pressed(KeyCode::ArrowLeft) {
        move_vec -= Vec3::X
    }
    if keys.pressed(KeyCode::ArrowRight) {
        move_vec += Vec3::X
    }
    transform.translation += move_vec.normalize_or_zero() * time.delta_secs() * 2.0;
}
