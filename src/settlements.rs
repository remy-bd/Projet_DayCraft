//! Persistent storage, village population and transactional trades.
use crate::{
    drops::{DroppedItem, WorldExtras},
    game::{raycast, Inventory, Item, Mob, MobKind, Mode, Player, Stack},
    world::{Block, Pos, World},
};
use macroquad::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct Chest {
    pub position: Pos,
    pub inventory: Inventory,
}
pub fn stack(item: Item, count: u16) -> Stack {
    Stack {
        item,
        count,
        loaded: 0,
        optic: 0,
    }
}
pub fn chest_index(extras: &mut WorldExtras, world: &World, position: Pos) -> usize {
    if let Some(i) = extras.chests.iter().position(|c| c.position == position) {
        return i;
    }
    let mut inventory = Inventory {
        slots: vec![None; 27],
        selected: 0,
    };
    if world.generation == 4 {
        if let Some(container) = crate::city::get().expect("City map").container(position) {
            for &(slot, stack) in &container.items {
                if slot < 27 && stack.valid() {
                    inventory.slots[slot] = Some(stack);
                }
            }
            if container.loot && inventory.slots.iter().all(Option::is_none) {
                let n = world
                    .seed
                    .wrapping_add((position[0] as u64).wrapping_mul(31))
                    .wrapping_add((position[2] as u64).wrapping_mul(97));
                for (item, count) in [
                    (Item::Bread, 4),
                    (Item::IronIngot, 4),
                    (Item::Emerald, 2),
                    (Item::Seeds, 8),
                    (Item::Coal, 6),
                    (Item::Gunpowder, 4),
                    (Item::StonePick, 1),
                ] {
                    inventory.add(item, count);
                }
                if n.is_multiple_of(3) {
                    inventory.add(Item::Ak, 1);
                    inventory.add(Item::Bullet, 60);
                }
            }
        }
    }
    if world
        .village_at(position[0], position[2])
        .is_some_and(|v| v.chests().contains(&position))
    {
        let n = world
            .seed
            .wrapping_add((position[0] as u64).wrapping_mul(31))
            .wrapping_add((position[2] as u64).wrapping_mul(97));
        for (slot, item, count) in [
            (2, Item::Bread, 3 + (n % 4) as u16),
            (5, Item::IronIngot, 2 + (n % 3) as u16),
            (10, Item::Emerald, 1 + (n % 5) as u16),
            (14, Item::Seeds, 8),
            (19, Item::Coal, 4),
            (23, Item::StonePick, 1),
        ] {
            inventory.slots[slot] = Some(stack(item, count));
        }
    }
    extras.chests.push(Chest {
        position,
        inventory,
    });
    extras.chests.len() - 1
}
pub fn move_stack(from: &mut Inventory, to: &mut Inventory, index: usize) -> bool {
    if let Some(s) = from.slots.get(index).copied().flatten() {
        if to.return_stack(s) {
            from.slots[index] = None;
            return true;
        }
    }
    false
}
pub const TRADES: [(Item, u16, Item, u16); 4] = [
    (Item::Wheat, 20, Item::Emerald, 1),
    (Item::Emerald, 1, Item::Bread, 6),
    (Item::Emerald, 4, Item::IronIngot, 3),
    (Item::Emerald, 8, Item::Sword, 1),
];
pub fn trade(inventory: &mut Inventory, index: usize) -> bool {
    let Some(&(input, count, output, quantity)) = TRADES.get(index) else {
        return false;
    };
    let mut next = inventory.clone();
    if next.consume(input, count) && next.add(output, quantity) {
        *inventory = next;
        true
    } else {
        false
    }
}
pub fn spawn_villages(extras: &mut WorldExtras, mobs: &mut Vec<Mob>, world: &World, player: Vec3) {
    if world.generation == 4 {
        let city = crate::city::get().expect("City map");
        let center = [
            (player.x.floor() as i32).div_euclid(16),
            (player.z.floor() as i32).div_euclid(16),
        ];
        for dz in -3..=3 {
            for dx in -3..=3 {
                let key = [center[0] + dx, center[1] + dz];
                if !world.chunks.contains_key(&key)
                    || extras.city_populated.contains(&key)
                    || mobs.len() > 990
                {
                    continue;
                }
                if let Some(indices) = city.residents.get(&key) {
                    for &i in indices {
                        let r = &city.metadata.residents[i];
                        let mut p = Vec3::from_array(r.position);
                        let height = r.kind.height();
                        if let Some(y) =
                            world.walkable(p.x.floor() as i32, p.z.floor() as i32, p.y, height)
                        {
                            p.y = y;
                            if crate::game::collides(
                                world,
                                p,
                                if r.kind == MobKind::Golem { 0.55 } else { 0.32 },
                                height,
                            ) {
                                continue;
                            }
                            mobs.push(Mob {
                                position: p,
                                kind: r.kind,
                                health: if r.kind == MobKind::Golem {
                                    80.
                                } else if r.kind == MobKind::Villager || r.kind.hostile() {
                                    20.
                                } else {
                                    10.
                                },
                                phase: i as f32,
                                home: matches!(r.kind, MobKind::Villager | MobKind::Golem)
                                    .then_some(p.to_array()),
                                ..Default::default()
                            });
                        }
                    }
                }
                // Keep existing trading/defence available at the six entry districts.
                for (i, spawn) in city.metadata.spawns.iter().enumerate() {
                    let p = Vec3::from_array(spawn.position);
                    if [
                        (p.x.floor() as i32).div_euclid(16),
                        (p.z.floor() as i32).div_euclid(16),
                    ] != key
                    {
                        continue;
                    }
                    for (kind, offset) in [(MobKind::Villager, 3), (MobKind::Golem, -3)] {
                        let position = [offset, 5, 7, 9].into_iter().find_map(|d| {
                            [(d, 0), (0, d), (-d, 0), (0, -d)]
                                .into_iter()
                                .find_map(|(dx, dz)| {
                                    let (x, z) = (p.x.floor() as i32 + dx, p.z.floor() as i32 + dz);
                                    let y = world.walkable(x, z, p.y, kind.height())?;
                                    let position = vec3(x as f32 + 0.5, y, z as f32 + 0.5);
                                    (!crate::game::collides(
                                        world,
                                        position,
                                        if kind == MobKind::Golem { 0.55 } else { 0.32 },
                                        kind.height(),
                                    ))
                                    .then_some(position)
                                })
                        });
                        let position = position.or_else(|| {
                            (-4..=4).find_map(|dy| {
                                (-12i32..=12).find_map(|dz| {
                                    (-12i32..=12).find_map(|dx| {
                                        if dx.abs().max(dz.abs()) < 2 {
                                            return None;
                                        }
                                        let (x, z) =
                                            (p.x.floor() as i32 + dx, p.z.floor() as i32 + dz);
                                        let y =
                                            world.walkable(x, z, p.y + dy as f32, kind.height())?;
                                        let position = vec3(x as f32 + 0.5, y, z as f32 + 0.5);
                                        (!crate::game::collides(
                                            world,
                                            position,
                                            if kind == MobKind::Golem { 0.55 } else { 0.32 },
                                            kind.height(),
                                        ))
                                        .then_some(position)
                                    })
                                })
                            })
                        });
                        if let Some(position) = position {
                            mobs.push(Mob {
                                position,
                                kind,
                                health: if kind == MobKind::Golem { 80. } else { 20. },
                                home: Some(position.to_array()),
                                phase: i as f32,
                                ..Default::default()
                            });
                        }
                    }
                }
                extras.city_populated.push(key);
            }
        }
        return;
    }
    for v in world.nearby_villages([player.x as i32, player.z as i32], 90) {
        let home = vec3(
            v.center[0] as f32 + 0.5,
            v.center[1] as f32 + 1.,
            v.center[2] as f32 + 0.5,
        );
        if home.distance_squared(player) > 110_f32.powi(2)
            || extras.visited_villages.contains(&v.region)
            || mobs.len() > 1000
        {
            continue;
        }
        for (i, p) in v.residents().into_iter().enumerate() {
            let position = Vec3::from_array(p.map(|n| n as f32)) + vec3(0.5, 0., 0.5);
            mobs.push(Mob {
                position,
                kind: MobKind::Villager,
                health: 20.,
                phase: i as f32,
                home: Some(if v.legacy {
                    home.to_array()
                } else {
                    position.to_array()
                }),
                ..Default::default()
            });
        }
        for x in if v.legacy { vec![6.] } else { vec![-23., 23.] } {
            let mut position = home + vec3(x, 0., -4.);
            if v.terrain.is_some() {
                position.y = world.height_at(position.x.floor() as i32, position.z.floor() as i32)
                    as f32
                    + 1.;
            }
            mobs.push(Mob {
                position,
                kind: MobKind::Golem,
                health: 100.,
                home: Some(home.to_array()),
                ..Default::default()
            });
        }
        extras.visited_villages.push(v.region);
        for position in v.chests() {
            let c = chest_index(extras, world, position);
            if world.get(position) != Block::Chest {
                extras.chests[c].inventory.slots.fill(None);
            }
        }
    }
}
pub fn target_mob(world: &World, mobs: &[Mob], player: &Player) -> Option<usize> {
    let eye = player.eye();
    let dir = player.direction();
    let wall = raycast(world, eye, dir, 5.).map_or(5., |h| h.distance);
    mobs.iter()
        .enumerate()
        .filter(|(_, m)| m.health > 0.)
        .filter_map(|(i, m)| {
            let offset = m.position + Vec3::Y * m.kind.height() * 0.55 - eye;
            let along = offset.dot(dir);
            (along > 0. && along < wall && (offset - dir * along).length() < 0.65)
                .then_some((i, along))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
}
pub fn feed(mobs: &mut [Mob], index: usize, inventory: &mut Inventory, mode: Mode) -> bool {
    let Some(m) = mobs.get_mut(index) else {
        return false;
    };
    if !m.kind.animal()
        || m.love > 0.
        || m.breed_cooldown > 0.
        || m.baby > 0.
        || !inventory.selected().is_some_and(|s| s.item == Item::Wheat)
    {
        return false;
    }
    m.love = 30.;
    if mode == Mode::Survival {
        inventory.take_selected();
    }
    true
}
pub fn breed(mobs: &mut Vec<Mob>) {
    if mobs.len() >= 256 {
        return;
    }
    let mut babies = Vec::new();
    for i in 0..mobs.len() {
        if mobs[i].love <= 0. || !mobs[i].kind.animal() {
            continue;
        }
        for j in i + 1..mobs.len() {
            if mobs[j].kind == mobs[i].kind
                && mobs[j].love > 0.
                && mobs[j].position.distance_squared(mobs[i].position) < 2_f32.powi(2)
            {
                mobs[i].love = 0.;
                mobs[j].love = 0.;
                mobs[i].breed_cooldown = 180.;
                mobs[j].breed_cooldown = 180.;
                babies.push(Mob {
                    position: mobs[i].position,
                    kind: mobs[i].kind,
                    health: 10.,
                    baby: 180.,
                    ..Default::default()
                });
                break;
            }
        }
    }
    mobs.extend(babies);
}
fn loose(extras: &mut WorldExtras, stack: Stack, position: Vec3) {
    extras.drops.push(DroppedItem {
        stack,
        position: (position + Vec3::Y * 0.3).to_array(),
        velocity: [0.4, 2., 0.2],
        age: 0.,
    });
}
pub fn loot_dead(extras: &mut WorldExtras, mobs: &mut Vec<Mob>) {
    mobs.retain(|m| {
        if m.health > 0. {
            return true;
        }
        if extras.drops.len() > 1020 {
            return true;
        }
        if m.baby <= 0. {
            let loot = match m.kind {
                MobKind::Sheep => vec![(Item::Wool, 1), (Item::RawMutton, 2)],
                MobKind::Cow => vec![(Item::RawBeef, 3)],
                MobKind::Pig => vec![(Item::RawPork, 3)],
                MobKind::Zombie => vec![(Item::Coal, 1)],
                MobKind::Creeper if m.fuse < 1.5 => vec![(Item::Gunpowder, 2)],
                MobKind::Creeper => vec![],
                MobKind::Golem => vec![(Item::IronIngot, 3)],
                MobKind::Villager => vec![],
            };
            for (item, count) in loot {
                loose(extras, stack(item, count), m.position);
            }
        }
        false
    });
}
pub fn spill_broken_chests(extras: &mut WorldExtras, world: &World) {
    if world.generation == 4 {
        for c in &crate::city::get().expect("City map").metadata.containers {
            if world.edited(c.position) && world.get(c.position).material() != Block::Chest {
                chest_index(extras, world, c.position);
            }
        }
    }
    for i in 0..extras.chests.len() {
        if world.get(extras.chests[i].position).material() == Block::Chest {
            continue;
        }
        for j in 0..27 {
            if extras.drops.len() >= 1024 {
                return;
            }
            if let Some(s) = extras.chests[i].inventory.slots[j].take() {
                let p = Vec3::from_array(extras.chests[i].position.map(|v| v as f32))
                    + Vec3::splat(0.5);
                loose(extras, s, p);
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn city_population_is_safe_and_created_once_in_each_starting_district() {
        for i in 0..6 {
            let mut world = World::city(i);
            let p = Vec3::from_array(world.spawn());
            world.stream(
                [
                    (p.x.floor() as i32).div_euclid(16),
                    (p.z.floor() as i32).div_euclid(16),
                ],
                1,
                9,
            );
            let mut extras = WorldExtras::default();
            let mut mobs = Vec::new();
            spawn_villages(&mut extras, &mut mobs, &world, p);
            for kind in [MobKind::Villager, MobKind::Golem] {
                assert!(
                    mobs.iter().any(|m| m.kind == kind),
                    "District {i} missing {}",
                    kind.name()
                );
            }
            for mob in &mobs {
                assert!(!crate::game::collides(
                    &world,
                    mob.position,
                    if mob.kind == MobKind::Golem {
                        0.55
                    } else {
                        0.32
                    },
                    mob.kind.height()
                ));
            }
            let count = mobs.len();
            spawn_villages(&mut extras, &mut mobs, &world, p);
            assert_eq!(count, mobs.len());
        }
    }
    #[test]
    fn village_loot_population_and_mobs_survive_save_without_respawning() {
        let world = World::new(64195485);
        let v = world
            .nearby_villages([0, 0], 1000)
            .into_iter()
            .next()
            .unwrap();
        let mut extras = WorldExtras::default();
        let mut mobs = Vec::new();
        let position = Vec3::from_array(v.center.map(|p| p as f32)) + Vec3::Y;
        spawn_villages(&mut extras, &mut mobs, &world, position);
        let population = mobs.len();
        assert!(population >= 6);
        spawn_villages(&mut extras, &mut mobs, &world, position);
        assert_eq!(mobs.len(), population);
        assert!(mobs.iter().any(|m| m.kind == MobKind::Golem));
        let c = chest_index(&mut extras, &world, v.chests()[0]);
        assert!(extras.chests[c].inventory.count(Item::Emerald) > 0);
        extras.chests[c].inventory.slots.fill(None);
        extras.mobs = Some(mobs.clone());
        let path =
            std::env::temp_dir().join(format!("voxel-village-test-{}.json", std::process::id()));
        let player = Player::new(position.to_array());
        let inventory = Inventory::new(Mode::Survival);
        crate::game::save_game(
            &path,
            &world,
            &player,
            &inventory,
            0.2,
            Mode::Survival,
            &extras,
        )
        .unwrap();
        let (world, _, _, _, _, mut restored) = crate::game::load_game(&path).unwrap();
        assert_eq!(restored.mobs.as_ref().unwrap().len(), population);
        let c = chest_index(&mut restored, &world, v.chests()[0]);
        assert!(restored.chests[c]
            .inventory
            .slots
            .iter()
            .all(Option::is_none));
        let _ = std::fs::remove_file(path);
    }
    #[test]
    fn golems_defend_villagers_and_creepers_fuse_only_with_a_clear_path() {
        let mut world = World::new(14);
        for x in -4..=4 {
            for z in -4..=4 {
                for y in 69..=76 {
                    world.set([x, y, z], if y == 69 { Block::Stone } else { Block::Air });
                }
            }
        }
        let mut player = Player::new([0.5, 70., 3.]);
        let mut mobs = vec![
            Mob {
                kind: MobKind::Golem,
                position: vec3(0.5, 70., 0.5),
                health: 100.,
                ..Default::default()
            },
            Mob {
                kind: MobKind::Zombie,
                position: vec3(1.5, 70., 0.5),
                health: 20.,
                ..Default::default()
            },
        ];
        crate::game::update_mobs(&mut mobs, &world, &mut player, 0.1, true, Mode::Creative);
        assert!(mobs[1].health < 20.);
        mobs = vec![Mob {
            kind: MobKind::Creeper,
            position: vec3(0.5, 70., 1.),
            health: 20.,
            ..Default::default()
        }];
        world.set([0, 71, 2], Block::Stone);
        for _ in 0..20 {
            assert!(crate::game::update_mobs(
                &mut mobs,
                &world,
                &mut player,
                0.1,
                true,
                Mode::Survival
            )
            .is_empty());
        }
        assert_eq!(mobs[0].fuse, 0.);
        world.set([0, 71, 2], Block::Air);
        let mut explosions = 0;
        for _ in 0..20 {
            explosions +=
                crate::game::update_mobs(&mut mobs, &world, &mut player, 0.1, true, Mode::Survival)
                    .len();
        }
        assert_eq!(explosions, 1);
    }
    #[test]
    fn trading_and_container_transfers_are_atomic_and_keep_weapon_metadata() {
        let mut bag = Inventory::new(Mode::Survival);
        bag.add(Item::Wheat, 20);
        assert!(trade(&mut bag, 0));
        assert_eq!(bag.count(Item::Wheat), 0);
        assert_eq!(bag.count(Item::Emerald), 1);
        let before = bag.slots.clone();
        assert!(!trade(&mut bag, 3));
        assert_eq!(bag.slots, before);
        let gun = Stack {
            item: Item::M200,
            count: 1,
            loaded: 3,
            optic: 2,
        };
        bag.slots[0] = Some(gun);
        let mut chest = Inventory {
            slots: vec![Some(stack(Item::Block(Block::Stone), 64)); 27],
            selected: 0,
        };
        assert!(!move_stack(&mut bag, &mut chest, 0));
        assert_eq!(bag.slots[0], Some(gun));
        chest.slots[4] = None;
        assert!(move_stack(&mut bag, &mut chest, 0));
        assert_eq!(chest.slots[4], Some(gun));
    }
    #[test]
    fn breeding_requires_two_fed_adults_and_has_a_cooldown() {
        let mut mobs = vec![
            Mob {
                position: Vec3::ZERO,
                love: 30.,
                kind: MobKind::Cow,
                ..Default::default()
            },
            Mob {
                position: Vec3::X,
                love: 30.,
                kind: MobKind::Cow,
                ..Default::default()
            },
        ];
        breed(&mut mobs);
        assert_eq!(mobs.len(), 3);
        assert!(mobs[2].baby > 0.);
        breed(&mut mobs);
        assert_eq!(mobs.len(), 3);
    }
}

#[cfg(test)]
mod pursuit_tests {
    use super::*;
    #[test]
    fn zombies_catch_villagers_and_golems_catch_and_kill_the_zombies() {
        let mut world = World::new(14);
        for x in -40..=40 {
            for z in -3..=3 {
                for y in 69..=74 {
                    world.set(
                        [x, y, z],
                        if y == 69 || y == 73 {
                            Block::Stone
                        } else {
                            Block::Air
                        },
                    );
                }
            }
        }
        let mut player = Player::new([0.5, 70., 2.5]);
        let mut mobs = vec![
            Mob {
                kind: MobKind::Villager,
                position: vec3(0.5, 70., 0.5),
                health: 20.,
                ..Default::default()
            },
            Mob {
                kind: MobKind::Zombie,
                position: vec3(6.5, 70., 0.5),
                health: 20.,
                ..Default::default()
            },
        ];
        for _ in 0..100 {
            crate::game::update_mobs(&mut mobs, &world, &mut player, 0.1, false, Mode::Creative);
        }
        assert!(
            mobs[0].health < 20.,
            "Zombie never caught the fleeing villager"
        );
        mobs.push(Mob {
            kind: MobKind::Golem,
            position: mobs[1].position + Vec3::X * 6.,
            health: 100.,
            ..Default::default()
        });
        for _ in 0..120 {
            crate::game::update_mobs(&mut mobs, &world, &mut player, 0.1, false, Mode::Creative);
        }
        assert!(mobs[1].health <= 0., "Golem never stopped the zombie");
    }
}
