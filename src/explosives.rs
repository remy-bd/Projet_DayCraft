//! Throwable grenades and persistent remotely triggered charges.
use crate::{
    combat::Combat,
    drops::WorldExtras,
    game::{raycast, Inventory, Item, Mob, Mode, Player},
    world::{Pos, World, CHUNK},
};
use macroquad::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct Grenade {
    #[serde(with = "grenade_vec3")]
    pub position: Vec3,
    #[serde(with = "grenade_vec3")]
    pub velocity: Vec3,
    pub age: f32,
}
mod grenade_vec3 {
    use super::*;
    pub fn serialize<S: serde::Serializer>(v: &Vec3, s: S) -> Result<S::Ok, S::Error> {
        v.to_array().serialize(s)
    }
    pub fn deserialize<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec3, D::Error> {
        Ok(Vec3::from_array(<[f32; 3]>::deserialize(d)?))
    }
}
impl Grenade {
    pub fn valid(&self) -> bool {
        self.position.is_finite()
            && self.position.abs().max_element() < 1_000_000.
            && self.velocity.is_finite()
            && self.velocity.abs().max_element() <= 100.
            && self.age.is_finite()
            && (0. ..=3.2).contains(&self.age)
    }
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Charge {
    pub position: [f32; 3],
    pub normal: [i32; 3],
}
impl Charge {
    pub fn valid(&self) -> bool {
        self.position
            .iter()
            .all(|v| v.is_finite() && v.abs() < 1_000_000.)
            && (0. ..crate::city::HEIGHT as f32).contains(&self.position[1])
            && self
                .normal
                .iter()
                .map(|v| v.unsigned_abs() as u64)
                .sum::<u64>()
                == 1
    }
}
fn loaded(world: &World, p: Vec3) -> bool {
    world.chunks.contains_key(&[
        (p.x.floor() as i32).div_euclid(CHUNK),
        (p.z.floor() as i32).div_euclid(CHUNK),
    ])
}
impl Combat {
    pub fn deploy(
        &mut self,
        world: &World,
        player: &Player,
        inventory: &mut Inventory,
        extras: &mut WorldExtras,
        mode: Mode,
    ) -> Result<(), &'static str> {
        if self.throw_time > 0. {
            return Err("Attendez la fin du geste");
        }
        let Some(item) = inventory.selected().map(|s| s.item) else {
            return Err("Aucun explosif sélectionné");
        };
        let eye = player.eye();
        let dir = player.direction();
        if !eye.is_finite() || !loaded(world, eye) {
            return Err("Terrain indisponible");
        }
        match item {
            Item::HandGrenade => {
                if self.grenades.len() >= 32 {
                    return Err("Trop de grenades actives");
                }
                let distance =
                    raycast(world, eye, dir, 0.5).map_or(0.5, |h| (h.distance - 0.12).max(0.));
                let position = eye + dir * distance;
                if crate::game::collides(world, position - Vec3::Y * 0.1, 0.1, 0.2) {
                    return Err("Pas assez de place pour lancer");
                }
                self.grenades.push(Grenade {
                    position,
                    velocity: dir * 13. + Vec3::Y * 2.8,
                    age: 0.,
                });
            }
            Item::C4 => {
                if extras.charges.len() >= 64 {
                    return Err("Maximum de 64 charges posées");
                }
                let Some(hit) = raycast(world, eye, dir, 6.) else {
                    return Err("Visez une surface pour poser le C4");
                };
                let normal: Pos = std::array::from_fn(|i| hit.previous[i] - hit.block[i]);
                if normal.iter().map(|v| v.unsigned_abs() as u64).sum::<u64>() != 1 {
                    return Err("Surface inaccessible");
                }
                let position =
                    eye + dir * hit.distance + Vec3::from_array(normal.map(|v| v as f32)) * 0.055;
                let charge = Charge {
                    position: position.to_array(),
                    normal,
                };
                if !charge.valid()
                    || extras
                        .charges
                        .iter()
                        .any(|c| Vec3::from_array(c.position).distance_squared(position) < 0.08)
                {
                    return Err("Surface déjà occupée");
                }
                extras.charges.push(charge);
            }
            _ => return Err("Cet objet n'est pas un explosif"),
        }
        if mode == Mode::Survival {
            inventory.take_selected();
        }
        self.throw_time = 0.6;
        self.throw_item = Some(item);
        Ok(())
    }
    pub fn detonate_charges(
        &mut self,
        world: &mut World,
        mobs: &mut [Mob],
        player: &mut Player,
        extras: &mut WorldExtras,
        mode: Mode,
    ) -> usize {
        let mut count = 0;
        extras.charges.retain(|charge| {
            let p = Vec3::from_array(charge.position);
            if loaded(world, p) && p.distance_squared(player.eye()) < 128_f32.powi(2) {
                self.blast(world, mobs, player, mode, p, 4.5);
                count += 1;
                false
            } else {
                true
            }
        });
        count
    }
    pub fn update_grenades(
        &mut self,
        world: &mut World,
        mobs: &mut [Mob],
        player: &mut Player,
        mode: Mode,
        dt: f32,
    ) -> usize {
        if !dt.is_finite() || dt <= 0. {
            return 0;
        }
        self.throw_time = (self.throw_time - dt).max(0.);
        let mut count = 0;
        for mut g in std::mem::take(&mut self.grenades) {
            if !loaded(world, g.position) {
                self.grenades.push(g);
                continue;
            }
            let mut remaining = dt.min(3.2 - g.age);
            while remaining > 0. {
                let step = remaining.min(1. / 120.);
                let next = g.position + (g.velocity - Vec3::Y * (9.8 * step)) * step;
                if [-0.10, 0.10].into_iter().any(|x| {
                    [-0.10, 0.10]
                        .into_iter()
                        .any(|z| !loaded(world, next + vec3(x, 0., z)))
                }) {
                    break;
                }
                g.velocity.y -= 9.8 * step;
                for axis in 0..3 {
                    let mut p = g.position;
                    p[axis] += g.velocity[axis] * step;
                    let blocked = crate::game::collides(world, p - Vec3::Y * 0.10, 0.10, 0.20);
                    if blocked {
                        g.velocity[axis] *= -0.42;
                        g.velocity *= 0.82;
                    } else {
                        g.position = p;
                    }
                }
                g.age += step;
                remaining -= step;
            }
            if g.age >= 3.2 - 0.0001 {
                self.hand_grenade_blast(world, mobs, player, mode, g.position);
                count += 1;
            } else {
                self.grenades.push(g);
            }
        }
        count
    }
}

