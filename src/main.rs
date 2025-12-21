use std::f32::consts::TAU;

use bevy::{
    camera::visibility::NoFrustumCulling,
    color::palettes::css::{FUCHSIA, ORANGE},
    image::ImageLoaderSettings,
    prelude::*,
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
            speed: 1000.0,
            ..Default::default()
        })
        .add_plugins(ClipmapPlugin)
        .add_systems(Startup, setup)
        .add_systems(Update, update)
        .run();
}

/// set up a simple 3D scene
fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
) {
    let target = commands
        .spawn((
            Camera3d::default(),
            Transform::from_xyz(0.0, 150.0, 0.0).looking_at(Vec3::ZERO, Vec3::Y),
            FlyCam,
            // EnvironmentMapLight {
            //     diffuse_map: asset_server.load("pisa_diffuse_rgb9e5_zstd.ktx2"),
            //     specular_map: asset_server.load("pisa_specular_rgb9e5_zstd.ktx2"),
            //     intensity: 1000.0,
            //     ..Default::default()
            // },
            AmbientLight {
                ..Default::default()
            },
        ))
        .id();

    for _ in 0..1 {
        commands.spawn((
            DirectionalLight {
                shadows_enabled: true,
                ..Default::default()
            },
            NoFrustumCulling,
            Transform::default(),
            Mesh3d(meshes.add(Sphere::new(512.0))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: ORANGE.into(),
                unlit: true,
                ..Default::default()
            })),
        ));
    }

    commands.spawn(Clipmap {
        square_side: 16,
        levels: 6,
        base_scale: 16.0,
        texel_size: 1.0,
        target,
        color: asset_server.load("color_2048x2048.png"),
        heightmap: asset_server.load_with_settings(
            "heightmap_1024x1024.ktx2",
            |settings: &mut ImageLoaderSettings| {
                settings.is_srgb = false;
            },
        ),
        horizon: asset_server.load_with_settings(
            "heightmap_horizon_512x512_8.ktx2",
            |settings: &mut ImageLoaderSettings| {
                settings.is_srgb = false;
            },
        ),
        horizon_coeffs: 8,
        min: -1312.5,
        max: 1312.5,
        wireframe: false,
    });
}

fn update(mut lights: Query<&mut Transform, With<DirectionalLight>>, time: Res<Time>) {
    let cnt = lights.count();
    for (i, mut transform) in lights.iter_mut().enumerate() {
        let angle = 0.1 * time.elapsed_secs() + (TAU * i as f32 / cnt as f32);
        *transform =
            Transform::from_translation(Vec3::new(angle.cos() * 8192.0, angle.sin() * 8192.0, 0.0))
                .looking_at(Vec3::ZERO, Vec3::Y);
    }
}
