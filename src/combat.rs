#[cfg(test)]
use crate::game::MobKind;
use crate::game::{Inventory, Item, Mob, Mode, Player};
use crate::world::{Block, World, CHUNK};
use macroquad::prelude::{vec3, Vec2, Vec3};

const ROCKET_SPEED: f32 = 75.0;
const ROCKET_LIFETIME: f32 = 8.0;
const BLAST_RADIUS: f32 = 3.0;
const DAMAGE_RADIUS: f32 = 4.0;

#[derive(Debug, PartialEq, Eq)]
pub enum FireResult {
    Idle,
    Empty,
    Reloading,
    Fired(Item),
}

pub struct Rocket {
    pub position: Vec3,
    pub direction: Vec3,
    pub age: f32,
}

pub struct Blast {
    pub position: Vec3,
    pub age: f32,
    pub radius: f32,
}

pub struct Tracer {
    pub start: Vec3,
    pub end: Vec3,
    pub life: f32,
}

pub struct Casing {
    pub position: Vec3,
    pub velocity: Vec3,
    pub age: f32,
}

pub struct Impact {
    pub position: Vec3,
    pub normal: Vec3,
    pub age: f32,
    pub color: [u8; 3],
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum FireMode {
    #[default]
    Auto,
    Semi,
    Burst,
}

#[derive(Default)]
pub struct WeaponInput {
    pub aiming: bool,
    pub sprinting: bool,
    pub movement: f32,
    pub sway: Vec2,
}

pub struct Reload {
    slot: usize,
    weapon: Item,
    loaded: u16,
    pub elapsed: f32,
    pub duration: f32,
}

#[derive(Default)]
pub struct Combat {
    pub grenades: Vec<crate::explosives::Grenade>,
    pub throw_time: f32,
    pub throw_item: Option<Item>,
    pub rockets: Vec<Rocket>,
    pub blasts: Vec<Blast>,
    pub tracers: Vec<Tracer>,
    pub muzzle: f32,
    pub hit_marker: f32,
    pub shots: u32,
    pub explosions: u32,
    pub casings: Vec<Casing>,
    pub impacts: Vec<Impact>,
    pub aim: f32,
    pub sprint: f32,
    pub walk: f32,
    pub sway: Vec2,
    pub recoil: f32,
    pub shake: f32,
    pub bloom: f32,
    pub equip_time: f32,
    pub inspect_time: f32,
    pub fire_mode: FireMode,
    pub reloads: u32,
    pub zoom: f32,
    pub bolt_time: f32,
    camera_time: f32,
    burst_left: u8,
    pub reload: Option<Reload>,
    equipped: Option<Item>,
    equipped_slot: usize,
    trigger_down: bool,
    movement: f32,
    cooldown: f32,
}

impl Combat {
    pub fn view_direction(&self, direction: Vec3) -> Vec3 {
        (direction
            + Vec3::Y * self.recoil * 0.35
            + vec3(
                (self.camera_time * 61.).sin(),
                (self.camera_time * 47.).cos(),
                0.,
            ) * self.shake)
            .normalize_or_zero()
    }

    pub fn shot_ray(
        &self,
        world: &World,
        mobs: &[Mob],
        eye: Vec3,
        direction: Vec3,
        muzzle: Vec3,
    ) -> (Vec3, Vec3) {
        let wall = first_solid(world, eye, direction, 180.).unwrap_or(180.);
        let distance = nearest_mob(mobs, eye, direction, wall).map_or(wall, |(_, d)| d);
        let target = eye + direction * distance;
        // The visible barrel may cross a wall; keep the physical exit on our side.
        let offset = (muzzle - eye).clamp_length_max((distance - 0.01).max(0.));
        let muzzle = eye + offset;
        let length = offset.length();
        let origin = first_solid(world, eye, offset.normalize_or_zero(), length)
            .map_or(muzzle, |d| {
                eye + offset.normalize_or_zero() * (d - 0.002).max(0.)
            });
        let delta = target - origin;
        (
            origin,
            if delta.length_squared() < 0.000001 {
                direction
            } else {
                delta.normalize()
            },
        )
    }

    fn sync_equipped(&mut self, inventory: &Inventory) {
        let weapon = inventory
            .selected()
            .map(|s| s.item)
            .filter(|i| i.magazine_capacity() > 0);
        if self.equipped != weapon || self.equipped_slot != inventory.selected {
            self.equip_time = if weapon.is_some() { 0.34 } else { 0. };
            self.fire_mode = weapon.map_or(FireMode::Semi, |w| crate::weapons::modes(w)[0]);
            self.burst_left = 0;
            self.bolt_time = 0.;
            self.equipped = weapon;
            self.equipped_slot = inventory.selected;
            self.reload = None;
            self.inspect_time = 0.;
            self.trigger_down = true;
        }
    }

    pub fn tick_weapon(
        &mut self,
        inventory: &mut Inventory,
        mode: Mode,
        dt: f32,
        input: WeaponInput,
    ) {
        self.sync_equipped(inventory);
        if !dt.is_finite() || dt <= 0. {
            return;
        }
        let dt = dt.min(0.1);
        self.zoom = inventory
            .selected()
            .filter(|s| crate::weapons::spec(s.item).is_some_and(|w| w.scoped))
            .map_or(1.45, |s| crate::weapons::magnification(s.optic));
        self.bolt_time = (self.bolt_time - dt).max(0.);
        self.equip_time = (self.equip_time - dt).max(0.);
        self.inspect_time = (self.inspect_time - dt).max(0.);
        let k = 1. - (-dt * 12.).exp();
        let aiming =
            input.aiming && !input.sprinting && self.reload.is_none() && self.inspect_time == 0.;
        self.aim += (if aiming { 1. } else { 0. } - self.aim) * k;
        self.sprint += (if input.sprinting { 1. } else { 0. } - self.sprint) * k;
        self.movement = input.movement.clamp(0., 1.);
        self.walk += dt * self.movement * if input.sprinting { 15. } else { 9. };
        self.sway += (input.sway.clamp(Vec2::splat(-0.12), Vec2::splat(0.12)) - self.sway) * k;
        if let Some(reload) = self.reload.as_mut() {
            if inventory.selected != reload.slot
                || !inventory
                    .selected()
                    .is_some_and(|s| s.item == reload.weapon && s.loaded == reload.loaded)
            {
                self.reload = None;
                return;
            }
            reload.elapsed += dt;
            if reload.elapsed >= reload.duration {
                let weapon = reload.weapon;
                let amount =
                    (weapon.magazine_capacity() - reload.loaded).min(if mode == Mode::Creative {
                        weapon.magazine_capacity()
                    } else {
                        inventory.count(ammunition(weapon))
                    });
                if amount > 0
                    && (mode == Mode::Creative || inventory.consume(ammunition(weapon), amount))
                {
                    inventory.slots[inventory.selected].as_mut().unwrap().loaded += amount;
                    self.reloads = self.reloads.saturating_add(1);
                }
                self.reload = None;
            }
        }
    }

