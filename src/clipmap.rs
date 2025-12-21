use std::{
    collections::HashMap,
    f32::consts::{FRAC_PI_2, PI},
};

use bevy::{
    asset::RenderAssetUsages,
    color::palettes::css::{BLUE, LIME, RED, TEAL},
    pbr::{ExtendedMaterial, MaterialExtension},
    prelude::*,
    render::{
        mesh::{Indices, PrimitiveTopology},
        primitives::Aabb,
        render_resource::{AsBindGroup, ShaderRef},
        view::NoFrustumCulling,
    },
};

pub struct ClipmapPlugin;

#[derive(Component)]
struct Handles {
    square: Handle<Mesh>,
    filler: Handle<Mesh>,
    center: Handle<Mesh>,
    trim: Handle<Mesh>,
    stitch: Handle<Mesh>,
}

impl Plugin for ClipmapPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<
            ExtendedMaterial<StandardMaterial, GridMaterial>,
        >::default())
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

    fn add_triangle(&mut self, x1: i32, y1: i32, x2: i32, y2: i32, x3: i32, y3: i32) {
        let p1 = self.add_vertex(x1, y1);
        let p2 = self.add_vertex(x2, y2);
        let p3 = self.add_vertex(x3, y3);
        self.indices.extend([p1, p2, p3]);
    }

    fn add_square(&mut self, x: i32, y: i32) {
        let p1 = self.add_vertex(x, y);
        let p2 = self.add_vertex(x, y + 1);
        let p3 = self.add_vertex(x + 1, y + 1);
        let p4 = self.add_vertex(x + 1, y);
        self.indices.extend([p1, p2, p3]);
        self.indices.extend([p1, p3, p4]);
    }

    fn build(self, wireframe: bool) -> Mesh {
        if wireframe {
            Mesh::new(PrimitiveTopology::LineList, RenderAssetUsages::all())
                .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.vertices)
                .with_inserted_indices(Indices::U32(
                    self.indices
                        .chunks_exact(3)
                        .flat_map(|x: &[u32]| {
                            let p0 = x[0];
                            let p1 = x[1];
                            let p2 = x[2];
                            [p0, p1, p1, p2, p2, p0]
                        })
                        .collect::<Vec<_>>(),
                ))
        } else {
            Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::all())
                .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.vertices)
                .with_inserted_indices(Indices::U32(self.indices))
        }
    }
}

