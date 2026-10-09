//! Chunk meshes, an embedded pixel atlas, and the day/night scene.
use std::collections::HashMap;

use macroquad::camera::Camera;
use macroquad::prelude::*;
use macroquad::window::miniquad::{
    BlendFactor, BlendState, BlendValue, Comparison, CullFace, Equation, FrontFaceOrder,
    PipelineParams, ShaderSource, UniformDesc, UniformType,
};

use crate::game::{fluid_at, Mob, Player, RayHit};
use crate::graphics::Graphics;
use crate::world::{Block, ChunkKey, Pos, World, CHUNK, HEIGHT};

const TILE: usize = 16;
const ATLAS: usize = 128;
const MAX_VERTICES: usize = 12_000;
const NORMALS: [Pos; 6] = [
    [1, 0, 0],
    [-1, 0, 0],
    [0, 1, 0],
    [0, -1, 0],
    [0, 0, 1],
    [0, 0, -1],
];
const CORNERS: [[[f32; 3]; 4]; 6] = [
    [[1., 0., 1.], [1., 0., 0.], [1., 1., 0.], [1., 1., 1.]],
    [[0., 0., 0.], [0., 0., 1.], [0., 1., 1.], [0., 1., 0.]],
    [[0., 1., 1.], [1., 1., 1.], [1., 1., 0.], [0., 1., 0.]],
    [[0., 0., 0.], [1., 0., 0.], [1., 0., 1.], [0., 0., 1.]],
    [[0., 0., 1.], [1., 0., 1.], [1., 1., 1.], [0., 1., 1.]],
    [[1., 0., 0.], [0., 0., 0.], [0., 1., 0.], [1., 1., 0.]],
];
const FACE_SHADE: [f32; 6] = [0.86, 0.72, 1.0, 0.53, 0.91, 0.78];
const CHEST_BODY_UV: [[usize; 4]; 6] = [
    [28, 33, 14, 10],
    [0, 33, 14, 10],
    [14, 19, 14, 14],
    [28, 19, 14, 14],
    [14, 33, 14, 10],
    [42, 33, 14, 10],
];
const CHEST_LID_UV: [[usize; 4]; 6] = [
    [28, 14, 14, 5],
    [0, 14, 14, 5],
    [14, 0, 14, 14],
    [28, 0, 14, 14],
    [14, 14, 14, 5],
    [42, 14, 14, 5],
];
const CHEST_LATCH_UV: [[usize; 4]; 6] = [
    [3, 1, 1, 4],
    [0, 1, 1, 4],
    [1, 0, 2, 1],
    [3, 0, 2, 1],
    [1, 1, 2, 4],
    [4, 1, 2, 4],
];

struct ChunkMeshes {
    opaque: Vec<Mesh>,
    translucent: Vec<Mesh>,
    opaque_bounds: Vec<[Vec3; 2]>,
    translucent_bounds: Vec<[Vec3; 2]>,
    torches: Vec<Vec3>,
    sea_water: bool,
}

pub struct Renderer {
    pub enhanced: bool,
    graphics: Graphics,
    atlas: Texture2D,
    opaque: Material,
    translucent: Material,
    meshes: HashMap<ChunkKey, ChunkMeshes>,
    shadows: crate::shadows::Shadows,
    creatures: crate::mob_models::MobModels,
    sky: crate::sky::Sky,
    reflection: Option<RenderTarget>,
}

impl Renderer {
    pub fn new() -> Self {
        let pixels = combined_atlas_pixels();
        let atlas = Texture2D::from_rgba8(ATLAS as u16, atlas_height() as u16, &pixels);
        atlas.set_filter(FilterMode::Nearest);
        let sky = crate::sky::Sky::new();
        let opaque = scene_material(false);
        let translucent = scene_material(true);
        opaque.set_texture("SkyMap", sky.reflection());
        translucent.set_texture("SkyMap", sky.reflection());
        opaque.set_texture("PlanarMap", sky.reflection());
        translucent.set_texture("PlanarMap", sky.reflection());
        Self {
            enhanced: true,
            graphics: Graphics::new(),
            atlas,
            opaque,
            translucent,
            meshes: HashMap::new(),
            shadows: crate::shadows::Shadows::new(),
            creatures: crate::mob_models::MobModels::new(),
            sky,
            reflection: None,
        }
    }

    pub fn assert_creature_opacity(&self) {
        self.creatures.assert_sheep_opaque();
    }

    fn reflection_target(&mut self) -> RenderTarget {
        let width = (screen_width() * 0.5).ceil().max(1.) as u32;
        let height = (screen_height() * 0.5).ceil().max(1.) as u32;
        if self.reflection.as_ref().is_none_or(|target| {
            target.texture.width() as u32 != width || target.texture.height() as u32 != height
        }) {
            let target = render_target_ex(
                width,
                height,
                RenderTargetParams {
                    depth: true,
                    ..Default::default()
                },
            );
            target.texture.set_filter(FilterMode::Linear);
            self.reflection = Some(target);
        }
        self.reflection.as_ref().unwrap().clone()
    }

    /// Rebuild close chunks first, with a fixed amount of work per frame.
    pub fn sync(&mut self, world: &mut World, center: ChunkKey, budget: usize) {
        self.meshes.retain(|key, _| world.chunks.contains_key(key));
        let mut dirty: Vec<_> = world
            .chunks
            .iter()
            .filter(|(key, chunk)| chunk.dirty || !self.meshes.contains_key(*key))
            .map(|(&key, _)| key)
            .collect();
        dirty.sort_unstable_by_key(|key| {
            let x = i64::from(key[0]) - i64::from(center[0]);
            let z = i64::from(key[1]) - i64::from(center[1]);
            x * x + z * z
        });
        for key in dirty.into_iter().take(budget) {
            self.meshes
                .insert(key, build_chunk(world, key, &self.atlas));
            if let Some(chunk) = world.chunks.get_mut(&key) {
                chunk.dirty = false;
            }
        }
    }

    /// Number of ready chunks and triangles; useful in the debug HUD.
    pub fn stats(&self) -> (usize, usize) {
        (
            self.meshes.len(),
            self.meshes
                .values()
                .map(|chunk| {
                    chunk
                        .opaque
                        .iter()
                        .chain(&chunk.translucent)
                        .map(|mesh| mesh.indices.len() / 3)
                        .sum::<usize>()
                })
                .sum(),
        )
    }

    /// Native smoke proof: a fluid behind a wall must leave the wall's pixel intact,
    /// while its exposed edge must still be visible. This reads the real GPU output.
    pub fn assert_depth_occlusion(&mut self) {
        let targets = [None, Some(self.graphics.target())];
        for target in targets {
            let camera = Camera3D {
                position: vec3(0., 0., 4.),
                target: Vec3::ZERO,
                up: Vec3::Y,
                fovy: 72_f32.to_radians(),
                render_target: target.clone(),
                ..Default::default()
            };
            for material in [&self.opaque, &self.translucent] {
                material.set_uniform("ShadowProjection", Mat4::IDENTITY);
                material.set_uniform("ShadowEnabled", 0.0f32);
                material.set_texture("ShadowMap", self.shadows.target.texture.clone());
                material.set_uniform("Eye", [0_f32, 0., 4.]);
                material.set_uniform("FogColor", [0_f32; 3]);
                material.set_uniform("FogRange", [100_f32, 200.]);
                material.set_uniform("Daylight", 1_f32);
                material.set_uniform("SceneTime", 0_f32);
                material.set_uniform("Visuals", 0_f32);
                for name in TORCH_UNIFORMS {
                    material.set_uniform(name, [0_f32; 4]);
                }
            }
            let wall = depth_probe_mesh(&self.atlas, Block::Stone, 1., 0.65);
            let capture = |fluid: Option<Block>, effect: bool| {
                set_camera(&camera);
                clear_background(BLACK);
                gl_use_material(&self.opaque);
                draw_mesh(&wall);
                if let Some(block) = fluid {
                    gl_use_material(if block == Block::Water {
                        &self.translucent
                    } else {
                        &self.opaque
                    });
                    draw_mesh(&depth_probe_mesh(&self.atlas, block, 0., 1.5));
                }
                if effect {
                    gl_use_material(&self.translucent);
                    let mut mesh = Mesh {
                        vertices: Vec::new(),
                        indices: Vec::new(),
                        texture: None,
                    };
                    effect_box(
                        &mut mesh,
                        Vec3::ZERO,
                        vec3(3., 3., 0.1),
                        Vec3::Z,
                        Color::new(1., 0.65, 0.15, 0.85),
                        1.,
                    );
                    draw_mesh(&mesh);
                }
                gl_use_default_material();
                set_default_camera();
                if target.is_some() {
                    self.graphics.present();
                    set_default_camera();
                }
                let (width, height) = macroquad::window::miniquad::window::screen_size();
                [0.5, 0.635].map(|fraction| {
                    let mut pixel = [0_u8; 4];
                    // A screen-copy texture would disturb Miniquad's cached texture binding
                    // between these probes; read the framebuffer without changing textures.
                    unsafe {
                        use macroquad::window::miniquad::gl;
                        gl::glReadPixels(
                            (width * fraction) as i32,
                            height as i32 / 2,
                            1,
                            1,
                            gl::GL_RGBA,
                            gl::GL_UNSIGNED_BYTE,
                            pixel.as_mut_ptr().cast(),
                        );
                    }
                    pixel
                })
            };
            let baseline = capture(None, false);
            assert_ne!(baseline[0][..3], [0, 0, 0]);
            for block in [Block::Water, Block::Lava] {
                let pixels = capture(Some(block), false);
                assert_eq!(
                    pixels[0], baseline[0],
                    "{block:?} was visible through an opaque wall"
                );
                assert_ne!(
                    pixels[1], baseline[1],
                    "the exposed {block:?} surface was not rendered"
                );
            }
            let pixels = capture(None, true);
            assert_eq!(
                pixels[0], baseline[0],
                "weapon effects were visible through a wall"
            );
            assert_ne!(
                pixels[1], baseline[1],
                "exposed weapon effects were not rendered"
            );
        }
    }

    pub fn assert_partial_geometry(&mut self) {
        let mapped = |source| {
            Block::Map(
                crate::city::get()
                    .unwrap()
                    .metadata
                    .states
                    .iter()
                    .position(|s| s.source == source)
                    .unwrap() as u16,
            )
        };
        let targets = [None, Some(self.graphics.target())];
        for target in targets {
            let camera = Camera3D {
                position: vec3(8.5, 12., 8.5),
                target: vec3(8.5, 5., 8.5),
                up: Vec3::Z,
                projection: Projection::Orthographics,
                fovy: 1.8,
                aspect: Some(screen_width() / screen_height()),
                z_near: 0.1,
                z_far: 20.,
                render_target: target.clone(),
                ..Default::default()
            };
            self.opaque.set_uniform("ShadowProjection", Mat4::IDENTITY);
            self.opaque.set_uniform("ShadowEnabled", 0_f32);
            self.opaque
                .set_texture("ShadowMap", self.shadows.target.texture.clone());
            self.opaque.set_uniform("Eye", camera.position);
            self.opaque.set_uniform("FogColor", [0_f32; 3]);
            self.opaque.set_uniform("FogRange", [100_f32, 200.]);
            self.opaque.set_uniform("Daylight", 1_f32);
            self.opaque.set_uniform("SceneTime", 0_f32);
            self.opaque.set_uniform("Visuals", 0_f32);
            self.opaque.set_uniform("SunDirection", Vec3::Y);
            for name in TORCH_UNIFORMS {
                self.opaque.set_uniform(name, [0_f32; 4]);
            }
            for block in [
                Block::Chest,
                Block::Door,
                Block::OpenDoor,
                mapped("moss_block"),
                mapped("moss_carpet"),
                mapped("poppy"),
                mapped("allium"),
                mapped("dandelion"),
                mapped("white_tulip"),
            ] {
                let mut world = World::new(1);
                let mut blocks = vec![Block::Air; (CHUNK * CHUNK * HEIGHT) as usize];
                for z in 4..=12 {
                    for x in 4..=12 {
                        blocks[(x + CHUNK * (z + CHUNK * 4)) as usize] = Block::Stone;
                    }
                }
                world.chunks.insert(
                    [0, 0],
                    crate::world::Chunk {
                        blocks,
                        dirty: true,
                    },
                );
                let gap_points = match block {
                    Block::Chest => [
                        vec3(8.035, 5., 8.5),
                        vec3(8.965, 5., 8.5),
                        vec3(8.5, 5., 8.035),
                        vec3(8.25, 5., 8.965),
                    ],
                    Block::Door => [
                        vec3(8.25, 5., 8.4),
                        vec3(8.75, 5., 8.4),
                        vec3(8.5, 5., 8.1),
                        vec3(8.5, 5., 8.6),
                    ],
                    Block::OpenDoor => [
                        vec3(8.4, 5., 8.25),
                        vec3(8.8, 5., 8.25),
                        vec3(8.4, 5., 8.75),
                        vec3(8.8, 5., 8.75),
                    ],
                    _ => [
                        vec3(8.035, 5., 8.035),
                        vec3(8.965, 5., 8.035),
                        vec3(8.035, 5., 8.965),
                        vec3(8.965, 5., 8.965),
                    ],
                };
                let mut samples = Vec::new();
                for placed in [Block::Air, block] {
                    world.chunks.get_mut(&[0, 0]).unwrap().blocks
                        [(8 + CHUNK * (8 + CHUNK * 5)) as usize] = placed;
                    let meshes = build_chunk(&world, [0, 0], &self.atlas);
                    set_camera(&camera);
                    clear_background(MAGENTA);
                    gl_use_material(&self.opaque);
                    for mesh in &meshes.opaque {
                        draw_mesh(mesh);
                    }
                    gl_use_default_material();
                    set_default_camera();
                    if target.is_some() {
                        self.graphics.present();
                        set_default_camera();
                    }
                    samples.push(gap_points.map(|point| {
                        let q = camera.matrix().project_point3(point);
                        crate::graphics::screen_pixel(q.x * 0.5 + 0.5, q.y * 0.5 + 0.5)
                    }));
                }
                for (baseline, actual) in samples[0].iter().zip(&samples[1]) {
                    assert!(baseline[1] > 30, "The reference terrain was not rendered");
                    if block == mapped("moss_block") || block == mapped("moss_carpet") {
                        assert!(actual[1] > 30, "Moss left a transparent hole: {actual:?}");
                        assert_ne!(
                            &baseline[..3],
                            &actual[..3],
                            "Moss did not cover the terrain"
                        );
                        continue;
                    }
                    assert!(
                        (0..3).all(|channel| baseline[channel].abs_diff(actual[channel]) < 8),
                        "Terrain disappeared through {block:?}: {samples:?}"
                    );
                }
            }
        }
    }

