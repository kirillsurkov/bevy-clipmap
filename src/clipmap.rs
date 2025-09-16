use std::{
    collections::HashMap,
    f32::consts::{FRAC_PI_2, PI},
};

use bevy::{
    asset::RenderAssetUsages,
    color::palettes::css::{BLUE, LIME, RED, TEAL},
    prelude::*,
    render::mesh::{Indices, PrimitiveTopology},
};

pub struct ClipmapPlugin {
    pub square_side: u32,
}

#[derive(Resource)]
struct State {
    square_side: u32,
    handle_square: Handle<Mesh>,
    handle_filler: Handle<Mesh>,
    handle_center: Handle<Mesh>,
    handle_trim: Handle<Mesh>,
}

impl Plugin for ClipmapPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(State {
            square_side: self.square_side,
            handle_square: Handle::default(),
            handle_filler: Handle::default(),
            handle_center: Handle::default(),
            handle_trim: Handle::default(),
        })
        .add_systems(Startup, setup)
        .add_systems(PreUpdate, (init_clipmaps, init_grids))
        .add_systems(Update, update_grids);
    }
}

struct MeshBuilder {
    unique_vertices: HashMap<(i32, i32), u32>,
    vertices: Vec<[f32; 3]>,
    indices: Vec<u32>,
}

impl MeshBuilder {
    fn new() -> Self {
        Self {
            unique_vertices: HashMap::new(),
            vertices: vec![],
            indices: vec![],
        }
    }

    fn add_vertex(&mut self, x: i32, y: i32) -> u32 {
        if let Some(index) = self.unique_vertices.get(&(x, y)) {
            *index
        } else {
            let index = self.vertices.len() as u32;
            self.vertices.push([x as f32, 0.0, y as f32]);
            self.unique_vertices.insert((x, y), index);
            index
        }
    }

    fn add(&mut self, x: i32, y: i32) {
        let p1 = self.add_vertex(x, y);
        let p2 = self.add_vertex(x, y + 1);
        let p3 = self.add_vertex(x + 1, y + 1);
        let p4 = self.add_vertex(x + 1, y);
        self.indices.extend([p1, p2, p3]);
        self.indices.extend([p1, p3, p4]);
    }

    fn build(self) -> Mesh {
        Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::all())
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.vertices)
            .with_inserted_indices(Indices::U32(self.indices))
    }
}

fn setup(mut meshes: ResMut<Assets<Mesh>>, mut state: ResMut<State>) {
    let square_side = state.square_side as i32 - 1;
    let side = square_side * 4 + 1;

    let mut square = MeshBuilder::new();
    let mut filler = MeshBuilder::new();
    let mut center = MeshBuilder::new();
    let mut trim = MeshBuilder::new();

    for xy in 0..(side + 1) * (side + 1) {
        let x = xy % (side + 1);
        let y = xy / (side + 1);
        if x < square_side && y < square_side {
            square.add(x, y);
        }
        if (x == side / 2 || y == side / 2) && x < side && y < side {
            center.add(x - side / 2, y - side / 2);
            let range = square_side..=side - square_side - 1;
            if !range.contains(&x) || !range.contains(&y) {
                filler.add(x - (side + 1) / 2 + 1, y - (side + 1) / 2 + 1);
            }
        }
        if x == side || y == side {
            trim.add(x - (side + 1) / 2, y - (side + 1) / 2);
        }
    }

    state.handle_square = meshes.add(square.build());
    state.handle_filler = meshes.add(filler.build());
    state.handle_center = meshes.add(center.build());
    state.handle_trim = meshes.add(trim.build());
}

#[derive(Component)]
pub struct Clipmap {
    pub levels: u32,
    pub base_scale: f32,
    pub target: Entity,
}

#[derive(Component)]
struct ClipmapGrid {
    level: u32,
    trim: Entity,
}

impl ClipmapGrid {
    fn scale(&self, base_scale: f32) -> f32 {
        base_scale * 2u32.pow(self.level) as f32
    }
}

