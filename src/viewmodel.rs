//! Embedded OBJ models, articulated hands and first-person animation.
use crate::{combat::Combat, game::Item};
use macroquad::prelude::*;
use std::collections::HashMap;

pub struct WeaponModel {
    pub body: Mesh,
    pub magazine: Mesh,
    pub barrel: Mesh,
    pub bolt: Mesh,
    pub scope: Mesh,
}

fn empty_mesh() -> Mesh {
    Mesh {
        vertices: Vec::new(),
        indices: Vec::new(),
        texture: None,
    }
}

/// Retain the downloaded model's UV seams and smooth normals, sharing indexed vertices.
pub fn parse_obj(obj: &str, mtl: &str) -> Result<WeaponModel, String> {
    let mut materials = HashMap::new();
    let mut material = "";
    for line in mtl.lines() {
        let fields: Vec<_> = line.split_whitespace().collect();
        match fields.as_slice() {
            ["newmtl", name] => material = name,
            ["Kd", r, g, b] => {
                let channel = |s: &str| -> Result<f32, String> {
                    let v: f32 = s.parse().map_err(|_| "Invalid MTL colour")?;
                    if !v.is_finite() {
                        return Err("Nonfinite MTL colour".into());
                    }
                    Ok(v.clamp(0., 1.).powf(1. / 2.2))
                };
                materials.insert(
                    material,
                    Color::new(channel(r)?, channel(g)?, channel(b)?, 1.),
                );
            }
            _ => {}
        }
    }
    let mut positions = Vec::new();
    let mut uvs = Vec::new();
    let mut normals = Vec::new();
    let mut indexed: [HashMap<([u32; 8], u32), u16>; 5] = Default::default();
    let mut model = WeaponModel {
        body: empty_mesh(),
        magazine: empty_mesh(),
        barrel: empty_mesh(),
        bolt: empty_mesh(),
        scope: empty_mesh(),
    };
    let mut group = "Body";
    let mut color = GRAY;
    for line in obj.lines() {
        let fields: Vec<_> = line.split_whitespace().collect();
        match fields.as_slice() {
            ["v", x, y, z, ..] => {
                let v = vec3(
                    x.parse().map_err(|_| "Invalid OBJ vertex")?,
                    y.parse().map_err(|_| "Invalid OBJ vertex")?,
                    z.parse().map_err(|_| "Invalid OBJ vertex")?,
                );
                if !v.is_finite() {
                    return Err("Nonfinite OBJ vertex".into());
                }
                positions.push(v);
            }
            ["g" | "o", name, ..] => group = name,
            ["vt", u, v, ..] => {
                let uv = vec2(
                    u.parse().map_err(|_| "Invalid OBJ UV")?,
                    v.parse().map_err(|_| "Invalid OBJ UV")?,
                );
                if !uv.is_finite() {
                    return Err("Nonfinite OBJ UV".into());
                }
                uvs.push(vec2(uv.x, 1. - uv.y));
            }
            ["vn", x, y, z, ..] => {
                let n = vec3(
                    x.parse().map_err(|_| "Invalid OBJ normal")?,
                    y.parse().map_err(|_| "Invalid OBJ normal")?,
                    z.parse().map_err(|_| "Invalid OBJ normal")?,
                );
                if !n.is_finite() {
                    return Err("Nonfinite OBJ normal".into());
                }
                normals.push(n.normalize_or_zero());
            }
            ["usemtl", name] => color = *materials.get(name).ok_or("Unknown OBJ material")?,
            ["f", face @ ..] => {
                if face.len() < 3 {
                    return Err("Invalid OBJ face".into());
                }
                let obj_index = |s: &str, len: usize| -> Result<usize, String> {
                    let n: isize = s.parse().map_err(|_| "Invalid OBJ index")?;
                    let i = if n < 0 { len as isize + n } else { n - 1 };
                    if i < 0 || i as usize >= len {
                        return Err("OBJ index out of range".into());
                    }
                    Ok(i as usize)
                };
                let vertices: Result<Vec<_>, String> = face
                    .iter()
                    .map(|v| {
                        let mut fields = v.split('/');
                        let p = positions[obj_index(fields.next().unwrap(), positions.len())?];
                        let uv = match fields.next() {
                            Some(s) if !s.is_empty() => uvs[obj_index(s, uvs.len())?],
                            _ => Vec2::ZERO,
                        };
                        let normal = match fields.next() {
                            Some(s) if !s.is_empty() => Some(normals[obj_index(s, normals.len())?]),
                            _ => None,
                        };
                        Ok((p, uv, normal))
                    })
                    .collect();
                let vertices = vertices?;
                let part = match group {
                    "Magazine" => 1,
                    "Barrel" => 2,
                    "Bolt" => 3,
                    "Scope" => 4,
                    _ => 0,
                };
                let mesh = match part {
                    1 => &mut model.magazine,
                    2 => &mut model.barrel,
                    3 => &mut model.bolt,
                    4 => &mut model.scope,
                    _ => &mut model.body,
                };
                for i in 1..vertices.len() - 1 {
                    if vertices[0].2.is_none() {
                        if mesh.vertices.len() + 3 > u16::MAX as usize {
                            return Err("OBJ too large".into());
                        }
                        triangle(
                            mesh,
                            [vertices[0].0, vertices[i].0, vertices[i + 1].0],
                            color,
                            0.,
                        );
                        continue;
                    }
                    for corner in [0, i, i + 1] {
                        let mut v = Vertex::new2(vertices[corner].0, vertices[corner].1, color);
                        v.normal = vertices[corner].2.unwrap_or(Vec3::Y).extend(0.);
                        let key = (
                            [
                                v.position.x,
                                v.position.y,
                                v.position.z,
                                v.uv.x,
                                v.uv.y,
                                v.normal.x,
                                v.normal.y,
                                v.normal.z,
                            ]
                            .map(f32::to_bits),
                            u32::from_ne_bytes(v.color),
                        );
                        let index = if let Some(&index) = indexed[part].get(&key) {
                            index
                        } else {
                            if mesh.vertices.len() >= u16::MAX as usize {
                                return Err("OBJ too large".into());
                            }
                            let index = mesh.vertices.len() as u16;
                            mesh.vertices.push(v);
                            indexed[part].insert(key, index);
                            index
                        };
                        mesh.indices.push(index);
                    }
                }
            }
            _ => {}
        }
    }
    if model.body.indices.is_empty() {
        return Err("Empty OBJ model".into());
    }
    Ok(model)
}