    pub fn assert_shader_effects(&mut self) {
        self.graphics.assert_effects();
        self.assert_sun_shadows();
        self.sky.assert_effects();
        let target = self.graphics.target();
        self.sky.draw(
            vec3(0., 40., 0.),
            vec3(0.2, 0.65, 1.).normalize(),
            vec3(0.4, 0.5, 0.8).normalize(),
            1.,
            72_f32.to_radians(),
            0.,
            Some(target),
        );
        set_default_camera();
        self.graphics.present();
        set_default_camera();
        let center = crate::graphics::screen_pixel(0.5, 0.5);
        let edge = crate::graphics::screen_pixel(0.8, 0.7);
        assert!(
            center[0] as u32 + center[1] as u32 + center[2] as u32 > 120
                && edge[0] as u32 + edge[1] as u32 + edge[2] as u32 > 120,
            "Composited atmosphere is black or shrunk: {center:?} {edge:?}"
        );
        self.assert_water_reflections();
        self.assert_shore_reflections();
    }

    fn assert_water_reflections(&self) {
        let camera = Camera3D {
            position: vec3(0., 2.2, 5.),
            target: Vec3::ZERO,
            fovy: 72_f32.to_radians(),
            z_near: 0.1,
            z_far: 30.,
            ..Default::default()
        };
        let mut mesh = empty_mesh(&self.atlas);
        for (corner, p) in CORNERS[2].iter().enumerate() {
            let mut vertex = Vertex::new2(
                vec3((p[0] - 0.5) * 8., 0., (p[2] - 0.5) * 8.),
                tile_uv(9)[corner],
                Color::new(1., 1., 1., 175. / 255.),
            );
            vertex.normal = vec4(1., 3., 1., 0.);
            mesh.vertices.push(vertex);
        }
        mesh.indices.extend([0, 1, 2, 0, 2, 3]);
        self.translucent
            .set_uniform("ShadowProjection", Mat4::IDENTITY);
        self.translucent.set_uniform("ShadowEnabled", 0_f32);
        self.translucent
            .set_texture("ShadowMap", self.shadows.target.texture.clone());
        self.translucent.set_uniform("Eye", camera.position);
        self.translucent.set_uniform("FogColor", [0_f32; 3]);
        self.translucent.set_uniform("FogRange", [100_f32, 200.]);
        self.translucent.set_uniform("Daylight", 1_f32);
        self.translucent.set_uniform("SceneTime", 0_f32);
        self.translucent.set_uniform("Visuals", 1_f32);
        self.translucent.set_uniform("PlanarEnabled", 0_f32);
        self.translucent
            .set_uniform("SunDirection", vec3(0.3, 0.7, 0.1).normalize());
        for name in TORCH_UNIFORMS {
            self.translucent.set_uniform(name, [0_f32; 4]);
        }
        let mut pixels = Vec::new();
        for rgba in [[255, 0, 0, 255], [0, 0, 255, 255]] {
            let sky = Texture2D::from_rgba8(1, 1, &rgba);
            self.translucent.set_texture("SkyMap", sky);
            set_camera(&camera);
            clear_background(BLACK);
            gl_use_material(&self.translucent);
            draw_mesh(&mesh);
            gl_use_default_material();
            set_default_camera();
            pixels.push(crate::graphics::screen_pixel(0.5, 0.5));
        }
        assert!(
            pixels[0][0] as i32 > pixels[1][0] as i32 + 35,
            "Water ignored the red sky map: {pixels:?}"
        );
        assert!(
            pixels[1][2] as i32 > pixels[0][2] as i32 + 35,
            "Water ignored the blue sky map: {pixels:?}"
        );
        self.translucent
            .set_texture("SkyMap", self.sky.reflection());
    }

    /// Uses the same chunk mesher and mirrored terrain pass as the live lake.
    fn assert_shore_reflections(&mut self) {
        let plane = 1.;
        let camera = Camera3D {
            position: vec3(8.5, 4., 14.),
            target: vec3(8.5, plane, 8.),
            up: Vec3::Y,
            fovy: 72_f32.to_radians(),
            z_near: 0.1,
            z_far: 40.,
            ..Default::default()
        };
        let target = self.reflection_target();
        let reflected = mirrored_camera(&camera, plane, target.clone());
        let mut world = World::new(1);
        let mut blocks = vec![Block::Air; (CHUNK * CHUNK * HEIGHT) as usize];
        for y in 1..5 {
            for z in 3..5 {
                for x in 5..12 {
                    blocks[(x + CHUNK * (z + CHUNK * y)) as usize] = Block::Snow;
                }
            }
        }
        blocks[(12 + CHUNK * 8) as usize] = Block::Brick;
        world.chunks.insert(
            [0, 0],
            crate::world::Chunk {
                blocks,
                dirty: true,
            },
        );
        let mut shore = build_chunk(&world, [0, 0], &self.atlas);
        let mut water = empty_mesh(&self.atlas);
        for (index, p) in CORNERS[2].iter().enumerate() {
            let mut v = Vertex::new2(
                vec3(p[0] * 16., plane, p[2] * 16.),
                tile_uv(9)[index],
                WHITE,
            );
            v.normal = vec4(1., 3., 1., 0.);
            water.vertices.push(v);
        }
        water.indices.extend([0, 1, 2, 0, 2, 3]);
        for material in [&self.opaque, &self.translucent] {
            material.set_uniform("ShadowEnabled", 0_f32);
            material.set_uniform("ShadowProjection", Mat4::IDENTITY);
            material.set_uniform("FogRange", [100_f32, 200.]);
            material.set_uniform("FogColor", [0_f32; 3]);
            material.set_uniform("Daylight", 1_f32);
            material.set_uniform("SceneTime", 0_f32);
            material.set_uniform("Visuals", 1_f32);
            material.set_uniform("SunDirection", Vec3::Y);
            material.set_uniform("WaterPlane", plane);
            for name in TORCH_UNIFORMS {
                material.set_uniform(name, [0_f32; 4]);
            }
        }
        self.translucent
            .set_texture("SkyMap", Texture2D::from_rgba8(1, 1, &[0, 0, 0, 255]));
        self.translucent
            .set_texture("PlanarMap", target.texture.clone());
        self.translucent
            .set_uniform("PlanarProjection", reflected.matrix());
        self.translucent.set_uniform("PlanarEnabled", 1_f32);
        let point = camera.matrix().project_point3(vec3(8.5, plane, 9.));
        let mut samples = Vec::new();
        for color in [[255, 35, 35, 255], [35, 35, 255, 255]] {
            for mesh in &mut shore.opaque {
                for vertex in &mut mesh.vertices {
                    vertex.color = color;
                }
            }
            set_camera(&reflected);
            clear_background(BLANK);
            self.opaque.set_uniform("Eye", reflected.position);
            self.opaque.set_uniform("ReflectionPass", 1_f32);
            gl_use_material(&self.opaque);
            for mesh in &shore.opaque {
                draw_mesh(mesh);
            }
            gl_use_default_material();
            set_camera(&camera);
            self.opaque.set_uniform("ReflectionPass", 0_f32);
            clear_background(BLACK);
            self.translucent.set_uniform("Eye", camera.position);
            gl_use_material(&self.translucent);
            draw_mesh(&water);
            gl_use_default_material();
            set_default_camera();
            samples.push(crate::graphics::screen_pixel(
                point.x * 0.5 + 0.5,
                point.y * 0.5 + 0.5,
            ));
        }
        assert!(
            samples[0][0] as i32 > samples[1][0] as i32 + 20
                && samples[1][2] as i32 > samples[0][2] as i32 + 20,
            "Lake water did not reflect the actual terrain pass: {samples:?}"
        );
        let below = reflected.matrix().project_point3(vec3(12.5, 0.5, 8.5));
        let image = target.texture.get_texture_data();
        let pixel = image.get_pixel(
            ((below.x * 0.5 + 0.5) * image.width as f32) as u32,
            ((below.y * 0.5 + 0.5) * image.height as f32) as u32,
        );
        assert!(
            pixel.a < 0.01,
            "Underwater terrain leaked into the shore reflection: {pixel:?}"
        );
        self.translucent.set_uniform("PlanarEnabled", 0_f32);
        self.translucent
            .set_texture("SkyMap", self.sky.reflection());
        self.opaque.set_uniform("ReflectionPass", 0_f32);
    }

