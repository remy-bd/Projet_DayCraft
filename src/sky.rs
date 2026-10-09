//! Camera-correct atmosphere and raymarched clouds, also sampled by water.
use macroquad::prelude::*;

pub struct Sky {
    material: Material,
    scene: Option<RenderTarget>,
    environment: RenderTarget,
}

fn view_basis(direction: Vec3) -> (Vec3, Vec3) {
    let right = direction.cross(Vec3::Y).normalize_or_zero();
    (right, right.cross(direction).normalize_or_zero())
}

impl Sky {
    pub fn new() -> Self {
        let material = load_material(
            ShaderSource::Glsl {
                vertex: VERTEX,
                fragment: FRAGMENT,
            },
            MaterialParams {
                uniforms: vec![
                    UniformDesc::new("SkyEye", UniformType::Float3),
                    UniformDesc::new("ViewDirection", UniformType::Float3),
                    UniformDesc::new("ViewRight", UniformType::Float3),
                    UniformDesc::new("ViewUp", UniformType::Float3),
                    UniformDesc::new("SunDirection", UniformType::Float3),
                    UniformDesc::new("Daylight", UniformType::Float1),
                    UniformDesc::new("SceneTime", UniformType::Float1),
                    UniformDesc::new("SkyMode", UniformType::Float1),
                    UniformDesc::new("Aspect", UniformType::Float1),
                    UniformDesc::new("TanHalfFov", UniformType::Float1),
                    UniformDesc::new("CloudAmount", UniformType::Float1),
                ],
                ..Default::default()
            },
        )
        .expect("Atmosphere and volumetric cloud shader");
        material.set_uniform("CloudAmount", 1_f32);
        let environment = render_target(256, 128);
        environment.texture.set_filter(FilterMode::Linear);
        Self {
            material,
            scene: None,
            environment,
        }
    }

    pub fn reflection(&self) -> Texture2D {
        self.environment.texture.clone()
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &mut self,
        eye: Vec3,
        direction: Vec3,
        sun: Vec3,
        daylight: f32,
        fovy: f32,
        time: f32,
        destination: Option<RenderTarget>,
    ) {
        let width = (screen_width() * 0.5).ceil().max(1.) as u32;
        let height = (screen_height() * 0.5).ceil().max(1.) as u32;
        if self.scene.as_ref().is_none_or(|target| {
            target.texture.width() as u32 != width || target.texture.height() as u32 != height
        }) {
            let target = render_target(width, height);
            target.texture.set_filter(FilterMode::Linear);
            self.scene = Some(target);
        }
        let (right, up) = view_basis(direction);
        self.material.set_uniform("SkyEye", eye);
        self.material.set_uniform("ViewDirection", direction);
        self.material.set_uniform("ViewRight", right);
        self.material.set_uniform("ViewUp", up);
        self.material.set_uniform("SunDirection", sun);
        self.material.set_uniform("Daylight", daylight);
        self.material.set_uniform("SceneTime", time);
        self.material
            .set_uniform("Aspect", screen_width() / screen_height());
        self.material.set_uniform("TanHalfFov", (fovy * 0.5).tan());
        for (target, mode) in [
            (&self.environment, 1_f32),
            (self.scene.as_ref().unwrap(), 0.),
        ] {
            let w = target.texture.width();
            let h = target.texture.height();
            set_camera(&Camera2D {
                render_target: Some(target.clone()),
                ..Camera2D::from_display_rect(Rect::new(0., 0., w, h))
            });
            clear_background(BLACK);
            self.material.set_uniform("SkyMode", mode);
            gl_use_material(&self.material);
            draw_mesh(&Mesh {
                vertices: vec![
                    Vertex::new2(vec3(0., 0., 0.), vec2(0., 0.), WHITE),
                    Vertex::new2(vec3(w, 0., 0.), vec2(1., 0.), WHITE),
                    Vertex::new2(vec3(w, h, 0.), vec2(1., 1.), WHITE),
                    Vertex::new2(vec3(0., h, 0.), vec2(0., 1.), WHITE),
                ],
                indices: vec![0, 1, 2, 0, 2, 3],
                texture: None,
            });
            gl_use_default_material();
            set_default_camera();
        }
        set_camera(&Camera2D {
            render_target: destination,
            ..Camera2D::from_display_rect(Rect::new(0., 0., screen_width(), screen_height()))
        });
        draw_texture_ex(
            &self.scene.as_ref().unwrap().texture,
            0.,
            0.,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(screen_width(), screen_height())),
                flip_y: true,
                ..Default::default()
            },
        );
    }

    pub fn assert_effects(&mut self) {
        let eye = vec3(0., 40., 0.);
        let direction = vec3(0.2, 0.65, 1.).normalize();
        let sun = vec3(0.4, 0.5, 0.8).normalize();
        let fovy = 72_f32.to_radians();
        let sample = || {
            let mut pixels = Vec::new();
            for y in [0.2, 0.4, 0.6, 0.8] {
                for x in [0.15, 0.35, 0.55, 0.75, 0.9] {
                    pixels.push(crate::graphics::screen_pixel(x, y));
                }
            }
            pixels
        };
        self.material.set_uniform("CloudAmount", 0_f32);
        self.draw(eye, direction, sun, 1., fovy, 0., None);
        set_default_camera();
        let clear = sample();
        self.material.set_uniform("CloudAmount", 1_f32);
        self.draw(eye, direction, sun, 1., fovy, 0., None);
        set_default_camera();
        let clouds = sample();
        self.draw(eye, direction, sun, 1., fovy, 120., None);
        set_default_camera();
        let moved = sample();
        let changed = |a: &[[u8; 4]], b: &[[u8; 4]]| {
            a.iter()
                .zip(b)
                .filter(|(x, y)| (0..3).map(|c| x[c].abs_diff(y[c]) as u32).sum::<u32>() > 20)
                .count()
        };
        assert!(
            changed(&clear, &clouds) >= 3,
            "Volumetric clouds are not visible"
        );
        assert!(
            changed(&clouds, &moved) >= 3,
            "Clouds did not move with time"
        );
        self.material.set_uniform("CloudAmount", 0_f32);
        self.draw(eye, sun, sun, 1., fovy, 0., None);
        set_default_camera();
        let solar = crate::graphics::screen_pixel(0.5, 0.5);
        let edge = crate::graphics::screen_pixel(0.56, 0.56);
        assert!(
            solar[1] as i32 > edge[1] as i32 + 15,
            "The round sun was not aligned with the view ray"
        );
        self.draw(eye, direction, -Vec3::Y, 0., fovy, 0., None);
        set_default_camera();
        let night = sample();
        let energy = |pixels: &[[u8; 4]]| {
            pixels
                .iter()
                .map(|p| p[0] as u32 + p[1] as u32 + p[2] as u32)
                .sum::<u32>()
        };
        assert!(
            energy(&clear) > energy(&night) * 3,
            "The atmosphere ignored the day/night cycle"
        );
        self.material.set_uniform("CloudAmount", 1_f32);
    }
}