fn triangle(mesh: &mut Mesh, points: [Vec3; 3], color: Color, emission: f32) {
    let normal = (points[1] - points[0])
        .cross(points[2] - points[0])
        .normalize_or_zero();
    let offset = mesh.vertices.len() as u16;
    for p in points {
        let mut vertex = Vertex::new2(p, Vec2::ZERO, color);
        vertex.normal = normal.extend(emission);
        mesh.vertices.push(vertex);
    }
    mesh.indices.extend([offset, offset + 1, offset + 2]);
}

fn transformed(source: &Mesh, matrix: Mat4) -> Mesh {
    let mut mesh = Mesh {
        vertices: source.vertices.clone(),
        indices: source.indices.clone(),
        texture: source.texture.clone(),
    };
    for v in &mut mesh.vertices {
        v.position = matrix.transform_point3(v.position);
        v.normal = matrix
            .transform_vector3(v.normal.truncate())
            .normalize_or_zero()
            .extend(v.normal.w);
    }
    mesh
}

pub struct Pose {
    pub weapon: Mat4,
    pub magazine: Mat4,
    pub barrel: Mat4,
    pub right_hand: Vec3,
    pub left_hand: Vec3,
    pub bolt: f32,
}

fn smooth(t: f32) -> f32 {
    let t = t.clamp(0., 1.);
    t * t * (3. - 2. * t)
}

/// Sight centres and barrel exits measured on the imported OBJ, in metres.
pub fn sight_points(item: Item) -> (Vec3, Vec3, Vec3) {
    let (x, rear_y, rear_z, front_y, front_z, bore_y, muzzle_z) = match item {
        Item::Ak => (-0.00679, 0.138, -0.2044, 0.124, -0.440, 0.077, -0.5292),
        Item::Ak74 => (0.02251, 0.0857, -0.2865, 0.13454, -0.5915, 0.0947, -0.6768),
        Item::Akm => (-0.01046, 0.139, -0.214, 0.1355, -0.607, 0.089, -0.6336),
        Item::M4A1 => (-0.00028, 0.13427, -0.0611, 0.1343, -0.431, 0.079, -0.6048),
        Item::Famas => (0., 0.140, -0.097, 0.134, -0.386, 0.036, -0.5472),
        Item::M16A4 => (-0.000787, 0.1330, -0.03967, 0.1317, -0.530, 0.0713, -0.720),
        Item::RocketLauncher => (0.001736, 0.005, -0.333, -0.002, -0.513, -0.0523, -0.5256),
        Item::M200 => (0., 0.100, 0.045, 0.100, -0.30, 0.027, -1.008),
        Item::Tundra => (-0.012, 0.120, 0.04, 0.120, -0.25, 0.066, -0.8136),
        Item::Aw50 => (-0.0202, 0.102, 0.075, 0.102, -0.30, 0.0273, -0.972),
        Item::Svd => (0., 0.173, 0.05, 0.173, -0.30, 0.092, -0.8784),
        Item::Mp5k => (0.00463, 0.1355, 0.041, 0.1295, -0.226, 0.1043, -0.234),
        _ => (0., 0., 0., 0., -1., 0., -1.),
    };
    (
        vec3(x, rear_y, rear_z),
        vec3(
            if item == Item::Ak74 { 0.03155 } else { x },
            front_y,
            front_z,
        ),
        vec3(
            if item == Item::Ak74 { 0.02498 } else { x },
            bore_y,
            muzzle_z,
        ),
    )
}

pub fn bolt_contact(item: Item) -> Vec3 {
    match item {
        Item::M200 => vec3(0.078, -0.002, 0.056),
        Item::Tundra => vec3(0.040, 0.039, -0.010),
        Item::Aw50 => vec3(0.057, 0.006, 0.002),
        _ => vec3(0.065, 0.03, -0.05),
    }
}