    fn assert_sun_shadows(&self) {
        let eye = vec3(0., 10., 0.);
        let sun = vec3(0.8, 1., 0.3).normalize();
        let mut floor = vec![empty_mesh(&self.atlas)];
        let mut pillar = vec![empty_mesh(&self.atlas)];
        append_box(
            &mut floor,
            &self.atlas,
            vec3(-6., -0.2, -6.),
            vec3(12., 0.2, 12.),
            0,
            1.,
            0.,
        );
        append_box(
            &mut pillar,
            &self.atlas,
            vec3(-0.5, 0., -0.5),
            vec3(1., 3., 1.),
            0,
            1.,
            0.,
        );
        let projection = self.shadows.begin(Vec3::ZERO, sun, 0., HEIGHT);
        for mesh in floor.iter().chain(&pillar) {
            draw_mesh(mesh);
        }
        gl_use_default_material();
        set_default_camera();
        let camera = Camera3D {
            position: eye,
            target: Vec3::ZERO,
            up: Vec3::Z,
            projection: Projection::Orthographics,
            fovy: 12.,
            aspect: Some(screen_width() / screen_height()),
            z_near: 0.1,
            z_far: 30.,
            ..Default::default()
        };
        for (name, value) in [
            ("Eye", [eye.x, eye.y, eye.z]),
            ("FogColor", [0., 0., 0.]),
            ("SunDirection", sun.to_array()),
        ] {
            self.opaque.set_uniform(name, value);
        }
        self.opaque.set_uniform("FogRange", [100_f32, 200.]);
        self.opaque.set_uniform("Daylight", 1_f32);
        self.opaque.set_uniform("Visuals", 1_f32);
        self.opaque.set_uniform("SceneTime", 0_f32);
        self.opaque.set_uniform("ShadowProjection", projection);
        self.opaque
            .set_texture("ShadowMap", self.shadows.target.texture.clone());
        for name in TORCH_UNIFORMS {
            self.opaque.set_uniform(name, [0_f32; 4]);
        }
        let points = [vec3(-1.6, 0., -0.6), vec3(2.5, 0., 1.5)];
        let mut samples = Vec::new();
        for enabled in [0_f32, 1.] {
            set_camera(&camera);
            clear_background(BLACK);
            self.opaque.set_uniform("ShadowEnabled", enabled);
            gl_use_material(&self.opaque);
            for mesh in &floor {
                draw_mesh(mesh);
            }
            gl_use_default_material();
            set_default_camera();
            samples.push(points.map(|p| {
                let q = camera.matrix().project_point3(p);
                crate::graphics::screen_pixel(q.x * 0.5 + 0.5, q.y * 0.5 + 0.5)
            }));
        }
        assert!(
            samples[1][0][0] + 30 < samples[0][0][0],
            "Sunlight shadow did not darken the ground: {samples:?}"
        );
        assert!(
            samples[1][1][0].abs_diff(samples[0][1][0]) < 8,
            "Unshadowed ground became dark: {samples:?}"
        );
        self.opaque.set_uniform("ShadowEnabled", 0_f32);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &mut self,
        world: &World,
        player: &Player,
        mobs: &[Mob],
        hit: Option<&RayHit>,
        time: f32,
        radius: i32,
        mining_progress: f32,
        combat: &crate::combat::Combat,
        drops: &[crate::drops::DroppedItem],
        viewmodel: &crate::viewmodel::ViewModel,
        charges: &[crate::explosives::Charge],
    ) {
        let angle = time.rem_euclid(1.) * std::f32::consts::TAU;
        let daylight = if self.enhanced {
            ((angle.sin() + 0.12) / 0.30).clamp(0., 1.)
        } else {
            (angle.sin() * 1.7 + 0.18).clamp(0., 1.)
        };
        let dusk = (1. - angle.sin().abs() * 4.).max(0.) * (0.4 + 0.6 * daylight);
        let night = Color::new(0.035, 0.055, 0.12, 1.);
        let day = Color::new(0.64, 0.79, 0.9, 1.);
        let mut fog = mix_color(night, day, daylight);
        fog = mix_color(fog, Color::new(0.98, 0.64, 0.40, 1.), dusk * 0.32);
        let eye = player.eye();
        let direction = combat.view_direction(player.direction());
        let distance = radius.max(2) as f32 * CHUNK as f32;
        let fog_range = if self.enhanced {
            [distance * 0.72, distance * 1.20]
        } else {
            [distance * 0.53, distance * 1.02]
        };
        let sun = vec3(angle.cos() * 0.85, angle.sin(), 0.45).normalize();
        let shadow_enabled = self.enhanced && sun.y > 0.05;
        let water_plane = world.sea() as f32 + 0.88;
        let planar_target = if self.enhanced
            && eye.y > water_plane + 0.05
            && eye.y < water_plane + 64.
            && self.meshes.iter().any(|(key, chunk)| {
                chunk.sea_water
                    && vec2(
                        (key[0] * CHUNK + 8) as f32 - eye.x,
                        (key[1] * CHUNK + 8) as f32 - eye.z,
                    )
                    .length_squared()
                        < 96_f32.powi(2)
            }) {
            Some(self.reflection_target())
        } else {
            None
        };
        let mut shadow_projection = Mat4::IDENTITY;
        if shadow_enabled {
            shadow_projection = self
                .shadows
                .begin(eye, sun, get_time() as f32, world.height());
            for (key, chunk) in &self.meshes {
                if vec3(
                    (key[0] * CHUNK + 8) as f32,
                    eye.y,
                    (key[1] * CHUNK + 8) as f32,
                )
                .distance_squared(eye)
                    < 85_f32.powi(2)
                {
                    draw_visible(&chunk.opaque, &chunk.opaque_bounds, shadow_projection);
                }
            }
            for mob in mobs {
                if mob.health > 0. && mob.position.distance_squared(eye) < 70_f32.powi(2) {
                    self.creatures.draw(mob, true);
                }
            }
            gl_use_default_material();
            set_default_camera();
        }
        self.creatures.lighting(
            sun,
            daylight,
            eye,
            fog,
            fog_range,
            shadow_projection,
            &self.shadows.target.texture,
            shadow_enabled,
        );
        let target = if self.enhanced {
            Some(self.graphics.target())
        } else {
            None
        };
        if let Some(target) = &target {
            set_camera(&Camera2D {
                render_target: Some(target.clone()),
                ..Camera2D::from_display_rect(Rect::new(0., 0., screen_width(), screen_height()))
            });
            clear_background(BLACK);
        }
        if self.enhanced {
            // Translate the atmosphere's height reference above the imported buildings.
            self.sky.draw(
                eye - Vec3::Y * (world.height() - HEIGHT) as f32,
                direction,
                sun,
                daylight,
                crate::weapons::field_of_view(combat.aim, combat.zoom),
                get_time() as f32,
                target.clone(),
            );
        } else {
            draw_sky_gradient(fog, daylight, direction.y);
        }
        let camera = Camera3D {
            position: eye,
            target: eye + direction,
            up: Vec3::Y,
            fovy: crate::weapons::field_of_view(combat.aim, combat.zoom),
            z_near: 0.04,
            z_far: (distance * 4.).max(280.),
            render_target: target.clone(),
            ..Default::default()
        };
        set_camera(&camera);
        if !self.enhanced {
            draw_celestials(eye, angle, daylight);
        }
        let mut torches: Vec<_> = self
            .meshes
            .values()
            .flat_map(|chunk| chunk.torches.iter().copied())
            .filter(|position| position.distance_squared(eye) < (distance + 7.).powi(2))
            .collect();
        torches
            .sort_unstable_by(|a, b| a.distance_squared(eye).total_cmp(&b.distance_squared(eye)));
        for material in [&self.opaque, &self.translucent] {
            material.set_uniform("ShadowProjection", shadow_projection);
            material.set_uniform(
                "ShadowEnabled",
                if shadow_enabled { 1.0f32 } else { 0.0f32 },
            );
            material.set_texture("ShadowMap", self.shadows.target.texture.clone());
            material.set_uniform("Eye", [eye.x, eye.y, eye.z]);
            material.set_uniform("FogColor", [fog.r, fog.g, fog.b]);
            material.set_uniform("FogRange", fog_range);
            material.set_uniform("Daylight", daylight);
            material.set_uniform("SceneTime", get_time() as f32);
            material.set_uniform("Visuals", if self.enhanced { 1_f32 } else { 0_f32 });
            material.set_uniform("SunDirection", sun);
            material.set_uniform("ReflectionPass", 0_f32);
            material.set_uniform("WaterPlane", water_plane);
            material.set_uniform("PlanarEnabled", 0_f32);
            for (index, name) in TORCH_UNIFORMS.iter().enumerate() {
                let value = torches.get(index).map_or([0.; 4], |p| [p.x, p.y, p.z, 1.]);
                material.set_uniform(name, value);
            }
        }
        let mut visible: Vec<_> = self
            .meshes
            .iter()
            .filter(|(key, _)| {
                let delta = vec3(
                    (key[0] * CHUNK) as f32 + 8. - eye.x,
                    0.,
                    (key[1] * CHUNK) as f32 + 8. - eye.z,
                );
                delta.length_squared() <= (distance + 23.).powi(2)
                    && (delta.length_squared() < 24_f32.powi(2)
                        || vec3(direction.x, 0., direction.z).dot(delta) > -19.)
            })
            .collect();
        if let Some(reflection) = planar_target {
            let reflected = mirrored_camera(&camera, water_plane, reflection.clone());
            let projection = reflected.matrix();
            set_camera(&reflected);
            clear_background(BLANK);
            self.opaque.set_uniform("ReflectionPass", 1_f32);
            self.opaque.set_uniform("Eye", reflected.position);
            gl_use_material(&self.opaque);
            for (key, chunk) in &visible {
                let delta = vec2(
                    (key[0] * CHUNK + 8) as f32 - eye.x,
                    (key[1] * CHUNK + 8) as f32 - eye.z,
                );
                if delta.length_squared() <= 128_f32.powi(2) {
                    draw_visible(&chunk.opaque, &chunk.opaque_bounds, projection);
                }
            }
            self.creatures.lighting(
                sun,
                daylight,
                reflected.position,
                fog,
                fog_range,
                shadow_projection,
                &self.shadows.target.texture,
                shadow_enabled,
            );
            for mob in mobs {
                if mob.health > 0.
                    && mob.position.y >= water_plane
                    && mob.position.distance_squared(eye) < 96_f32.powi(2)
                {
                    self.creatures.draw(mob, false);
                }
            }
            gl_use_default_material();
            set_camera(&camera);
            self.opaque.set_uniform("ReflectionPass", 0_f32);
            self.opaque.set_uniform("Eye", eye);
            self.creatures.lighting(
                sun,
                daylight,
                eye,
                fog,
                fog_range,
                shadow_projection,
                &self.shadows.target.texture,
                shadow_enabled,
            );
            self.translucent.set_uniform("PlanarProjection", projection);
            self.translucent.set_uniform("PlanarEnabled", 1_f32);
            self.translucent
                .set_texture("PlanarMap", reflection.texture);
        }
        gl_use_material(&self.opaque);
        for (_, chunk) in &visible {
            draw_visible(&chunk.opaque, &chunk.opaque_bounds, camera.matrix());
        }
        for mob in mobs {
            if mob.health > 0.0 && mob.position.distance_squared(player.position) < distance.powi(2)
            {
                self.creatures.draw(mob, false);
            }
        }
        if !self.enhanced {
            draw_clouds(eye, daylight, world.height());
        }
        for drop in drops {
            let position = Vec3::from_array(drop.position);
            if position.distance_squared(eye) > 24_f32.powi(2) {
                continue;
            }
            let position =
                position + Vec3::Y * (0.12 + (get_time() as f32 * 2. + drop.age).sin() * 0.025);
            if crate::weapons::spec(drop.stack.item).is_some() {
                viewmodel.draw_loose(drop.stack, position, get_time() as f32 * 0.8, daylight);
            } else if !crate::item_icons::draw_loose(
                drop.stack.item,
                position,
                get_time() as f32 * 0.8,
                daylight,
            ) {
                gl_use_default_material();
                let color = drop
                    .stack
                    .item
                    .block()
                    .map_or(Color::new(0.88, 0.67, 0.28, 1.), |b| {
                        let [r, g, b] = b.color(4);
                        Color::from_rgba(r, g, b, 255)
                    });
                draw_cube(position, Vec3::splat(0.22), None, color);
            }
        }
        gl_use_material(&self.translucent);
        // ponytail: sort transparent chunks, not individual faces; intersecting glass
        // constructions would need a per-face sort or order-independent transparency.
        visible.sort_unstable_by(|(a, _), (b, _)| {
            let d = |k: &ChunkKey| {
                ((k[0] * CHUNK) as f32 + 8. - eye.x).powi(2)
                    + ((k[1] * CHUNK) as f32 + 8. - eye.z).powi(2)
            };
            d(b).total_cmp(&d(a))
        });
        for (_, chunk) in visible {
            draw_visible(
                &chunk.translucent,
                &chunk.translucent_bounds,
                camera.matrix(),
            );
        }
        let effects = combat_mesh(combat);
        if !effects.vertices.is_empty() {
            draw_mesh(&effects);
        }
        gl_use_default_material();
        crate::explosives::draw_world(combat, charges, eye);
        if let Some(hit) = hit {
            let p = vec3(
                hit.block[0] as f32,
                hit.block[1] as f32,
                hit.block[2] as f32,
            );
            draw_cube_wires(
                p + Vec3::splat(0.5),
                Vec3::splat(1.007),
                Color::new(0.05, 0.075, 0.1, 0.85),
            );
            if mining_progress > 0.02 {
                draw_cracks(p, mining_progress);
            }
        }
        set_default_camera();
        if target.is_some() {
            self.graphics.present();
        }
        if let Some(fluid) = fluid_at(world, eye) {
            draw_rectangle(
                0.,
                0.,
                screen_width(),
                screen_height(),
                if fluid == Block::Lava {
                    Color::new(0.91, 0.24, 0.025, 0.84)
                } else {
                    Color::new(0.04, 0.29, 0.58, 0.38)
                },
            );
        }
    }
}

fn mirror_point(point: Vec3, plane: f32) -> Vec3 {
    vec3(point.x, 2. * plane - point.y, point.z)
}

fn mirrored_camera(camera: &Camera3D, plane: f32, target: RenderTarget) -> Camera3D {
    Camera3D {
        position: mirror_point(camera.position, plane),
        target: mirror_point(camera.target, plane),
        up: Vec3::Y,
        fovy: camera.fovy,
        aspect: Some(screen_width() / screen_height()),
        projection: camera.projection,
        z_near: camera.z_near,
        z_far: camera.z_far,
        render_target: Some(target),
        ..Default::default()
    }
}

fn depth_probe_mesh(texture: &Texture2D, block: Block, z: f32, half: f32) -> Mesh {
    let mut mesh = empty_mesh(texture);
    let uv = tile_uv(tile_for(block, 4));
    for (index, (x, y)) in [(-half, -half), (half, -half), (half, half), (-half, half)]
        .into_iter()
        .enumerate()
    {
        let mut vertex = Vertex::new2(vec3(x, y, z), uv[index], WHITE);
        if block == Block::Water {
            vertex.color[3] = 175;
        }
        vertex.normal = vec4(
            1.,
            0.,
            if block == Block::Water { 1. } else { 0. },
            if block == Block::Lava { 1. } else { 0. },
        );
        mesh.vertices.push(vertex);
    }
    mesh.indices.extend([0, 1, 2, 0, 2, 3]);
    mesh
}