fn init_clipmaps(mut commands: Commands, clipmaps: Query<(Entity, &Clipmap), Added<Clipmap>>) {
    for (entity, clipmap) in clipmaps {
        let mut entity = commands.entity(entity);
        entity.insert_if_new((Transform::default(), Visibility::default()));
        for level in 0..clipmap.levels {
            entity.with_child(ClipmapGrid {
                level,
                trim: Entity::PLACEHOLDER,
            });
        }
    }
}

fn init_grids(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    state: Res<State>,
    clipmaps: Query<&Clipmap>,
    mut grids: Query<(Entity, &mut ClipmapGrid, &ChildOf), Added<ClipmapGrid>>,
) {
    for (entity, mut grid, clipmap) in &mut grids {
        let clipmap = clipmaps.get(clipmap.parent()).unwrap();
        commands.entity(entity).insert_if_new((
            Transform::from_scale(Vec3::splat(grid.scale(clipmap.base_scale))),
            Visibility::default(),
        ));
        for y in 0..4 {
            for x in 0..4 {
                if grid.level != 0 && (x == 1 || x == 2) && (y == 1 || y == 2) {
                    continue;
                }
                let offset_x = if x >= 2 { 1.0 } else { 0.0 };
                let offset_y = if y >= 2 { 1.0 } else { 0.0 };
                commands.entity(entity).with_child((
                    Mesh3d(state.handle_square.clone_weak()),
                    MeshMaterial3d(
                        materials.add(StandardMaterial {
                            base_color: if (x % 2 == 0) ^ (y % 2 == 0) {
                                BLUE
                            } else {
                                TEAL
                            }
                            .into(),
                            unlit: true,
                            ..Default::default()
                        }),
                    ),
                    Transform::from_xyz(
                        (x - 2) as f32 * (state.square_side - 1) as f32 + offset_x,
                        0.0,
                        (y - 2) as f32 * (state.square_side - 1) as f32 + offset_y,
                    ),
                ));
            }
        }

        if grid.level == 0 {
            commands.entity(entity).with_child((
                Mesh3d(state.handle_center.clone_weak()),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: RED.into(),
                    unlit: true,
                    ..Default::default()
                })),
            ));
        } else {
            commands.entity(entity).with_child((
                Mesh3d(state.handle_filler.clone_weak()),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: RED.into(),
                    unlit: true,
                    ..Default::default()
                })),
            ));
        }

        grid.trim = commands
            .spawn((
                Mesh3d(state.handle_trim.clone_weak()),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: LIME.into(),
                    unlit: true,
                    ..Default::default()
                })),
            ))
            .id();

        commands.entity(entity).add_child(grid.trim);
    }
}

fn update_grids(
    mut transforms: Query<&mut Transform>,
    clipmaps: Query<&Clipmap>,
    grids: Query<(Entity, &ClipmapGrid, &ChildOf), With<Transform>>,
) {
    for (entity, grid, clipmap) in grids {
        let clipmap = clipmaps.get(clipmap.parent()).unwrap();
        let scale = grid.scale(clipmap.base_scale);
        let target_pos = transforms.get(clipmap.target).unwrap().translation;
        let snap_factor = (target_pos / scale).floor().as_ivec3();
        transforms.get_mut(entity).unwrap().translation = snap_factor.as_vec3() * scale;
        let mut trim_transform = transforms.get_mut(grid.trim).unwrap();
        let snap_mod2 = ((snap_factor.xz() % 2) + 2) % 2;
        trim_transform.rotation = Quat::from_rotation_y(match snap_mod2 {
            IVec2 { x: 0, y: 0 } => 0.0,
            IVec2 { x: 0, y: 1 } => FRAC_PI_2,
            IVec2 { x: 1, y: 0 } => -FRAC_PI_2,
            IVec2 { x: 1, y: 1 } => PI,
            _ => unreachable!(),
        });
        trim_transform.translation = match snap_mod2 {
            IVec2 { x: 0, y: 0 } => Vec3::new(1.0, 0.0, 1.0),
            IVec2 { x: 0, y: 1 } => Vec3::new(1.0, 0.0, 0.0),
            IVec2 { x: 1, y: 0 } => Vec3::new(0.0, 0.0, 1.0),
            IVec2 { x: 1, y: 1 } => Vec3::new(0.0, 0.0, 0.0),
            _ => unreachable!(),
        };
    }
}