/// Convert the separately projected viewmodel muzzle into the world camera.
pub fn world_muzzle(item: Item, combat: &Combat, time: f32, eye: Vec3, direction: Vec3) -> Vec3 {
    let local = pose(item, combat, time)
        .weapon
        .transform_point3(sight_points(item).2);
    let right = direction.cross(Vec3::Y).normalize_or_zero();
    let up = right.cross(direction).normalize_or_zero();
    let scale = (crate::weapons::field_of_view(combat.aim, combat.zoom) * 0.5).tan()
        / 29_f32.to_radians().tan();
    eye + right * local.x * scale + up * local.y * scale - direction * local.z
}

pub fn pose(item: Item, combat: &Combat, time: f32) -> Pose {
    let launcher = item == Item::RocketLauncher;
    let aim = combat.aim;
    let scoped = crate::weapons::spec(item).is_some_and(|s| s.scoped);
    let bob = vec3(
        (combat.walk * 0.5).sin() * 0.008,
        combat.walk.sin().abs() * 0.012,
        0.,
    );
    let hip = if launcher {
        vec3(0.30, -0.10, -0.86)
    } else {
        vec3(0.28, -0.14 - if scoped { 0.045 } else { 0. }, -0.80)
    };
    let (rear, front, _) = sight_points(item);
    let up = if item == Item::Ak74 {
        vec3(0.196, 0.9806, 0.)
    } else {
        Vec3::Y
    };
    let alignment = Mat4::look_at_rh(rear, front, up);
    let relief = match item {
        Item::Ak | Item::M4A1 | Item::M16A4 => 0.16,
        Item::RocketLauncher => 0.30,
        _ => 0.20,
    };
    let sight = vec3(0., 0., -relief) - alignment.transform_vector3(rear);
    let mut position = hip.lerp(sight, aim) + bob * (1. - aim);
    position.y += (time * 1.8).sin() * 0.002 * (1. - aim);
    position += vec3(
        -combat.sway.x * 0.15 * (1. - aim),
        combat.sway.y * 0.10 * (1. - aim),
        combat.recoil * 1.7,
    );
    position.y -= combat.sprint * 0.19 + smooth(combat.equip_time / 0.34) * 0.65;
    let mut rotation = vec3(
        combat.recoil * 1.8 * (1. - aim) - combat.sprint * 0.35,
        0.,
        -combat.sprint * 0.34,
    );
    if combat.inspect_time > 0. {
        let phase = (2.6 - combat.inspect_time) / 2.6;
        let wave = (phase * std::f32::consts::PI).sin();
        rotation += vec3(-0.12, 0.65, 0.55) * wave;
        position += vec3(-0.08, 0.07, -0.03) * wave;
    }
    let reload = combat.reload_progress().unwrap_or(0.);
    if combat.reload.is_some() {
        let tilt = smooth(reload / 0.12) * (1. - smooth((reload - 0.83) / 0.17));
        rotation += vec3(0.15, 0.15, -0.35) * tilt;
        position += vec3(-0.035, 0.025, 0.035) * tilt;
    }
    let matrix = Mat4::from_translation(position)
        * Mat4::from_rotation_z(rotation.z)
        * Mat4::from_rotation_y(rotation.y)
        * Mat4::from_rotation_x(rotation.x)
        * Mat4::from_mat3(Mat3::from_mat4(alignment));
    // Contact points measured on each imported model, in metres.
    let (grip, foregrip, magazine) = match item {
        Item::Ak => ((-0.015, -0.073), (0.038, -0.36), (-0.060, -0.27)),
        Item::Ak74 => ((-0.083, -0.060), (0.030, -0.46), (-0.08, -0.25)),
        Item::Akm => ((-0.01, -0.03), (0.055, -0.35), (-0.012, -0.235)),
        Item::M4A1 => ((-0.025, -0.01), (0.038, -0.31), (-0.045, -0.15)),
        Item::Famas => ((-0.060, -0.13), (0.005, -0.31), (-0.06, 0.005)),
        Item::M16A4 => ((-0.026, -0.043), (0.040, -0.39), (-0.03, -0.175)),
        Item::RocketLauncher => ((-0.082, -0.065), (-0.060, -0.31), (0., 0.)),
        Item::M200 => ((-0.083, 0.11), (-0.031, -0.31), (-0.06, -0.115)),
        Item::Tundra => ((-0.033, 0.052), (0.005, -0.30), (0., -0.10)),
        Item::Aw50 => ((-0.060, 0.08), (-0.006, -0.31), (-0.066, -0.245)),
        Item::Svd => ((0.006, 0.057), (0.075, -0.43), (0.025, -0.135)),
        Item::Mp5k => ((-0.025, 0.035), (0.025, -0.184), (-0.065, -0.125)),
        _ => ((0., 0.), (0., 0.), (0., 0.)),
    };
    let mut right = vec3(0.021, grip.0 - 0.025, grip.1);
    let support = vec3(-0.022, foregrip.0 - 0.025, foregrip.1);
    let magazine_point = vec3(-0.022, magazine.0, magazine.1);
    let mut mag_offset = Vec3::ZERO;
    let mut left = support;
    if combat.reload.is_some() {
        if launcher {
            let reach = smooth(reload / 0.2);
            let insertion = smooth((reload - 0.35) / 0.38);
            left = support
                .lerp(vec3(-0.038, -0.073, -0.065 - insertion * 0.08), reach)
                .lerp(support, smooth((reload - 0.8) / 0.2));
        } else {
            let down = smooth((reload - 0.12) / 0.23);
            let up = smooth((reload - 0.48) / 0.25);
            mag_offset = vec3(-0.05, -0.40, 0.04) * (down - up);
            left = support.lerp(magazine_point + mag_offset, smooth(reload / 0.13));
            left = left.lerp(support, smooth((reload - 0.88) / 0.12));
            if (0.76..0.9).contains(&reload) {
                left = left.lerp(
                    vec3(0.075, -0.015, -0.17),
                    ((reload - 0.76) / 0.14 * std::f32::consts::PI).sin(),
                );
            }
        }
    }
    if combat.bolt_time > 0. && crate::weapons::spec(item).is_some_and(|s| s.bolt) {
        let progress = 1. - combat.bolt_time / crate::weapons::spec(item).unwrap().interval;
        let reach = smooth(progress / 0.18) * (1. - smooth((progress - 0.76) / 0.24));
        right = right.lerp(
            bolt_contact(item) + Vec3::Z * (progress * std::f32::consts::PI).sin() * 0.10,
            reach,
        );
    }
    Pose {
        weapon: matrix,
        magazine: matrix
            * Mat4::from_translation(mag_offset)
            * Mat4::from_rotation_x(
                (mag_offset.y
                    * if matches!(item, Item::Ak | Item::Ak74 | Item::Akm | Item::Svd) {
                        0.7
                    } else {
                        0.08
                    })
                .abs(),
            ),
        barrel: matrix
            * Mat4::from_translation(vec3(0., 0., -0.10))
            * Mat4::from_rotation_x(if launcher {
                -0.55 * smooth(reload / 0.18) * (1. - smooth((reload - 0.78) / 0.22))
            } else {
                0.
            })
            * Mat4::from_translation(vec3(0., 0., 0.10)),
        right_hand: matrix.transform_point3(right),
        left_hand: matrix.transform_point3(left),
        bolt: if combat.bolt_time > 0. {
            ((1. - combat.bolt_time / crate::weapons::spec(item).map_or(1., |s| s.interval))
                * std::f32::consts::PI)
                .sin()
                .max(0.)
                * 0.10
        } else {
            0.
        } + combat.recoil * 0.9
            + if combat.reload.is_some() && (0.77..0.9).contains(&reload) {
                ((reload - 0.77) / 0.12 * std::f32::consts::PI)
                    .sin()
                    .max(0.)
                    * 0.045
            } else {
                0.
            },
    }
}