fn scene_material(translucent: bool) -> Material {
    let mut uniforms = vec![
        UniformDesc::new("Eye", UniformType::Float3),
        UniformDesc::new("FogColor", UniformType::Float3),
        UniformDesc::new("FogRange", UniformType::Float2),
        UniformDesc::new("Daylight", UniformType::Float1),
        UniformDesc::new("SceneTime", UniformType::Float1),
        UniformDesc::new("Visuals", UniformType::Float1),
        UniformDesc::new("SunDirection", UniformType::Float3),
        UniformDesc::new("ShadowProjection", UniformType::Mat4),
        UniformDesc::new("ShadowEnabled", UniformType::Float1),
        UniformDesc::new("PlanarProjection", UniformType::Mat4),
        UniformDesc::new("PlanarEnabled", UniformType::Float1),
        UniformDesc::new("ReflectionPass", UniformType::Float1),
        UniformDesc::new("WaterPlane", UniformType::Float1),
    ];
    uniforms.extend(
        TORCH_UNIFORMS
            .iter()
            .map(|name| UniformDesc::new(name, UniformType::Float4)),
    );
    let fragment = format!(
        "#version 100\nprecision highp float;\n{}\n{}",
        include_str!("../assets/shadow.glsl"),
        FRAGMENT_SHADER
            .lines()
            .skip(2)
            .collect::<Vec<_>>()
            .join("\n")
    );
    load_material(
        ShaderSource::Glsl {
            vertex: VERTEX_SHADER,
            fragment: &fragment,
        },
        MaterialParams {
            pipeline_params: PipelineParams {
                depth_test: Comparison::LessOrEqual,
                // Miniquad's GL backend also uses this flag to enable depth testing.
                // Disabling it makes water/glass overwrite nearer opaque blocks.
                depth_write: true,
                cull_face: if translucent {
                    CullFace::Nothing
                } else {
                    CullFace::Back
                },
                front_face_order: FrontFaceOrder::CounterClockwise,
                color_blend: Some(BlendState::new(
                    Equation::Add,
                    BlendFactor::Value(BlendValue::SourceAlpha),
                    BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
                )),
                ..Default::default()
            },
            uniforms,
            textures: vec!["ShadowMap".into(), "SkyMap".into(), "PlanarMap".into()],
        },
    )
    .unwrap_or_else(|error| panic!("Impossible de compiler le shader du monde : {error}"))
}

const TORCH_UNIFORMS: [&str; 8] = [
    "Torch0", "Torch1", "Torch2", "Torch3", "Torch4", "Torch5", "Torch6", "Torch7",
];

const VERTEX_SHADER: &str = r#"#version 100
attribute vec3 position;
attribute vec2 texcoord;
attribute vec4 color0;
attribute vec4 normal;
uniform mat4 Model;
uniform mat4 Projection;
uniform float SceneTime;
uniform float Visuals;
varying mediump vec2 uv;
varying lowp vec4 color;
varying highp vec3 world;
varying mediump vec3 lightData;
varying mediump vec3 faceNormal;
void main() {
    vec4 p = Model * vec4(position, 1.0);
    if (Visuals > 0.5 && (normal.y < -0.5 || normal.y > 6.5)) {
        float weight = normal.y < -0.5 ? fract(position.y) : 1.0;
        float gust = 0.55 + 0.45 * sin(p.x * 0.075 + p.z * 0.055 + SceneTime * 0.55);
        vec2 sway = vec2(
            sin(p.x * 0.7 + p.z * 0.45 + SceneTime * 1.4) * 0.075
              + sin(p.z * 1.7 + SceneTime * 2.6) * 0.035,
            cos(p.z * 0.65 - p.x * 0.25 + SceneTime * 1.15) * 0.055
        );
        p.xz += sway * weight * gust;
    }
    gl_Position = Projection * p;
    world = p.xyz;
    uv = texcoord;
    color = color0 / 255.0;
    lightData = vec3(normal.x, normal.z, normal.w);
    float face = normal.y > 6.5 ? normal.y - 6.0 : normal.y;
    faceNormal = face < 0.5 ? vec3(0.0, 1.0, 0.0)
               : face < 1.5 ? vec3(1.0, 0.0, 0.0)
               : face < 2.5 ? vec3(-1.0, 0.0, 0.0)
               : face < 3.5 ? vec3(0.0, 1.0, 0.0)
               : face < 4.5 ? vec3(0.0, -1.0, 0.0)
               : face < 5.5 ? vec3(0.0, 0.0, 1.0) : vec3(0.0, 0.0, -1.0);
}"#;

const FRAGMENT_SHADER: &str = r#"#version 100
precision mediump float;
varying mediump vec2 uv;
varying lowp vec4 color;
varying highp vec3 world;
varying mediump vec3 lightData;
varying mediump vec3 faceNormal;
uniform sampler2D Texture;
uniform sampler2D SkyMap;
uniform sampler2D PlanarMap;
uniform mat4 PlanarProjection;
uniform float PlanarEnabled;
uniform float ReflectionPass;
uniform float WaterPlane;
uniform vec3 Eye;
uniform vec3 FogColor;
uniform vec2 FogRange;
uniform float Daylight;
uniform float SceneTime;
uniform float Visuals;
uniform vec3 SunDirection;
uniform vec4 Torch0;
uniform vec4 Torch1;
uniform vec4 Torch2;
uniform vec4 Torch3;
uniform vec4 Torch4;
uniform vec4 Torch5;
uniform vec4 Torch6;
uniform vec4 Torch7;
float torchLight(vec4 torch) {
    if (torch.w < 0.5) return 0.0;
    float falloff = max(0.0, 1.0 - distance(world, torch.xyz) / 7.0);
    return falloff * falloff * torch.w;
}
vec3 skyReflection(vec3 ray) {
    vec2 coord = vec2(0.5 + atan(ray.x, ray.z) / 6.2831853,
                      0.5 + asin(clamp(ray.y, -1.0, 1.0)) / 3.1415927);
    return texture2D(SkyMap, coord).rgb;
}
void main() {
    if (ReflectionPass > 0.5 && world.y < WaterPlane + 0.015) discard;
    vec4 texel = texture2D(Texture, uv) * color;
    if (texel.a < 0.06) discard;
    float sky = lightData.x > 0.0 ? lightData.x : 1.0;
    float local = max(max(torchLight(Torch0), torchLight(Torch1)),
                      max(torchLight(Torch2), torchLight(Torch3)));
    local = max(local, max(max(torchLight(Torch4), torchLight(Torch5)),
                           max(torchLight(Torch6), torchLight(Torch7))));
    float brightness = max(max((0.20 + 0.80 * Daylight) * sky, local), lightData.z);
    vec3 tint = mix(vec3(0.68, 0.76, 1.0), vec3(1.0, 0.99, 0.94), Daylight);
    tint = mix(tint, vec3(1.0, 0.77, 0.48), local * (1.0 - sky * Daylight) * 0.65);
    tint = mix(tint, vec3(1.0, 0.92, 0.75), lightData.z);
    vec3 rgb = texel.rgb * tint * brightness;
    if (Visuals > 0.5) {
        vec3 sun = normalize(SunDirection);
        float visibility=sunVisibility(world,faceNormal,sun);
        float diffuse = max(0.0, dot(faceNormal, sun));
        rgb *= mix(1.0,0.43+0.57*visibility,Daylight*sky*(1.0-lightData.z));
        rgb *= mix(1.0, 0.78 + diffuse * 0.30, Daylight * sky * (1.0 - lightData.z));
        if (lightData.y > 0.5 && lightData.y < 1.5 && faceNormal.y > 0.5) {
            vec3 waterNormal = normalize(vec3(
                sin(world.x * 0.9 + world.z * 0.45 + SceneTime * 1.15) * 0.13
                  + sin(world.x * 4.7 + world.z * 3.1 + SceneTime * 1.8) * 0.045,
                1.0,
                cos(world.z * 0.8 - world.x * 0.35 - SceneTime * 0.9) * 0.11
                  + sin(world.z * 5.3 - world.x * 2.5 - SceneTime) * 0.045));
            vec3 view = normalize(Eye - world);
            float fresnel = 0.02 + 0.98 * pow(1.0 - max(0.0, dot(view, waterNormal)), 5.0);
            vec3 reflection = skyReflection(reflect(-view, waterNormal));
            if (PlanarEnabled > 0.5 && abs(world.y - WaterPlane) < 0.16) {
                vec4 projected = PlanarProjection * vec4(world, 1.0);
                vec2 coord = projected.xy / max(projected.w, 0.001) * 0.5 + 0.5;
                coord += waterNormal.xz * 0.026;
                if (projected.w > 0.0 && coord.x > 0.003 && coord.x < 0.997
                    && coord.y > 0.003 && coord.y < 0.997) {
                    vec4 shore = texture2D(PlanarMap, coord);
                    float border = smoothstep(0.0, 0.04,
                        min(min(coord.x, 1.0 - coord.x), min(coord.y, 1.0 - coord.y)));
                    reflection = mix(reflection, shore.rgb, shore.a * border);
                }
            }
            rgb = mix(rgb * vec3(0.48, 0.74, 0.87), reflection, 0.24 + fresnel * 0.72);
            float specular = pow(max(0.0, dot(waterNormal, normalize(sun + view))), 180.0) * Daylight * sky * visibility;
            vec3 solar = mix(vec3(1.0, 0.47, 0.22), vec3(1.0, 0.94, 0.80), smoothstep(0.03, 0.45, sun.y));
            rgb += solar * specular * 2.8;
            texel.a = mix(0.78, 0.98, fresnel);
        }
    }
    if (lightData.y > 1.5) {
        float pulse = 0.5 + 0.5 * sin(world.x * 2.7 + world.z * 1.9 + SceneTime * 0.65);
        rgb *= mix(vec3(1.0, 0.77, 0.55), vec3(1.0, 1.0, 0.84), pulse);
    } else if (lightData.y > 0.5) {
        float wave = sin(world.x * 1.5 + SceneTime * 0.8)
                   * cos(world.z * 1.8 - SceneTime * 0.6);
        rgb += vec3(0.015, 0.04, 0.055) * wave * brightness;
    }
    float fog = smoothstep(FogRange.x, FogRange.y, distance(Eye.xz, world.xz));
    if(Visuals>0.5){
        float haze=(1.0-exp(-distance(Eye,world)*0.0028))*exp(-max(0.0,world.y-WaterPlane)*0.035);
        float sunGlow=pow(max(0.0,dot(normalize(world-Eye),normalize(SunDirection))),12.0)*Daylight;
        vec3 atmosphere=FogColor+vec3(0.15,0.09,0.03)*sunGlow;
        rgb=mix(rgb,atmosphere,min(0.3,haze));
    }
    vec3 distanceColor = Visuals > 0.5 ? skyReflection(normalize(world - Eye)) : FogColor;
    gl_FragColor = vec4(mix(rgb, distanceColor, fog), texel.a);
}"#;

fn empty_mesh(texture: &Texture2D) -> Mesh {
    Mesh {
        vertices: Vec::new(),
        indices: Vec::new(),
        texture: Some(texture.clone()),
    }
}