    pub fn start_reload(&mut self, inventory: &Inventory, mode: Mode) -> bool {
        self.sync_equipped(inventory);
        if self.reload.is_some() || self.equip_time > 0. {
            return false;
        }
        let Some(stack) = inventory.selected() else {
            return false;
        };
        if stack.item.magazine_capacity() == 0
            || stack.loaded >= stack.item.magazine_capacity()
            || (mode == Mode::Survival && inventory.count(ammunition(stack.item)) == 0)
        {
            return false;
        }
        self.inspect_time = 0.;
        self.burst_left = 0;
        self.reload = Some(Reload {
            slot: inventory.selected,
            weapon: stack.item,
            loaded: stack.loaded,
            elapsed: 0.,
            duration: crate::weapons::spec(stack.item).unwrap().reload
                - if stack.loaded > 0 { 0.5 } else { 0. },
        });
        true
    }

    pub fn reload_progress(&self) -> Option<f32> {
        self.reload
            .as_ref()
            .map(|r| (r.elapsed / r.duration).clamp(0., 1.))
    }

    pub fn cancel_trigger(&mut self) {
        self.burst_left = 0;
        self.trigger_down = true;
    }
    pub fn cycle_mode(&mut self, item: Item) {
        let modes = crate::weapons::modes(item);
        self.fire_mode =
            modes[(modes.iter().position(|m| *m == self.fire_mode).unwrap_or(0) + 1) % modes.len()];
        self.cancel_trigger();
    }