fn charge_model(p: Vec3, normal: [i32; 3]) {
    let size = vec3(
        if normal[0] != 0 { 0.09 } else { 0.28 },
        if normal[1] != 0 { 0.09 } else { 0.19 },
        if normal[2] != 0 { 0.09 } else { 0.28 },
    );
    draw_cube(p, size, None, Color::new(0.29, 0.33, 0.18, 1.));
    draw_cube(
        p + Vec3::from_array(normal.map(|v| v as f32)) * 0.06,
        size * 0.42,
        None,
        Color::new(0.08, 0.1, 0.09, 1.),
    );
    draw_sphere(
        p + Vec3::from_array(normal.map(|v| v as f32)) * 0.09,
        0.022,
        None,
        RED,
    );
}

pub fn draw_world(combat: &Combat, charges: &[Charge], eye: Vec3) {
    for charge in charges {
        let p = Vec3::from_array(charge.position);
        if p.distance_squared(eye) < 100_f32.powi(2) {
            charge_model(p, charge.normal);
        }
    }
    for g in &combat.grenades {
        draw_sphere(g.position, 0.105, None, Color::new(0.22, 0.29, 0.16, 1.));
        draw_cube(
            g.position + Vec3::Y * 0.09,
            vec3(0.04, 0.08, 0.04),
            None,
            GRAY,
        );
    }
    for blast in &combat.blasts {
        if blast.position.distance_squared(eye) > 120_f32.powi(2) {
            continue;
        }
        let t = blast.age;
        for i in 0..28 {
            let a = i as f32 * 2.399963;
            let y = 0.25 + (i % 7) as f32 * 0.14;
            let dir = vec3(a.cos(), y, a.sin()).normalize();
            let speed = blast.radius * (1.5 + (i % 5) as f32 * 0.21);
            let p = blast.position + dir * speed * t - Vec3::Y * (4.9 * t * t);
            if t < 1.7 {
                draw_cube(
                    p,
                    Vec3::splat(0.055 + (i % 3) as f32 * 0.015),
                    None,
                    Color::new(0.34, 0.28, 0.20, (1. - t / 1.7).max(0.)),
                );
            }
        }
    }
    smoke(combat, eye);
}