fn embedded_models() -> Vec<(Item, WeaponModel)> {
    vec![
        (
            Item::Ak,
            parse_obj(
                include_str!("../assets/weapons/real/ak74u.obj"),
                include_str!("../assets/weapons/real/ak74u.mtl"),
            )
            .expect("Embedded Ak"),
        ),
        (
            Item::Ak74,
            parse_obj(
                include_str!("../assets/weapons/real/ak74.obj"),
                include_str!("../assets/weapons/real/ak74.mtl"),
            )
            .expect("Embedded Ak74"),
        ),
        (
            Item::Akm,
            parse_obj(
                include_str!("../assets/weapons/real/akm.obj"),
                include_str!("../assets/weapons/real/akm.mtl"),
            )
            .expect("Embedded Akm"),
        ),
        (
            Item::M4A1,
            parse_obj(
                include_str!("../assets/weapons/real/m4a1.obj"),
                include_str!("../assets/weapons/real/m4a1.mtl"),
            )
            .expect("Embedded M4A1"),
        ),
        (
            Item::Famas,
            parse_obj(
                include_str!("../assets/weapons/real/famas.obj"),
                include_str!("../assets/weapons/real/famas.mtl"),
            )
            .expect("Embedded Famas"),
        ),
        (
            Item::M16A4,
            parse_obj(
                include_str!("../assets/weapons/real/m16a4.obj"),
                include_str!("../assets/weapons/real/m16a4.mtl"),
            )
            .expect("Embedded M16A4"),
        ),
        (
            Item::RocketLauncher,
            parse_obj(
                include_str!("../assets/weapons/real/m79.obj"),
                include_str!("../assets/weapons/real/m79.mtl"),
            )
            .expect("Embedded RocketLauncher"),
        ),
        (
            Item::M200,
            parse_obj(
                include_str!("../assets/weapons/real/m200.obj"),
                include_str!("../assets/weapons/real/m200.mtl"),
            )
            .expect("Embedded M200"),
        ),
        (
            Item::Tundra,
            parse_obj(
                include_str!("../assets/weapons/real/tundra.obj"),
                include_str!("../assets/weapons/real/tundra.mtl"),
            )
            .expect("Embedded Tundra"),
        ),
        (
            Item::Aw50,
            parse_obj(
                include_str!("../assets/weapons/real/aw50.obj"),
                include_str!("../assets/weapons/real/aw50.mtl"),
            )
            .expect("Embedded Aw50"),
        ),
        (
            Item::Svd,
            parse_obj(
                include_str!("../assets/weapons/real/svd.obj"),
                include_str!("../assets/weapons/real/svd.mtl"),
            )
            .expect("Embedded Svd"),
        ),
        (
            Item::Mp5k,
            parse_obj(
                include_str!("../assets/weapons/real/mp5k.obj"),
                include_str!("../assets/weapons/real/mp5k.mtl"),
            )
            .expect("Embedded Mp5k"),
        ),
    ]
}