fn build_chunk(world: &World, key: ChunkKey, texture: &Texture2D) -> ChunkMeshes {
    let mut opaque = vec![empty_mesh(texture)];
    let mut translucent = vec![empty_mesh(texture)];
    let mut torches = Vec::new();
    let mut sea_water = false;
    let mut lava_lights = HashMap::new();
    let Some(chunk) = world.chunks.get(&key) else {
        return ChunkMeshes {
            opaque,
            translucent,
            opaque_bounds: Vec::new(),
            translucent_bounds: Vec::new(),
            torches,
            sea_water,
        };
    };
    let height = (chunk.blocks.len() / (CHUNK * CHUNK) as usize) as i32;
    let mut tops = [0_i32; 256];
    for z in 0..CHUNK {
        for x in 0..CHUNK {
            for y in (0..height).rev() {
                let block = chunk.blocks[(x + CHUNK * (z + CHUNK * y)) as usize];
                if occludes(block) {
                    tops[(x + z * CHUNK) as usize] = y;
                    break;
                }
            }
        }
    }
    for y in 0..height {
        for z in 0..CHUNK {
            for x in 0..CHUNK {
                let block = chunk.blocks[(x + CHUNK * (z + CHUNK * y)) as usize];
                if block == Block::Air {
                    continue;
                }
                let p = [key[0] * CHUNK + x, y, key[1] * CHUNK + z];
                let base = vec3(p[0] as f32, p[1] as f32, p[2] as f32);
                let sky = if y >= tops[(x + z * CHUNK) as usize] - 1 {
                    1.
                } else {
                    0.43
                };
                if let Block::Map(id) = block {
                    let state = crate::city::state(id).expect("Validated map block");
                    if matches!(state.source.as_str(), "chest" | "trapped_chest") {
                        let turns = match state.properties.get("facing").map(String::as_str) {
                            Some("east") => 1,
                            Some("north") => 2,
                            Some("west") => 3,
                            _ => 0,
                        };
                        append_chest(&mut opaque, texture, base, sky, turns);
                        continue;
                    }
                    if state.emission > 0. {
                        lava_lights
                            .entry([x / 4, y / 4, z / 4])
                            .or_insert(base + vec3(0.5, 0.7, 0.5));
                    }
                    if !state.full {
                        let target = if state.material == Block::Glass {
                            &mut translucent
                        } else {
                            &mut opaque
                        };
                        for bounds in &state.boxes {
                            append_city_box(target, texture, world, p, *bounds, state, sky);
                        }
                        continue;
                    }
                }
                if block == Block::Torch {
                    torches.push(base + vec3(0.5, 0.77, 0.5));
                    append_box(
                        &mut opaque,
                        texture,
                        base + vec3(0.42, 0., 0.42),
                        vec3(0.16, 0.63, 0.16),
                        16,
                        sky,
                        0.75,
                    );
                    append_box(
                        &mut opaque,
                        texture,
                        base + vec3(0.36, 0.57, 0.36),
                        vec3(0.28, 0.24, 0.28),
                        17,
                        sky,
                        1.,
                    );
                    continue;
                }
                if block == Block::Chest {
                    append_chest(&mut opaque, texture, base, sky, 0);
                    continue;
                }
                if matches!(block, Block::Door | Block::OpenDoor) {
                    let (offset, size) = if block == Block::Door {
                        (vec3(0., 0., 0.78), vec3(1., 1., 0.18))
                    } else {
                        (vec3(0.02, 0., 0.), vec3(0.18, 1., 1.))
                    };
                    append_box(&mut opaque, texture, base + offset, size, 40, sky, 0.);
                    append_box(
                        &mut opaque,
                        texture,
                        base + offset + Vec3::Y,
                        size,
                        41,
                        sky,
                        0.,
                    );
                    continue;
                }
                if let Some(stage) = block
                    .crop_stage()
                    .filter(|_| !matches!(block, Block::Map(_)))
                {
                    append_crop(&mut opaque, texture, base, stage, sky);
                    continue;
                }
                let material = block.material();
                let emission = match block {
                    Block::Map(id) => crate::city::state(id).unwrap().emission,
                    Block::Lava => 1.,
                    _ => 0.,
                };
                let target = if matches!(material, Block::Water | Block::Glass) {
                    &mut translucent
                } else {
                    &mut opaque
                };
                let fluid_height = if matches!(block, Block::Water | Block::Lava) {
                    world.fluid_height(p)
                } else {
                    None
                };
                if block == Block::Water
                    && y == world.sea()
                    && world.get([p[0], p[1] + 1, p[2]]) == Block::Air
                    && fluid_height.is_some_and(|height| (height - 0.88).abs() < 0.01)
                {
                    sea_water = true;
                }
                if block == Block::Lava
                    && NORMALS
                        .iter()
                        .any(|normal| world.get(add_pos(p, *normal)) != Block::Lava)
                {
                    // ponytail: one light per 4x4x4 lava region; the nearest eight
                    // lights share the existing shader budget with torches.
                    lava_lights
                        .entry([x / 4, y / 4, z / 4])
                        .or_insert(base + vec3(0.5, fluid_height.unwrap_or(1.) * 0.8, 0.5));
                }
                for face in 0..6 {
                    let neighbor_pos = add_pos(p, NORMALS[face]);
                    let neighbor = world.get(neighbor_pos);
                    let bounds = if let Some(height) = fluid_height {
                        fluid_face_bounds(
                            block,
                            neighbor,
                            face,
                            height,
                            if neighbor == block && !matches!(face, 2 | 3) {
                                world.fluid_height(neighbor_pos)
                            } else {
                                None
                            },
                        )
                    } else if face_visible(block, neighbor) {
                        Some((0., 1.))
                    } else {
                        None
                    };
                    let Some((bottom, top)) = bounds else {
                        continue;
                    };
                    if target.last().unwrap().vertices.len() + 4 > MAX_VERTICES {
                        target.push(empty_mesh(texture));
                    }
                    let mesh = target.last_mut().unwrap();
                    let first = mesh.vertices.len() as u16;
                    let uv = tile_uv(tile_for(block, face));
                    let alpha = if block == Block::Water { 175 } else { 255 };
                    for (corner, coords) in CORNERS[face].iter().enumerate() {
                        let mut point = vec3(coords[0], coords[1], coords[2]);
                        point.y = if point.y > 0.5 { top } else { bottom };
                        let shade = if block == Block::Lava {
                            1.
                        } else {
                            FACE_SHADE[face] * ambient_occlusion(world, p, face, *coords)
                        };
                        let mut vertex = Vertex::new2(base + point, uv[corner], WHITE);
                        vertex.color = [
                            (shade * 255.) as u8,
                            (shade * 255.) as u8,
                            (shade * 255.) as u8,
                            alpha,
                        ];
                        // Optional normal channels carry skylight, fluid type and emission.
                        let fluid = match block {
                            Block::Water => 1.,
                            Block::Lava => 2.,
                            _ => 0.,
                        };
                        vertex.normal = vec4(
                            sky,
                            (face + 1 + if material == Block::Leaves { 6 } else { 0 }) as f32,
                            fluid,
                            emission,
                        );
                        mesh.vertices.push(vertex);
                    }
                    mesh.indices
                        .extend([first, first + 1, first + 2, first, first + 2, first + 3]);
                }
            }
        }
    }
    opaque.retain(|mesh| !mesh.vertices.is_empty());
    translucent.retain(|mesh| !mesh.vertices.is_empty());
    let mut lava_lights: Vec<_> = lava_lights.into_iter().collect();
    lava_lights.sort_unstable_by_key(|(key, _)| *key);
    torches.extend(lava_lights.into_iter().map(|(_, position)| position));
    ChunkMeshes {
        opaque_bounds: opaque.iter().map(mesh_bounds).collect(),
        translucent_bounds: translucent.iter().map(mesh_bounds).collect(),
        opaque,
        translucent,
        torches,
        sea_water,
    }
}

fn mesh_bounds(mesh: &Mesh) -> [Vec3; 2] {
    let mut low = Vec3::splat(f32::INFINITY);
    let mut high = Vec3::splat(f32::NEG_INFINITY);
    for v in &mesh.vertices {
        low = low.min(v.position);
        high = high.max(v.position);
    }
    [low - Vec3::splat(0.25), high + Vec3::splat(0.25)]
}
fn in_view(projection: Mat4, [low, high]: [Vec3; 2]) -> bool {
    let m = projection.transpose().to_cols_array_2d();
    let row = |i| Vec4::from_array(m[i]);
    [
        row(3) + row(0),
        row(3) - row(0),
        row(3) + row(1),
        row(3) - row(1),
        row(3) + row(2),
        row(3) - row(2),
    ]
    .into_iter()
    .all(|p| {
        p.dot(vec4(
            if p.x >= 0. { high.x } else { low.x },
            if p.y >= 0. { high.y } else { low.y },
            if p.z >= 0. { high.z } else { low.z },
            1.,
        )) >= 0.
    })
}
fn draw_visible(meshes: &[Mesh], bounds: &[[Vec3; 2]], projection: Mat4) {
    for (mesh, bounds) in meshes.iter().zip(bounds) {
        if in_view(projection, *bounds) {
            draw_mesh(mesh);
        }
    }
}