const VERTEX: &str = r#"#version 100
attribute vec3 position;attribute vec2 texcoord;
uniform mat4 Model;uniform mat4 Projection;
varying highp vec2 uv;
void main(){uv=texcoord;gl_Position=Projection*Model*vec4(position,1.0);}
"#;

const FRAGMENT: &str = r#"#version 100
precision highp float;
varying highp vec2 uv;
uniform vec3 SkyEye;uniform vec3 ViewDirection;uniform vec3 ViewRight;uniform vec3 ViewUp;
uniform vec3 SunDirection;uniform float Daylight;uniform float SceneTime;
uniform float SkyMode;uniform float Aspect;uniform float TanHalfFov;uniform float CloudAmount;
float hash3(vec3 p){p=fract(p*0.1031);p+=dot(p,p.yzx+33.33);return fract((p.x+p.y)*p.z);}
float noise3(vec3 p){
    vec3 i=floor(p),f=fract(p);f=f*f*(3.0-2.0*f);
    return mix(mix(mix(hash3(i),hash3(i+vec3(1,0,0)),f.x),
                   mix(hash3(i+vec3(0,1,0)),hash3(i+vec3(1,1,0)),f.x),f.y),
               mix(mix(hash3(i+vec3(0,0,1)),hash3(i+vec3(1,0,1)),f.x),
                   mix(hash3(i+vec3(0,1,1)),hash3(i+vec3(1,1,1)),f.x),f.y),f.z);
}
float density(vec3 p){
    float height=(p.y-145.0)/90.0;
    if(height<0.0||height>1.0)return 0.0;
    vec3 q=p*vec3(0.009,0.017,0.009)+vec3(SceneTime*0.013,0.0,SceneTime*0.005);
    float coverage=smoothstep(0.30,0.70,noise3(vec3(q.x*0.36,0.37,q.z*0.36)));
    // Separate broad coverage, rounded volume and edge erosion; no flat noise sheet.
    float mass=noise3(q)*0.58+noise3(q*2.03+17.0)*0.27+noise3(q*4.13+31.0)*0.15;
    float profile=smoothstep(0.0,0.12,height)*(1.0-smoothstep(0.52,1.0,height));
    float threshold=mix(0.65,0.39,coverage);
    return smoothstep(threshold,threshold+0.14,mass)*profile;
}
vec3 atmosphere(vec3 ray,vec3 sun){
    float height=max(0.0,ray.y),horizon=exp(-height*3.6);
    vec3 night=mix(vec3(0.009,0.014,0.038),vec3(0.032,0.044,0.074),horizon);
    vec3 day=mix(vec3(0.12,0.33,0.64),vec3(0.66,0.77,0.91),horizon);
    vec3 sky=mix(night,day,Daylight);
    float toward=pow(max(0.0,dot(normalize(ray.xz+0.0001),normalize(sun.xz+0.0001))),3.0);
    float dusk=(1.0-smoothstep(0.02,0.38,abs(sun.y)))*smoothstep(-0.15,0.015,sun.y);
    vec3 sunset=mix(vec3(0.46,0.35,0.63),vec3(1.0,0.57,0.29),toward);
    sky=mix(sky,sunset,dusk*exp(-height*3.8)*0.86);
    float mu=max(0.0,dot(ray,sun)),solar=smoothstep(-0.06,-0.005,sun.y);
    vec3 sunlight=mix(vec3(1.0,0.48,0.20),vec3(1.0,0.94,0.79),smoothstep(0.03,0.45,sun.y));
    sky+=sunlight*(pow(mu,48.0)*0.16+pow(mu,220.0)*0.38)*solar;
    sky+=sunlight*smoothstep(cos(0.026),cos(0.022),mu)*4.5*solar;
    float lunar=max(0.0,dot(ray,-sun));
    sky+=vec3(0.52,0.64,0.87)*smoothstep(cos(0.018),cos(0.015),lunar)*(1.0-Daylight);
    vec3 starCell=floor(ray*420.0);
    float stars=smoothstep(0.9993,1.0,hash3(starCell))*(1.0-Daylight)*smoothstep(0.05,0.2,ray.y);
    return sky+vec3(stars*0.7);
}
vec3 clouds(vec3 ray,vec3 sun,vec3 sky){
    if(ray.y<=0.025||CloudAmount<0.5)return sky;
    float start=max(0.0,(145.0-SkyEye.y)/ray.y);
    float end=min(2600.0,(235.0-SkyEye.y)/ray.y);
    if(end<=start)return sky;
    float stepSize=(end-start)/20.0;
    // Stable subpixel ray offsets remove the horizontal march bands without flicker.
    float jitter=0.15+0.70*hash3(vec3(floor(uv*vec2(1311.0,777.0)),0.31));
    float transmittance=1.0;vec3 scattered=vec3(0.0);
    float dusk=(1.0-smoothstep(0.03,0.40,abs(sun.y)))*smoothstep(-0.15,0.02,sun.y);
    vec3 ambient=mix(vec3(0.030,0.043,0.068),vec3(0.56,0.65,0.79),Daylight);
    ambient=mix(ambient,vec3(0.58,0.37,0.50),dusk*0.70);
    vec3 sunlight=mix(vec3(1.0,0.49,0.27),vec3(1.0,0.94,0.83),smoothstep(0.03,0.4,sun.y));
    float solar=smoothstep(-0.10,0.06,sun.y);
    float forward=0.55+pow(max(0.0,dot(ray,sun)),8.0)*0.65;
    for(int i=0;i<20;i++){
        vec3 p=SkyEye+ray*(start+(float(i)+jitter)*stepSize);
        float d=density(p);
        if(d>0.005){
            float shadow=exp(-(density(p+sun*18.0)*1.2+density(p+sun*45.0)*1.7));
            float absorb=1.0-exp(-d*stepSize*0.046);
            float altitude=clamp((p.y-145.0)/90.0,0.0,1.0);
            vec3 bounce=ambient*mix(0.58,1.12,smoothstep(0.05,0.9,altitude));
            vec3 light=bounce+sunlight*shadow*forward*solar*0.85;
            scattered+=transmittance*absorb*light;
            transmittance*=1.0-absorb;
            if(transmittance<0.02)break;
        }
    }
    return sky*transmittance+scattered;
}
void main(){
    vec3 ray;
    if(SkyMode>0.5){
        float yaw=(uv.x-0.5)*6.2831853,pitch=(0.5-uv.y)*3.1415927;
        ray=vec3(sin(yaw)*cos(pitch),sin(pitch),cos(yaw)*cos(pitch));
    }else{
        vec2 screen=vec2(uv.x*2.0-1.0,1.0-uv.y*2.0);
        ray=normalize(ViewDirection+ViewRight*screen.x*TanHalfFov*Aspect+ViewUp*screen.y*TanHalfFov);
    }
    vec3 sun=normalize(SunDirection);
    gl_FragColor=vec4(clouds(ray,sun,atmosphere(ray,sun)),1.0);
}
"#;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn view_rays_follow_camera_yaw_pitch_and_zoom() {
        for direction in [Vec3::Z, Vec3::X, vec3(-0.4, 0.7, -1.).normalize()] {
            let (right, up) = view_basis(direction);
            assert!(right.length() > 0.99 && up.length() > 0.99);
            assert!(right.dot(up).abs() < 0.0001);
            assert!(direction.dot(right).abs() < 0.0001);
            assert!(direction.dot(up).abs() < 0.0001);
            let wide = (direction + right * 36_f32.to_radians().tan()).normalize();
            let zoom = (direction + right * 9_f32.to_radians().tan()).normalize();
            assert!(zoom.dot(direction) > wide.dot(direction));
        }
    }
}