pub struct ViewModel {
    models: Vec<(Item, WeaponModel)>,
    material: Material,
}

impl ViewModel {
    pub fn new() -> Self {
        let mut models = embedded_models();
        let textures: [Texture2D; 12] = [
            Texture2D::from_file_with_format(
                include_bytes!("../assets/weapons/real/ak74u.png"),
                Some(ImageFormat::Png),
            ),
            Texture2D::from_file_with_format(
                include_bytes!("../assets/weapons/real/ak74.png"),
                Some(ImageFormat::Png),
            ),
            Texture2D::from_file_with_format(
                include_bytes!("../assets/weapons/real/akm.png"),
                Some(ImageFormat::Png),
            ),
            Texture2D::from_file_with_format(
                include_bytes!("../assets/weapons/real/m4a1.png"),
                Some(ImageFormat::Png),
            ),
            Texture2D::from_file_with_format(
                include_bytes!("../assets/weapons/real/famas.png"),
                Some(ImageFormat::Png),
            ),
            Texture2D::from_file_with_format(
                include_bytes!("../assets/weapons/real/m16a4.png"),
                Some(ImageFormat::Png),
            ),
            Texture2D::from_file_with_format(
                include_bytes!("../assets/weapons/real/m79.png"),
                Some(ImageFormat::Png),
            ),
            Texture2D::from_file_with_format(
                include_bytes!("../assets/weapons/real/m200.png"),
                Some(ImageFormat::Png),
            ),
            Texture2D::from_file_with_format(
                include_bytes!("../assets/weapons/real/tundra.png"),
                Some(ImageFormat::Png),
            ),
            Texture2D::from_file_with_format(
                include_bytes!("../assets/weapons/real/aw50.png"),
                Some(ImageFormat::Png),
            ),
            Texture2D::from_file_with_format(
                include_bytes!("../assets/weapons/real/svd.png"),
                Some(ImageFormat::Png),
            ),
            Texture2D::from_file_with_format(
                include_bytes!("../assets/weapons/real/mp5k.png"),
                Some(ImageFormat::Png),
            ),
        ];
        for ((_, model), texture) in models.iter_mut().zip(textures) {
            texture.set_filter(FilterMode::Linear);
            for mesh in [
                &mut model.body,
                &mut model.magazine,
                &mut model.barrel,
                &mut model.bolt,
                &mut model.scope,
            ] {
                mesh.texture = Some(texture.clone());
            }
        }
        Self {
            models,
            material: load_material(
                ShaderSource::Glsl {
                    vertex: VERTEX,
                    fragment: FRAGMENT,
                },
                MaterialParams {
                    uniforms: vec![UniformDesc::new("Daylight", UniformType::Float1)],
                    pipeline_params: PipelineParams {
                        depth_test: Comparison::LessOrEqual,
                        depth_write: true,
                        cull_face: miniquad::CullFace::Nothing,
                        ..Default::default()
                    },
                    ..Default::default()
                },
            )
            .expect("Weapon lighting shader"),
        }
    }

