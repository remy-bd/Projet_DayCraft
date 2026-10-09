//! Actual downloaded, rigged meshes; eight baked poses keep animation inexpensive.
use crate::game::Mob;
use macroquad::prelude::*;
use macroquad::window::miniquad::{
    Comparison, CullFace, PipelineParams, ShaderSource, UniformDesc, UniformType,
};

pub struct MobModels {
    poses: Vec<Vec<Mesh>>,
    material: Material,
}
impl MobModels {
    pub fn new() -> Self {
        let poses = crate::mob_assets::POSES
            .iter()
            .zip(crate::mob_assets::SKINS)
            .map(|(poses, skin)| {
                let texture = Texture2D::from_file_with_format(skin, Some(ImageFormat::Png));
                texture.set_filter(FilterMode::Nearest);
                poses
                    .iter()
                    .map(|obj| {
                        let mut mesh = crate::viewmodel::parse_obj(obj, "newmtl Skin\nKd 1 1 1\n")
                            .expect("Downloaded mob OBJ")
                            .body;
                        mesh.texture = Some(texture.clone());
                        mesh
                    })
                    .collect()
            })
            .collect();
        let fragment = format!(
            "#version 100\nprecision highp float;\n{}\n{}",
            include_str!("../assets/shadow.glsl"),
            FRAGMENT
        );
        let material = load_material(
            ShaderSource::Glsl {
                vertex: VERTEX,
                fragment: &fragment,
            },
            MaterialParams {
                pipeline_params: PipelineParams {
                    depth_test: Comparison::LessOrEqual,
                    depth_write: true,
                    cull_face: CullFace::Nothing,
                    ..Default::default()
                },
                uniforms: vec![
                    UniformDesc::new("SunDirection", UniformType::Float3),
                    UniformDesc::new("Daylight", UniformType::Float1),
                    UniformDesc::new("Eye", UniformType::Float3),
                    UniformDesc::new("FogColor", UniformType::Float3),
                    UniformDesc::new("FogRange", UniformType::Float2),
                    UniformDesc::new("ShadowProjection", UniformType::Mat4),
                    UniformDesc::new("ShadowEnabled", UniformType::Float1),
                    UniformDesc::new("MobTint", UniformType::Float3),
                ],
                textures: vec!["ShadowMap".into()],
            },
        )
        .expect("Mob shader");
        Self { poses, material }
    }
    #[allow(clippy::too_many_arguments)]
    pub fn lighting(
        &self,
        sun: Vec3,
        daylight: f32,
        eye: Vec3,
        fog: Color,
        range: [f32; 2],
        projection: Mat4,
        shadow: &Texture2D,
        enabled: bool,
    ) {
        self.material.set_uniform("SunDirection", sun);
        self.material.set_uniform("Daylight", daylight);
        self.material.set_uniform("Eye", eye);
        self.material.set_uniform("FogColor", [fog.r, fog.g, fog.b]);
        self.material.set_uniform("FogRange", range);
        self.material.set_uniform("ShadowProjection", projection);
        self.material
            .set_uniform("ShadowEnabled", if enabled { 1.0f32 } else { 0.0f32 });
        self.material.set_texture("ShadowMap", shadow.clone());
    }
    pub fn assert_sheep_opaque(&self) {
        let shadow = Texture2D::from_rgba8(1, 1, &[255; 4]);
        let center = vec3(0., 0.8, 0.);
        for pose in 0..8 {
            for side in [Vec3::X, -Vec3::X, Vec3::Z, -Vec3::Z] {
                let eye = center + side * 4.;
                self.lighting(
                    Vec3::Y,
                    1.,
                    eye,
                    MAGENTA,
                    [100., 200.],
                    Mat4::IDENTITY,
                    &shadow,
                    false,
                );
                set_camera(&Camera3D {
                    position: eye,
                    target: center,
                    up: Vec3::Y,
                    fovy: 0.9,
                    ..Default::default()
                });
                clear_background(MAGENTA);
                self.draw(
                    &Mob {
                        health: 10.,
                        walking: true,
                        phase: (pose as f32 + 0.1) / 7.,
                        ..Default::default()
                    },
                    false,
                );
                gl_use_default_material();
                set_default_camera();
                for x in [0.48, 0.5, 0.52] {
                    for y in [0.48, 0.5, 0.52] {
                        let pixel = crate::graphics::screen_pixel(x, y);
                        assert!(
                            pixel[1] > 10 && pixel[0] < 245,
                            "Transparent sheep torso: pose {pose}, view {side:?}, pixel {pixel:?}"
                        );
                    }
                }
            }
        }
    }