fn add_pos(a: Pos, b: Pos) -> Pos {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn face_visible(block: Block, neighbor: Block) -> bool {
    if let Block::Map(id) = neighbor {
        if !crate::city::state(id).is_some_and(|state| state.full) {
            return true;
        }
        if occludes(neighbor) {
            return false;
        }
    }
    let block = block.material();
    let neighbor = neighbor.material();
    if neighbor.crop_stage().is_some() {
        return true;
    }
    if matches!(
        neighbor,
        Block::Air | Block::Torch | Block::Chest | Block::Door | Block::OpenDoor
    ) {
        return true;
    }
    if block == neighbor
        && matches!(
            block,
            Block::Water | Block::Lava | Block::Glass | Block::Leaves
        )
    {
        return false;
    }
    matches!(
        neighbor,
        Block::Water | Block::Lava | Block::Glass | Block::Leaves
    ) && (!matches!(block, Block::Water | Block::Lava | Block::Glass) || neighbor != Block::Leaves)
}

/// Keep a level's mesh at the same height used by swimming and eye immersion.
/// A taller cell reveals only the strip above its same-fluid neighbor.
fn fluid_face_bounds(
    block: Block,
    neighbor: Block,
    face: usize,
    height: f32,
    neighbor_height: Option<f32>,
) -> Option<(f32, f32)> {
    if block == neighbor {
        if matches!(face, 2 | 3) {
            return None;
        }
        let bottom = neighbor_height.unwrap_or(1.);
        return (height > bottom + 0.001).then_some((bottom, height));
    }
    // A shallow fluid retains a surface even underneath a solid ceiling.
    if face == 2 && height < 1. {
        return Some((0., height));
    }
    face_visible(block, neighbor).then_some((0., height))
}

fn occludes(block: Block) -> bool {
    if let Block::Map(id) = block {
        return crate::city::state(id).is_some_and(|state| {
            state.full
                && !matches!(
                    state.material,
                    Block::Air | Block::Water | Block::Lava | Block::Glass | Block::Leaves
                )
        });
    }
    if block.crop_stage().is_some() {
        return false;
    }
    !matches!(
        block,
        Block::Air
            | Block::Water
            | Block::Lava
            | Block::Glass
            | Block::Leaves
            | Block::Torch
            | Block::Chest
            | Block::Door
            | Block::OpenDoor
    )
}

fn ambient_occlusion(world: &World, p: Pos, face: usize, corner: [f32; 3]) -> f32 {
    let normal = NORMALS[face];
    let axes = match face {
        0 | 1 => [1, 2],
        2 | 3 => [0, 2],
        _ => [0, 1],
    };
    let mut a = [0; 3];
    let mut b = [0; 3];
    a[axes[0]] = if corner[axes[0]] < 0.5 { -1 } else { 1 };
    b[axes[1]] = if corner[axes[1]] < 0.5 { -1 } else { 1 };
    let outside = add_pos(p, normal);
    let side_a = occludes(world.get(add_pos(outside, a)));
    let side_b = occludes(world.get(add_pos(outside, b)));
    let diagonal = occludes(world.get(add_pos(add_pos(outside, a), b)));
    let amount = if side_a && side_b {
        3
    } else {
        side_a as u8 + side_b as u8 + diagonal as u8
    };
    1. - amount as f32 * 0.115
}

fn tile_for(block: Block, face: usize) -> usize {
    match block {
        Block::Map(id) => crate::city::state(id).expect("Validated map block").tiles[face],
        Block::Bed => {
            if face == 2 {
                31
            } else {
                32
            }
        }
        Block::Sandstone => {
            if matches!(face, 2 | 3) {
                48
            } else {
                33
            }
        }
        Block::Path => {
            if face == 2 {
                34
            } else if face == 3 {
                3
            } else {
                49
            }
        }
        Block::Chest => 35,
        Block::Door | Block::OpenDoor => 40,
        Block::Air => 0,
        Block::Grass => {
            if face == 2 {
                1
            } else if face == 3 {
                3
            } else {
                2
            }
        }
        Block::Dirt => 3,
        Block::Stone => 4,
        Block::Sand => 5,
        Block::Wood => {
            if face == 2 || face == 3 {
                7
            } else {
                6
            }
        }
        Block::Leaves => 8,
        Block::Water => 9,
        Block::Planks => 10,
        Block::Cobble => 11,
        Block::Glass => 12,
        Block::Brick => 13,
        Block::CoalOre => 14,
        Block::IronOre => 15,
        Block::Torch => 16,
        Block::Workbench => {
            if face == 2 {
                18
            } else if face == 3 {
                10
            } else if face == 4 {
                42
            } else {
                19
            }
        }
        Block::Furnace => {
            if matches!(face, 2 | 3) {
                43
            } else if face == 4 {
                21
            } else {
                20
            }
        }
        Block::Snow => {
            if face == 2 {
                22
            } else {
                23
            }
        }
        Block::Bedrock => 24,
        Block::Lava => 25,
        Block::Farmland => {
            if face == 2 {
                26
            } else {
                3
            }
        }
        Block::Wheat0 | Block::Wheat1 | Block::Wheat2 | Block::Wheat3 => {
            27 + block.crop_stage().unwrap() as usize
        }
    }
}

fn tile_uv(tile: usize) -> [Vec2; 4] {
    let x = (tile % 8 * TILE) as f32;
    let y = (tile / 8 * TILE) as f32;
    let u0 = (x + 0.08) / ATLAS as f32;
    let v0 = (y + 0.08) / atlas_height() as f32;
    let u1 = (x + TILE as f32 - 0.08) / ATLAS as f32;
    let v1 = (y + TILE as f32 - 0.08) / atlas_height() as f32;
    [vec2(u0, v1), vec2(u1, v1), vec2(u1, v0), vec2(u0, v0)]
}

fn chest_uv([x, y, width, height]: [usize; 4]) -> [Vec2; 4] {
    let u0 = (64. + x as f32 + 0.08) / ATLAS as f32;
    let v0 = (64. + y as f32 + 0.08) / atlas_height() as f32;
    let u1 = (64. + (x + width) as f32 - 0.08) / ATLAS as f32;
    let v1 = (64. + (y + height) as f32 - 0.08) / atlas_height() as f32;
    [vec2(u0, v1), vec2(u1, v1), vec2(u1, v0), vec2(u0, v0)]
}

#[allow(clippy::too_many_arguments)]
fn append_city_box(
    meshes: &mut Vec<Mesh>,
    texture: &Texture2D,
    world: &World,
    p: Pos,
    bounds: crate::city::Box3,
    state: &crate::city::State,
    sky: f32,
) {
    let base = Vec3::new(p[0] as f32, p[1] as f32, p[2] as f32);
    let origin = Vec3::new(bounds[0], bounds[1], bounds[2]);
    let size = Vec3::new(bounds[3], bounds[4], bounds[5]) - origin;
    for face in 0usize..6 {
        let axis = match face {
            0 | 1 => 0,
            2 | 3 => 1,
            _ => 2,
        };
        let boundary = if face.is_multiple_of(2) {
            bounds[axis + 3] == 1.
        } else {
            bounds[axis] == 0.
        };
        if boundary && occludes(world.get(add_pos(p, NORMALS[face]))) {
            continue;
        }
        if meshes.last().unwrap().vertices.len() + 4 > MAX_VERTICES {
            meshes.push(empty_mesh(texture));
        }
        let mesh = meshes.last_mut().unwrap();
        let first = mesh.vertices.len() as u16;
        let tile = tile_uv(state.tiles[face]);
        for corner in CORNERS[face] {
            let local = origin + Vec3::from_array(corner) * size;
            let coords = match face {
                0 => vec2(1. - local.z, 1. - local.y),
                1 => vec2(local.z, 1. - local.y),
                2 => vec2(local.x, local.z),
                3 => vec2(local.x, 1. - local.z),
                4 => vec2(local.x, 1. - local.y),
                _ => vec2(1. - local.x, 1. - local.y),
            }
            .clamp(Vec2::ZERO, Vec2::ONE);
            let uv = vec2(
                tile[0].x + (tile[1].x - tile[0].x) * coords.x,
                tile[2].y + (tile[0].y - tile[2].y) * coords.y,
            );
            let shade = FACE_SHADE[face] * ambient_occlusion(world, p, face, local.to_array());
            let mut vertex = Vertex::new2(base + local, uv, Color::new(shade, shade, shade, 1.));
            vertex.normal = vec4(
                sky,
                (face
                    + 1
                    + if state.material == Block::Leaves {
                        6
                    } else {
                        0
                    }) as f32,
                0.,
                state.emission,
            );
            mesh.vertices.push(vertex);
        }
        mesh.indices
            .extend([first, first + 1, first + 2, first, first + 2, first + 3]);
    }
}

fn append_chest(meshes: &mut Vec<Mesh>, texture: &Texture2D, base: Vec3, sky: f32, turns: usize) {
    if meshes.last().unwrap().vertices.len() + 72 > MAX_VERTICES {
        meshes.push(empty_mesh(texture));
    }
    let first = meshes.last().unwrap().vertices.len();
    for (offset, size, faces) in [
        (vec3(0.07, 0., 0.07), vec3(0.86, 0.60, 0.86), CHEST_BODY_UV),
        (vec3(0.07, 0.60, 0.07), vec3(0.86, 0.28, 0.86), CHEST_LID_UV),
        (
            vec3(0.43, 0.50, 0.93),
            vec3(0.14, 0.22, 0.065),
            CHEST_LATCH_UV,
        ),
    ] {
        append_textured_box(
            meshes,
            texture,
            base + offset,
            size,
            faces.map(chest_uv),
            sky,
            0.,
        );
    }
    for vertex in &mut meshes.last_mut().unwrap().vertices[first..] {
        for _ in 0..turns {
            let local = vertex.position - base;
            vertex.position = base + vec3(local.z, local.y, 1. - local.x);
            vertex.normal.y = match vertex.normal.y as usize {
                1 => 6.,
                2 => 5.,
                5 => 1.,
                6 => 2.,
                _ => vertex.normal.y,
            };
        }
    }
}

fn append_crop(meshes: &mut Vec<Mesh>, texture: &Texture2D, base: Vec3, stage: u8, sky: f32) {
    if meshes.last().unwrap().vertices.len() + 8 > MAX_VERTICES {
        meshes.push(empty_mesh(texture));
    }
    let mesh = meshes.last_mut().unwrap();
    let height = [0.25, 0.45, 0.70, 0.94][stage.min(3) as usize];
    let uv = tile_uv(27 + stage as usize);
    for (a, b) in [
        (vec3(0.08, 0., 0.08), vec3(0.92, 0., 0.92)),
        (vec3(0.08, 0., 0.92), vec3(0.92, 0., 0.08)),
    ] {
        let first = mesh.vertices.len() as u16;
        for (corner, point) in [a, b, b + Vec3::Y * height, a + Vec3::Y * height]
            .into_iter()
            .enumerate()
        {
            let mut vertex = Vertex::new2(base + point, uv[corner], WHITE);
            vertex.normal = vec4(sky, -1., 0., 0.);
            mesh.vertices.push(vertex);
        }
        mesh.indices.extend([
            first,
            first + 1,
            first + 2,
            first,
            first + 2,
            first + 3,
            first + 2,
            first + 1,
            first,
            first + 3,
            first + 2,
            first,
        ]);
    }
}

fn append_box(
    meshes: &mut Vec<Mesh>,
    texture: &Texture2D,
    origin: Vec3,
    size: Vec3,
    tile: usize,
    sky: f32,
    emission: f32,
) {
    append_textured_box(
        meshes,
        texture,
        origin,
        size,
        [tile_uv(tile); 6],
        sky,
        emission,
    );
}

fn append_textured_box(
    meshes: &mut Vec<Mesh>,
    texture: &Texture2D,
    origin: Vec3,
    size: Vec3,
    faces: [[Vec2; 4]; 6],
    sky: f32,
    emission: f32,
) {
    if meshes.last().unwrap().vertices.len() + 24 > MAX_VERTICES {
        meshes.push(empty_mesh(texture));
    }
    let mesh = meshes.last_mut().unwrap();
    for face in 0..6 {
        let first = mesh.vertices.len() as u16;
        for (corner, coords) in CORNERS[face].iter().enumerate() {
            let mut vertex = Vertex::new2(
                origin + vec3(coords[0], coords[1], coords[2]) * size,
                faces[face][corner],
                Color::new(FACE_SHADE[face], FACE_SHADE[face], FACE_SHADE[face], 1.),
            );
            vertex.normal = vec4(sky, (face + 1) as f32, 0., emission);
            mesh.vertices.push(vertex);
        }
        mesh.indices
            .extend([first, first + 1, first + 2, first, first + 2, first + 3]);
    }
}

fn hash(x: usize, y: usize, tile: usize) -> u32 {
    let mut n = (x as u32)
        .wrapping_mul(374_761_393)
        .wrapping_add((y as u32).wrapping_mul(668_265_263))
        .wrapping_add((tile as u32).wrapping_mul(2_246_822_519));
    n = (n ^ (n >> 13)).wrapping_mul(1_274_126_177);
    n ^ (n >> 16)
}

fn atlas_height() -> usize {
    crate::city::get()
        .expect("Validated city atlas")
        .metadata
        .atlas_height
}

fn combined_atlas_pixels() -> Vec<u8> {
    let mut image = Image::from_file_with_format(
        include_bytes!("../assets/maps/city-atlas.png"),
        Some(ImageFormat::Png),
    )
    .expect("Imported city atlas");
    assert_eq!(
        (image.width as usize, image.height as usize),
        (ATLAS, atlas_height())
    );
    let original = atlas_pixels();
    image.bytes[..original.len()].copy_from_slice(&original);
    image.bytes
}

fn atlas_pixels() -> Vec<u8> {
    let mut pixels = vec![255; ATLAS * ATLAS * 4];
    for tile in 0..37 {
        for y in 0..TILE {
            for x in 0..TILE {
                let noise = (hash(x, y, tile) % 25) as i16 - 12;
                let mut alpha = 255;
                let mut rgb = match tile {
                    0 => [255, 255, 255],
                    33 => {
                        if y % 6 == 0 {
                            [190, 174, 118]
                        } else {
                            [225, 211, 153]
                        }
                    }
                    34 => [166, 137, 83],
                    35 => {
                        if y == 4 || y == 11 || x == 1 || x == 14 {
                            [71, 48, 29]
                        } else if (7..=9).contains(&x) && (6..=10).contains(&y) {
                            [218, 210, 175]
                        } else {
                            [164, 108, 47]
                        }
                    }
                    36 => {
                        if x == 1 || x == 14 || y % 8 == 0 {
                            [90, 58, 29]
                        } else if y < 5 && (3..=12).contains(&x) {
                            [177, 196, 167]
                        } else {
                            [162, 118, 62]
                        }
                    }
                    31 => {
                        if y < 5 {
                            [232, 231, 220]
                        } else {
                            [179, 42, 40]
                        }
                    }
                    32 => {
                        if y < 9 {
                            [179, 42, 40]
                        } else {
                            [126, 88, 47]
                        }
                    }
                    1 => [99, 150, 55],
                    2 if y < 3 + (hash(x, 0, tile) % 3) as usize => [93, 145, 51],
                    2 | 3 => [133, 95, 65],
                    4 | 14 | 15 | 20 | 21 => [132, 138, 145],
                    5 => [225, 212, 152],
                    6 => {
                        if (x + (y / 5) % 2) % 4 == 0 {
                            [88, 65, 39]
                        } else {
                            [123, 93, 53]
                        }
                    }
                    7 => {
                        let ring = (x.abs_diff(7).max(y.abs_diff(7))) % 3;
                        if ring == 0 {
                            [128, 92, 48]
                        } else {
                            [178, 141, 82]
                        }
                    }
                    8 => {
                        if hash(x, y, tile).is_multiple_of(11) {
                            alpha = 0;
                        }
                        [66, 119, 46]
                    }
                    9 => [58, 133, 191],
                    10 => {
                        if y % 4 == 0 || (x + if (y / 4) % 2 == 0 { 0 } else { 8 }) % 16 == 0 {
                            [137, 101, 57]
                        } else {
                            [186, 147, 86]
                        }
                    }
                    11 => {
                        let edge = y % 5 == 0 || (x + (y / 5) * 3) % 6 == 0;
                        if edge {
                            [86, 94, 101]
                        } else {
                            [141, 148, 151]
                        }
                    }
                    12 => {
                        alpha = if x == 0 || y == 0 || x == 15 || y == 15 {
                            190
                        } else if x.abs_diff(y) <= 1 {
                            82
                        } else {
                            30
                        };
                        [189, 225, 234]
                    }
                    13 => {
                        if y % 5 == 0 || (x + (y / 5 % 2) * 4) % 8 == 0 {
                            [185, 164, 139]
                        } else {
                            [163, 78, 58]
                        }
                    }
                    16 => [155, 104, 49],
                    17 => {
                        if y < 5 {
                            [255, 227, 94]
                        } else {
                            [255, 151, 35]
                        }
                    }
                    18 => {
                        if x % 5 == 0 || y % 5 == 0 {
                            [88, 58, 33]
                        } else {
                            [176, 127, 70]
                        }
                    }
                    19 => {
                        if (3..6).contains(&x) || (11..14).contains(&x) {
                            [91, 66, 43]
                        } else {
                            [158, 110, 58]
                        }
                    }
                    22 => [239, 245, 247],
                    23 => {
                        if y < 5 {
                            [231, 239, 243]
                        } else {
                            [151, 166, 174]
                        }
                    }
                    24 => [71, 74, 81],
                    25 => {
                        let crack = (x + y / 3) % 7 == 0 || (y + x / 4) % 6 == 0;
                        if crack {
                            [164, 38, 7]
                        } else if hash(x / 3, y / 3, tile).is_multiple_of(3) {
                            [255, 202, 46]
                        } else {
                            [250, 112, 12]
                        }
                    }
                    26 => {
                        if y % 4 == 0 {
                            [63, 39, 25]
                        } else {
                            [112, 74, 43]
                        }
                    }
                    27..=30 => {
                        let stem = x % 4 == 1 && y >= 3;
                        let leaf = y >= 6 && y % 4 == 0 && x % 4 < 3;
                        let ear = tile >= 29 && y < 6 && x % 4 < 3;
                        if !(stem || leaf || ear) {
                            alpha = 0;
                        }
                        if tile == 30 {
                            if ear {
                                [238, 201, 80]
                            } else {
                                [181, 159, 56]
                            }
                        } else if tile == 29 {
                            if ear {
                                [187, 196, 65]
                            } else {
                                [105, 155, 44]
                            }
                        } else {
                            [72, 145, 37]
                        }
                    }
                    _ => [255, 255, 255],
                };
                if matches!(tile, 14 | 15) {
                    let blob =
                        hash(x / 3, y / 3, tile).is_multiple_of(5) && (x % 3 != 0 || y % 3 != 0);
                    if blob {
                        rgb = if tile == 14 {
                            [40, 44, 49]
                        } else {
                            [180, 128, 93]
                        };
                    }
                }
                if tile == 21 && (3..13).contains(&x) && (5..12).contains(&y) {
                    rgb = if x == 3 || x == 12 || y == 5 || y == 11 {
                        [82, 86, 90]
                    } else {
                        [39, 38, 40]
                    };
                }
                let index = ((tile / 8 * TILE + y) * ATLAS + tile % 8 * TILE + x) * 4;
                for c in 0..3 {
                    pixels[index + c] =
                        (rgb[c] as i16 + if tile == 0 { 0 } else { noise }).clamp(0, 255) as u8;
                }
                pixels[index + 3] = alpha;
            }
        }
    }
    overlay_block_textures(&mut pixels);
    pixels
}

const BLOCK_TEXTURES: &[(usize, &[u8])] = &[
    (1, include_bytes!("../assets/blocks/grass_block_top.png")),
    (2, include_bytes!("../assets/blocks/grass_block_side.png")),
    (3, include_bytes!("../assets/blocks/dirt.png")),
    (4, include_bytes!("../assets/blocks/stone.png")),
    (5, include_bytes!("../assets/blocks/sand.png")),
    (6, include_bytes!("../assets/blocks/oak_log.png")),
    (7, include_bytes!("../assets/blocks/oak_log_top.png")),
    (8, include_bytes!("../assets/blocks/oak_leaves.png")),
    (10, include_bytes!("../assets/blocks/oak_planks.png")),
    (11, include_bytes!("../assets/blocks/cobblestone.png")),
    (12, include_bytes!("../assets/blocks/glass.png")),
    (13, include_bytes!("../assets/blocks/bricks.png")),
    (14, include_bytes!("../assets/blocks/coal_ore.png")),
    (15, include_bytes!("../assets/blocks/iron_ore.png")),
    (
        18,
        include_bytes!("../assets/blocks/crafting_table_top.png"),
    ),
    (
        19,
        include_bytes!("../assets/blocks/crafting_table_side.png"),
    ),
    (20, include_bytes!("../assets/blocks/furnace_side.png")),
    (21, include_bytes!("../assets/blocks/furnace_front.png")),
    (22, include_bytes!("../assets/blocks/snow.png")),
    (23, include_bytes!("../assets/blocks/snow.png")),
    (24, include_bytes!("../assets/blocks/bedrock.png")),
    (26, include_bytes!("../assets/blocks/farmland.png")),
    (33, include_bytes!("../assets/blocks/sandstone.png")),
    (34, include_bytes!("../assets/blocks/grass_path_top.png")),
    (40, include_bytes!("../assets/blocks/oak_door_bottom.png")),
    (41, include_bytes!("../assets/blocks/oak_door_top.png")),
    (
        42,
        include_bytes!("../assets/blocks/crafting_table_front.png"),
    ),
    (43, include_bytes!("../assets/blocks/furnace_top.png")),
    (48, include_bytes!("../assets/blocks/sandstone_top.png")),
    (49, include_bytes!("../assets/blocks/grass_path_side.png")),
];

fn overlay_block_textures(pixels: &mut [u8]) {
    for &(tile, png) in BLOCK_TEXTURES {
        let source = Image::from_file_with_format(png, Some(ImageFormat::Png))
            .expect("Embedded block texture");
        assert_eq!((source.width, source.height), (TILE as u16, TILE as u16));
        for y in 0..TILE {
            for x in 0..TILE {
                let mut rgba: [u8; 4] = source.bytes[(x + y * TILE) * 4..][..4].try_into().unwrap();
                if tile == 1 || tile == 8 {
                    let tint = if tile == 1 {
                        [0.56, 1.18, 0.46]
                    } else {
                        [0.52, 1.02, 0.46]
                    };
                    for c in 0..3 {
                        rgba[c] = (rgba[c] as f32 * tint[c]).min(255.) as u8;
                    }
                }
                // Door windows have opaque tinted panes; the surrounding wood keeps its original pixels.
                if tile == 41 && rgba[3] == 0 {
                    rgba = [86, 116, 135, 255];
                } else if tile == 12 && rgba[3] == 0 {
                    rgba = [188, 215, 229, 30];
                } else if tile == 49 && rgba[3] == 0 {
                    // Minecraft paths are inset; this full-height voxel needs an opaque top rim.
                    let top = (4 * TILE * ATLAS + 2 * TILE + x) * 4;
                    rgba.copy_from_slice(&pixels[top..top + 4]);
                }
                let dst = ((tile / 8 * TILE + y) * ATLAS + tile % 8 * TILE + x) * 4;
                pixels[dst..dst + 4].copy_from_slice(&rgba);
            }
        }
    }
    let chest = Image::from_file_with_format(
        include_bytes!("../assets/blocks/chest.png"),
        Some(ImageFormat::Png),
    )
    .expect("Embedded chest texture");
    assert_eq!((chest.width, chest.height), (64, 64));
    for y in 0..64 {
        let dst = ((64 + y) * ATLAS + 64) * 4;
        pixels[dst..dst + 64 * 4].copy_from_slice(&chest.bytes[y * 64 * 4..(y + 1) * 64 * 4]);
    }
    // The flat front tile also serves item/debug representations of the inset model.
    for y in 0..TILE {
        for x in 0..TILE {
            let (sx, sy) = if (7..=9).contains(&x) && (4..=8).contains(&y) {
                (1 + (x - 7) * 2 / 3, 1 + (y - 4) * 4 / 5)
            } else if y < 5 {
                (14 + x * 14 / TILE, 14 + y)
            } else {
                (14 + x * 14 / TILE, 33 + (y - 5) * 10 / 11)
            };
            let src = (sx + sy * 64) * 4;
            let dst = ((4 * TILE + y) * ATLAS + 3 * TILE + x) * 4;
            pixels[dst..dst + 4].copy_from_slice(&chest.bytes[src..src + 4]);
        }
    }
}

fn mix_color(a: Color, b: Color, t: f32) -> Color {
    Color::new(
        a.r + (b.r - a.r) * t,
        a.g + (b.g - a.g) * t,
        a.b + (b.b - a.b) * t,
        a.a + (b.a - a.a) * t,
    )
}

fn draw_sky_gradient(horizon: Color, daylight: f32, pitch: f32) {
    clear_background(horizon);
    let zenith = mix_color(
        Color::new(0.015, 0.025, 0.065, 1.),
        Color::new(0.26, 0.57, 0.83, 1.),
        daylight,
    );
    let horizon_y = screen_height() * (0.5 + pitch * 0.7);
    let step = screen_height() / 36.;
    for row in 0..36 {
        let y = row as f32 * step;
        let blend = ((horizon_y - y) / (screen_height() * 0.75)).clamp(0., 1.);
        draw_rectangle(
            0.,
            y,
            screen_width(),
            step + 1.,
            mix_color(horizon, zenith, blend),
        );
    }
}

fn sky_square(center: Vec3, eye: Vec3, size: f32, color: Color) {
    let normal = (eye - center).normalize();
    let right = normal.cross(Vec3::Y).normalize_or_zero() * size;
    let up = right.normalize_or_zero().cross(normal) * size;
    let mut mesh = Mesh {
        vertices: Vec::new(),
        indices: vec![0, 1, 2, 0, 2, 3],
        texture: None,
    };
    for p in [
        center - right - up,
        center + right - up,
        center + right + up,
        center - right + up,
    ] {
        mesh.vertices.push(Vertex::new2(p, Vec2::ZERO, color));
    }
    draw_mesh(&mesh);
}

fn draw_celestials(eye: Vec3, angle: f32, daylight: f32) {
    let solar = vec3(angle.cos(), angle.sin(), 0.22).normalize();
    let sun = eye + solar * 180.;
    sky_square(sun, eye, 14., Color::new(1., 0.85, 0.51, 0.055));
    sky_square(sun, eye, 9., Color::new(1., 0.9, 0.61, 0.10));
    sky_square(sun, eye, 6., Color::new(1., 0.97, 0.78, 1.));
    let moon = eye - solar * 180.;
    sky_square(moon, eye, 5., Color::new(0.79, 0.85, 0.95, 1.));
    sky_square(
        moon + vec3(0.5, 0.4, 0.5),
        eye,
        1.2,
        Color::new(0.58, 0.65, 0.77, 1.),
    );
    if daylight < 0.55 {
        for i in 0..75 {
            let a = hash(i, 0, 30) as f32 / u32::MAX as f32 * std::f32::consts::TAU;
            let h = 0.12 + (hash(i, 1, 30) % 800) as f32 / 1000.;
            let horizontal = (1. - h * h).sqrt();
            let center = eye + vec3(a.cos() * horizontal, h, a.sin() * horizontal) * 170.;
            sky_square(
                center,
                eye,
                0.16 + (i % 4) as f32 * 0.06,
                Color::new(0.88, 0.92, 1., (1. - daylight * 1.8).max(0.)),
            );
        }
    }
}

fn draw_clouds(eye: Vec3, daylight: f32, world_height: i32) {
    let drift = (get_time() as f32 * 0.35).rem_euclid(1024.);
    let gx = ((eye.x - drift) / 28.).floor() as i32;
    let gz = (eye.z / 28.).floor() as i32;
    for z in gz - 4..=gz + 4 {
        for x in gx - 4..=gx + 4 {
            let value = hash(x as usize, z as usize, 32);
            if !value.is_multiple_of(4) {
                continue;
            }
            let center = vec3(
                x as f32 * 28. + drift,
                world_height as f32 + 32. + (value % 3) as f32,
                z as f32 * 28.,
            );
            let color = mix_color(
                Color::new(0.40, 0.44, 0.58, 1.),
                Color::new(0.96, 0.97, 0.98, 1.),
                daylight,
            );
            draw_shaded_box(center, vec3(12. + (value % 8) as f32, 2., 9.), color);
            draw_shaded_box(center + vec3(7., 0., 4.), vec3(8., 2., 8.), color);
        }
    }
}

fn draw_shaded_box(center: Vec3, size: Vec3, color: Color) {
    let mut mesh = Mesh {
        vertices: Vec::with_capacity(24),
        indices: Vec::with_capacity(36),
        texture: None,
    };
    for face in 0..6 {
        let first = mesh.vertices.len() as u16;
        let shade = FACE_SHADE[face];
        for coords in CORNERS[face] {
            let mut vertex = Vertex::new2(
                center + (vec3(coords[0], coords[1], coords[2]) - Vec3::splat(0.5)) * size,
                Vec2::ZERO,
                Color::new(color.r * shade, color.g * shade, color.b * shade, color.a),
            );
            vertex.normal = vec4(1., (face + 1) as f32, 0., 0.);
            mesh.vertices.push(vertex);
        }
        mesh.indices
            .extend([first, first + 1, first + 2, first, first + 2, first + 3]);
    }
    draw_mesh(&mesh);
}

fn combat_mesh(combat: &crate::combat::Combat) -> Mesh {
    let mut mesh = Mesh {
        vertices: Vec::new(),
        indices: Vec::new(),
        texture: None,
    };
    for rocket in &combat.rockets {
        let forward = rocket.direction.normalize_or_zero();
        effect_box(
            &mut mesh,
            rocket.position,
            vec3(0.04, 0.04, 0.09),
            forward,
            Color::new(0.72, 0.53, 0.18, 1.),
            0.2,
        );
        effect_box(
            &mut mesh,
            rocket.position + forward * 0.045,
            vec3(0.035, 0.035, 0.035),
            forward,
            Color::new(0.30, 0.36, 0.16, 1.),
            0.2,
        );
    }
    for casing in &combat.casings {
        effect_box(
            &mut mesh,
            casing.position,
            vec3(0.024, 0.024, 0.07),
            casing.velocity.normalize_or_zero(),
            Color::new(0.80, 0.60, 0.22, 1.),
            0.3,
        );
    }
    for impact in &combat.impacts {
        let c = impact.color;
        effect_box(
            &mut mesh,
            impact.position,
            vec3(0.045, 0.045, 0.015),
            impact.normal,
            Color::new(0.09, 0.08, 0.07, (1. - impact.age / 1.2).max(0.)),
            0.1,
        );
        if impact.age < 0.22 {
            for i in 0..5 {
                let a = i as f32 * 2.4;
                let drift =
                    vec3(a.cos(), 0.6, a.sin()) * impact.age * 1.3 + impact.normal * impact.age;
                effect_box(
                    &mut mesh,
                    impact.position + drift,
                    Vec3::splat(0.028),
                    Vec3::Z,
                    Color::from_rgba(c[0], c[1], c[2], 220),
                    0.3,
                );
            }
        }
    }
    for tracer in &combat.tracers {
        let delta = tracer.end - tracer.start;
        let length = delta.length();
        if length > 0.001 {
            effect_box(
                &mut mesh,
                (tracer.start + tracer.end) * 0.5,
                vec3(0.018, 0.018, length),
                delta / length,
                Color::new(1., 0.86, 0.43, (tracer.life / 0.09).clamp(0., 1.)),
                1.,
            );
        }
    }
    mesh
}

fn effect_box(
    mesh: &mut Mesh,
    center: Vec3,
    size: Vec3,
    forward: Vec3,
    color: Color,
    emission: f32,
) {
    let forward = if forward.length_squared() > 0.000_001 {
        forward.normalize()
    } else {
        Vec3::Z
    };
    let reference = if forward.y.abs() > 0.98 {
        Vec3::X
    } else {
        Vec3::Y
    };
    let right = reference.cross(forward).normalize();
    let up = forward.cross(right);
    for (face, corners) in CORNERS.iter().enumerate() {
        let first = mesh.vertices.len() as u16;
        for coords in corners {
            let local = (vec3(coords[0], coords[1], coords[2]) - Vec3::splat(0.5)) * size;
            let mut vertex = Vertex::new2(
                center + right * local.x + up * local.y + forward * local.z,
                Vec2::ZERO,
                Color::new(
                    color.r * FACE_SHADE[face],
                    color.g * FACE_SHADE[face],
                    color.b * FACE_SHADE[face],
                    color.a,
                ),
            );
            vertex.normal = vec4(1., (face + 1) as f32, 0., emission);
            mesh.vertices.push(vertex);
        }
        mesh.indices
            .extend([first, first + 1, first + 2, first, first + 2, first + 3]);
    }
}

fn draw_cracks(p: Vec3, progress: f32) {
    let count = (progress.clamp(0., 1.) * 12.).ceil() as usize;
    for face in 0..6 {
        let normal = vec3(
            NORMALS[face][0] as f32,
            NORMALS[face][1] as f32,
            NORMALS[face][2] as f32,
        );
        let origin = vec3(
            CORNERS[face][0][0],
            CORNERS[face][0][1],
            CORNERS[face][0][2],
        );
        let u = vec3(
            CORNERS[face][1][0],
            CORNERS[face][1][1],
            CORNERS[face][1][2],
        ) - origin;
        let v = vec3(
            CORNERS[face][3][0],
            CORNERS[face][3][1],
            CORNERS[face][3][2],
        ) - origin;
        let mut previous = vec2(0.5, 0.5);
        for i in 0..count {
            let point = vec2(
                0.10 + (hash(i, face, 29) % 800) as f32 / 1000.,
                0.10 + (hash(i, face + 7, 29) % 800) as f32 / 1000.,
            );
            draw_line_3d(
                p + origin + u * previous.x + v * previous.y + normal * 0.006,
                p + origin + u * point.x + v * point.y + normal * 0.006,
                Color::new(0.08, 0.07, 0.07, 0.80),
            );
            previous = if i % 3 == 2 { vec2(0.5, 0.5) } else { point };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mesh_culling_keeps_crossing_faces_and_rejects_hidden_camera_regions() {
        let projection = Mat4::perspective_rh_gl(72f32.to_radians(), 1., 0.04, 280.)
            * Mat4::look_at_rh(vec3(0., 2., 5.), vec3(0., 2., 0.), Vec3::Y);
        assert!(in_view(projection, [vec3(-1., 1., -1.), vec3(1., 3., 1.)]));
        assert!(!in_view(
            projection,
            [vec3(-1., 1., 10.), vec3(1., 3., 12.)]
        ));
        assert!(!in_view(
            projection,
            [vec3(30., 1., -1.), vec3(32., 3., 1.)]
        ));
        assert!(in_view(
            projection,
            [vec3(-1., -100., -1.), vec3(1., 300., 1.)]
        ));
        assert!(!in_view(
            projection,
            [vec3(-1., -100., -1.), vec3(1., -80., 1.)]
        ));
        assert!(in_view(
            projection,
            [vec3(-30., -30., -30.), vec3(30., 30., 30.)]
        ));
    }

    #[test]
    fn water_mirror_preserves_plane_and_reflects_camera_height() {
        let plane = crate::world::SEA as f32 + 0.88;
        for point in [
            vec3(4., plane, 7.),
            vec3(-3., plane + 6., 2.),
            vec3(1., plane - 4., 9.),
        ] {
            let reflected = mirror_point(point, plane);
            assert_eq!(reflected.x, point.x);
            assert_eq!(reflected.z, point.z);
            assert!((reflected.y + point.y - plane * 2.).abs() < 0.0001);
            assert!(mirror_point(reflected, plane).distance(point) < 0.0001);
        }
    }

    #[test]
    fn weapon_effect_boxes_keep_finite_geometry_for_vertical_and_zero_directions() {
        for direction in [Vec3::Y, -Vec3::Y, Vec3::ZERO, vec3(0.3, 0.5, -0.8)] {
            let mut mesh = Mesh {
                vertices: Vec::new(),
                indices: Vec::new(),
                texture: None,
            };
            let center = vec3(4., 7., -9.);
            effect_box(&mut mesh, center, vec3(0.2, 0.2, 1.), direction, WHITE, 1.);
            assert_eq!(mesh.vertices.len(), 24);
            assert_eq!(mesh.indices.len(), 36);
            assert!(mesh
                .vertices
                .iter()
                .all(|vertex| vertex.position.is_finite()));
            let average = mesh
                .vertices
                .iter()
                .map(|vertex| vertex.position)
                .sum::<Vec3>()
                / 24.;
            assert!(average.distance(center) < 0.0001);
            for face in mesh.vertices.as_chunks::<4>().0 {
                let normal = (face[1].position - face[0].position)
                    .cross(face[2].position - face[0].position);
                let face_center = face.iter().map(|vertex| vertex.position).sum::<Vec3>() / 4.;
                assert!(normal.dot(face_center - center) > 0.);
            }
        }
    }

    #[test]
    fn atlas_and_face_geometry_are_consistent() {
        let pixels = atlas_pixels();
        assert_eq!(pixels.len(), ATLAS * ATLAS * 4);
        assert_eq!(pixels, atlas_pixels());
        for face in 0..6 {
            let c = CORNERS[face].map(|p| vec3(p[0], p[1], p[2]));
            let n = vec3(
                NORMALS[face][0] as f32,
                NORMALS[face][1] as f32,
                NORMALS[face][2] as f32,
            );
            assert!((c[1] - c[0]).cross(c[2] - c[0]).dot(n) > 0.99);
        }
        assert!(!face_visible(Block::Stone, Block::Stone));
        assert!(face_visible(Block::Stone, Block::Water));
        assert!(!face_visible(Block::Water, Block::Water));
        assert!(face_visible(Block::Water, Block::Air));
        assert_eq!(tile_for(Block::Grass, 2), 1);
        assert_eq!(tile_for(Block::Grass, 3), 3);
        assert!(face_visible(Block::Water, Block::Lava));
        assert!(face_visible(Block::Lava, Block::Water));
        assert!(!face_visible(Block::Lava, Block::Lava));
        for tile in 0..26 {
            for uv in tile_uv(tile) {
                assert!(uv.x >= 0. && uv.x <= 1. && uv.y >= 0. && uv.y <= 1.);
            }
        }
    }

    #[test]
    fn imported_moss_is_opaque_and_flowers_never_hide_terrain() {
        let pixels = combined_atlas_pixels();
        let city = crate::city::get().unwrap();
        for source in ["moss_block", "moss_carpet"] {
            let state = city
                .metadata
                .states
                .iter()
                .find(|s| s.source == source)
                .unwrap();
            for tile in state.tiles {
                for y in 0..TILE {
                    for x in 0..TILE {
                        let index = ((tile / 8 * TILE + y) * ATLAS + tile % 8 * TILE + x) * 4;
                        assert_eq!(pixels[index + 3], 255, "{source} texture has a hole");
                    }
                }
            }
        }
        for source in [
            "allium",
            "azure_bluet",
            "blue_orchid",
            "dandelion",
            "lily_of_the_valley",
            "orange_tulip",
            "oxeye_daisy",
            "pink_tulip",
            "poppy",
            "red_tulip",
            "white_tulip",
            "cornflower",
            "sunflower",
            "wildflowers",
        ] {
            let (id, state) = city
                .metadata
                .states
                .iter()
                .enumerate()
                .find(|(_, s)| s.source == source)
                .unwrap();
            assert_eq!(state.material, Block::Leaves, "{source} is a plant");
            assert!(state.colliders.is_empty(), "{source} blocks movement");
            assert!(!state.full);
            assert!(
                face_visible(Block::Stone, Block::Map(id as u16)),
                "{source} hid terrain"
            );
        }
    }

    #[test]
    fn imported_atlas_and_partial_blocks_preserve_legacy_pixels_and_visible_terrain() {
        let pixels = combined_atlas_pixels();
        assert_eq!(pixels.len(), ATLAS * atlas_height() * 4);
        assert_eq!(&pixels[..ATLAS * ATLAS * 4], atlas_pixels());
        let city = crate::city::get().unwrap();
        let mut partial_count = 0;
        let mut opaque_count = 0;
        for (id, state) in city.metadata.states.iter().enumerate() {
            let block = Block::Map(id as u16);
            for tile in state.tiles {
                for uv in tile_uv(tile) {
                    assert!(uv.x > 0. && uv.x < 1. && uv.y > 0. && uv.y < 1.);
                    assert!(uv.y >= ATLAS as f32 / atlas_height() as f32);
                }
            }
            if !state.full {
                assert!(
                    face_visible(Block::Stone, block),
                    "Partial {:?} hid adjacent terrain",
                    state.name
                );
                assert!(!occludes(block));
                partial_count += 1;
            } else if !matches!(
                state.material,
                Block::Air | Block::Water | Block::Lava | Block::Glass | Block::Leaves
            ) {
                assert!(!face_visible(Block::Stone, block));
                assert!(occludes(block));
                for tile in state.tiles {
                    for y in 0..TILE {
                        for x in 0..TILE {
                            let index = ((tile / 8 * TILE + y) * ATLAS + tile % 8 * TILE + x) * 4;
                            assert!(
                                pixels[index + 3] >= 16,
                                "Opaque {} hides terrain behind a texture hole",
                                state.source
                            );
                        }
                    }
                }
                opaque_count += 1;
            }
        }
        assert!(partial_count > 500 && opaque_count > 200);
    }

    #[test]
    fn partial_blocks_preserve_neighbor_faces_and_opaque_textures() {
        let pixels = atlas_pixels();
        for block in [Block::Chest, Block::Door, Block::OpenDoor] {
            assert!(
                face_visible(Block::Stone, block),
                "{block:?} leaves uncovered parts of neighboring terrain faces"
            );
            assert!(!occludes(block), "{block:?} is not a full opaque cube");
            let tile = tile_for(block, 4);
            for y in 0..TILE {
                for x in 0..TILE {
                    let index = ((tile / 8 * TILE + y) * ATLAS + tile % 8 * TILE + x) * 4;
                    assert_eq!(pixels[index + 3], 255, "{block:?} texture has a hole");
                }
            }
        }
    }

    #[test]
    fn embedded_textures_keep_per_face_details_and_valid_chest_uvs() {
        let pixels = atlas_pixels();
        for &(tile, png) in BLOCK_TEXTURES {
            assert!(
                tile % 8 < 4 || tile / 8 < 4,
                "Tile overlaps the chest sheet"
            );
            let image = Image::from_file_with_format(png, Some(ImageFormat::Png)).unwrap();
            assert_eq!((image.width, image.height), (16, 16));
            for y in 0..TILE {
                for x in 0..TILE {
                    let src = (x + y * TILE) * 4;
                    let dst = ((tile / 8 * TILE + y) * ATLAS + tile % 8 * TILE + x) * 4;
                    if !matches!(tile, 1 | 8 | 12 | 41 | 49) {
                        assert_eq!(pixels[dst..dst + 4], image.bytes[src..src + 4]);
                    }
                    if !matches!(tile, 8 | 12) {
                        assert_eq!(pixels[dst + 3], 255);
                    }
                }
            }
        }
        for block in [Block::Workbench, Block::Furnace] {
            let faces = [tile_for(block, 0), tile_for(block, 2), tile_for(block, 4)];
            assert_ne!(faces[0], faces[1]);
            assert_ne!(faces[1], faces[2]);
            assert_ne!(faces[0], faces[2]);
        }
        assert_ne!(tile_uv(40), tile_uv(41));
        for [x, y, width, height] in CHEST_BODY_UV
            .into_iter()
            .chain(CHEST_LID_UV)
            .chain(CHEST_LATCH_UV)
        {
            assert!(x + width <= 64 && y + height <= 64);
            for sy in y..y + height {
                for sx in x..x + width {
                    let index = ((64 + sy) * ATLAS + 64 + sx) * 4;
                    assert_eq!(
                        pixels[index + 3],
                        255,
                        "Chest face samples a transparent margin"
                    );
                }
            }
        }
    }

    #[test]
    fn fluid_mesh_exposes_only_actual_surface_and_level_differences() {
        assert_eq!(
            fluid_face_bounds(Block::Water, Block::Air, 2, 0.33, None),
            Some((0., 0.33))
        );
        assert_eq!(
            fluid_face_bounds(Block::Water, Block::Water, 0, 0.88, Some(0.33)),
            Some((0.33, 0.88))
        );
        assert_eq!(
            fluid_face_bounds(Block::Water, Block::Water, 0, 0.33, Some(0.88)),
            None
        );
        assert_eq!(
            fluid_face_bounds(Block::Water, Block::Water, 0, 0.88, Some(0.88)),
            None
        );
        assert_eq!(
            fluid_face_bounds(Block::Water, Block::Water, 2, 1., Some(0.88)),
            None
        );
        assert_eq!(
            fluid_face_bounds(Block::Water, Block::Stone, 2, 0.88, None),
            Some((0., 0.88))
        );
        assert_eq!(
            fluid_face_bounds(Block::Lava, Block::Water, 0, 1., Some(0.22)),
            Some((0., 1.))
        );
        for face in [0, 1, 4, 5] {
            let (bottom, top) =
                fluid_face_bounds(Block::Water, Block::Water, face, 0.88, Some(0.33)).unwrap();
            let c = CORNERS[face].map(|p| vec3(p[0], if p[1] > 0.5 { top } else { bottom }, p[2]));
            let n = vec3(
                NORMALS[face][0] as f32,
                NORMALS[face][1] as f32,
                NORMALS[face][2] as f32,
            );
            assert!((c[1] - c[0]).cross(c[2] - c[0]).dot(n) > 0.);
        }
    }
}