    pub fn inspect(&mut self) {
        if self.equipped.is_some() && self.reload.is_none() && self.equip_time == 0. {
            self.inspect_time = 2.6;
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn fire(
        &mut self,
        world: &mut World,
        inventory: &mut Inventory,
        mobs: &mut [Mob],
        eye: Vec3,
        direction: Vec3,
        mode: Mode,
        held: bool,
    ) -> FireResult {
        self.sync_equipped(inventory);
        let pressed = held && !self.trigger_down;
        self.trigger_down = held;
        let Some(weapon) = inventory.selected().map(|stack| stack.item) else {
            return FireResult::Idle;
        };
        let Some(spec) = crate::weapons::spec(weapon) else {
            return FireResult::Idle;
        };
        // Reject disallowed modes even if state was restored or changed by a fixture.
        if !crate::weapons::modes(weapon).contains(&self.fire_mode) {
            self.fire_mode = crate::weapons::modes(weapon)[0];
        }
        if self.reload.is_some()
            || self.equip_time > 0.
            || self.sprint > 0.45
            || self.bolt_time > 0.
            || self.cooldown > 0.
            || !valid_position(eye)
            || !direction.is_finite()
            || !direction.length_squared().is_finite()
            || direction.length_squared() < 0.000001
        {
            return FireResult::Idle;
        }
        if self.fire_mode == FireMode::Burst && pressed && self.burst_left == 0 {
            self.burst_left = 3;
        }
        let requested = match self.fire_mode {
            FireMode::Auto => held,
            FireMode::Semi => pressed,
            FireMode::Burst => self.burst_left > 0,
        };
        if !requested || (weapon == Item::RocketLauncher && self.rockets.len() >= 8) {
            return FireResult::Idle;
        }
        let interval = spec.interval;
        if inventory.selected().unwrap().loaded == 0 {
            if self.start_reload(inventory, mode) {
                return FireResult::Reloading;
            }
            self.cooldown = 0.22;
            return FireResult::Empty;
        }
        inventory.slots[inventory.selected].as_mut().unwrap().loaded -= 1;
        self.burst_left = self.burst_left.saturating_sub(1);
        if spec.bolt {
            self.bolt_time = spec.interval;
        }
        self.inspect_time = 0.;
        let direction = direction.normalize();
        let right = direction.cross(Vec3::Y).normalize_or_zero();
        let up = right.cross(direction).normalize_or_zero();
        let spread = (0.0015 + self.movement * 0.014 + self.bloom * 0.003) * (1. - self.aim * 0.82);
        let angle = self.shots as f32 * 2.399963;
        let direction =
            (direction + right * angle.cos() * spread + up * angle.sin() * spread).normalize();
        self.cooldown = interval;
        self.muzzle = 0.10;
        self.recoil = (self.recoil + spec.recoil).min(0.15);
        self.bloom = (self.bloom + 0.6).min(4.);
        self.shots = self.shots.saturating_add(1);
        if weapon != Item::RocketLauncher {
            let range = if spec.scoped { 180. } else { 90. };
            let wall = first_solid(world, eye, direction, range).unwrap_or(range);
            let target = nearest_mob(mobs, eye, direction, wall);
            let distance = if let Some((index, distance)) = target {
                let headshot = (eye + direction * distance).y
                    > mobs[index].position.y + mob_height(&mobs[index]) * 0.80;
                mobs[index].health =
                    (mobs[index].health - spec.damage * if headshot { 2. } else { 1. }).max(0.0);
                self.hit_marker = 0.15;
                distance
            } else {
                wall
            };
            if self.tracers.len() >= 24 {
                self.tracers.remove(0);
            }
            self.tracers.push(Tracer {
                start: eye,
                end: eye + direction * distance,
                life: 0.09,
            });
            if self.casings.len() >= 96 {
                self.casings.remove(0);
            }
            self.casings.push(Casing {
                position: eye + right * 0.20 - up * 0.16,
                velocity: right * 2.2 + Vec3::Y * 1.3 + direction * 0.5,
                age: 0.,
            });
            if target.is_none() && wall < range {
                let position = eye + direction * wall;
                let cell = (position + direction * 0.001).floor().as_ivec3().to_array();
                let block = world.get(cell);
                let material = block.material();
                let metal_door = if let Block::Map(id) = block {
                    crate::city::state(id).is_some_and(|s| {
                        s.source.starts_with("iron_") || s.source.contains("copper")
                    })
                } else {
                    false
                };
                if material == Block::Glass
                    || matches!(weapon, Item::Aw50 | Item::M200)
                        && !metal_door
                        && matches!(
                            material,
                            Block::Wood
                                | Block::Planks
                                | Block::Workbench
                                | Block::Bed
                                | Block::Chest
                                | Block::Door
                        )
                {
                    world.set(cell, Block::Air);
                }
                if self.impacts.len() >= 48 {
                    self.impacts.remove(0);
                }
                self.impacts.push(Impact {
                    position: position - direction * 0.012,
                    normal: -direction,
                    age: 0.,
                    color: block.color(0),
                });
            }
        } else {
            self.rockets.push(Rocket {
                position: eye,
                direction,
                age: 0.0,
            });
        }
        FireResult::Fired(weapon)
    }

    pub fn update(
        &mut self,
        world: &mut World,
        mobs: &mut [Mob],
        player: &mut Player,
        mode: Mode,
        dt: f32,
    ) -> usize {
        if !dt.is_finite() || dt <= 0.0 {
            return 0;
        }
        let dt = dt.min(ROCKET_LIFETIME);
        self.camera_time += dt;
        self.cooldown = (self.cooldown - dt).max(0.0);
        self.muzzle = (self.muzzle - dt).max(0.0);
        self.hit_marker = (self.hit_marker - dt).max(0.0);
        self.recoil *= (-dt * 14.).exp();
        self.shake *= (-dt * 8.).exp();
        self.bloom = (self.bloom - dt * 2.).max(0.);
        for casing in &mut self.casings {
            casing.age += dt;
            casing.velocity.y -= 9. * dt;
            let next = casing.position + casing.velocity * dt;
            if world.get(next.floor().as_ivec3().to_array()).solid() {
                casing.velocity *= 0.25;
                casing.velocity.y = casing.velocity.y.abs() * 0.5;
            } else {
                casing.position = next;
            }
        }
        self.casings.retain(|c| c.age < 4.);
        for impact in &mut self.impacts {
            impact.age += dt;
        }
        self.impacts.retain(|i| i.age < 1.2);
        for tracer in &mut self.tracers {
            tracer.life -= dt;
        }
        self.tracers.retain(|tracer| tracer.life > 0.0);
        for blast in &mut self.blasts {
            blast.age += dt;
        }
        self.blasts.retain(|blast| blast.age < 3.0);
        let mut exploded = 0;
        for mut rocket in std::mem::take(&mut self.rockets) {
            if !valid_position(rocket.position)
                || !rocket.direction.is_finite()
                || !rocket.direction.length_squared().is_finite()
                || rocket.direction.length_squared() < 0.000001
                || !rocket.age.is_finite()
                || !(0.0..=ROCKET_LIFETIME).contains(&rocket.age)
            {
                continue;
            }
            let mut remaining = dt.min((ROCKET_LIFETIME - rocket.age).max(0.0));
            let mut hit = false;
            // Small swept arcs keep grenades from tunnelling through thin voxel walls.
            while remaining > 0. && !hit {
                let step = remaining.min(1. / 120.);
                let velocity = rocket.direction * ROCKET_SPEED;
                let displacement = velocity * step - Vec3::Y * (4.9 * step * step);
                let distance = displacement.length();
                let direction = displacement.normalize_or_zero();
                let wall = first_solid(world, rocket.position, direction, distance);
                let target =
                    nearest_mob(mobs, rocket.position, direction, wall.unwrap_or(distance));
                let impact = target.map(|(_, d)| d).or(wall);
                rocket.position += direction * impact.unwrap_or(distance);
                rocket.direction.y -= 9.8 * step / ROCKET_SPEED;
                rocket.age += step;
                remaining -= step;
                hit = impact.is_some();
            }
            if hit || rocket.age >= ROCKET_LIFETIME - 0.00001 {
                self.explode(world, mobs, player, mode, rocket.position);
                exploded += 1;
            } else {
                self.rockets.push(rocket);
            }
        }
        exploded
    }

    pub fn explode(
        &mut self,
        world: &mut World,
        mobs: &mut [Mob],
        player: &mut Player,
        mode: Mode,
        position: Vec3,
    ) {
        self.blast(world, mobs, player, mode, position, BLAST_RADIUS);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn blast(
        &mut self,
        world: &mut World,
        mobs: &mut [Mob],
        player: &mut Player,
        mode: Mode,
        position: Vec3,
        radius: f32,
    ) {
        self.blast_with_damage(
            world,
            mobs,
            player,
            mode,
            position,
            radius,
            24. * radius / BLAST_RADIUS,
            radius + (DAMAGE_RADIUS - BLAST_RADIUS),
        );
    }

    pub fn hand_grenade_blast(
        &mut self,
        world: &mut World,
        mobs: &mut [Mob],
        player: &mut Player,
        mode: Mode,
        position: Vec3,
    ) {
        // Fragmentation reaches beyond the crater, even when a grenade rests on the ground.
        self.blast_with_damage(world, mobs, player, mode, position, 2.7, 56., 6.5);
    }

    #[allow(clippy::too_many_arguments)]
    fn blast_with_damage(
        &mut self,
        world: &mut World,
        mobs: &mut [Mob],
        player: &mut Player,
        mode: Mode,
        position: Vec3,
        radius: f32,
        damage: f32,
        damage_radius: f32,
    ) {
        if !valid_position(position)
            || !radius.is_finite()
            || !(1. ..=5.).contains(&radius)
            || !damage.is_finite()
            || damage <= 0.
            || !damage_radius.is_finite()
            || damage_radius <= 0.
        {
            return;
        }
        // Evaluate cover before terrain destruction, so walls protect on this blast.
        let damage_at = |target: Vec3| {
            let delta = target - position;
            let distance = delta.length();
            let exposure = if distance > 0.2
                && crate::game::raycast(
                    world,
                    position + delta.normalize() * 0.15,
                    delta,
                    distance - 0.15,
                )
                .is_some()
            {
                0.12
            } else {
                1.
            };
            blast_damage(distance, damage_radius, damage) * exposure
        };
        for mob in mobs {
            if mob.health <= 0. {
                continue;
            }
            let damage = damage_at(mob.position + Vec3::Y * mob_height(mob) * 0.5);
            mob.health = (mob.health - damage).max(0.);
            if damage > 0. {
                self.hit_marker = 0.15;
            }
        }
        if mode == Mode::Survival {
            let damage = damage_at(player.position + Vec3::Y * 0.9);
            player.combat_damage(damage);
            player.velocity +=
                (player.position + Vec3::Y * 0.9 - position).normalize_or_zero() * damage * 0.22;
        }
        let low = (position - Vec3::splat(radius)).floor().as_ivec3();
        let high = (position + Vec3::splat(radius)).floor().as_ivec3();
        for x in low.x..=high.x {
            for z in low.z..=high.z {
                if !world
                    .chunks
                    .contains_key(&[x.div_euclid(CHUNK), z.div_euclid(CHUNK)])
                {
                    continue;
                }
                for y in low.y.max(1)..=high.y.min(world.height() - 1) {
                    let cell = [x, y, z];
                    if (vec3(x as f32 + 0.5, y as f32 + 0.5, z as f32 + 0.5) - position)
                        .length_squared()
                        <= radius * radius
                        && !matches!(
                            world.get(cell),
                            Block::Air | Block::Bedrock | Block::Water | Block::Lava
                        )
                    {
                        world.set(cell, Block::Air);
                    }
                }
            }
        }
        self.explosions = self.explosions.saturating_add(1);
        self.shake = self
            .shake
            .max((1. - player.eye().distance(position) / 32.).max(0.) * 0.10);
        if self.blasts.len() >= 24 {
            self.blasts.remove(0);
        }
        self.blasts.push(Blast {
            position,
            age: 0.,
            radius,
        });
    }
}

fn ammunition(weapon: Item) -> Item {
    crate::weapons::spec(weapon)
        .expect("weapon ammunition")
        .ammo
}

fn valid_position(position: Vec3) -> bool {
    position.is_finite() && position.abs().max_element() < 1_000_000.0
}

fn blast_damage(distance: f32, radius: f32, damage: f32) -> f32 {
    damage * (1.0 - distance / radius).clamp(0.0, 1.0)
}

fn mob_height(mob: &Mob) -> f32 {
    mob.kind.height() * if mob.baby > 0. { 0.55 } else { 1. }
}

fn nearest_mob(mobs: &[Mob], start: Vec3, direction: Vec3, max: f32) -> Option<(usize, f32)> {
    mobs.iter()
        .enumerate()
        .filter(|(_, mob)| mob.health > 0.0)
        .filter_map(|(index, mob)| {
            let low = mob.position + vec3(-0.5, 0.0, -0.6);
            let high = mob.position + vec3(0.5, mob_height(mob), 0.95);
            let mut near: f32 = 0.0;
            let mut far = max;
            for axis in 0..3 {
                if direction[axis].abs() < 0.000001 {
                    if start[axis] < low[axis] || start[axis] > high[axis] {
                        return None;
                    }
                } else {
                    let a = (low[axis] - start[axis]) / direction[axis];
                    let b = (high[axis] - start[axis]) / direction[axis];
                    near = near.max(a.min(b));
                    far = far.min(a.max(b));
                    if near > far {
                        return None;
                    }
                }
            }
            (near < max).then_some((index, near))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
}

/// Traverse the whole shot segment so fast projectiles cannot jump through a wall.
fn first_solid(world: &World, origin: Vec3, direction: Vec3, max: f32) -> Option<f32> {
    let mut cell = origin.floor().as_ivec3().to_array();
    let mut distances = [f32::INFINITY; 3];
    let mut increments = [f32::INFINITY; 3];
    let mut steps = [0; 3];
    for axis in 0..3 {
        if direction[axis] > 0.0 {
            steps[axis] = 1;
            distances[axis] = (cell[axis] as f32 + 1.0 - origin[axis]) / direction[axis];
            increments[axis] = 1.0 / direction[axis];
        } else if direction[axis] < 0.0 {
            steps[axis] = -1;
            distances[axis] = (cell[axis] as f32 - origin[axis]) / direction[axis];
            increments[axis] = -1.0 / direction[axis];
        }
    }
    let mut distance = 0.0;
    while distance <= max {
        if world.get(cell).solid() {
            return Some(distance);
        }
        let axis = if distances[0] <= distances[1] && distances[0] <= distances[2] {
            0
        } else if distances[1] <= distances[2] {
            1
        } else {
            2
        };
        distance = distances[axis];
        cell[axis] += steps[axis];
        distances[axis] += increments[axis];
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::Stack;

    fn weapon_inventory(weapon: Item, ammo: Item, count: u16) -> Inventory {
        let mut inventory = Inventory::new(Mode::Survival);
        inventory.slots.fill(None);
        let loaded = weapon.magazine_capacity().min(count);
        inventory.slots[0] = Some(Stack {
            item: weapon,
            count: 1,
            loaded,
            optic: 0,
        });
        if count > loaded {
            inventory.add(ammo, count - loaded);
        }
        inventory
    }

    fn finish_reload(combat: &mut Combat, inventory: &mut Inventory, mode: Mode) {
        for _ in 0..300 {
            combat.tick_weapon(inventory, mode, 1. / 60., WeaponInput::default());
        }
    }

    fn ready_combat(inventory: &Inventory) -> Combat {
        let mut combat = Combat::default();
        combat.sync_equipped(inventory);
        combat.equip_time = 0.;
        combat.trigger_down = false;
        combat
    }

    fn zombie(position: Vec3) -> Mob {
        Mob {
            position,
            kind: MobKind::Zombie,
            health: 20.0,
            phase: 0.0,
            attack_timer: 0.0,
            ..Default::default()
        }
    }

    #[test]
    fn every_weapon_reloads_only_its_own_caliber() {
        for gun in crate::weapons::GUNS {
            let ammo = ammunition(gun);
            let mut inventory = weapon_inventory(gun, ammo, 0);
            for other in crate::weapons::AMMO {
                if other != ammo {
                    inventory.add(other, 64);
                }
            }
            let mut combat = ready_combat(&inventory);
            assert!(!combat.start_reload(&inventory, Mode::Survival));
            inventory.add(ammo, 3);
            assert!(combat.start_reload(&inventory, Mode::Survival));
            assert_eq!(inventory.count(ammo), 3);
            finish_reload(&mut combat, &mut inventory, Mode::Survival);
            let loaded = 3.min(gun.magazine_capacity());
            assert_eq!(inventory.selected().unwrap().loaded, loaded);
            assert_eq!(inventory.count(ammo), 3 - loaded);
            for other in crate::weapons::AMMO {
                if other != ammo {
                    assert_eq!(inventory.count(other), 64);
                }
            }
        }
    }

    #[test]
    fn m16_burst_is_three_shots_and_has_no_automatic_mode() {
        for hold_trigger in [false, true] {
            let mut world = World::new(7);
            let mut player = Player::new([0.5, 70., 0.5]);
            let mut inventory = weapon_inventory(Item::M16A4, Item::Ammo556, 30);
            let mut combat = ready_combat(&inventory);
            combat.cycle_mode(Item::M16A4);
            assert_eq!(combat.fire_mode, FireMode::Burst);
            combat.trigger_down = false;
            for frame in 0..120 {
                combat.fire(
                    &mut world,
                    &mut inventory,
                    &mut [],
                    player.eye(),
                    -Vec3::Z,
                    Mode::Survival,
                    frame == 0 || hold_trigger,
                );
                combat.tick_weapon(
                    &mut inventory,
                    Mode::Survival,
                    1. / 60.,
                    WeaponInput::default(),
                );
                combat.update(&mut world, &mut [], &mut player, Mode::Survival, 1. / 60.);
            }
            assert_eq!(combat.shots, 3);
            assert_eq!(inventory.selected().unwrap().loaded, 27);
            combat.cycle_mode(Item::M16A4);
            assert_eq!(combat.fire_mode, FireMode::Semi);
            combat.fire_mode = FireMode::Auto;
            combat.trigger_down = false;
            for _ in 0..120 {
                combat.fire(
                    &mut world,
                    &mut inventory,
                    &mut [],
                    player.eye(),
                    -Vec3::Z,
                    Mode::Survival,
                    true,
                );
                combat.update(&mut world, &mut [], &mut player, Mode::Survival, 1. / 60.);
            }
            assert_eq!(combat.fire_mode, FireMode::Semi);
            assert_eq!(combat.shots, 4);
        }
    }

    #[test]
    fn bolt_actions_require_cycle_and_a_new_trigger_press() {
        for gun in [Item::M200, Item::Tundra, Item::Aw50] {
            let mut world = World::new(7);
            let mut player = Player::new([0.5, 70., 0.5]);
            let mut inventory = weapon_inventory(gun, ammunition(gun), 5);
            let mut combat = ready_combat(&inventory);
            assert_eq!(
                combat.fire(
                    &mut world,
                    &mut inventory,
                    &mut [],
                    player.eye(),
                    -Vec3::Z,
                    Mode::Survival,
                    true
                ),
                FireResult::Fired(gun)
            );
            assert!(combat.bolt_time > 0.);
            for _ in 0..180 {
                combat.tick_weapon(
                    &mut inventory,
                    Mode::Survival,
                    1. / 60.,
                    WeaponInput::default(),
                );
                combat.update(&mut world, &mut [], &mut player, Mode::Survival, 1. / 60.);
                combat.fire(
                    &mut world,
                    &mut inventory,
                    &mut [],
                    player.eye(),
                    -Vec3::Z,
                    Mode::Survival,
                    true,
                );
            }
            assert_eq!(combat.shots, 1);
            combat.fire(
                &mut world,
                &mut inventory,
                &mut [],
                player.eye(),
                -Vec3::Z,
                Mode::Survival,
                false,
            );
            assert_eq!(
                combat.fire(
                    &mut world,
                    &mut inventory,
                    &mut [],
                    player.eye(),
                    -Vec3::Z,
                    Mode::Survival,
                    true
                ),
                FireResult::Fired(gun)
            );
        }
    }

    #[test]
    fn automatic_fire_consumes_ammo_only_on_a_shot_and_stops_on_release() {
        let mut world = World::new(7);
        let mut player = Player::new([0.5, 65.0, 0.5]);
        let mut inventory = weapon_inventory(Item::Ak, Item::Bullet, 2);
        let mut combat = ready_combat(&inventory);
        let eye = player.eye();
        assert_eq!(
            combat.fire(
                &mut world,
                &mut inventory,
                &mut [],
                eye,
                Vec3::ZERO,
                Mode::Survival,
                true
            ),
            FireResult::Idle
        );
        assert_eq!(
            combat.fire(
                &mut world,
                &mut inventory,
                &mut [],
                eye,
                vec3(f32::NAN, 0.0, 1.0),
                Mode::Survival,
                true
            ),
            FireResult::Idle
        );
        assert_eq!(
            combat.fire(
                &mut world,
                &mut inventory,
                &mut [],
                eye,
                Vec3::X,
                Mode::Survival,
                true
            ),
            FireResult::Fired(Item::Ak)
        );
        assert_eq!(inventory.selected().unwrap().loaded, 1);
        assert_eq!(
            combat.fire(
                &mut world,
                &mut inventory,
                &mut [],
                eye,
                Vec3::X,
                Mode::Survival,
                true
            ),
            FireResult::Idle
        );
        combat.update(&mut world, &mut [], &mut player, Mode::Survival, 0.12);
        assert_eq!(
            combat.fire(
                &mut world,
                &mut inventory,
                &mut [],
                eye,
                Vec3::X,
                Mode::Survival,
                false
            ),
            FireResult::Idle
        );
        assert_eq!(inventory.selected().unwrap().loaded, 1);
        assert_eq!(
            combat.fire(
                &mut world,
                &mut inventory,
                &mut [],
                eye,
                Vec3::X,
                Mode::Survival,
                true
            ),
            FireResult::Fired(Item::Ak)
        );
        combat.update(&mut world, &mut [], &mut player, Mode::Survival, 0.12);
        assert_eq!(
            combat.fire(
                &mut world,
                &mut inventory,
                &mut [],
                eye,
                Vec3::X,
                Mode::Survival,
                true
            ),
            FireResult::Empty
        );
        assert_eq!(combat.shots, 2);
        combat.update(&mut world, &mut [], &mut player, Mode::Creative, 0.25);
        assert!(combat.start_reload(&inventory, Mode::Creative));
        finish_reload(&mut combat, &mut inventory, Mode::Creative);
        assert_eq!(inventory.selected().unwrap().loaded, 30);
        assert_eq!(
            combat.fire(
                &mut world,
                &mut inventory,
                &mut [],
                eye,
                Vec3::X,
                Mode::Creative,
                true
            ),
            FireResult::Fired(Item::Ak)
        );
        assert_eq!(inventory.count(Item::Bullet), 0);
        combat.update(&mut world, &mut [], &mut player, Mode::Creative, 1.0);
        assert!(combat.tracers.is_empty());
        assert_eq!(combat.muzzle, 0.0);
    }

    #[test]
    fn launchers_require_reload_and_reject_invalid_updates() {
        let mut world = World::new(7);
        let mut player = Player::new([0.5, 65.0, 0.5]);
        let mut inventory = weapon_inventory(Item::RocketLauncher, Item::RocketAmmo, 2);
        let mut combat = ready_combat(&inventory);
        assert_eq!(
            combat.fire(
                &mut world,
                &mut inventory,
                &mut [],
                player.eye(),
                Vec3::Y,
                Mode::Survival,
                true
            ),
            FireResult::Fired(Item::RocketLauncher)
        );
        combat.update(&mut world, &mut [], &mut player, Mode::Survival, f32::NAN);
        assert_eq!(combat.rockets[0].age, 0.0);
        combat.update(&mut world, &mut [], &mut player, Mode::Survival, 0.99);
        assert_eq!(
            combat.fire(
                &mut world,
                &mut inventory,
                &mut [],
                player.eye(),
                Vec3::Y,
                Mode::Survival,
                true
            ),
            FireResult::Idle
        );
        assert_eq!(inventory.count(Item::RocketAmmo), 1);
        combat.update(&mut world, &mut [], &mut player, Mode::Survival, 0.02);
        assert!(combat.start_reload(&inventory, Mode::Survival));
        assert_eq!(inventory.count(Item::RocketAmmo), 1);
        finish_reload(&mut combat, &mut inventory, Mode::Survival);
        assert_eq!(inventory.selected().unwrap().loaded, 1);
        combat.fire(
            &mut world,
            &mut inventory,
            &mut [],
            player.eye(),
            Vec3::Y,
            Mode::Survival,
            false,
        );
        assert_eq!(
            combat.fire(
                &mut world,
                &mut inventory,
                &mut [],
                player.eye(),
                Vec3::Y,
                Mode::Survival,
                true
            ),
            FireResult::Fired(Item::RocketLauncher)
        );
        assert_eq!(inventory.count(Item::RocketAmmo), 0);
        assert_eq!(combat.rockets.len(), 2);
    }

    #[test]
    fn bullets_hit_only_the_nearest_creature_and_cannot_cross_a_wall() {
        let mut world = World::new(7);
        let mut player = Player::new([0.5, 65.0, 0.5]);
        let mut inventory = weapon_inventory(Item::Ak, Item::Bullet, 5);
        let mut combat = ready_combat(&inventory);
        let mut mobs = [zombie(vec3(8.5, 65.0, 0.5)), zombie(vec3(4.5, 65.0, 0.5))];
        combat.fire(
            &mut world,
            &mut inventory,
            &mut mobs,
            player.eye(),
            Vec3::X,
            Mode::Survival,
            true,
        );
        assert_eq!(mobs[0].health, 20.0);
        assert_eq!(mobs[1].health, 8.0);
        world.set([2, 66, 0], Block::Stone);
        combat.update(&mut world, &mut mobs, &mut player, Mode::Survival, 0.12);
        combat.fire(
            &mut world,
            &mut inventory,
            &mut mobs,
            player.eye(),
            Vec3::X,
            Mode::Survival,
            true,
        );
        assert_eq!(mobs[1].health, 8.0);
        assert!((combat.tracers.last().unwrap().end.x - 2.0).abs() < 0.0001);
    }

    #[test]
    fn swept_rocket_destroys_loaded_terrain_preserves_fluids_and_hurts_survival_player() {
        let mut world = World::new(7);
        world.stream([0, 0], 0, 1);
        let mut player = Player::new([0.5, 65.0, 0.5]);
        let mut inventory = weapon_inventory(Item::RocketLauncher, Item::RocketAmmo, 1);
        world.set([3, 66, 0], Block::Stone);
        world.set([4, 66, 0], Block::Bedrock);
        world.set([3, 65, 1], Block::Water);
        world.set([3, 65, 2], Block::Lava);
        world.set([3, 65, 0], Block::Farmland);
        world.set([3, 66, 1], Block::Glass);
        world.set([2, 65, 0], Block::Farmland);
        world.set([2, 66, 0], Block::Wheat3);
        let mut combat = ready_combat(&inventory);
        assert_eq!(
            combat.fire(
                &mut world,
                &mut inventory,
                &mut [],
                player.eye(),
                Vec3::X,
                Mode::Survival,
                true
            ),
            FireResult::Fired(Item::RocketLauncher)
        );
        assert_eq!(inventory.count(Item::RocketAmmo), 0);
        assert_eq!(
            combat.update(&mut world, &mut [], &mut player, Mode::Survival, 0.5),
            1
        );
        assert!(combat.rockets.is_empty());
        assert_eq!(combat.explosions, 1);
        assert!((combat.blasts[0].position.x - 3.0).abs() < 0.0001);
        assert_eq!(world.get([3, 66, 0]), Block::Air);
        assert_eq!(world.get([3, 66, 1]), Block::Air);
        assert_eq!(world.get([4, 66, 0]), Block::Bedrock);
        assert_eq!(world.get([3, 65, 1]), Block::Water);
        assert_eq!(world.get([3, 65, 2]), Block::Lava);
        assert!(world.is_fluid_source([3, 65, 1]));
        assert_eq!(world.crop_count(), 0);
        assert!(player.health < 20.0);
        let health = player.health;
        combat.update(&mut world, &mut [], &mut player, Mode::Survival, 4.0);
        assert_eq!(player.health, health);
        assert!(combat.blasts.is_empty());
    }

    #[test]
    fn rocket_hits_creatures_and_explosion_has_bounded_falloff_and_creative_immunity() {
        let mut world = World::new(7);
        let mut player = Player::new([0.5, 65.0, 0.5]);
        let mut inventory = weapon_inventory(Item::RocketLauncher, Item::RocketAmmo, 1);
        let mut mobs = [zombie(vec3(3.0, 65.0, 0.5)), zombie(vec3(7.0, 65.0, 0.5))];
        let mut combat = ready_combat(&inventory);
        combat.fire(
            &mut world,
            &mut inventory,
            &mut mobs,
            player.eye(),
            Vec3::X,
            Mode::Creative,
            true,
        );
        assert_eq!(
            combat.update(&mut world, &mut mobs, &mut player, Mode::Creative, 0.5),
            1
        );
        assert!(mobs[0].health < 20.0);
        assert_eq!(mobs[1].health, 20.0);
        assert_eq!(player.health, 20.0);
        assert_eq!(blast_damage(0.0, DAMAGE_RADIUS, 24.), 24.0);
        assert_eq!(blast_damage(2.0, DAMAGE_RADIUS, 24.), 12.0);
        assert_eq!(blast_damage(4.0, DAMAGE_RADIUS, 24.), 0.0);
    }

    #[test]
    fn explosions_leave_unloaded_chunks_unchanged_and_rockets_expire() {
        let mut world = World::new(7);
        world.stream([0, 0], 0, 1);
        world.set([15, 66, 0], Block::Stone);
        world.set([16, 66, 0], Block::Brick);
        let mut player = Player::new([0.5, 65.0, 0.5]);
        let mut combat = Combat::default();
        combat.explode(
            &mut world,
            &mut [],
            &mut player,
            Mode::Creative,
            vec3(15.5, 66.5, 0.5),
        );
        assert_eq!(world.get([15, 66, 0]), Block::Air);
        assert_eq!(world.get([16, 66, 0]), Block::Brick);
        let mut inventory = weapon_inventory(Item::RocketLauncher, Item::RocketAmmo, 1);
        finish_reload(&mut combat, &mut inventory, Mode::Creative);
        combat.fire(
            &mut world,
            &mut inventory,
            &mut [],
            player.eye(),
            Vec3::Y,
            Mode::Creative,
            false,
        );
        combat.fire(
            &mut world,
            &mut inventory,
            &mut [],
            player.eye(),
            Vec3::Y,
            Mode::Creative,
            true,
        );
        assert_eq!(
            combat.update(&mut world, &mut [], &mut player, Mode::Creative, 8.0),
            1
        );
        assert!(combat.rockets.is_empty());
        assert_eq!(combat.explosions, 2);
        assert!(
            combat.blasts.last().unwrap().position.y > 65.0
                && combat.blasts.last().unwrap().position.y < 400.0
        );
    }

    #[test]
    fn m79_reaches_a_distant_wall_without_tunnelling() {
        let mut world = World::new(7);
        world.stream([2, 0], 3, 100);
        for x in 0..=95 {
            world.set([x, 60, 0], Block::Stone);
            for y in 61..=70 {
                world.set([x, y, 0], if x == 80 { Block::Brick } else { Block::Air });
            }
        }
        let mut player = Player::new([0.5, 61., 0.5]);
        let mut inventory = weapon_inventory(Item::RocketLauncher, Item::RocketAmmo, 1);
        let mut combat = ready_combat(&inventory);
        assert_eq!(
            combat.fire(
                &mut world,
                &mut inventory,
                &mut [],
                vec3(0.5, 62.5, 0.5),
                vec3(1., 0.10, 0.),
                Mode::Creative,
                true
            ),
            FireResult::Fired(Item::RocketLauncher)
        );
        for _ in 0..100 {
            combat.update(&mut world, &mut [], &mut player, Mode::Creative, 1. / 60.);
        }
        assert_eq!(combat.explosions, 1);
        assert!(combat.rockets.is_empty());
        assert_eq!(world.get([80, 64, 0]), Block::Air);
        assert!(world.get([81, 68, 0]) == Block::Air);
    }

    #[test]
    fn muzzle_rays_converge_on_sights_and_never_skip_near_walls_or_targets() {
        let mut world = World::new(7);
        let combat = Combat {
            recoil: 0.04,
            ..Default::default()
        };
        let eye = vec3(0.5, 66.5, 0.5);
        let direction = combat.view_direction(Vec3::X);
        assert!(direction.y > 0.);
        world.set([15, 66, 0], Block::Brick);
        let muzzle = eye + vec3(1.2, -0.15, 0.3);
        let target = eye + direction * first_solid(&world, eye, direction, 180.).unwrap();
        let (start, dir) = combat.shot_ray(&world, &[], eye, direction, muzzle);
        assert_eq!(start, muzzle);
        assert!(dir.cross(target - start).length() < 0.0001);
        let mut inventory = weapon_inventory(Item::Ak, Item::Bullet, 30);
        let mut shooting = ready_combat(&inventory);
        shooting.fire(
            &mut world,
            &mut inventory,
            &mut [],
            start,
            dir,
            Mode::Creative,
            true,
        );
        assert_eq!(shooting.tracers[0].start, muzzle);
        assert!((shooting.tracers[0].end.x - 15.).abs() < 0.001);
        world.set([1, 66, 0], Block::Brick);
        let (start, dir) = combat.shot_ray(&world, &[], eye, Vec3::X, eye + Vec3::X * 2.);
        assert!(start.x < 1. && first_solid(&world, start, dir, 10.).unwrap() < 0.01);
        let close = [Mob {
            position: eye - Vec3::Y * 0.5 + Vec3::Z * 0.9,
            kind: MobKind::Zombie,
            health: 20.,
            phase: 0.,
            attack_timer: 0.,
            ..Default::default()
        }];
        let (start, dir) = combat.shot_ray(&world, &close, eye, Vec3::Z, eye + Vec3::Z * 2.);
        assert!(start.z < eye.z + 0.6 && dir.z > 0.);
        let mut overlapping = close;
        overlapping[0].position.z -= 0.6;
        let (start, dir) = combat.shot_ray(&world, &overlapping, eye, Vec3::Z, eye + Vec3::Z * 2.);
        assert_eq!((start, dir), (eye, Vec3::Z));
        let mut shooting = ready_combat(&inventory);
        assert_eq!(
            shooting.fire(
                &mut world,
                &mut inventory,
                &mut overlapping,
                start,
                dir,
                Mode::Creative,
                true
            ),
            FireResult::Fired(Item::Ak)
        );
        assert!(overlapping[0].health < 20.);
    }

    #[test]
    fn reload_conserves_ammunition_and_cancellation_never_consumes_reserves() {
        let mut inventory = weapon_inventory(Item::Ak, Item::Bullet, 42);
        inventory.slots[0].as_mut().unwrap().loaded = 10;
        let mut combat = ready_combat(&inventory);
        assert!(combat.start_reload(&inventory, Mode::Survival));
        for _ in 0..60 {
            combat.tick_weapon(
                &mut inventory,
                Mode::Survival,
                1. / 60.,
                WeaponInput::default(),
            );
        }
        assert_eq!(inventory.selected().unwrap().loaded, 10);
        assert_eq!(inventory.count(Item::Bullet), 12);
        inventory.quick_transfer(0);
        combat.tick_weapon(
            &mut inventory,
            Mode::Survival,
            1. / 60.,
            WeaponInput::default(),
        );
        assert!(combat.reload.is_none());
        assert_eq!(inventory.count(Item::Bullet), 12);
        assert_eq!(inventory.slots[9].unwrap().loaded, 10);
        inventory.quick_transfer(9);
        finish_reload(&mut combat, &mut inventory, Mode::Survival);
        assert!(combat.start_reload(&inventory, Mode::Survival));
        finish_reload(&mut combat, &mut inventory, Mode::Survival);
        assert_eq!(inventory.selected().unwrap().loaded, 22);
        assert_eq!(inventory.count(Item::Bullet), 0);
        assert_eq!(combat.reloads, 1);
        assert!(!combat.start_reload(&inventory, Mode::Survival));
    }

    #[test]
    fn semi_auto_requires_release_and_aim_is_smooth_and_disabled_while_sprinting() {
        let mut world = World::new(7);
        let mut inventory = weapon_inventory(Item::Ak, Item::Bullet, 30);
        let mut combat = ready_combat(&inventory);
        combat.tick_weapon(
            &mut inventory,
            Mode::Survival,
            0.1,
            WeaponInput {
                aiming: true,
                ..Default::default()
            },
        );
        assert!(combat.aim > 0. && combat.aim < 1.);
        combat.fire_mode = FireMode::Semi;
        combat.fire(
            &mut world,
            &mut inventory,
            &mut [],
            vec3(0.5, 66., 0.5),
            Vec3::Y,
            Mode::Survival,
            false,
        );
        assert_eq!(
            combat.fire(
                &mut world,
                &mut inventory,
                &mut [],
                vec3(0.5, 66., 0.5),
                Vec3::Y,
                Mode::Survival,
                true
            ),
            FireResult::Fired(Item::Ak)
        );
        combat.cooldown = 0.;
        assert_eq!(
            combat.fire(
                &mut world,
                &mut inventory,
                &mut [],
                vec3(0.5, 66., 0.5),
                Vec3::Y,
                Mode::Survival,
                true
            ),
            FireResult::Idle
        );
        combat.fire(
            &mut world,
            &mut inventory,
            &mut [],
            vec3(0.5, 66., 0.5),
            Vec3::Y,
            Mode::Survival,
            false,
        );
        assert_eq!(
            combat.fire(
                &mut world,
                &mut inventory,
                &mut [],
                vec3(0.5, 66., 0.5),
                Vec3::Y,
                Mode::Survival,
                true
            ),
            FireResult::Fired(Item::Ak)
        );
        for _ in 0..30 {
            combat.tick_weapon(
                &mut inventory,
                Mode::Survival,
                1. / 60.,
                WeaponInput {
                    aiming: true,
                    sprinting: true,
                    movement: 1.,
                    ..Default::default()
                },
            );
        }
        assert!(combat.aim < 0.01 && combat.sprint > 0.99);
        combat.cooldown = 0.;
        assert_eq!(
            combat.fire(
                &mut world,
                &mut inventory,
                &mut [],
                vec3(0.5, 66., 0.5),
                Vec3::Y,
                Mode::Survival,
                true
            ),
            FireResult::Idle
        );
        assert_eq!(inventory.selected().unwrap().loaded, 28);
    }

    #[test]
    fn equipping_blocks_shots_and_reload_until_the_weapon_is_ready() {
        let mut world = World::new(7);
        let mut inventory = weapon_inventory(Item::Ak, Item::Bullet, 40);
        let mut combat = Combat::default();
        let eye = vec3(0.5, 66., 0.5);
        assert_eq!(
            combat.fire(
                &mut world,
                &mut inventory,
                &mut [],
                eye,
                Vec3::Y,
                Mode::Survival,
                true
            ),
            FireResult::Idle
        );
        assert!(combat.equip_time > 0.);
        inventory.slots[0].as_mut().unwrap().loaded = 10;
        assert!(!combat.start_reload(&inventory, Mode::Survival));
        finish_reload(&mut combat, &mut inventory, Mode::Survival);
        assert_eq!(
            combat.fire(
                &mut world,
                &mut inventory,
                &mut [],
                eye,
                Vec3::Y,
                Mode::Survival,
                true
            ),
            FireResult::Fired(Item::Ak)
        );
        assert_eq!(inventory.selected().unwrap().loaded, 9);
    }
}

#[cfg(test)]
mod material_shot_tests {
    use super::*;
    #[test]
    fn city_materials_break_at_city_heights_and_explosions_reach_loaded_buildings() {
        let states = &crate::city::get().unwrap().metadata.states;
        let mapped = |material: Block| {
            Block::Map(
                states
                    .iter()
                    .position(|s| s.material == material && s.full)
                    .unwrap() as u16,
            )
        };
        for weapon in crate::weapons::GUNS {
            let mut world = World::city(0);
            world.stream([29, -13], 1, 9);
            let pos = [476, 300, -200];
            for x in 475..=477 {
                for z in -204..=-198 {
                    for y in 300..=303 {
                        world.set([x, y, z], Block::Air);
                    }
                }
            }
            let glass = mapped(Block::Glass);
            world.set(pos, glass);
            let mut inventory = Inventory::new(Mode::Creative);
            inventory.slots[0] = Some(Stack {
                item: weapon,
                count: 1,
                loaded: weapon.magazine_capacity(),
                optic: 0,
            });
            let mut combat = Combat::default();
            combat.sync_equipped(&inventory);
            combat.equip_time = 0.;
            combat.trigger_down = false;
            assert_eq!(
                combat.fire(
                    &mut world,
                    &mut inventory,
                    &mut [],
                    vec3(476.5, 300.5, -202.5),
                    Vec3::Z,
                    Mode::Creative,
                    true
                ),
                FireResult::Fired(weapon)
            );
            if weapon == Item::RocketLauncher {
                let mut player = Player::new([476.5, 300., -202.5]);
                for _ in 0..20 {
                    combat.update(&mut world, &mut [], &mut player, Mode::Creative, 0.05);
                }
            }
            assert_eq!(
                world.get(pos),
                Block::Air,
                "{} must break imported glass",
                weapon.name()
            );
            if weapon != Item::RocketLauncher {
                world.set(pos, mapped(Block::Wood));
                combat = Combat::default();
                combat.sync_equipped(&inventory);
                combat.equip_time = 0.;
                combat.trigger_down = false;
                combat.fire(
                    &mut world,
                    &mut inventory,
                    &mut [],
                    vec3(476.5, 300.5, -202.5),
                    Vec3::Z,
                    Mode::Creative,
                    true,
                );
                assert_eq!(
                    world.get(pos) == Block::Air,
                    matches!(weapon, Item::M200 | Item::Aw50)
                );
            }
        }
    }
    use crate::game::Stack;
    fn fire_at(block: Block, weapon: Item) -> (World, Combat) {
        let mut world = World::new(7);
        world.stream([0, 0], 1, 9);
        for z in 0..=8 {
            for y in 60..=63 {
                world.set([0, y, z], Block::Air);
            }
        }
        world.set([0, 61, 4], block);
        world.set([0, 61, 6], Block::Stone);
        let mut inventory = Inventory::new(Mode::Creative);
        inventory.slots[0] = Some(Stack {
            item: weapon,
            count: 1,
            loaded: weapon.magazine_capacity(),
            optic: 0,
        });
        let mut combat = Combat::default();
        combat.sync_equipped(&inventory);
        combat.equip_time = 0.;
        combat.trigger_down = false;
        let mut player = Player::new([0.5, 60., 0.5]);
        assert_eq!(
            combat.fire(
                &mut world,
                &mut inventory,
                &mut [],
                vec3(0.5, 61.5, 0.5),
                Vec3::Z,
                Mode::Creative,
                true
            ),
            FireResult::Fired(weapon)
        );
        if weapon == Item::RocketLauncher {
            for _ in 0..20 {
                combat.update(&mut world, &mut [], &mut player, Mode::Creative, 0.05);
            }
        }
        (world, combat)
    }
    #[test]
    fn all_calibers_shatter_glass_and_heavy_rifles_break_wood_only_on_first_hit() {
        for weapon in crate::weapons::GUNS {
            let (world, combat) = fire_at(Block::Glass, weapon);
            assert_eq!(world.get([0, 61, 4]), Block::Air, "{} glass", weapon.name());
            assert_eq!(combat.shots, 1);
            if weapon != Item::RocketLauncher {
                assert_eq!(
                    world.get([0, 61, 6]),
                    Block::Stone,
                    "A bullet must stop on the first hit"
                );
                for block in [Block::Wood, Block::Planks, Block::Stone, Block::Bedrock] {
                    let (world, _) = fire_at(block, weapon);
                    let expected = if matches!(weapon, Item::M200 | Item::Aw50)
                        && matches!(block, Block::Wood | Block::Planks)
                    {
                        Block::Air
                    } else {
                        block
                    };
                    assert_eq!(
                        world.get([0, 61, 4]),
                        expected,
                        "{} vs {}",
                        weapon.name(),
                        block.name()
                    );
                    assert_eq!(world.get([0, 61, 6]), Block::Stone);
                }
            }
        }
    }
}