    pub fn draw(&self, mob: &Mob, shadow: bool) {
        let pose = if mob.walking {
            (mob.phase * 7.) as usize % 8
        } else {
            0
        };
        let scale = if mob.baby > 0. { 0.55 } else { 1. };
        let transform = Mat4::from_scale_rotation_translation(
            Vec3::splat(scale),
            Quat::from_rotation_y(-mob.yaw),
            mob.position,
        );
        if !shadow {
            gl_use_material(&self.material);
            let flash: f32 = if mob.fuse > 0. && (mob.fuse * 8.).sin() > 0. {
                1.5
            } else {
                1.
            };
            self.material.set_uniform("MobTint", [flash, flash, flash]);
        }
        unsafe {
            let gl = get_internal_gl().quad_gl;
            gl.push_model_matrix(transform);
            draw_mesh(&self.poses[mob.kind as usize][pose]);
            gl.pop_model_matrix();
        }
        if !shadow && mob.love > 0. {
            gl_use_default_material();
            draw_sphere(
                mob.position + vec3(0., mob.kind.height() * scale + 0.3, 0.),
                0.10,
                None,
                Color::new(1., 0.15, 0.25, 1.),
            );
        }
    }
}
const VERTEX: &str = r#"#version 100
attribute vec3 position;
attribute vec2 texcoord;
attribute vec4 normal;
uniform mat4 Model;
uniform mat4 Projection;
varying highp vec3 world;
varying mediump vec3 faceNormal;
varying mediump vec2 uv;
void main(){vec4 p=Model*vec4(position,1.0);world=p.xyz;faceNormal=normalize((Model*vec4(normal.xyz,0.0)).xyz);uv=texcoord;gl_Position=Projection*p;}
"#;
const FRAGMENT: &str = r#"
uniform sampler2D Texture;
uniform vec3 SunDirection;
uniform float Daylight;
uniform vec3 Eye;
uniform vec3 FogColor;
uniform vec2 FogRange;
uniform vec3 MobTint;
varying highp vec3 world;
varying mediump vec3 faceNormal;
varying mediump vec2 uv;
void main(){
    vec4 t=texture2D(Texture,uv);if(t.a<0.1)discard;
    vec3 sun=normalize(SunDirection);float shadow=sunVisibility(world,faceNormal,sun);
    float light=0.22+Daylight*(0.32+0.48*max(0.0,dot(faceNormal,sun))*shadow);
    vec3 c=t.rgb*light*MobTint;
    float fog=smoothstep(FogRange.x,FogRange.y,distance(Eye.xz,world.xz));
    gl_FragColor=vec4(mix(c,FogColor,fog),1.0);
}
"#;
#[cfg(test)]
mod tests {
    #[test]
    fn every_sheep_core_face_samples_opaque_skin_in_all_poses() {
        use macroquad::prelude::*;
        let skin =
            Image::from_file_with_format(crate::mob_assets::SKINS[0], Some(ImageFormat::Png))
                .unwrap();
        for obj in crate::mob_assets::POSES[0] {
            let mesh = crate::viewmodel::parse_obj(obj, "newmtl Skin\nKd 1 1 1\n")
                .unwrap()
                .body;
            for face in mesh.indices.as_chunks::<3>().0 {
                let uv = face
                    .iter()
                    .map(|&i| mesh.vertices[i as usize].uv)
                    .sum::<Vec2>()
                    / 3.;
                if uv.x >= 0.5 {
                    continue;
                }
                let x = (uv.x * skin.width as f32) as u32;
                let y = (uv.y * skin.height as f32) as u32;
                assert!(
                    skin.get_pixel(x, y).a > 0.9,
                    "Sheep core has a transparent face at {uv:?}"
                );
            }
        }
    }
    #[test]
    fn downloaded_mobs_have_valid_distinct_textured_animation_frames() {
        for poses in crate::mob_assets::POSES {
            let mut changes = 0;
            for obj in poses {
                let mesh = crate::viewmodel::parse_obj(obj, "newmtl Skin\nKd 1 1 1\n")
                    .unwrap()
                    .body;
                assert!(mesh.indices.len() > 100);
                assert!(mesh
                    .vertices
                    .iter()
                    .all(|v| v.position.is_finite() && v.normal.is_finite()));
                assert!(mesh.vertices.iter().any(|v| v.uv.length_squared() > 0.1));
                changes += usize::from(obj != poses[0]);
            }
            assert!(changes >= 6);
        }
    }
}