    fn draw_scope(&self, matrix: Mat4, gun: Item, optic: u8) {
        let item = [Item::Tundra, Item::M200, Item::Aw50][optic.min(2) as usize];
        let model = &self.models.iter().find(|(i, _)| *i == item).unwrap().1;
        let mesh = &model.scope;
        let bounds = mesh.vertices.iter().fold(
            (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)),
            |(lo, hi), v| (lo.min(v.position), hi.max(v.position)),
        );
        let center = (bounds.0 + bounds.1) * 0.5;
        let scale = [0.70, 0.85, 1.0][optic.min(2) as usize];
        let receiver = match gun {
            Item::M200 => 0.043,
            Item::Tundra => 0.065,
            Item::Aw50 => 0.045,
            Item::Svd => 0.121,
            _ => 0.134,
        };
        let mount = Mat4::from_translation(vec3(
            0.,
            receiver + (bounds.1.y - bounds.0.y) * scale * 0.5,
            -0.10,
        )) * Mat4::from_scale(Vec3::splat(scale))
            * Mat4::from_translation(-center);
        draw_mesh(&transformed(mesh, matrix * mount));
    }

    pub fn draw_loose(&self, stack: crate::game::Stack, position: Vec3, phase: f32, daylight: f32) {
        let Some((_, model)) = self.models.iter().find(|(i, _)| *i == stack.item) else {
            return;
        };
        self.material.set_uniform("Daylight", daylight);
        gl_use_material(&self.material);
        let matrix = Mat4::from_translation(position)
            * Mat4::from_rotation_y(phase)
            * Mat4::from_scale(Vec3::splat(0.40));
        for mesh in [&model.body, &model.magazine, &model.barrel, &model.bolt] {
            draw_mesh(&transformed(mesh, matrix));
        }
        if crate::weapons::spec(stack.item).is_some_and(|s| s.scoped) {
            self.draw_scope(matrix, stack.item, stack.optic);
        }
    }

    pub fn draw(&self, item: Item, optic: u8, combat: &Combat, time: f32, daylight: f32) {
        clear_hand_depth();
        set_camera(&Camera3D {
            position: Vec3::ZERO,
            target: -Vec3::Z,
            up: Vec3::Y,
            fovy: 58_f32.to_radians(),
            z_near: 0.015,
            z_far: 8.,
            ..Default::default()
        });
        self.material.set_uniform("Daylight", daylight);
        gl_use_material(&self.material);
        let pose = pose(item, combat, time);
        let model = &self.models.iter().find(|(i, _)| *i == item).unwrap().1;
        draw_mesh(&transformed(&model.body, pose.weapon));
        draw_mesh(&transformed(&model.magazine, pose.magazine));
        draw_mesh(&transformed(&model.barrel, pose.barrel));
        draw_mesh(&transformed(
            &model.bolt,
            pose.weapon * Mat4::from_translation(vec3(0., 0., pose.bolt)),
        ));
        if crate::weapons::spec(item).is_some_and(|s| s.scoped) {
            self.draw_scope(pose.weapon, item, optic);
        }
        let mut arms = empty_mesh();
        arm(
            &mut arms,
            vec3(0.52, -0.58, -0.30),
            pose.right_hand,
            true,
            pose.weapon,
        );
        arm(
            &mut arms,
            vec3(-0.36, -0.58, -0.30),
            pose.left_hand,
            false,
            pose.weapon,
        );
        if item == Item::RocketLauncher {
            if let Some(p) = combat.reload_progress() {
                if (0.12..0.75).contains(&p) {
                    box_between(
                        &mut arms,
                        pose.left_hand + vec3(0., 0.02, -0.075),
                        pose.left_hand + vec3(0., 0.02, 0.025),
                        0.025,
                        0.025,
                        Color::new(0.72, 0.55, 0.20, 1.),
                    );
                }
            }
        }
        if combat.muzzle > 0. {
            let muzzle = pose.weapon.transform_point3(sight_points(item).2);
            let flash = combat.muzzle / 0.1;
            for i in 0..6 {
                let a = i as f32 * std::f32::consts::TAU / 6.;
                let r = 0.025 + flash * 0.085;
                triangle(
                    &mut arms,
                    [
                        muzzle,
                        muzzle
                            + pose
                                .weapon
                                .transform_vector3(vec3(a.cos() * r, a.sin() * r, -0.1)),
                        muzzle
                            + pose.weapon.transform_vector3(vec3(
                                (a + 0.3).cos() * r * 0.5,
                                (a + 0.3).sin() * r * 0.5,
                                -0.04,
                            )),
                    ],
                    Color::new(1., 0.78, 0.18, 1.),
                    1.,
                );
            }
        }
        draw_mesh(&arms);
        gl_use_default_material();
        set_default_camera();
    }
}

pub fn clear_hand_depth() {
    // Hands need separate depth from the scene but still occlude held objects.
    set_default_camera();
    unsafe {
        let mut gl = get_internal_gl();
        gl.flush();
        gl.quad_context
            .begin_default_pass(miniquad::PassAction::Clear {
                color: None,
                depth: Some(1.),
                stencil: None,
            });
        gl.quad_context.end_render_pass();
    }
}

/// Equal length two-bone chain with a stable outward elbow pole.
fn elbow(shoulder: Vec3, wrist: Vec3, right: bool) -> Vec3 {
    let delta = wrist - shoulder;
    let distance = delta.length().max(0.001);
    let pole = vec3(if right { 1. } else { -1. }, -0.4, 0.3);
    let side = (pole - delta * pole.dot(delta) / (distance * distance)).normalize_or_zero();
    (shoulder + wrist) * 0.5
        + side
            * (0.55_f32.powi(2) - (distance.min(1.09) * 0.5).powi(2))
                .max(0.)
                .sqrt()
}

fn arm(mesh: &mut Mesh, shoulder: Vec3, wrist: Vec3, right: bool, weapon: Mat4) {
    let joint = elbow(shoulder, wrist, right);
    let sleeve = Color::new(0.22, 0.29, 0.23, 1.);
    let glove = Color::new(0.11, 0.13, 0.12, 1.);
    box_between(mesh, shoulder, joint, 0.079, 0.067, sleeve);
    box_between(mesh, joint, wrist, 0.062, 0.046, sleeve);
    let forward = weapon.transform_vector3(-Vec3::Z).normalize();
    let up = weapon.transform_vector3(Vec3::Y).normalize();
    let side = weapon.transform_vector3(Vec3::X).normalize();
    box_between(
        mesh,
        wrist - forward * 0.037,
        wrist + forward * 0.04,
        0.044,
        0.035,
        glove,
    );
    for i in 0..4 {
        let knuckle = wrist
            + forward * (i as f32 * 0.018 - 0.016)
            + side * if right { -0.029 } else { 0.029 };
        let middle = knuckle + up * 0.035 + side * if right { -0.012 } else { 0.012 };
        let tip = middle + side * if right { 0.03 } else { -0.03 };
        box_between(mesh, knuckle, middle, 0.010, 0.009, glove);
        box_between(mesh, middle, tip, 0.009, 0.008, glove);
    }
    box_between(
        mesh,
        wrist - forward * 0.024,
        wrist + up * 0.044 - forward * 0.008,
        0.013,
        0.012,
        glove,
    );
}

