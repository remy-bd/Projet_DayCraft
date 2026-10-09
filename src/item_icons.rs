//! Original equipment silhouettes and imported food/workstation item textures.
use crate::game::Item;
use crate::world::Block;
use macroquad::prelude::*;
use std::cell::OnceCell;

const PIXELS: [&[u8]; 8] = [
    include_bytes!("../assets/blocks/mutton.png"),
    include_bytes!("../assets/blocks/cooked_mutton.png"),
    include_bytes!("../assets/blocks/beef.png"),
    include_bytes!("../assets/blocks/cooked_beef.png"),
    include_bytes!("../assets/blocks/porkchop.png"),
    include_bytes!("../assets/blocks/cooked_porkchop.png"),
    include_bytes!("../assets/blocks/crafting_table_front.png"),
    include_bytes!("../assets/blocks/furnace_front.png"),
];
thread_local! {
    static TEXTURES: OnceCell<[Texture2D;8]> = const { OnceCell::new() };
}

fn texture(item: Item) -> Option<Texture2D> {
    let index = match item {
        Item::RawMutton => Some(0),
        Item::CookedMutton => Some(1),
        Item::RawBeef => Some(2),
        Item::CookedBeef => Some(3),
        Item::RawPork => Some(4),
        Item::CookedPork => Some(5),
        Item::Block(Block::Workbench) => Some(6),
        Item::Block(Block::Furnace) => Some(7),
        _ => None,
    };
    index.map(|index| {
        TEXTURES.with(|cache| {
            let textures = cache.get_or_init(|| {
                PIXELS.map(|bytes| {
                    let texture = Texture2D::from_file_with_format(bytes, Some(ImageFormat::Png));
                    texture.set_filter(FilterMode::Nearest);
                    texture
                })
            });
            textures[index].clone()
        })
    })
}

pub fn draw(item: Item, x: f32, y: f32, size: f32) -> bool {
    if let Some(texture) = texture(item) {
        draw_texture_ex(
            &texture,
            x - size * 0.5,
            y - size * 0.5,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(size, size)),
                ..Default::default()
            },
        );
        return true;
    }
    if let Item::PlateCarrier(tier) | Item::BallisticHelmet(tier) = item {
        let palette = [
            Color::from_rgba(115, 120, 86, 255),
            Color::from_rgba(167, 133, 88, 255),
            Color::from_rgba(97, 129, 120, 255),
            Color::from_rgba(72, 88, 108, 255),
            Color::from_rgba(62, 58, 65, 255),
        ];
        let color = palette[usize::from(tier.clamp(1, 5) - 1)];
        let dark = Color::new(color.r * 0.50, color.g * 0.50, color.b * 0.50, 1.);
        if matches!(item, Item::PlateCarrier(_)) {
            draw_rectangle(
                x - size * 0.32,
                y - size * 0.20,
                size * 0.64,
                size * 0.65,
                color,
            );
            for side in [-1., 1.] {
                draw_rectangle(
                    x + side * size * 0.22 - size * 0.06,
                    y - size * 0.43,
                    size * 0.12,
                    size * 0.40,
                    color,
                );
            }
            draw_rectangle(
                x - size * 0.22,
                y - size * 0.07,
                size * 0.44,
                size * 0.32,
                dark,
            );
            for row in 0..3 {
                draw_line(
                    x - size * 0.18,
                    y + row as f32 * size * 0.10,
                    x + size * 0.18,
                    y + row as f32 * size * 0.10,
                    1.5,
                    color,
                );
            }
        } else {
            draw_ellipse(x, y - size * 0.08, size * 0.40, size * 0.31, 0., color);
            draw_rectangle(x - size * 0.42, y, size * 0.84, size * 0.14, dark);
            draw_rectangle(x - size * 0.40, y, size * 0.16, size * 0.27, color);
            draw_rectangle(x + size * 0.24, y, size * 0.16, size * 0.27, color);
            draw_line(
                x - size * 0.25,
                y + size * 0.20,
                x,
                y + size * 0.39,
                2.,
                dark,
            );
            draw_line(
                x,
                y + size * 0.39,
                x + size * 0.25,
                y + size * 0.20,
                2.,
                dark,
            );
        }
        draw_text(
            format!("T{tier}"),
            x + size * 0.18,
            y + size * 0.43,
            (size * 0.28).max(10.),
            YELLOW,
        );
        return true;
    }
    match item {
        Item::HandGrenade => {
            draw_ellipse(
                x,
                y + size * 0.06,
                size * 0.25,
                size * 0.35,
                0.,
                Color::from_rgba(78, 97, 61, 255),
            );
            for dy in [-0.1, 0.1, 0.25] {
                draw_line(
                    x - size * 0.18,
                    y + size * dy,
                    x + size * 0.18,
                    y + size * dy,
                    1.5,
                    BLACK,
                );
            }
            draw_rectangle(
                x - size * 0.09,
                y - size * 0.40,
                size * 0.18,
                size * 0.18,
                GRAY,
            );
            draw_line(
                x - size * 0.05,
                y - size * 0.33,
                x + size * 0.25,
                y + size * 0.18,
                3.,
                LIGHTGRAY,
            );
            draw_circle_lines(
                x - size * 0.12,
                y - size * 0.40,
                size * 0.10,
                1.5,
                LIGHTGRAY,
            );
            true
        }
        Item::C4 => {
            draw_rectangle(
                x - size * 0.40,
                y - size * 0.28,
                size * 0.80,
                size * 0.56,
                Color::from_rgba(193, 177, 133, 255),
            );
            draw_rectangle(
                x - size * 0.14,
                y - size * 0.22,
                size * 0.30,
                size * 0.40,
                DARKGRAY,
            );
            draw_rectangle(
                x - size * 0.10,
                y - size * 0.15,
                size * 0.22,
                size * 0.12,
                Color::from_rgba(76, 151, 99, 255),
            );
            draw_line(
                x + size * 0.15,
                y,
                x + size * 0.32,
                y - size * 0.33,
                2.,
                RED,
            );
            draw_line(
                x + size * 0.32,
                y - size * 0.33,
                x - size * 0.33,
                y - size * 0.35,
                2.,
                RED,
            );
            true
        }
        _ => false,
    }
}

