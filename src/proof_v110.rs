//! Native end-to-end checks use isolated saves and the actual menu hit boxes.
use crate::*;
impl App {
    pub(crate) fn v111_grenade_check(&mut self, frame: u32) {
        if frame == 0 {
            self.prepare_station_demo();
            self.combat = Combat::default();
            self.world.stream([0, 0], 1, 9);
            for x in 0..10 {
                for z in 0..8 {
                    for y in 40..50 {
                        self.world
                            .set([x, y, z], if y == 40 { Block::Stone } else { Block::Air });
                    }
                }
            }
            self.mobs = [3.5, 5.5, 7.5]
                .map(|x| Mob {
                    position: vec3(x, 41., 3.5),
                    kind: MobKind::Villager,
                    health: 20.,
                    ..Default::default()
                })
                .to_vec();
            self.player.position = vec3(1., 42., 10.);
            self.player.pitch = -0.25;
            self.mode = Mode::Creative;
            self.inventory.add(Item::HandGrenade, 1);
        }
        if frame == 140 {
            self.combat.grenades.push(explosives::Grenade {
                position: vec3(0.5, 41.101, 3.5),
                velocity: Vec3::ZERO,
                age: 3.199,
            });
            assert_eq!(
                self.combat.update_grenades(
                    &mut self.world,
                    &mut self.mobs,
                    &mut self.player,
                    self.mode,
                    0.01
                ),
                1
            );
            assert_eq!(self.mobs[0].health, 0.);
            assert!(self.mobs[1].health > 0. && self.mobs[1].health < 20.);
            assert_eq!(self.mobs[2].health, 20.);
            assert_eq!(self.combat.blasts[0].radius, 2.7);
        }
        if frame > 140 {
            self.combat.update(
                &mut self.world,
                &mut self.mobs,
                &mut self.player,
                self.mode,
                0.02,
            );
        }
    }
    pub(crate) fn v110_check(&mut self, frame: u32) {
        self.ui = UiInput::default();
        match frame {
            0 => {
                self.prepare_weapon_demo();
                self.mobs.clear();
                self.inventory = Inventory::new(Mode::Survival);
                self.player.position = vec3(0.5, 41., 4.5);
                self.player.plate = None;
                self.player.helmet = None;
                self.combat = Combat::default();
                self.extras = Default::default();
                self.enter(Screen::Playing);
                assert!(self.inventory.slots.iter().all(Option::is_none));
            }
            1 => {
                self.handle_menu_keys(false, true);
                assert_eq!(self.screen, Screen::Inventory);
            }
            2 => {
                assert_eq!(self.screen, Screen::Inventory);
                assert!(self.inventory.add(Item::Block(Block::Wood), 4));
            }
            3 => {
                self.ui = UiInput {
                    mouse: vec2(1140., 230.),
                    left: true,
                    right: false,
                }
            }
            4 => {
                assert_eq!(self.inventory.count(Item::Block(Block::Workbench)), 1);
                assert_eq!(self.inventory.count(Item::Block(Block::Wood)), 0);
                assert!(self.close_inventory());
                assert!(self.world.set([0, 41, 2], Block::Workbench));
                self.inventory.take_selected();
                self.inventory.add(Item::Block(Block::Wood), 1);
                self.active_station = Some([0, 41, 2]);
                self.enter(Screen::Crafting);
            }
            5 => {
                self.ui = UiInput {
                    mouse: vec2(1140., 230.),
                    left: true,
                    right: false,
                }
            }
            6 => {
                assert_eq!(self.inventory.count(Item::Block(Block::Planks)), 4);
                self.close_inventory();
                self.handle_menu_keys(false, true);
            }
            7 => {
                self.held = Some(Stack {
                    item: Item::PlateCarrier(5),
                    count: 1,
                    loaded: 0,
                    optic: 0,
                });
                self.ui = UiInput {
                    mouse: vec2(180., 240.),
                    left: true,
                    right: false,
                };
            }
            8 => {
                assert_eq!(self.player.plate, Some(5));
                assert!(self.held.is_none());
                self.held = Some(Stack {
                    item: Item::BallisticHelmet(5),
                    count: 1,
                    loaded: 0,
                    optic: 0,
                });
                self.ui = UiInput {
                    mouse: vec2(500., 240.),
                    left: true,
                    right: false,
                };
            }
            9 => {
                assert_eq!(self.player.helmet, Some(5));
                assert!(self.held.is_none());
                self.player.combat_damage(8.);
                assert_eq!(self.player.health, 18.);
            }
            10 => {
                self.close_inventory();
                self.world.set([2, 41, 2], Block::Furnace);
                for raw in [Item::RawBeef, Item::RawMutton, Item::RawPork] {
                    self.inventory.add(raw, 3);
                }
                self.inventory.add(Item::Coal, 3);
                for cooked in [Item::CookedBeef, Item::CookedMutton, Item::CookedPork] {
                    let r = recipes()
                        .into_iter()
                        .find(|r| r.result.0 == cooked)
                        .unwrap();
                    assert!(
                        craft_at(&mut self.inventory, &r, &self.world, &self.player, None).is_err()
                    );
                    assert!(craft_at(
                        &mut self.inventory,
                        &r,
                        &self.world,
                        &self.player,
                        Some([2, 41, 2])
                    )
                    .is_ok());
                    assert_eq!(self.inventory.count(cooked), 3);
                }
            }
            20 => {
                self.mode = Mode::Creative;
                self.inventory.slots.fill(None);
                self.handle_creative_key();
                self.inventory_tab = 3;
                self.catalog_page = 0;
                assert_eq!(self.screen, Screen::Creative);
            }
            21 => {
                self.ui = UiInput {
                    mouse: vec2(654., 378.),
                    left: true,
                    right: false,
                }
            }
            22 => {
                assert_eq!(self.catalog_page, 1);
                self.ui = UiInput {
                    mouse: vec2(125., 215.),
                    left: true,
                    right: false,
                };
            }
            23 => {
                assert_eq!(self.held.unwrap().item, Item::PlateCarrier(4));
                self.ui = UiInput {
                    mouse: vec2(125., 445.),
                    left: true,
                    right: false,
                };
            }
            24 => {
                assert_eq!(self.inventory.count(Item::PlateCarrier(4)), 1);
                self.ui = UiInput {
                    mouse: vec2(350., 690.),
                    left: true,
                    right: false,
                };
            }
            25 => {
                assert_eq!(self.inventory.count(Item::PlateCarrier(4)), 0);
                assert!(catalog(3).contains(&Item::PlateCarrier(4)));
                self.close_inventory();
            }
            30 => {
                self.mode = Mode::Survival;
                self.inventory.slots.fill(None);
                self.inventory.add(Item::C4, 1);
                self.player.yaw = 0.;
                self.player.pitch = 0.;
                self.world.set([0, 42, 2], Block::Stone);
                assert!(self
                    .combat
                    .deploy(
                        &self.world,
                        &self.player,
                        &mut self.inventory,
                        &mut self.extras,
                        self.mode
                    )
                    .is_ok());
                assert!(self.inventory.selected().is_none());
                assert_eq!(self.extras.charges.len(), 1);
                assert!(self.save());
                self.load();
                assert_eq!(self.extras.charges.len(), 1);
                assert_eq!(self.player.plate, Some(5));
                assert_eq!(self.player.helmet, Some(5));
            }
            31 => {
                assert!(self.remote_available());
                assert_eq!(
                    self.combat.detonate_charges(
                        &mut self.world,
                        &mut self.mobs,
                        &mut self.player,
                        &mut self.extras,
                        self.mode
                    ),
                    1
                );
                assert!(self.extras.charges.is_empty());
                assert_eq!(self.world.get([0, 42, 2]), Block::Air);
            }
            40 => {
                self.inventory.add(Item::HandGrenade, 1);
                self.combat.throw_time = 0.;
                assert!(self
                    .combat
                    .deploy(
                        &self.world,
                        &self.player,
                        &mut self.inventory,
                        &mut self.extras,
                        self.mode
                    )
                    .is_ok());
                assert!(self.inventory.selected().is_none());
                self.combat.update_grenades(
                    &mut self.world,
                    &mut self.mobs,
                    &mut self.player,
                    self.mode,
                    1.,
                );
                self.enter(Screen::Paused);
                assert!(self.save());
                self.load();
                assert_eq!(self.combat.grenades.len(), 1);
                assert!((self.combat.grenades[0].age - 1.).abs() < 0.001);
                self.enter(Screen::Paused);
            }
            41..=49 => {
                assert_eq!(self.screen, Screen::Paused);
                assert!((self.combat.grenades[0].age - 1.).abs() < 0.001);
            }
            50 => {
                self.enter(Screen::Playing);
                self.mode = Mode::Creative;
                self.player.position = vec3(0.5, 42., 8.5);
                self.player.pitch = -0.20;
            }
            51..=175 => {
                self.combat.update_grenades(
                    &mut self.world,
                    &mut self.mobs,
                    &mut self.player,
                    self.mode,
                    0.02,
                );
                self.combat.update(
                    &mut self.world,
                    &mut self.mobs,
                    &mut self.player,
                    self.mode,
                    0.02,
                );
            }
            176 => {
                assert!(self.combat.grenades.is_empty());
                assert_eq!(self.combat.explosions, 1);
                assert!(self.combat.blasts.iter().any(|b| b.radius == 2.7));
            }
            _ => {}
        }
    }
}