#[derive(Component)]
pub struct Clipmap {
    pub square_side: u32,
    pub levels: u32,
    pub base_scale: f32,
    pub target: Entity,
    pub heightmap: Handle<Image>,
    pub color: Handle<Image>,
    pub min: f32,
    pub max: f32,
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

fn init_clipmaps(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    clipmaps: Query<(Entity, &Clipmap), Added<Clipmap>>,
) {
    for (entity, clipmap) in clipmaps {
        let square_side = clipmap.square_side as i32 - 1;
        let side = square_side * 4 + 1;

        let mut square = MeshBuilder::new();
        let mut filler = MeshBuilder::new();
        let mut center = MeshBuilder::new();
        let mut trim = MeshBuilder::new();
        let mut stitch = MeshBuilder::new();

        for xy in 0..(side + 1) * (side + 1) {
            let x = xy % (side + 1);
            let y = xy / (side + 1);
            if x < square_side && y < square_side {
                square.add_square(x, y);
            }
            if (x == side / 2 || y == side / 2) && x < side && y < side {
                center.add_square(x - side / 2, y - side / 2);
                let range = square_side..side - square_side;
                if !range.contains(&x) || !range.contains(&y) {
                    filler.add_square(x - (side + 1) / 2 + 1, y - (side + 1) / 2 + 1);
                }
            }
            if x == side || y == side {
                trim.add_square(x - (side + 1) / 2, y - (side + 1) / 2);
            }
        }

        for x in 0..=side {
            stitch.add_triangle(
                x - side / 2,
                0 - side / 2,
                x - side / 2 + 1,
                0 - side / 2,
                x - side / 2 + 2,
                0 - side / 2,
            );
            stitch.add_triangle(
                x - side / 2 + 2,
                2 + side / 2,
                x - side / 2 + 1,
                2 + side / 2,
                x - side / 2,
                2 + side / 2,
            );
            stitch.add_triangle(
                0 - side / 2,
                x - side / 2 + 2,
                0 - side / 2,
                x - side / 2 + 1,
                0 - side / 2,
                x - side / 2,
            );
            stitch.add_triangle(
                2 + side / 2,
                x - side / 2,
                2 + side / 2,
                x - side / 2 + 1,
                2 + side / 2,
                x - side / 2 + 2,
            );
        }

        commands.entity(entity).insert((
            Transform::default(),
            Visibility::default(),
            Handles {
                square: meshes.add(square.build(false)),
                filler: meshes.add(filler.build(false)),
                center: meshes.add(center.build(false)),
                trim: meshes.add(trim.build(false)),
                stitch: meshes.add(stitch.build(false)),
            },
        ));

        for level in 0..clipmap.levels {
            commands.entity(entity).with_child(ClipmapGrid {
                level,
                trim: Entity::PLACEHOLDER,
            });
        }
    }
}

fn init_grids(
    mut commands: Commands,
    mut materials: ResMut<Assets<ExtendedMaterial<StandardMaterial, GridMaterial>>>,
    clipmaps: Query<(&Clipmap, &Handles)>,
    mut grids: Query<(Entity, &mut ClipmapGrid, &ChildOf), Added<ClipmapGrid>>,
) {
    for (entity, mut grid, clipmap) in &mut grids {
        let (clipmap, handles) = clipmaps.get(clipmap.parent()).unwrap();
        commands.entity(entity).insert((
            Transform::from_scale(Vec3::splat(grid.scale(clipmap.base_scale))),
            Visibility::default(),
        ));

        let terrain_material = materials.add(ExtendedMaterial {
            base: StandardMaterial::default(),
            extension: GridMaterial {
                heightmap: clipmap.heightmap.clone_weak(),
                color: clipmap.color.clone_weak(),
                lod: grid.level,
                minmax: Vec2 {
                    x: clipmap.min,
                    y: clipmap.max,
                },
                translation: Vec2::ZERO,
            },
        });

        for xy in 0..4 * 4 {
            let x = xy % 4;
            let y = xy / 4;

            if grid.level != 0 && (x == 1 || x == 2) && (y == 1 || y == 2) {
                continue;
            }

            let offset_x = if x >= 2 { 1.0 } else { 0.0 };
            let offset_y = if y >= 2 { 1.0 } else { 0.0 };

            commands.entity(entity).with_child((
                Mesh3d(handles.square.clone_weak()),
                MeshMaterial3d(terrain_material.clone()),
                Transform::from_translation(Vec3::new(
                    (x - 2) as f32 * (clipmap.square_side - 1) as f32 + offset_x,
                    0.0,
                    (y - 2) as f32 * (clipmap.square_side - 1) as f32 + offset_y,
                )),
            ));
        }

        if grid.level == 0 {
            commands.entity(entity).with_child((
                Mesh3d(handles.center.clone_weak()),
                MeshMaterial3d(terrain_material.clone()),
            ));
        } else {
            commands.entity(entity).with_child((
                Mesh3d(handles.filler.clone_weak()),
                MeshMaterial3d(terrain_material.clone()),
            ));

            commands.entity(entity).with_child((
                Mesh3d(handles.stitch.clone_weak()),
                MeshMaterial3d(terrain_material.clone()),
                Transform::from_scale(Vec3::splat(0.5)),
            ));
        }

        grid.trim = commands
            .spawn((
                Mesh3d(handles.trim.clone_weak()),
                MeshMaterial3d(terrain_material.clone()),
            ))
            .id();

        commands.entity(entity).add_child(grid.trim);
    }
}

fn update_grids(
    mut transforms: Query<&mut Transform>,
    mut aabbs: Query<&mut Aabb>,
    mut terrain_materials: ResMut<Assets<ExtendedMaterial<StandardMaterial, GridMaterial>>>,
    terrain_material_handles: Query<
        &MeshMaterial3d<ExtendedMaterial<StandardMaterial, GridMaterial>>,
    >,
    clipmaps: Query<&Clipmap>,
    grids: Query<(Entity, &ClipmapGrid, &ChildOf, &Children), With<Transform>>,
) {
    for (entity, grid, clipmap, children) in grids {
        let clipmap = clipmaps.get(clipmap.parent()).unwrap();
        let scale = grid.scale(clipmap.base_scale);
        let target_pos = transforms.get(clipmap.target).unwrap().translation;
        let snap_factor = (target_pos / scale).floor().as_ivec3();
        let snap_pos = snap_factor.as_vec3() * scale;
        transforms.get_mut(entity).unwrap().translation = snap_pos;

        let mut trim_transform = transforms.get_mut(grid.trim).unwrap();
        let snap_mod2 = ((snap_factor.xz() % 2) + 2) % 2;
        trim_transform.translation = (1 - snap_mod2).as_vec2().extend(0.0).xzy();
        trim_transform.rotation = Quat::from_rotation_y(match snap_mod2 {
            IVec2 { x: 0, y: 0 } => 0.0,
            IVec2 { x: 0, y: 1 } => FRAC_PI_2,
            IVec2 { x: 1, y: 0 } => -FRAC_PI_2,
            IVec2 { x: 1, y: 1 } => PI,
            _ => unreachable!(),
        });

        let grid_pos = (snap_pos + trim_transform.translation * scale).xz();
        for child in children {
            let Ok(material) = terrain_material_handles.get(*child) else {
                continue;
            };
            let Some(material) = terrain_materials.get_mut(material) else {
                continue;
            };
            let Ok(mut aabb) = aabbs.get_mut(*child) else {
                continue;
            };
            material.extension.translation = grid_pos;
            aabb.center.y = clipmap.min + (clipmap.max - clipmap.min) / 2.0;
            aabb.half_extents.y = (clipmap.max - clipmap.min) / 2.0;
        }
    }
}

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
struct GridMaterial {
    #[texture(100)]
    #[sampler(101)]
    heightmap: Handle<Image>,
    #[texture(102)]
    #[sampler(103)]
    color: Handle<Image>,
    #[uniform(104)]
    lod: u32,
    #[uniform(105)]
    minmax: Vec2,
    #[uniform(106)]
    translation: Vec2,
}

impl MaterialExtension for GridMaterial {
    fn vertex_shader() -> ShaderRef {
        "terrain.wgsl".into()
    }

    fn deferred_vertex_shader() -> ShaderRef {
        "terrain.wgsl".into()
    }

    fn fragment_shader() -> ShaderRef {
        "terrain.wgsl".into()
    }

    fn deferred_fragment_shader() -> ShaderRef {
        "terrain.wgsl".into()
    }
}