pub fn draw_loose(item: Item, position: Vec3, yaw: f32, daylight: f32) -> bool {
    if !matches!(
        item,
        Item::RawMutton
            | Item::CookedMutton
            | Item::RawBeef
            | Item::CookedBeef
            | Item::RawPork
            | Item::CookedPork
    ) {
        return false;
    }
    let Some(texture) = texture(item) else {
        return false;
    };
    thread_local! {static MATERIAL:OnceCell<Material>=const{OnceCell::new()};}
    MATERIAL.with(|slot| {
        let material=slot.get_or_init(||load_material(ShaderSource::Glsl {
            vertex:r#"#version 100
attribute vec3 position;attribute vec2 texcoord;attribute vec4 color0;uniform mat4 Model;uniform mat4 Projection;varying mediump vec2 uv;varying lowp vec4 color;void main(){uv=texcoord;color=color0/255.;gl_Position=Projection*Model*vec4(position,1.);}"#,
            fragment:r#"#version 100
precision mediump float;uniform sampler2D Texture;varying mediump vec2 uv;varying lowp vec4 color;void main(){vec4 c=texture2D(Texture,uv)*color;if(c.a<0.5)discard;gl_FragColor=c;}"#,
        },MaterialParams{pipeline_params:macroquad::window::miniquad::PipelineParams{depth_test:macroquad::window::miniquad::Comparison::LessOrEqual,depth_write:true,..Default::default()},..Default::default()}).expect("Dropped food shader"));
        let right=vec3(yaw.cos(),0.,yaw.sin())*0.2;
        let up=Vec3::Y*0.2;
        let light=0.35+0.65*daylight;
        let color=Color::new(light,light,light,1.);
        let vertices=[(position-right-up,vec2(0.,1.)),(position+right-up,vec2(1.,1.)),(position+right+up,vec2(1.,0.)),(position-right+up,vec2(0.,0.))].map(|(p,uv)|Vertex::new2(p,uv,color)).to_vec();
        gl_use_material(material);
        draw_mesh(&Mesh{vertices,indices:vec![0,1,2,0,2,3],texture:Some(texture)});
        gl_use_default_material();
    });
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn food_sprites_are_distinct_nonempty_imports() {
        for (i, bytes) in PIXELS[..6].iter().enumerate() {
            let image = Image::from_file_with_format(bytes, Some(ImageFormat::Png)).unwrap();
            assert_eq!((image.width, image.height), (16, 16));
            assert!(
                image
                    .bytes
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .filter(|p| p[3] > 0)
                    .count()
                    > 40
            );
            for other in &PIXELS[..i] {
                assert_ne!(bytes, other);
            }
        }
    }
}
