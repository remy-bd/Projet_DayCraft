//! Persistent loose stacks. Transfers commit only after the destination accepts them.
use crate::game::{raycast, Inventory, Player, Stack};
use crate::world::World;
use macroquad::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct DroppedItem {
    pub stack: Stack,
    pub position: [f32; 3],
    pub velocity: [f32; 3],
    pub age: f32,
}
#[derive(Default, Clone, Serialize, Deserialize)]
pub struct WorldExtras {
    #[serde(default)]
    pub grenades: Vec<crate::explosives::Grenade>,
    #[serde(default)]
    pub charges: Vec<crate::explosives::Charge>,
    pub drops: Vec<DroppedItem>,
    pub spawn: Option<[f32; 3]>,
    #[serde(default)]
    pub chests: Vec<crate::settlements::Chest>,
    #[serde(default)]
    pub visited_villages: Vec<[i32; 2]>,
    #[serde(default)]
    pub city_populated: Vec<crate::world::ChunkKey>,
    #[serde(default)]
    pub mobs: Option<Vec<crate::game::Mob>>,
}
impl WorldExtras {
    pub fn toss(&mut self, stack: Stack, player: &Player, world: &World) -> bool {
        if self.drops.len() >= 1024 || !stack.valid() {
            return false;
        }
        let dir = player.direction();
        let start = player.eye();
        let position = if raycast(world, start, dir, 0.6).is_none() {
            start + dir * 0.5
        } else {
            start
        };
        self.drops.push(DroppedItem {
            stack,
            position: position.to_array(),
            velocity: (dir * 3.5 + Vec3::Y * 1.5).to_array(),
            age: 0.,
        });
        true
    }
    pub fn update(
        &mut self,
        world: &World,
        player: &Player,
        inventory: &mut Inventory,
        dt: f32,
    ) -> usize {
        if !dt.is_finite() || dt < 0. {
            return 0;
        }
        let mut picked = 0;
        let dt = dt.clamp(0., 0.1);
        self.drops.retain_mut(|drop| {
            let mut p = Vec3::from_array(drop.position);
            let mut v = Vec3::from_array(drop.velocity);
            drop.age = (drop.age + dt).min(86400.);
            // Freeze outside loaded terrain; unloading a chunk must never lose loot.
            if world.chunks.contains_key(&[
                (p.x.floor() as i32).div_euclid(16),
                (p.z.floor() as i32).div_euclid(16),
            ]) {
                for _ in 0..4 {
                    v.y = (v.y - 12. * dt / 4.).max(-12.);
                    for axis in 0..3 {
                        let mut next = p;
                        next[axis] += v[axis] * dt / 4.;
                        let blocked =
                            crate::game::collides(world, next - Vec3::Y * 0.12, 0.12, 0.24);
                        if blocked {
                            v[axis] = 0.;
                            if axis == 1 {
                                v.x *= 0.7;
                                v.z *= 0.7;
                            }
                        } else {
                            p = next;
                        }
                    }
                }
            }
            drop.position = p.to_array();
            drop.velocity = v.to_array();
            let target = player.position + Vec3::Y * 0.8;
            let delta = target - p;
            if drop.age >= 1.0
                && delta.length() < 1.65
                && raycast(world, p, delta, delta.length()).is_none()
                && inventory.return_stack(drop.stack)
            {
                picked += 1;
                false
            } else {
                true
            }
        });
        picked
    }
    pub fn valid(&self) -> bool {
        let finite = |p: &[f32; 3]| p.iter().all(|v| v.is_finite() && v.abs() <= 1_000_000.);
        self.grenades.len() <= 32
            && self.grenades.iter().all(crate::explosives::Grenade::valid)
            && self.charges.len() <= 64
            && self.charges.iter().all(crate::explosives::Charge::valid)
            && self.chests.len() <= 8192
            && self.chests.iter().all(|c| {
                c.position[0].unsigned_abs() <= 1_000_000
                    && c.position[2].unsigned_abs() <= 1_000_000
                    && (1..crate::city::HEIGHT).contains(&c.position[1])
                    && c.inventory.slots.len() == 27
                    && c.inventory.selected == 0
                    && c.inventory.slots.iter().flatten().all(|s| s.valid())
            })
            && self.chests.iter().enumerate().all(|(i, c)| {
                !self.chests[..i]
                    .iter()
                    .any(|other| other.position == c.position)
            })
            && self.visited_villages.len() <= 8192
            && self.city_populated.len() <= 69344
            && self
                .city_populated
                .iter()
                .all(|p| p.iter().all(|v| v.unsigned_abs() <= 6251))
            && self
                .visited_villages
                .iter()
                .all(|p| p.iter().all(|v| v.unsigned_abs() <= 6251))
            && self.mobs.as_ref().is_none_or(|mobs| {
                mobs.len() <= 1024
                    && mobs.iter().all(|m| {
                        finite(&m.position.to_array())
                            && m.home.as_ref().is_none_or(finite)
                            && [
                                m.health,
                                m.phase,
                                m.attack_timer,
                                m.yaw,
                                m.love,
                                m.baby,
                                m.breed_cooldown,
                                m.fuse,
                            ]
                            .iter()
                            .all(|v| v.is_finite() && v.abs() < 1_000_000.)
                    })
            })
            && self.drops.len() <= 1024
            && self.spawn.as_ref().is_none_or(finite)
            && self.drops.iter().all(|d| {
                finite(&d.position)
                    && finite(&d.velocity)
                    && d.age.is_finite()
                    && (0. ..=86400.).contains(&d.age)
                    && d.stack.valid()
            })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{Item, Mode};
    use crate::world::Block;
    #[test]
    fn dropped_items_collide_and_cannot_be_picked_through_walls() {
        let mut world = World::new(7);
        for x in -2..=2 {
            for z in -2..=2 {
                for y in 69..=74 {
                    world.set(
                        [x, y, z],
                        if y == 69 || x == 1 {
                            Block::Stone
                        } else {
                            Block::Air
                        },
                    );
                }
            }
        }
        let mut player = Player::new([0.4, 70., 0.5]);
        let mut inventory = Inventory::new(Mode::Creative);
        let stack = Stack {
            item: Item::Ammo408,
            count: 12,
            loaded: 0,
            optic: 0,
        };
        let mut extras = WorldExtras {
            drops: vec![DroppedItem {
                stack,
                position: [2.25, 70.8, 0.5],
                velocity: [-8., 0., 0.],
                age: 2.,
            }],
            spawn: None,
            ..Default::default()
        };
        for _ in 0..120 {
            assert_eq!(extras.update(&world, &player, &mut inventory, 1. / 60.), 0);
        }
        assert!(extras.drops[0].position[0] >= 2.12);
        assert!(extras.drops[0].position[1] >= 70.12);
        assert!(extras.valid());
        let position = extras.drops[0].position;
        assert_eq!(extras.update(&world, &player, &mut inventory, f32::NAN), 0);
        assert_eq!(extras.drops[0].position, position);
        player.position = vec3(2.5, 70., 0.5);
        assert_eq!(extras.update(&world, &player, &mut inventory, 0.), 1);
        assert_eq!(inventory.count(Item::Ammo408), 12);
    }
    #[test]
    fn drops_keep_ammo_and_optics_and_wait_for_release() {
        let mut world = World::new(7);
        world.stream([0, 0], 1, 20);
        let player = Player::new([0.5, 70., 0.5]);
        let mut inventory = Inventory::new(Mode::Creative);
        let stack = Stack {
            item: Item::M200,
            count: 1,
            loaded: 4,
            optic: 2,
        };
        let mut extras = WorldExtras::default();
        assert!(extras.toss(stack, &player, &world));
        extras.drops[0].position = (player.position + Vec3::Y * 0.8).to_array();
        extras.drops[0].velocity = [0.; 3];
        assert_eq!(extras.update(&world, &player, &mut inventory, 0.), 0);
        extras.drops[0].age = 2.;
        assert_eq!(extras.update(&world, &player, &mut inventory, 0.), 1);
        assert_eq!(inventory.selected(), Some(stack));
        assert!(extras.toss(stack, &player, &world));
        extras.drops[0].position = (player.position + Vec3::Y * 0.8).to_array();
        extras.drops[0].age = 2.;
        inventory.slots.fill(Some(Stack {
            item: Item::Block(Block::Stone),
            count: 64,
            loaded: 0,
            optic: 0,
        }));
        assert_eq!(extras.update(&world, &player, &mut inventory, 0.), 0);
        assert_eq!(extras.drops[0].stack, stack);
        let restored: WorldExtras =
            serde_json::from_str(&serde_json::to_string(&extras).unwrap()).unwrap();
        assert!(restored.valid());
        assert_eq!(restored.drops[0].stack, stack);
    }
}