fn smoke(combat: &Combat, eye: Vec3) {
    if combat.blasts.is_empty() {
        return;
    }
    use macroquad::window::miniquad::{
        BlendFactor, BlendState, BlendValue, Comparison, Equation, PipelineParams,
    };
    struct Cloud {
        texture: Texture2D,
        material: Material,
    }
    thread_local! { static CLOUD:std::cell::OnceCell<Cloud>=const{std::cell::OnceCell::new()}; }
    CLOUD.with(|slot| {
        let mut particles=Vec::new();
        for b in &combat.blasts {
            if b.position.distance_squared(eye)>120_f32.powi(2) || b.age >= 3. {continue;}
            for i in 0..18 {
                let a=i as f32*2.399963;
                let dir=vec3(a.cos(),0.3+(i%4) as f32*0.35,a.sin());
                let t=b.age;
                let p=b.position+dir*b.radius*(1.-(-t*1.4).exp())+Vec3::Y*t*1.2;
                let fire=(1.-t/0.4).max(0.);
                let size=b.radius*(0.4+t*0.48);
                let color=Color::new(0.25+fire*0.75,0.24+fire*0.40,0.22+fire*0.06,(1.-t/3.0).max(0.)*0.20+fire*0.20);
                particles.push((p,size,color));
            }
            if b.age<0.16 {particles.push((b.position,b.radius*(0.2+b.age*7.),Color::new(1.,0.92,0.67,(1.-b.age/0.16)*0.9)));}
        }
        // Empty custom draws corrupt the following post-process on this GL backend.
        if particles.is_empty() {return;}
        let cloud=slot.get_or_init(|| {
            let mut image=Image::gen_image_color(64,64,BLANK);
            for y in 0..64 {for x in 0..64 {let r=vec2((x as f32-31.5)/31.5,(y as f32-31.5)/31.5).length(); image.set_pixel(x,y,Color::new(1.,1.,1.,(1.-r).max(0.).powf(1.7)));}}
            let texture=Texture2D::from_image(&image); texture.set_filter(FilterMode::Linear);
            let material=load_material(ShaderSource::Glsl{vertex:r#"#version 100
attribute vec3 position;attribute vec2 texcoord;attribute vec4 color0;uniform mat4 Model;uniform mat4 Projection;varying mediump vec2 uv;varying lowp vec4 color;void main(){uv=texcoord;color=color0/255.;gl_Position=Projection*Model*vec4(position,1.);}"#,fragment:r#"#version 100
precision mediump float;uniform sampler2D Texture;varying mediump vec2 uv;varying lowp vec4 color;void main(){vec4 c=texture2D(Texture,uv)*color;if(c.a<0.003)discard;gl_FragColor=c;}"#},MaterialParams{pipeline_params:PipelineParams{depth_test:Comparison::LessOrEqual,depth_write:true,color_blend:Some(BlendState::new(Equation::Add,BlendFactor::Value(BlendValue::SourceAlpha),BlendFactor::OneMinusValue(BlendValue::SourceAlpha))),..Default::default()},..Default::default()}).expect("Smoke shader");
            Cloud{texture,material}
        });
        particles.sort_by(|a,b| b.0.distance_squared(eye).total_cmp(&a.0.distance_squared(eye)));
        let mut mesh=Mesh{vertices:Vec::new(),indices:Vec::new(),texture:Some(cloud.texture.clone())};
        for (p,size,color) in particles {
            let forward=(eye-p).normalize_or_zero();
            let right=forward.cross(Vec3::Y).normalize_or_zero()*size;
            let up=right.normalize_or_zero().cross(forward)*size;
            let n=mesh.vertices.len() as u16;
            for (v,uv) in [(p-right-up,vec2(0.,0.)),(p+right-up,vec2(1.,0.)),(p+right+up,vec2(1.,1.)),(p-right+up,vec2(0.,1.))] {mesh.vertices.push(Vertex::new2(v,uv,color));}
            mesh.indices.extend([n,n+1,n+2,n,n+2,n+3]);
        }
        gl_use_material(&cloud.material);draw_mesh(&mesh);gl_use_default_material();
    });
}

pub fn assert_render_effects() {
    let mut graphics = crate::graphics::Graphics::new();
    let target = graphics.target();
    let eye = vec3(0., 0., 4.);
    let mut combat = Combat::default();
    let render = |combat: &Combat, wall: bool| {
        set_camera(&Camera3D {
            position: eye,
            target: Vec3::ZERO,
            up: Vec3::Y,
            fovy: 72_f32.to_radians(),
            render_target: Some(target.clone()),
            ..Default::default()
        });
        clear_background(Color::new(0.65, 0.65, 0.65, 1.));
        if wall {
            draw_cube(
                Vec3::ZERO,
                vec3(8., 8., 0.3),
                None,
                Color::new(0.65, 0.65, 0.65, 1.),
            );
        }
        smoke(combat, eye);
        graphics.present();
        set_default_camera();
        let mut samples = Vec::new();
        for y in [0.3, 0.4, 0.5, 0.6, 0.7] {
            for x in [0.3, 0.4, 0.5, 0.6, 0.7] {
                samples.push(crate::graphics::screen_pixel(x, y));
            }
        }
        samples
    };
    let empty = render(&combat, false);
    combat.blasts.push(crate::combat::Blast {
        position: vec3(1000., 0., 0.),
        age: 1.,
        radius: 0.4,
    });
    let distant = render(&combat, false);
    assert_eq!(
        empty, distant,
        "Culled smoke changed the scene presentation"
    );
    assert!(
        empty.iter().all(|p| p[0] > 80),
        "Empty smoke corrupted the full scene"
    );
    combat.blasts[0].position = vec3(0.130688, -1.103415, -1.);
    combat.blasts[0].age = 0.5;
    combat.blasts[0].radius = 1.;
    let visible = render(&combat, false);
    assert!(
        visible
            .iter()
            .zip(&empty)
            .any(|(a, b)| a[0] as i32 + 10 < b[0] as i32),
        "Explosion smoke was not drawn: empty={empty:?}, visible={visible:?}"
    );
    let wall = render(&Combat::default(), true);
    let hidden = render(&combat, true);
    assert_eq!(wall, hidden, "Explosion smoke leaked through solid terrain");
    for item in [Item::HandGrenade, Item::C4] {
        set_default_camera();
        clear_background(Color::new(0.6, 0.3, 0.2, 1.));
        draw_rectangle(
            0.,
            0.,
            screen_width(),
            screen_height(),
            Color::new(0.6, 0.3, 0.2, 1.),
        );
        set_default_camera();
        let backdrop = crate::graphics::screen_pixel(0.7, 0.25);
        draw_held(item, &Combat::default());
        let mut changed = false;
        for y in [0.18, 0.25, 0.32] {
            for x in [0.65, 0.70, 0.75] {
                let pixel = crate::graphics::screen_pixel(x, y);
                changed |= (0..3)
                    .map(|c| pixel[c].abs_diff(backdrop[c]) as u32)
                    .sum::<u32>()
                    > 70;
            }
        }
        assert!(
            changed,
            "The held explosive was hidden by scene depth: {item:?}"
        );
    }
}

pub fn draw_held(item: Item, combat: &Combat) {
    if !matches!(item, Item::HandGrenade | Item::C4) {
        return;
    }
    crate::viewmodel::clear_hand_depth();
    set_camera(&Camera3D {
        position: Vec3::ZERO,
        target: -Vec3::Z,
        up: Vec3::Y,
        fovy: 65_f32.to_radians(),
        z_near: 0.01,
        z_far: 4.,
        ..Default::default()
    });
    let gesture = (combat.throw_time / 0.6 * std::f32::consts::PI).sin();
    let p = vec3(0.24, -0.22 - gesture * 0.22, -0.65 - gesture * 0.2);
    draw_cube(
        p + vec3(0.01, -0.16, 0.09),
        vec3(0.1, 0.28, 0.1),
        None,
        Color::new(0.22, 0.29, 0.23, 1.),
    );
    draw_cube(
        p + vec3(0., -0.035, 0.015),
        vec3(0.13, 0.11, 0.12),
        None,
        Color::new(0.10, 0.12, 0.10, 1.),
    );
    if item == Item::C4 {
        charge_model(p, [0, 0, 1]);
    } else {
        draw_sphere(p, 0.087, None, Color::new(0.26, 0.32, 0.16, 1.));
        draw_cube(
            p + vec3(0.02, 0.095, 0.),
            vec3(0.025, 0.10, 0.03),
            None,
            GRAY,
        );
    }
    set_default_camera();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grenade_test_world() -> World {
        let mut world = World::new(7);
        world.stream([0, 0], 0, 1);
        for x in 0..14 {
            for z in 0..8 {
                for y in 40..47 {
                    world.set(
                        [x, y, z],
                        if y == 40 {
                            crate::world::Block::Stone
                        } else {
                            crate::world::Block::Air
                        },
                    );
                }
            }
        }
        world
    }

    #[test]
    fn fused_hand_grenade_kills_a_nearby_villager_and_fragmentation_has_bounded_falloff() {
        let mut world = grenade_test_world();
        let mut combat = Combat::default();
        combat.grenades.push(Grenade {
            position: vec3(0.5, 41.101, 3.5),
            velocity: Vec3::ZERO,
            age: 3.199,
        });
        let mut mobs = [3.5, 5.5, 7.5].map(|x| Mob {
            position: vec3(x, 41., 3.5),
            health: 20.,
            kind: crate::game::MobKind::Villager,
            ..Default::default()
        });
        let mut player = Player::new([3.5, 41., 3.5]);
        assert_eq!(
            combat.update_grenades(&mut world, &mut mobs, &mut player, Mode::Creative, 0.01,),
            1
        );
        assert!(combat.grenades.is_empty());
        assert_eq!(mobs[0].health, 0., "A villager three blocks away must die");
        assert!(mobs[1].health > 0. && mobs[1].health < 20.);
        assert_eq!(mobs[2].health, 20., "No damage beyond fragmentation reach");
        assert_eq!(player.health, 20., "Creative players remain immune");
        assert_eq!(combat.blasts[0].radius, 2.7);
        assert_eq!(world.get([2, 40, 3]), crate::world::Block::Air);
        assert_eq!(world.get([4, 40, 3]), crate::world::Block::Stone);
    }

    #[test]
    fn hand_grenade_damage_respects_survival_armor() {
        let mut world = grenade_test_world();
        let mut combat = Combat::default();
        let mut armored = Player::new([5.5, 41., 3.5]);
        armored.plate = Some(5);
        armored.helmet = Some(5);
        combat.hand_grenade_blast(
            &mut world,
            &mut [],
            &mut armored,
            Mode::Survival,
            vec3(2.5, 41.101, 3.5),
        );
        assert!(armored.health > 0. && armored.health < 20.);
        assert!(armored.velocity.length() > 0.);
        let mut world = grenade_test_world();
        let mut unarmored = Player::new([5.5, 41., 3.5]);
        combat.hand_grenade_blast(
            &mut world,
            &mut [],
            &mut unarmored,
            Mode::Survival,
            vec3(2.5, 41.101, 3.5),
        );
        assert_eq!(unarmored.health, 0.);
    }

    #[test]
    fn m79_and_c4_keep_their_existing_damage_profiles() {
        for radius in [3., 4.5] {
            let mut world = grenade_test_world();
            let source = vec3(2.5, 42., 3.5);
            let mut player = Player::new([12.5, 41., 3.5]);
            let mut mobs = [Mob {
                position: vec3(4.5, 41.025, 3.5),
                health: 100.,
                kind: crate::game::MobKind::Villager,
                ..Default::default()
            }];
            let mut combat = Combat::default();
            combat.blast(
                &mut world,
                &mut mobs,
                &mut player,
                Mode::Creative,
                source,
                radius,
            );
            let expected_damage = 24. * (1. - 2. / (radius + 1.)) * radius / 3.;
            assert!((100. - mobs[0].health - expected_damage).abs() < 0.0001);
            assert_eq!(combat.blasts[0].radius, radius);
        }
    }

    #[test]
    fn fuse_waits_at_unloaded_chunk_boundary() {
        let mut world = World::new(3);
        world.stream([0, 0], 0, 1);
        let mut combat = Combat::default();
        combat.grenades.push(Grenade {
            position: vec3(15.99, 70., 0.5),
            velocity: vec3(13., 0., 0.),
            age: 3.19,
        });
        let mut player = Player::new([2., 70., 2.]);
        assert_eq!(
            combat.update_grenades(&mut world, &mut [], &mut player, Mode::Creative, 0.05),
            0
        );
        assert_eq!(combat.grenades.len(), 1);
        assert_eq!(combat.grenades[0].age, 3.19);
        assert_eq!(combat.grenades[0].position.x, 15.99);
        world.stream([0, 0], 1, 20);
        assert_eq!(
            combat.update_grenades(&mut world, &mut [], &mut player, Mode::Creative, 0.05),
            1
        );
        assert!(combat.grenades.is_empty());
    }
    #[test]
    fn cover_reduces_damage_before_destruction_and_persistent_fuses_are_validated() {
        let mut world = World::new(7);
        world.stream([0, 0], 1, 20);
        for x in 0..7 {
            for y in 62..70 {
                for z in 0..7 {
                    world.set([x, y, z], crate::world::Block::Air);
                }
            }
        }
        for y in 62..70 {
            for z in 0..7 {
                world.set([3, y, z], crate::world::Block::Stone);
            }
        }
        let source = vec3(2.5, 65., 3.5);
        let mut mobs = [
            Mob {
                position: vec3(1.5, 64.1, 3.5),
                health: 20.,
                kind: crate::game::MobKind::Zombie,
                ..Default::default()
            },
            Mob {
                position: vec3(3.5, 64.1, 3.5),
                health: 20.,
                kind: crate::game::MobKind::Zombie,
                ..Default::default()
            },
        ];
        let mut player = Player::new([0.5, 65., 0.5]);
        let mut combat = Combat::default();
        world.set([2, 64, 3], crate::world::Block::Bedrock);
        world.set([2, 66, 3], crate::world::Block::Water);
        combat.hand_grenade_blast(&mut world, &mut mobs, &mut player, Mode::Creative, source);
        assert!(mobs[1].health > mobs[0].health + 5.);
        assert_eq!(world.get([3, 65, 3]), crate::world::Block::Air);
        assert_eq!(world.get([2, 64, 3]), crate::world::Block::Bedrock);
        assert_eq!(world.get([2, 66, 3]), crate::world::Block::Water);
        let mut extras = WorldExtras::default();
        extras.grenades.push(Grenade {
            position: source,
            velocity: vec3(1., 2., 3.),
            age: 1.7,
        });
        let restored: WorldExtras =
            serde_json::from_str(&serde_json::to_string(&extras).unwrap()).unwrap();
        assert!(restored.valid());
        assert_eq!(restored.grenades[0].age, 1.7);
        assert_eq!(restored.grenades[0].velocity, vec3(1., 2., 3.));
        extras.grenades[0].age = 4.;
        assert!(!extras.valid());
        extras.grenades[0].age = 1.;
        extras.grenades[0].velocity.x = f32::INFINITY;
        assert!(!extras.valid());
        assert!(!Charge {
            position: [2., 65., 3.],
            normal: [i32::MIN, i32::MIN, i32::MIN]
        }
        .valid());
    }

    #[test]
    fn last_charge_can_be_placed_and_failed_deployment_never_consumes_ammo() {
        let mut world = World::new(3);
        world.stream([0, 0], 1, 20);
        for x in 0..5 {
            for z in 0..5 {
                for y in 62..70 {
                    world.set([x, y, z], crate::world::Block::Air);
                }
            }
        }
        let mut player = Player::new([2.5, 64., 4.5]);
        player.yaw = 0.;
        player.pitch = 0.;
        let mut inv = Inventory::new(Mode::Survival);
        inv.add(Item::C4, 1);
        let mut extras = WorldExtras::default();
        let mut combat = Combat::default();
        assert!(combat
            .deploy(&world, &player, &mut inv, &mut extras, Mode::Survival)
            .is_err());
        assert_eq!(inv.count(Item::C4), 1);
        world.set([2, 65, 2], crate::world::Block::Stone);
        assert!(combat
            .deploy(&world, &player, &mut inv, &mut extras, Mode::Survival)
            .is_ok());
        assert!(inv.selected().is_none());
        assert_eq!(extras.charges.len(), 1);
        let first = extras.charges[0].position;
        combat.throw_time = 0.;
        inv.add(Item::C4, 1);
        assert!(combat
            .deploy(&world, &player, &mut inv, &mut extras, Mode::Survival)
            .is_err());
        assert_eq!(inv.count(Item::C4), 1);
        extras.charges.push(Charge {
            position: [10000., 65., 10000.],
            normal: [0, 1, 0],
        });
        assert_eq!(
            combat.detonate_charges(
                &mut world,
                &mut [],
                &mut player,
                &mut extras,
                Mode::Creative
            ),
            1
        );
        assert_eq!(extras.charges.len(), 1);
        assert_ne!(extras.charges[0].position, first);
    }
    #[test]
    fn grenade_bounces_fuses_and_charge_roundtrips_without_losing_items() {
        let mut world = World::new(77);
        world.stream([0, 0], 2, 100);
        for x in 0..8 {
            for z in 0..8 {
                for y in 40..49 {
                    world.set(
                        [x, y, z],
                        if y == 40 {
                            crate::world::Block::Stone
                        } else {
                            crate::world::Block::Air
                        },
                    );
                }
            }
        }
        let mut player = Player::new([3.5, 41., 3.5]);
        player.pitch = -0.2;
        let mut inv = Inventory::new(Mode::Survival);
        let mut extras = WorldExtras::default();
        let mut combat = Combat::default();
        inv.add(Item::HandGrenade, 2);
        assert!(combat
            .deploy(&world, &player, &mut inv, &mut extras, Mode::Survival)
            .is_ok());
        assert_eq!(inv.count(Item::HandGrenade), 1);
        assert!(combat
            .deploy(&world, &player, &mut inv, &mut extras, Mode::Survival)
            .is_err());
        assert_eq!(inv.count(Item::HandGrenade), 1);
        combat.grenades[0].position = vec3(3.5, 41.2, 3.5);
        combat.grenades[0].velocity = vec3(0., -3., 0.);
        combat.update_grenades(&mut world, &mut [], &mut player, Mode::Creative, 0.08);
        assert!(combat.grenades[0].velocity.y > 0.);
        for _ in 0..210 {
            combat.update_grenades(&mut world, &mut [], &mut player, Mode::Creative, 1. / 60.);
        }
        assert!(combat.grenades.is_empty());
        assert_eq!(combat.explosions, 1);
        extras.charges.push(Charge {
            position: [2., 44., 2.],
            normal: [0, 1, 0],
        });
        assert!(extras.valid());
        let restored: WorldExtras =
            serde_json::from_str(&serde_json::to_string(&extras).unwrap()).unwrap();
        assert_eq!(restored.charges.len(), 1);
        assert_eq!(
            combat.detonate_charges(
                &mut world,
                &mut [],
                &mut player,
                &mut extras,
                Mode::Creative
            ),
            1
        );
        assert!(extras.charges.is_empty());
        let old: WorldExtras = serde_json::from_str(r#"{"drops":[],"spawn":null}"#).unwrap();
        assert!(old.charges.is_empty());
        assert!(!Charge {
            position: [2., f32::NAN, 2.],
            normal: [0, 1, 0]
        }
        .valid());
        assert!(!Charge {
            position: [2., 44., 2.],
            normal: [0, 2, 0]
        }
        .valid());
    }
}