fn box_between(mesh: &mut Mesh, a: Vec3, b: Vec3, width: f32, height: f32, color: Color) {
    let axis = (b - a).normalize_or_zero();
    let right = axis
        .cross(if axis.y.abs() > 0.95 {
            Vec3::Z
        } else {
            Vec3::Y
        })
        .normalize_or_zero()
        * width;
    let up = axis.cross(right).normalize_or_zero() * height;
    let vertices = [
        a - right - up,
        a + right - up,
        a + right + up,
        a - right + up,
        b - right - up,
        b + right - up,
        b + right + up,
        b - right + up,
    ];
    for face in [
        [0, 3, 2, 1],
        [4, 5, 6, 7],
        [0, 1, 5, 4],
        [3, 7, 6, 2],
        [0, 4, 7, 3],
        [1, 2, 6, 5],
    ] {
        triangle(
            mesh,
            [vertices[face[0]], vertices[face[1]], vertices[face[2]]],
            color,
            0.,
        );
        triangle(
            mesh,
            [vertices[face[0]], vertices[face[2]], vertices[face[3]]],
            color,
            0.,
        );
    }
}

const VERTEX: &str = r#"#version 100
attribute vec3 position;
attribute vec4 color0;
attribute vec4 normal;
attribute vec2 texcoord;
varying mediump vec2 uv;
uniform mat4 Model;
uniform mat4 Projection;
varying lowp vec4 color;
varying mediump vec4 surface;
varying mediump vec3 point;
void main() { gl_Position = Projection * Model * vec4(position, 1.0); color=color0/255.0; surface=normal; point=position; uv=texcoord; }
"#;
const FRAGMENT: &str = r#"#version 100
precision mediump float;
varying lowp vec4 color;
varying mediump vec4 surface;
varying mediump vec3 point;
uniform float Daylight;
uniform sampler2D Texture;
varying mediump vec2 uv;
void main() {
    vec3 n=normalize(surface.xyz);
    vec3 light=normalize(vec3(-0.5, 0.9, 0.6));
    float diffuse=max(dot(n,light),0.0);
    float spec=pow(max(dot(reflect(-light,n),normalize(-point)),0.0),32.0);
    vec3 tint=mix(vec3(0.55,0.65,0.86),vec3(1.0,0.97,0.9),Daylight);
    vec3 albedo=color.rgb*texture2D(Texture,uv).rgb;
    vec3 rgb=albedo*tint*(0.46+diffuse*0.54)+vec3(spec*0.12);
    gl_FragColor=vec4(mix(rgb,color.rgb,surface.w),color.a);
}
"#;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn obj_uvs_smooth_normals_and_relative_indices_survive_import() {
        let obj="v 0 0 0\nv 1 0 0\nv 0 1 0\nvt .2 .8\nvn 0 0 1\nf -3/1/1 -2/1/1 -1/1/1\nv 0 0 1\nf -3/1/1 -2/1/1 -1/1/1";
        let model = parse_obj(obj, "").unwrap();
        assert_eq!(model.body.vertices.len(), 4);
        assert_eq!(model.body.indices.len(), 6);
        assert_eq!(
            model.body.vertices[model.body.indices[5] as usize].position,
            Vec3::Z
        );
        assert!(model
            .body
            .vertices
            .iter()
            .all(|v| v.normal.truncate() == Vec3::Z && (v.uv.y - 0.2).abs() < 0.001));
        assert!(parse_obj(&obj.replace("vt .2 .8", "vt NaN .8"), "").is_err());
        assert!(parse_obj(&obj.replace("-1/1/1", "-1/2/1"), "").is_err());
    }
    #[test]
    fn all_arsenal_models_are_rigged_and_distinct() {
        let models = embedded_models();
        assert_eq!(models.len(), 12);
        for (item, model) in &models {
            assert!(model.body.indices.len() > 100, "{} body", item.name());
            if *item == Item::RocketLauncher {
                assert!(model.barrel.indices.len() > 100);
            } else {
                assert!(
                    !model.magazine.indices.is_empty(),
                    "{} magazine",
                    item.name()
                );
            }
        }
        let silhouettes: std::collections::HashSet<_> = models
            .iter()
            .map(|(_, m)| {
                format!(
                    "{:?}",
                    m.body
                        .vertices
                        .iter()
                        .map(|v| v.position)
                        .collect::<Vec<_>>()
                )
            })
            .collect();
        assert_eq!(silhouettes.len(), 12);
    }
    #[test]
    fn internet_models_have_real_geometry_and_separate_magazine() {
        let ak = parse_obj(
            include_str!("../assets/weapons/ak.obj"),
            include_str!("../assets/weapons/ak.mtl"),
        )
        .unwrap();
        assert_eq!(
            (ak.body.indices.len() + ak.magazine.indices.len()) / 3,
            1256
        );
        assert!(ak.magazine.indices.len() > 100);
        let launcher = parse_obj(
            include_str!("../assets/weapons/launcher.obj"),
            include_str!("../assets/weapons/launcher.mtl"),
        )
        .unwrap();
        assert_eq!(launcher.body.indices.len() / 3, 1228);
        assert!(ak
            .body
            .vertices
            .iter()
            .all(|v| v.position.is_finite() && v.normal.is_finite()));
        assert!(parse_obj("v NaN 0 0\nf 1 1 1", "").is_err());
        assert!(parse_obj("v 0 0 0\nf 1 2 3", "").is_err());
    }
    #[test]
    fn poses_follow_aim_recoil_and_articulated_hands() {
        let mut combat = Combat::default();
        let hip = pose(Item::Ak, &combat, 0.);
        combat.aim = 1.;
        let aim = pose(Item::Ak, &combat, 0.);
        assert!(
            aim.weapon
                .transform_point3(sight_points(Item::Ak).0)
                .x
                .abs()
                < 0.001
        );
        assert!(aim.right_hand.distance(hip.right_hand) > 0.1);
        combat.recoil = 0.08;
        assert!(
            pose(Item::Ak, &combat, 0.)
                .weapon
                .transform_point3(Vec3::ZERO)
                .z
                > aim.weapon.transform_point3(Vec3::ZERO).z
        );
        for hand in [hip.left_hand, hip.right_hand, aim.left_hand, aim.right_hand] {
            assert!(elbow(vec3(0.65, -0.7, 0.08), hand, true).is_finite());
        }
    }

    #[test]
    fn reloading_detaches_the_magazine_and_the_support_hand_follows_it() {
        use crate::game::{Inventory, Mode, Stack};
        let mut inventory = Inventory::new(Mode::Survival);
        inventory.slots[0] = Some(Stack {
            item: Item::Ak,
            count: 1,
            loaded: 10,
            optic: 0,
        });
        inventory.add(Item::Bullet, 20);
        let mut combat = Combat::default();
        for _ in 0..25 {
            combat.tick_weapon(&mut inventory, Mode::Survival, 1. / 60., Default::default());
        }
        let idle = pose(Item::Ak, &combat, 0.);
        assert!(combat.start_reload(&inventory, Mode::Survival));
        combat.reload.as_mut().unwrap().elapsed = 0.8;
        let reload = pose(Item::Ak, &combat, 0.);
        let offset = reload.weapon.inverse() * reload.magazine;
        assert!(offset.transform_point3(Vec3::ZERO).y < -0.3);
        assert!(reload.left_hand.distance(idle.left_hand) > 0.15);
        assert!(reload.right_hand.distance(idle.right_hand) < 0.2);
        assert!(reload.weapon.is_finite() && reload.magazine.is_finite());
    }

    #[test]
    fn shouldered_sights_stay_on_the_camera_axis_and_stocks_are_behind_the_eye() {
        let mut combat = Combat::default();
        combat.aim = 1.;
        combat.walk = 4.7;
        combat.sway = vec2(0.1, -0.1);
        combat.recoil = 0.025;
        for (gun, model) in embedded_models() {
            let matrix = pose(gun, &combat, 5.).weapon;
            let (rear, front, _) = sight_points(gun);
            for point in [rear, front] {
                let p = matrix.transform_point3(point);
                assert!(
                    p.x.abs() < 0.0001 && p.y.abs() < 0.0001 && p.z < -0.015,
                    "{gun:?}: {p:?}"
                );
            }
            if gun != Item::Mp5k {
                let back = model
                    .body
                    .vertices
                    .iter()
                    .max_by(|a, b| a.position.z.total_cmp(&b.position.z))
                    .unwrap();
                assert!(
                    matrix.transform_point3(back.position).z > 0.,
                    "{gun:?}: stock in front of eye"
                );
            }
        }
    }

    #[test]
    fn sniper_bolts_and_operating_hand_share_the_right_side() {
        for (gun, model) in embedded_models()
            .into_iter()
            .filter(|(gun, _)| crate::weapons::spec(*gun).unwrap().bolt)
        {
            let contact = bolt_contact(gun);
            assert!(contact.x > 0.03 && !model.bolt.vertices.is_empty());
            let nearest = model
                .bolt
                .vertices
                .iter()
                .map(|v| v.position.distance(contact))
                .fold(f32::INFINITY, f32::min);
            assert!(
                nearest < 0.025,
                "{gun:?}: hand misses actual handle by {nearest}"
            );
            let interval = crate::weapons::spec(gun).unwrap().interval;
            let mut combat = Combat::default();
            combat.bolt_time = interval * 0.5;
            let p = pose(gun, &combat, 0.);
            let hand = p.weapon.inverse().transform_point3(p.right_hand);
            assert!(
                hand.distance(contact + Vec3::Z * p.bolt) < 0.001,
                "{gun:?}: hand and bolt diverged"
            );
        }
    }

    #[test]
    fn muzzle_uses_the_same_screen_projection_in_both_cameras() {
        for gun in crate::weapons::GUNS {
            for aim in [0., 0.5, 1.] {
                let mut combat = Combat::default();
                combat.aim = aim;
                combat.zoom = 1.45;
                let local = pose(gun, &combat, 2.)
                    .weapon
                    .transform_point3(sight_points(gun).2);
                let world = world_muzzle(gun, &combat, 2., Vec3::ZERO, -Vec3::Z);
                let a = local.truncate() / -local.z / 29_f32.to_radians().tan();
                let b = world.truncate()
                    / -world.z
                    / (crate::weapons::field_of_view(aim, combat.zoom) * 0.5).tan();
                assert!(a.distance(b) < 0.0001, "{gun:?}");
            }
        }
    }
}
