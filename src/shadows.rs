//! Real projected sunlight shadows, filtered in the world and creature shaders.
use macroquad::camera::Camera;
use macroquad::prelude::*;
use macroquad::window::miniquad::{Comparison, CullFace, PipelineParams, ShaderSource};
pub struct Shadows {
    pub target: RenderTarget,
    material: Material,
}
impl Shadows {
    pub fn new() -> Self {
        let target = render_target_ex(
            1024,
            1024,
            RenderTargetParams {
                depth: true,
                ..Default::default()
            },
        );
        target.texture.set_filter(FilterMode::Nearest);
        let material = load_material(
            ShaderSource::Glsl {
                vertex: VERTEX,
                fragment: FRAGMENT,
            },
            MaterialParams {
                uniforms: vec![UniformDesc::new("SceneTime", UniformType::Float1)],
                pipeline_params: PipelineParams {
                    depth_test: Comparison::LessOrEqual,
                    depth_write: true,
                    cull_face: CullFace::Nothing,
                    ..Default::default()
                },
                ..Default::default()
            },
        )
        .expect("Sun shadow shader");
        Self { target, material }
    }
    pub fn begin(&self, eye: Vec3, sun: Vec3, time: f32, height: i32) -> Mat4 {
        let center = vec3((eye.x / 2.).floor() * 2., eye.y, (eye.z / 2.).floor() * 2.);
        let distance = (height as f32).max(100.);
        let camera = Camera3D {
            position: center + sun * distance,
            target: center,
            up: Vec3::Z,
            projection: Projection::Orthographics,
            fovy: 112.,
            aspect: Some(1.),
            z_near: 1.,
            z_far: distance * 2. + 20.,
            render_target: Some(self.target.clone()),
            ..Default::default()
        };
        let matrix = camera.matrix();
        set_camera(&camera);
        clear_background(WHITE);
        self.material.set_uniform("SceneTime", time);
        gl_use_material(&self.material);
        matrix
    }
}
const VERTEX: &str = r#"#version 100
attribute vec3 position;attribute vec2 texcoord;attribute vec4 normal;
uniform mat4 Model;uniform mat4 Projection;uniform float SceneTime;
varying mediump vec2 uv;
void main(){uv=texcoord;vec4 p=Model*vec4(position,1.0);
if(normal.y<-.5||normal.y>6.5){
float weight=normal.y<-.5?fract(position.y):1.0;
float gust=.55+.45*sin(p.x*.075+p.z*.055+SceneTime*.55);
vec2 sway=vec2(sin(p.x*.7+p.z*.45+SceneTime*1.4)*.075+sin(p.z*1.7+SceneTime*2.6)*.035,cos(p.z*.65-p.x*.25+SceneTime*1.15)*.055);
p.xz+=sway*weight*gust;
}
gl_Position=Projection*p;}"#;
const FRAGMENT: &str = r#"#version 100
precision highp float;
uniform sampler2D Texture;varying mediump vec2 uv;
void main(){if(texture2D(Texture,uv).a<0.4)discard;
vec4 enc=fract(gl_FragCoord.z*vec4(1.0,255.0,65025.0,16581375.0));
enc-=enc.yzww*vec4(1.0/255.0,1.0/255.0,1.0/255.0,0.0);
gl_FragColor=enc;}
"#;
