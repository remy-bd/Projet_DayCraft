//! Optional scene-only color grading, soft bloom and edge smoothing.
use macroquad::prelude::*;

pub struct Graphics {
    target: Option<RenderTarget>,
    material: Material,
}

impl Graphics {
    pub fn new() -> Self {
        let material = load_material(
            ShaderSource::Glsl {
                vertex: VERTEX,
                fragment: FRAGMENT,
            },
            MaterialParams {
                uniforms: vec![UniformDesc::new("Texel", UniformType::Float2)],
                ..Default::default()
            },
        )
        .unwrap_or_else(|error| panic!("Shader graphique : {error}"));
        Self {
            target: None,
            material,
        }
    }

    pub fn target(&mut self) -> RenderTarget {
        let width = screen_width().max(1.) as u32;
        let height = screen_height().max(1.) as u32;
        if self.target.as_ref().is_none_or(|target| {
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
            self.target = Some(target);
        }
        self.target.as_ref().unwrap().clone()
    }

    pub fn present(&self) {
        let target = self.target.as_ref().unwrap();
        set_default_camera();
        self.material.set_uniform(
            "Texel",
            [1. / target.texture.width(), 1. / target.texture.height()],
        );
        gl_use_material(&self.material);
        draw_texture_ex(
            &target.texture,
            0.,
            0.,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(screen_width(), screen_height())),
                flip_y: true,
                ..Default::default()
            },
        );
        gl_use_default_material();
    }

    pub fn assert_effects(&mut self) {
        let target = self.target();
        set_camera(&Camera2D {
            render_target: Some(target.clone()),
            ..Camera2D::from_display_rect(Rect::new(0., 0., screen_width(), screen_height()))
        });
        clear_background(Color::new(0.4, 0.4, 0.4, 1.));
        // A colored patch at the top also detects upside-down presentation.
        draw_rectangle(0., 0., screen_width(), screen_height() * 0.15, RED);
        self.present();
        set_default_camera(); // flush the post-processing draw before reading pixels
        let center = screen_pixel(0.5, 0.5);
        let corner = screen_pixel(0.1, 0.2);
        let top = screen_pixel(0.5, 0.95);
        assert!(
            center[0] > corner[0],
            "La vignette doit assombrir les bords"
        );
        assert!(
            top[0] > top[1] * 2,
            "La scène doit conserver son orientation"
        );
        assert_ne!(
            center[0], 102,
            "Le shader doit appliquer sa courbe de couleur"
        );
    }
}

pub fn screen_pixel(x: f32, y: f32) -> [u8; 4] {
    let mut pixel = [0; 4];
    // SAFETY: four bytes are sufficient for one RGBA pixel, read synchronously.
    unsafe {
        use macroquad::window::miniquad::gl;
        gl::glReadPixels(
            (screen_width() * x) as i32,
            (screen_height() * y) as i32,
            1,
            1,
            gl::GL_RGBA,
            gl::GL_UNSIGNED_BYTE,
            pixel.as_mut_ptr().cast(),
        );
    }
    pixel
}

const VERTEX: &str = r#"#version 100
attribute vec3 position;
attribute vec2 texcoord;
uniform mat4 Model;
uniform mat4 Projection;
varying mediump vec2 uv;
void main() { gl_Position = Projection * Model * vec4(position, 1.0); uv = texcoord; }
"#;

const FRAGMENT: &str = r#"#version 100
precision mediump float;
varying mediump vec2 uv;
uniform sampler2D Texture;
uniform vec2 Texel;
float luminance(vec3 c) { return dot(c, vec3(0.2126, 0.7152, 0.0722)); }
vec3 glow(vec2 offset) {
    vec3 c = texture2D(Texture, clamp(uv + offset * Texel, vec2(0.0), vec2(1.0))).rgb;
    return c * smoothstep(0.82, 1.0, max(c.r, max(c.g, c.b)));
}
void main() {
    vec3 rgb = texture2D(Texture, uv).rgb;
    vec3 neighbors = texture2D(Texture, uv + vec2(Texel.x, 0.0)).rgb
                   + texture2D(Texture, uv - vec2(Texel.x, 0.0)).rgb
                   + texture2D(Texture, uv + vec2(0.0, Texel.y)).rgb
                   + texture2D(Texture, uv - vec2(0.0, Texel.y)).rgb;
    float contrast = abs(luminance(neighbors * 0.25) - luminance(rgb));
    rgb = mix(rgb, neighbors * 0.25, smoothstep(0.06, 0.25, contrast) * 0.22);
    vec3 bloom = glow(vec2(-5.0, 0.0)) + glow(vec2(5.0, 0.0))
               + glow(vec2(0.0, -5.0)) + glow(vec2(0.0, 5.0))
               + glow(vec2(-3.0, -3.0)) + glow(vec2(3.0, -3.0))
               + glow(vec2(-3.0, 3.0)) + glow(vec2(3.0, 3.0));
    rgb += bloom * 0.022;
    rgb = mix(vec3(luminance(rgb)), rgb, 1.10);
    // Filmic highlight roll-off keeps sunlight and lava bright without white clipping.
    rgb = pow(max(rgb,vec3(0.0)),vec3(2.2)) * 1.25;
    rgb = clamp((rgb*(2.51*rgb+0.03))/(rgb*(2.43*rgb+0.59)+0.14),0.0,1.0);
    rgb = pow(rgb,vec3(1.0/2.2));
    float vignette = 1.0 - smoothstep(0.15, 0.72, length(uv - 0.5)) * 0.16;
    gl_FragColor = vec4(clamp(rgb * vignette, 0.0, 1.0), 1.0);
}
"#;
