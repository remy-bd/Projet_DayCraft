#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod audio;
mod city;
mod combat;
mod controls;
mod drops;
#[cfg(test)]
mod equipment_tests;
mod explosives;
mod game;
mod graphics;
mod item_icons;
mod mob_assets;
mod mob_models;
mod proof_v110;
mod render;
mod settlements;
mod shadows;
mod sky;
mod viewmodel;
mod villages;
mod weapon_audio;
mod weapons;
mod world;

use audio::{Audio, Effect, WeaponSound};
use combat::{Combat, FireMode, FireResult, WeaponInput};
use controls::{
    default_bindings, key_name, rebind, slot_key, Action, FocusKeys, Preferences, ACTIONS,
};
use game::*;
use macroquad::prelude::*;
use render::Renderer;
use std::path::PathBuf;
use world::{Block, World, CHUNK};

const INK: Color = Color::new(0.055, 0.085, 0.12, 1.0);
const PANEL: Color = Color::new(0.065, 0.105, 0.14, 0.96);
const MUTED: Color = Color::new(0.58, 0.68, 0.70, 1.0);
const ACCENT: Color = Color::new(0.70, 0.91, 0.37, 1.0);
const PAPER: Color = Color::new(0.94, 0.96, 0.88, 1.0);

fn advance_day_cycle(time: f32, dt: f32) -> f32 {
    let duration = if time > 0.53 && time < 0.97 {
        600.
    } else {
        1200.
    };
    (time + dt / duration).rem_euclid(1.)
}

fn window_conf() -> macroquad::conf::Conf {
    let args: Vec<String> = std::env::args().collect();
    let dimensions = args
        .iter()
        .position(|s| s == "--size")
        .and_then(|i| args.get(i + 1))
        .and_then(|s| s.split_once('x'))
        .and_then(|(w, h)| Some((w.parse::<i32>().ok()?, h.parse::<i32>().ok()?)))
        .filter(|(w, h)| (640..=3840).contains(w) && (400..=2160).contains(h))
        .unwrap_or((1280, 800));
    macroquad::conf::Conf {
        miniquad_conf: Conf {
            window_title: "VOXEL v1.13 — Apocalypse City".into(),
            window_width: dimensions.0,
            window_height: dimensions.1,
            sample_count: 4,
            window_resizable: true,
            ..Default::default()
        },
        draw_call_vertex_capacity: 65536,
        draw_call_index_capacity: 131072,
        ..Default::default()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Screen {
    Title,
    Playing,
    Inventory,
    Creative,
    Crafting,
    Paused,
    Controls,
    Dead,
}

#[derive(Clone, Copy, Default)]
struct UiInput {
    mouse: Vec2,
    left: bool,
    right: bool,
}

#[derive(Default)]
struct MiningLatch {
    armed: bool,
}

impl MiningLatch {
    fn held(&mut self, playing: bool, event_down: bool, physical_down: bool) -> bool {
        if !playing {
            self.armed = false;
            return false;
        }
        if !physical_down {
            self.armed = true;
        }
        self.armed && event_down && physical_down
    }
}

fn menu_shortcut(screen: Screen, escape: bool, inventory: bool) -> Option<Screen> {
    match screen {
        Screen::Playing if escape => Some(Screen::Paused),
        Screen::Playing if inventory => Some(Screen::Inventory),
        Screen::Paused if escape => Some(Screen::Playing),
        Screen::Controls if escape => Some(Screen::Paused),
        Screen::Inventory | Screen::Creative | Screen::Crafting if escape || inventory => {
            Some(Screen::Playing)
        }
        _ => None,
    }
}

struct App {
    world: World,
    player: Player,
    inventory: Inventory,
    extras: drops::WorldExtras,
    inventory_tab: usize,
    catalog_page: usize,
    active_station: Option<world::Pos>,
    active_chest: Option<world::Pos>,
    trading: bool,
    mobs: Vec<Mob>,
    combat: Combat,
    renderer: Renderer,
    viewmodel: viewmodel::ViewModel,
    audio: Audio,
    screen: Screen,
    mode: Mode,
    title_mode: Mode,
    time: f32,
    radius: i32,
    azerty: bool,
    sensitivity: f32,
    selected_spawn: u8,
    debug: bool,
    hints: bool,
    fullscreen: bool,
    toast: String,
    toast_timer: f32,
    mine_target: Option<world::Pos>,
    mine_progress: f32,
    action_timer: f32,
    held: Option<Stack>,
    recipe_page: usize,
    save_path: PathBuf,
    autosave: f32,
    spawn_timer: f32,
    village_hint: Option<villages::Village>,
    village_hint_timer: f32,
    step_timer: f32,
    has_game: bool,
    ui: UiInput,
    mining: MiningLatch,
    fluid_timer: f32,
    farm_timer: f32,
    bindings: [KeyCode; 17],
    preferences_path: PathBuf,
    invert_y: bool,
    toggle_sprint: bool,
    sprint_latched: bool,
    rebinding: Option<Action>,
    camera_mouse: Option<Vec2>,
    camera_size: Vec2,
    placing: MiningLatch,
    place_timer: f32,
    tooltip: Option<(String, String)>,
    hover_key: String,
    hover_time: f32,
    focus_keys: FocusKeys,
}

impl App {
    fn scripted_city_check(&mut self, frame: u32) {
        if frame == 0 {
            assert_eq!(self.world.generation, 4);
            assert_eq!(self.world.height(), 384);
            assert_eq!(self.player.position.to_array(), self.world.spawn());
            assert!(self.inventory.slots.iter().all(Option::is_none));
            assert!(!collides(&self.world, self.player.position, 0.3, 1.8));
            assert!(city_catalog(0).len() > 400);
        }
        if frame == 30 {
            for i in 0..6 {
                self.ui = UiInput {
                    left: true,
                    mouse: vec2(70. + (i % 2) as f32 * 235., 490. + (i / 2) as f32 * 44.),
                    ..Default::default()
                };
                self.draw_title();
                assert_eq!(self.selected_spawn, i as u8);
            }
            self.selected_spawn = self.world.city_spawn;
            self.ui = UiInput::default();
        }
        if frame == 60 {
            settlements::spawn_villages(
                &mut self.extras,
                &mut self.mobs,
                &self.world,
                self.player.position,
            );
            let population = self.mobs.len();
            settlements::spawn_villages(
                &mut self.extras,
                &mut self.mobs,
                &self.world,
                self.player.position,
            );
            assert_eq!(self.mobs.len(), population);
            assert!(!self.extras.city_populated.is_empty());
            assert!(self
                .world
                .nearest_village(
                    [self.player.position.x as i32, self.player.position.z as i32],
                    768
                )
                .is_none());
            assert!(self.save());
            let (world, player, inventory, _, _, extras) = load_game(&self.save_path).unwrap();
            assert_eq!(world.city_spawn, self.selected_spawn);
            assert_eq!(world.spawn(), self.world.spawn());
            assert_eq!(player.position, self.player.position);
            assert_eq!(inventory.slots, self.inventory.slots);
            assert_eq!(extras.city_populated, self.extras.city_populated);
        }
        if frame == 90 {
            let old_screen = self.screen;
            self.ui = UiInput {
                left: true,
                mouse: vec2(460., 442.),
                ..Default::default()
            };
            self.draw_dead();
            assert_eq!(self.player.position.to_array(), self.world.spawn());
            self.player.yaw =
                city::get().unwrap().metadata.spawns[self.selected_spawn as usize].yaw;
            self.screen = old_screen;
            self.ui = UiInput::default();
            self.toast_timer = 0.;
            set_cursor_grab(false);
            show_mouse(true);
        }
    }
    fn new(save_path: PathBuf, audio: Audio) -> Self {
        let preferences_path = save_path.with_file_name("settings.json");
        let world = World::city(0);
        let mut player = Player::new(world.spawn());
        player.yaw = city::get().expect("City map").metadata.spawns[0].yaw;
        player.pitch = -0.18;
        let mobs = initial_mobs(&world);
        Self {
            world,
            player,
            mobs,
            combat: Combat::default(),
            inventory: Inventory::new(Mode::Creative),
            extras: Default::default(),
            inventory_tab: 0,
            catalog_page: 0,
            active_station: None,
            active_chest: None,
            trading: false,
            renderer: Renderer::new(),
            viewmodel: viewmodel::ViewModel::new(),
            audio,
            screen: Screen::Title,
            mode: Mode::Creative,
            title_mode: Mode::Creative,
            time: 0.22,
            radius: 6,
            azerty: true,
            sensitivity: 0.0025,
            selected_spawn: 0,
            debug: false,
            hints: true,
            fullscreen: false,
            toast: String::new(),
            toast_timer: 0.0,
            mine_target: None,
            mine_progress: 0.0,
            action_timer: 0.0,
            held: None,
            recipe_page: 0,
            save_path,
            autosave: 0.0,
            spawn_timer: 0.0,
            village_hint: None,
            village_hint_timer: 0.0,
            step_timer: 0.0,
            has_game: false,
            ui: UiInput::default(),
            mining: MiningLatch::default(),
            fluid_timer: 0.0,
            farm_timer: 0.0,
            bindings: default_bindings(true),
            preferences_path,
            invert_y: false,
            toggle_sprint: false,
            sprint_latched: false,
            rebinding: None,
            camera_mouse: None,
            camera_size: Vec2::ZERO,
            placing: MiningLatch::default(),
            place_timer: 0.0,
            tooltip: None,
            hover_key: String::new(),
            hover_time: 0.0,
            focus_keys: FocusKeys::default(),
        }
    }

    fn message(&mut self, message: impl Into<String>) {
        self.toast = message.into();
        self.toast_timer = 4.0;
    }

    fn enter(&mut self, screen: Screen) {
        self.audio.play(Effect::Click);
        self.screen = screen;
        self.mine_progress = 0.0;
        self.mine_target = None;
        self.ui.left = false;
        self.ui.right = false;
        self.combat.cancel_trigger();
        self.mining.armed = false;
        self.placing.armed = false;
        self.place_timer = 0.0;
        self.sprint_latched = false;
        self.camera_mouse = None;
        self.rebinding = None;
        self.hover_key.clear();
        self.hover_time = 0.0;
        self.tooltip = None;
        set_cursor_grab(screen == Screen::Playing);
        show_mouse(screen != Screen::Playing);
    }

    fn new_game(&mut self) {
        if self.has_game && !self.save() {
            return;
        }
        self.mode = self.title_mode;
        self.world = World::city(self.selected_spawn);
        self.village_hint = None;
        self.village_hint_timer = 0.;
        self.player = Player::new(self.world.spawn());
        self.player.yaw =
            city::get().expect("City map").metadata.spawns[self.selected_spawn as usize].yaw;
        self.player.pitch = -0.16;
        self.inventory = Inventory::new(self.mode);
        self.extras = Default::default();
        self.active_chest = None;
        self.active_station = None;
        self.trading = false;
        self.mobs = initial_mobs(&self.world);
        self.combat = Combat::default();
        let enhanced = self.renderer.enhanced;
        self.renderer = Renderer::new();
        self.renderer.enhanced = enhanced;
        self.farm_timer = 0.0;
        self.time = 0.22;
        self.held = None;
        self.has_game = true;
        self.autosave = 0.0;
        self.enter(Screen::Playing);
        self.message("Bienvenue ! E : sac · C : créatif · Clic droit établi : craft");
    }

    fn save(&mut self) -> bool {
        if !self.has_game {
            return true;
        }
        self.extras.mobs = Some(self.mobs.clone());
        self.extras.grenades = self.combat.grenades.clone();
        match save_game(
            &self.save_path,
            &self.world,
            &self.player,
            &self.inventory,
            self.time,
            self.mode,
            &self.extras,
        ) {
            Ok(()) => {
                self.message("Monde sauvegardé");
                self.autosave = 0.0;
                true
            }
            Err(error) => {
                self.message(format!("Sauvegarde impossible : {error}"));
                false
            }
        }
    }

    fn load(&mut self) {
        match load_game(&self.save_path) {
            Ok((world, player, inventory, time, mode, extras)) => {
                self.selected_spawn = world.city_spawn;
                self.world = world;
                self.village_hint = None;
                self.village_hint_timer = 0.;
                self.player = player;
                self.inventory = inventory;
                self.extras = extras;
                self.time = time;
                self.mode = mode;
                self.title_mode = mode;
                self.mobs = self
                    .extras
                    .mobs
                    .take()
                    .unwrap_or_else(|| initial_mobs(&self.world));
                self.active_chest = None;
                self.trading = false;
                self.combat = Combat::default();
                self.combat.grenades = std::mem::take(&mut self.extras.grenades);
                self.active_station = None;
                let enhanced = self.renderer.enhanced;
                self.renderer = Renderer::new();
                self.renderer.enhanced = enhanced;
                self.farm_timer = 0.0;
                self.held = None;
                self.has_game = true;
                self.autosave = 0.0;
                self.enter(if self.player.health > 0.0 {
                    Screen::Playing
                } else {
                    Screen::Dead
                });
                self.message("Votre monde a été restauré");
            }
            Err(error) => self.message(format!("Chargement impossible : {error}")),
        }
    }

    fn close_inventory(&mut self) -> bool {
        if let Some(stack) = self.held {
            if !self.inventory.return_stack(stack) {
                self.message("Replacez l'objet tenu dans une case libre");
                return false;
            }
            self.held = None;
        }
        self.active_chest = None;
        self.active_station = None;
        self.trading = false;
        self.enter(Screen::Playing);
        true
    }

    fn update(&mut self, dt: f32) {
        let focused = window_focused();
        self.ui = UiInput {
            mouse: ui_mouse(),
            left: is_mouse_button_pressed(MouseButton::Left) && focused,
            right: is_mouse_button_pressed(MouseButton::Right) && focused,
        };
        self.toast_timer = (self.toast_timer - dt).max(0.0);
        self.action_timer = (self.action_timer - dt).max(0.0);
        self.place_timer = (self.place_timer - dt).max(0.0);
        if !focused {
            self.focus_keys.lost_focus(get_keys_down());
            if self.screen == Screen::Playing {
                self.enter(Screen::Paused);
            }
            return;
        }
        self.focus_keys
            .fresh_events(get_keys_pressed().into_iter().chain(get_keys_released()));
        if self.screen == Screen::Controls {
            if let Some(action) = self.rebinding {
                if is_key_pressed(KeyCode::Escape) {
                    self.rebinding = None;
                } else if let Some(key) = get_keys_pressed().into_iter().min_by_key(|k| *k as u32) {
                    if rebind(&mut self.bindings, action, key) {
                        self.rebinding = None;
                        self.persist_preferences();
                    } else {
                        self.message("Touche réservée : choisissez une lettre ou une autre touche");
                    }
                }
                return;
            }
        }
        if is_key_pressed(KeyCode::F11) {
            self.fullscreen = !self.fullscreen;
            miniquad::window::set_fullscreen(self.fullscreen);
            self.camera_mouse = None;
        }
        if is_key_pressed(KeyCode::F1) {
            self.hints = !self.hints;
            self.persist_preferences();
        }
        if is_key_pressed(KeyCode::F3) {
            self.debug = !self.debug;
        }
        if self.handle_menu_keys(
            is_key_pressed(KeyCode::Escape),
            self.pressed(Action::Inventory),
        ) {
            return;
        }
        if self.pressed(Action::Creative) {
            self.handle_creative_key();
            return;
        }
        if self.has_game
            && matches!(self.screen, Screen::Playing | Screen::Paused)
            && is_key_pressed(KeyCode::F5)
        {
            self.save();
        }
        if self.screen != Screen::Playing {
            return;
        }
        if self.mode == Mode::Creative && self.pressed(Action::Fly) {
            self.player.flying = !self.player.flying;
            self.player.velocity = Vec3::ZERO;
        }
        let size = vec2(screen_width(), screen_height());
        let mouse = Vec2::from(mouse_position());
        let delta = camera_delta(&mut self.camera_mouse, &mut self.camera_size, mouse, size);
        let sensitivity = self.sensitivity / (1. + self.combat.aim * (self.combat.zoom - 1.));
        self.player.yaw += delta.x * sensitivity;
        self.player.pitch = (self.player.pitch
            - delta.y * sensitivity * if self.invert_y { -1.0 } else { 1.0 })
        .clamp(-1.54, 1.54);
        if self.toggle_sprint && self.pressed(Action::Sprint) {
            self.sprint_latched = !self.sprint_latched;
        }
        let input = Movement {
            forward: self.down(Action::Forward) as i32 as f32
                - self.down(Action::Backward) as i32 as f32,
            right: self.down(Action::Right) as i32 as f32 - self.down(Action::Left) as i32 as f32,
            jump: self.down(Action::Jump),
            jump_pressed: self.pressed(Action::Jump),
            sprint: if self.toggle_sprint {
                self.sprint_latched
            } else {
                self.down(Action::Sprint)
            },
            sneak: self.down(Action::Sneak),
            descend: self.down(Action::Sneak),
        };
        let old_health = self.player.health;
        self.player.update(&self.world, input, dt, self.mode);
        self.step_timer = (self.step_timer - dt).max(0.0);
        if self.player.grounded
            && (input.forward != 0.0 || input.right != 0.0)
            && self.step_timer == 0.0
        {
            self.audio.play(Effect::Step);
            self.step_timer = if input.sprint { 0.26 } else { 0.40 };
        }
        self.time = advance_day_cycle(self.time, dt);
        let night = self.time > 0.53 && self.time < 0.97;
        settlements::spawn_villages(
            &mut self.extras,
            &mut self.mobs,
            &self.world,
            self.player.position,
        );
        self.village_hint_timer = (self.village_hint_timer - dt).max(0.);
        if self.village_hint_timer == 0. {
            self.village_hint = self.world.nearest_village(
                [self.player.position.x as i32, self.player.position.z as i32],
                768,
            );
            self.village_hint_timer = 2.;
        }
        let explosions = update_mobs(
            &mut self.mobs,
            &self.world,
            &mut self.player,
            dt,
            night,
            self.mode,
        );
        for at in explosions {
            self.combat.explode(
                &mut self.world,
                &mut self.mobs,
                &mut self.player,
                self.mode,
                at,
            );
            self.audio.play(Effect::Explosion);
        }
        if self.combat.update(
            &mut self.world,
            &mut self.mobs,
            &mut self.player,
            self.mode,
            dt,
        ) > 0
        {
            self.audio.play(Effect::Explosion);
        }
        if self.combat.update_grenades(
            &mut self.world,
            &mut self.mobs,
            &mut self.player,
            self.mode,
            dt,
        ) > 0
        {
            self.audio.play(Effect::Explosion);
        }
        if self.player.health < old_health {
            self.audio.play(Effect::Hurt);
        }
        settlements::loot_dead(&mut self.extras, &mut self.mobs);
        settlements::spill_broken_chests(&mut self.extras, &self.world);
        self.mobs.retain(|m| {
            m.home.is_some()
                || m.health <= 0.
                || m.position.distance_squared(self.player.position) < 120_f32.powi(2)
        });
        self.spawn_timer = (self.spawn_timer - dt).max(0.0);
        let wild_mobs = self
            .mobs
            .iter()
            .filter(|m| {
                m.home.is_none()
                    && m.health > 0.
                    && m.position.distance_squared(self.player.position) < 100_f32.powi(2)
            })
            .count();
        if wild_mobs < 10 && self.mobs.len() < 1024 && self.spawn_timer == 0.0 {
            let p = self.player.position;
            let a = get_time() as f32 * 0.13;
            let x = p.x + a.cos() * 24.0;
            let z = p.z + a.sin() * 24.0;
            let ix = x.floor() as i32;
            let iz = z.floor() as i32;
            let y = self.world.walkable(ix, iz, p.y, 1.8).unwrap_or(-1.);
            if y > 0.
                && self.world.get([ix, y as i32, iz]) == Block::Air
                && self.world.get([ix, y as i32 + 1, iz]) == Block::Air
            {
                self.mobs.push(Mob {
                    position: vec3(x, y, z),
                    kind: if night {
                        if (a * 10.) as i32 % 2 == 0 {
                            MobKind::Creeper
                        } else {
                            MobKind::Zombie
                        }
                    } else {
                        match (a * 5.) as i32 % 3 {
                            0 => MobKind::Sheep,
                            1 => MobKind::Cow,
                            _ => MobKind::Pig,
                        }
                    },
                    health: if night { 20. } else { 10. },
                    phase: a,
                    attack_timer: 1.0,
                    ..Default::default()
                });
            }
            self.spawn_timer = 2.0;
        }
        self.select_hotbar();
        let armed = selected_weapon(&self.inventory).is_some();
        let explosive = self
            .inventory
            .selected()
            .is_some_and(|s| matches!(s.item, Item::HandGrenade | Item::C4));
        let reloads = self.combat.reloads;
        let old_reload = self.combat.reload_progress();
        let old_bolt = self.combat.bolt_time;
        let placement = self.placing.held(
            true,
            is_mouse_button_down(MouseButton::Right),
            physical_mouse_down(MouseButton::Right),
        );
        self.combat.tick_weapon(
            &mut self.inventory,
            self.mode,
            dt,
            WeaponInput {
                aiming: armed && placement,
                sprinting: armed && input.sprint && (input.forward != 0. || input.right != 0.),
                movement: if input.forward != 0. || input.right != 0. {
                    1.
                } else {
                    0.
                },
                sway: delta * 0.0025,
            },
        );
        if self.remote_available() && self.pressed(Action::Reload) {
            let count = self.combat.detonate_charges(
                &mut self.world,
                &mut self.mobs,
                &mut self.player,
                &mut self.extras,
                self.mode,
            );
            if count > 0 {
                self.audio.play(Effect::C4Explosion);
                self.message("Charges déclenchées");
            } else {
                self.message("Aucune charge chargée à moins de 128 blocs");
            }
        }
        if armed
            && self.pressed(Action::Reload)
            && self.combat.start_reload(&self.inventory, self.mode)
        {
            self.weapon_sound(WeaponSound::Open);
        }
        if old_reload.is_some_and(|p| p < 0.65)
            && self.combat.reload_progress().is_some_and(|p| p >= 0.65)
        {
            self.weapon_sound(WeaponSound::Insert);
        }
        if self.combat.reloads > reloads {
            self.weapon_sound(WeaponSound::Bolt);
        }
        if let Some(weapon) = selected_weapon(&self.inventory) {
            let spec = weapons::spec(weapon).unwrap();
            if spec.bolt
                && old_bolt > spec.interval * 0.72
                && self.combat.bolt_time <= spec.interval * 0.72
            {
                self.weapon_sound(WeaponSound::Bolt);
            }
            if self.pressed(Action::FireMode) {
                self.combat.cycle_mode(weapon);
                self.audio.play(Effect::Click);
            }
            if self.pressed(Action::Optic) {
                self.cycle_optic();
            }
        }
        if self.pressed(Action::Drop) {
            self.drop_stack(self.input_down(KeyCode::LeftControl));
        }
        if self
            .extras
            .update(&self.world, &self.player, &mut self.inventory, dt)
            > 0
        {
            self.audio.play(Effect::Click);
        }
        if armed && self.pressed(Action::Inspect) {
            self.combat.inspect();
        }
        let target = raycast(&self.world, self.player.eye(), self.player.direction(), 6.0);
        if (is_mouse_button_pressed(MouseButton::Middle) || self.pressed(Action::PickBlock))
            && self.mode == Mode::Creative
        {
            if let Some(hit) =
                raycast_with_fluids(&self.world, self.player.eye(), self.player.direction(), 6.0)
            {
                let block = self.world.get(hit.block);
                self.inventory.slots[self.inventory.selected] = Some(Stack {
                    item: Item::Block(block),
                    count: 64,
                    loaded: 0,
                    optic: 0,
                });
            }
        }
        let mining = self.mining.held(
            true,
            is_mouse_button_down(MouseButton::Left),
            physical_mouse_down(MouseButton::Left),
        );
        if selected_weapon(&self.inventory).is_some() {
            self.mine(None, dt, false);
            self.fire_weapon(mining);
        } else if self
            .inventory
            .selected()
            .is_some_and(|s| matches!(s.item, Item::HandGrenade | Item::C4))
        {
            self.mine(None, dt, false);
        } else {
            self.mine(target.as_ref(), dt, mining);
        }
        if self.mode == Mode::Creative && self.pressed(Action::RemoveFluid) {
            if let Some(hit) =
                raycast_with_fluids(&self.world, self.player.eye(), self.player.direction(), 6.0)
            {
                if matches!(self.world.get(hit.block), Block::Water | Block::Lava) {
                    let source = self.world.is_fluid_source(hit.block);
                    self.world.set(hit.block, Block::Air);
                    self.audio.play(Effect::Break);
                    self.message(if source {
                        "Source retirée : l'écoulement se résorbe"
                    } else {
                        "Écoulement retiré : une source active peut l'alimenter"
                    });
                }
            }
        }
        if self
            .inventory
            .selected()
            .is_some_and(|s| matches!(s.item, Item::HandGrenade | Item::C4))
            && self.ui.right
        {
            let deploying = self.inventory.selected().unwrap().item;
            match self.combat.deploy(
                &self.world,
                &self.player,
                &mut self.inventory,
                &mut self.extras,
                self.mode,
            ) {
                Ok(()) => {
                    self.audio.play(Effect::Place);
                    self.message(if deploying == Item::C4 {
                        "C4 posé · R déclenche les charges"
                    } else {
                        "Grenade lancée · explosion dans 3,2 secondes"
                    });
                }
                Err(message) => self.message(message),
            }
        }
        if !armed && !explosive && self.ui.right && self.player.equip_selected(&mut self.inventory)
        {
            self.audio.play(Effect::Craft);
            self.message("Équipement balistique porté");
            return;
        }
        if !armed && !explosive && self.ui.right && !self.down(Action::Sneak) {
            if let Some(i) = settlements::target_mob(&self.world, &self.mobs, &self.player) {
                if self.mobs[i].kind == MobKind::Villager {
                    self.trading = true;
                    self.active_chest = None;
                    self.enter(Screen::Inventory);
                    return;
                }
                if settlements::feed(&mut self.mobs, i, &mut self.inventory, self.mode) {
                    self.audio.play(Effect::Craft);
                    self.message("Animal nourri : rapprochez deux adultes pour un petit");
                    return;
                }
            }
            if self.use_bucket() {
                return;
            }
        }
        if !armed && !explosive && (self.ui.right || (placement && self.place_timer <= 0.0)) {
            self.place_timer = if self.ui.right { 0.30 } else { 0.16 };
            if let Some(ref hit) = target {
                let target_block = self.world.get(hit.block).material();
                if target_block == Block::Chest && !self.down(Action::Sneak) {
                    settlements::chest_index(&mut self.extras, &self.world, hit.block);
                    self.active_chest = Some(hit.block);
                    self.trading = false;
                    self.enter(Screen::Inventory);
                    return;
                } else if matches!(target_block, Block::Door | Block::OpenDoor)
                    && !self.down(Action::Sneak)
                {
                    if self.ui.right {
                        let block = self.world.get(hit.block);
                        if let Block::Map(id) = block {
                            let state = city::state(id).expect("Map door");
                            if let Some(next) = state.open_variant {
                                if target_block == Block::Door
                                    || !player_intersects_block(&self.player, hit.block)
                                {
                                    let other =
                                        [hit.block[0], hit.block[1] + state.partner, hit.block[2]];
                                    self.world.set(hit.block, Block::Map(next));
                                    if state.partner != 0 {
                                        if let Block::Map(partner) = self.world.get(other) {
                                            if let Some(next) = city::state(partner)
                                                .filter(|s| s.source == state.source)
                                                .and_then(|s| s.open_variant)
                                            {
                                                self.world.set(other, Block::Map(next));
                                            }
                                        }
                                    }
                                    self.audio.play(Effect::Click);
                                }
                            }
                        } else if target_block == Block::Door
                            || (!player_intersects_block(&self.player, hit.block)
                                && !player_intersects_block(
                                    &self.player,
                                    [hit.block[0], hit.block[1] + 1, hit.block[2]],
                                ))
                        {
                            self.world.set(
                                hit.block,
                                if target_block == Block::Door {
                                    Block::OpenDoor
                                } else {
                                    Block::Door
                                },
                            );
                            self.audio.play(Effect::Click);
                        }
                    }
                } else if target_block == Block::Bed && !self.down(Action::Sneak) {
                    self.extras.spawn = Some([
                        hit.block[0] as f32 + 0.5,
                        hit.block[1] as f32 + 1.1,
                        hit.block[2] as f32 + 0.5,
                    ]);
                    if night {
                        self.time = 0.05;
                        self.player.health = 20.;
                        self.message("Bonne nuit : point de retour enregistré");
                    } else {
                        self.message("Point de retour enregistré dans ce lit");
                    }
                } else if matches!(target_block, Block::Workbench | Block::Furnace)
                    && !self.down(Action::Sneak)
                {
                    self.active_station = Some(hit.block);
                    self.recipe_page = 0;
                    self.enter(Screen::Crafting);
                    return;
                } else if let Some(stack) = self.inventory.selected() {
                    if let Some(result) =
                        farm_use(&mut self.world, &mut self.inventory, hit, self.mode)
                    {
                        match result {
                            Ok(message) => {
                                self.audio.play(Effect::Place);
                                self.message(message);
                            }
                            Err(message) => {
                                if self.ui.right {
                                    self.message(message);
                                }
                            }
                        }
                    } else if let Some(block) = stack.item.block() {
                        let at = hit.previous;
                        let pair = if let Block::Map(id) = block {
                            city::state(id).and_then(|s| {
                                s.partner_state
                                    .map(|id| ([at[0], at[1] + s.partner, at[2]], id))
                            })
                        } else {
                            None
                        };
                        if at[1] > 0
                            && at[1] < self.world.height()
                            && !player_intersects_block(&self.player, at)
                            && (!self.world.get(at).solid())
                            && (block != Block::Door
                                || (at[1] + 1 < self.world.height()
                                    && self.world.get([at[0], at[1] + 1, at[2]]) == Block::Air
                                    && !player_intersects_block(
                                        &self.player,
                                        [at[0], at[1] + 1, at[2]],
                                    )))
                            && pair.is_none_or(|(p, _)| {
                                p[1] > 0
                                    && p[1] < self.world.height()
                                    && self.world.get(p) == Block::Air
                                    && !player_intersects_block(&self.player, p)
                            })
                            && self.world.set(at, block)
                        {
                            if let Some((p, id)) = pair {
                                self.world.set(p, Block::Map(id));
                            }
                            self.audio.play(Effect::Place);
                            if self.mode == Mode::Survival {
                                self.inventory.take_selected();
                            }
                        }
                    } else if eat_selected(&mut self.inventory, &mut self.player, self.mode) {
                        self.message("Repas : nourriture et vie restaurées");
                    }
                }
            } else if eat_selected(&mut self.inventory, &mut self.player, self.mode) {
                self.message("Repas : nourriture et vie restaurées");
            }
        }
        if self.player.health <= 0.0 {
            self.enter(Screen::Dead);
        }
        self.autosave += dt;
        if self.autosave >= 60.0 {
            self.save();
        }
    }

    fn remote_available(&self) -> bool {
        self.inventory
            .selected()
            .is_some_and(|s| s.item == Item::C4)
            || (selected_weapon(&self.inventory).is_none() && !self.extras.charges.is_empty())
    }

    fn fire_weapon(&mut self, held: bool) {
        if self.screen != Screen::Playing {
            return;
        }
        let eye = self.player.eye();
        let direction = self.combat.view_direction(self.player.direction());
        let muzzle = self
            .inventory
            .selected()
            .filter(|s| weapons::spec(s.item).is_some())
            .map_or(eye, |s| {
                viewmodel::world_muzzle(s.item, &self.combat, get_time() as f32, eye, direction)
            });
        let (origin, direction) =
            self.combat
                .shot_ray(&self.world, &self.mobs, eye, direction, muzzle);
        match self.combat.fire(
            &mut self.world,
            &mut self.inventory,
            &mut self.mobs,
            origin,
            direction,
            self.mode,
            held,
        ) {
            FireResult::Fired(item) => self.audio.weapon(item, WeaponSound::Shot),
            FireResult::Reloading => self.weapon_sound(WeaponSound::Open),
            FireResult::Empty if self.action_timer <= 0.0 => {
                self.audio.play(Effect::DryFire);
                self.message("Plus de munitions : fabriquez-en près d'un établi");
                self.action_timer = 0.5;
            }
            _ => {}
        }
    }

    fn update_fluids(&mut self, dt: f32) {
        if self.screen != Screen::Playing {
            return;
        }
        self.fluid_timer += dt;
        if self.fluid_timer >= 0.10 {
            self.fluid_timer %= 0.10;
            self.world.tick_fluids(512);
        }
        self.farm_timer += dt;
        if self.farm_timer >= 20.0 {
            self.farm_timer %= 20.0;
            self.world.grow_crops(self.time < 0.5 || self.time > 0.98);
        }
    }

    fn mine(&mut self, hit: Option<&RayHit>, dt: f32, active: bool) {
        if !active {
            self.mine_progress = 0.0;
            self.mine_target = None;
            return;
        }
        let mut mob_target = None;
        let mut closest = hit.map_or(4.5, |h| h.distance.min(4.5));
        for (i, mob) in self.mobs.iter().enumerate() {
            let offset = mob.position + vec3(0.0, 0.8, 0.0) - self.player.eye();
            let along = offset.dot(self.player.direction());
            if along > 0.0
                && along < closest
                && (offset - self.player.direction() * along).length() < 0.7
            {
                mob_target = Some(i);
                closest = along;
            }
        }
        if let Some(i) = mob_target {
            self.mine_progress = 0.0;
            self.mine_target = None;
            if self.action_timer <= 0.0 {
                let damage = if self
                    .inventory
                    .selected()
                    .is_some_and(|s| s.item == Item::Sword)
                {
                    6.0
                } else {
                    2.0
                };
                self.mobs[i].health -= damage;
                self.audio.play(Effect::Hurt);
                self.action_timer = 0.38;
            }
            return;
        }
        let Some(hit) = hit else {
            self.mine_progress = 0.0;
            return;
        };
        let block = self.world.get(hit.block);
        if matches!(block, Block::Bedrock | Block::Water | Block::Lava) {
            return;
        }
        if self.mine_target != Some(hit.block) {
            self.mine_target = Some(hit.block);
            self.mine_progress = 0.0;
        }
        let tool = self.inventory.selected().map(|s| s.item);
        let duration = if self.mode == Mode::Creative {
            0.16
        } else {
            block.hardness() / mining_speed(tool, block)
        };
        self.mine_progress += dt / duration.max(0.06);
        if self.mine_progress >= 1.0 {
            if block.crop_stage().is_some() {
                match harvest_crop(&mut self.world, &mut self.inventory, hit.block, self.mode) {
                    Ok(_) => self.audio.play(Effect::Break),
                    Err(message) => self.message(message),
                }
                self.mine_progress = 0.0;
                return;
            }
            if block.material() == Block::Chest {
                settlements::chest_index(&mut self.extras, &self.world, hit.block);
            }
            let drop = drop_for(block);
            if self.mode == Mode::Survival
                && can_harvest(tool, block)
                && !collect_block(&mut self.inventory, &self.world, hit.block, drop)
            {
                self.message("Inventaire plein");
                self.mine_progress = 0.0;
                return;
            }
            if self.world.set(hit.block, Block::Air) {
                self.audio.play(Effect::Break);
                if self.mode == Mode::Survival
                    && block.material() == Block::Leaves
                    && macroquad::rand::gen_range(0, 5) == 0
                {
                    self.inventory.add(Item::Apple, 1);
                }
                self.mine_progress = 0.0;
            }
        }
    }

    fn draw_title(&mut self) -> bool {
        draw_rectangle(
            0.0,
            0.0,
            1280.0,
            800.0,
            Color::new(0.025, 0.065, 0.095, 0.42),
        );
        draw_rectangle(0.0, 0.0, 590.0, 800.0, Color::new(0.025, 0.06, 0.085, 0.86));
        text("APOCALYPSE CITY", 62.0, 94.0, 17.0, ACCENT);
        text("VOXEL", 57.0, 184.0, 86.0, PAPER);
        text("/ RUST", 357.0, 184.0, 30.0, ACCENT);
        text("Explorez. Construisez. Survivez.", 62.0, 226.0, 25.0, PAPER);
        text(
            "Explorez la ville et ses environs.",
            62.0,
            260.0,
            18.0,
            MUTED,
        );
        text("VOTRE AVENTURE", 62.0, 321.0, 15.0, MUTED);
        if self.button(
            Rect::new(62.0, 339.0, 221.0, 62.0),
            "CRÉATIF",
            self.title_mode == Mode::Creative,
        ) {
            self.title_mode = Mode::Creative;
        }
        if self.button(
            Rect::new(298.0, 339.0, 221.0, 62.0),
            "SURVIE",
            self.title_mode == Mode::Survival,
        ) {
            self.title_mode = Mode::Survival;
        }
        text(
            if self.title_mode == Mode::Creative {
                "Blocs illimités · vol libre · construction"
            } else {
                "Ressources · outils · faim · créatures nocturnes"
            },
            62.0,
            429.0,
            17.0,
            MUTED,
        );
        text("CHOISIR LE POINT DE DÉPART", 62., 465., 14., MUTED);
        for i in 0..6 {
            let spawn = &city::get().expect("City map").metadata.spawns[i];
            let rect = Rect::new(
                62. + (i % 2) as f32 * 235.,
                480. + (i / 2) as f32 * 44.,
                222.,
                38.,
            );
            if self.button(
                rect,
                &format!("{} · {}", i + 1, spawn.name),
                self.selected_spawn as usize == i,
            ) {
                self.selected_spawn = i as u8;
            }
            self.tip(rect, &spawn.name, &spawn.description);
        }
        if self.button(
            Rect::new(62.0, 626.0, 457.0, 51.0),
            "EXPLORER LA VILLE  →",
            true,
        ) {
            self.new_game();
        }
        let can_continue = self.has_game || self.save_path.exists();
        if can_continue
            && self.button(
                Rect::new(62.0, 688.0, 457.0, 40.0),
                "REPRENDRE LA PARTIE",
                false,
            )
        {
            if self.has_game {
                self.enter(Screen::Playing);
            } else {
                self.load();
            }
        }
        text(
            "APOCALYPSE CITY v1.32  /  6 DÉPARTS  /  SOLO",
            62.0,
            748.0,
            13.0,
            MUTED,
        );
        draw_rectangle(925.0, 52.0, 294.0, 95.0, Color::new(0.04, 0.09, 0.11, 0.70));
        text("LE TERRAIN DE JEU", 945.0, 80.0, 13.0, ACCENT);
        text("Une ville à explorer", 945.0, 109.0, 20.0, PAPER);
        text("La map originale, bloc par bloc", 945.0, 131.0, 14.0, MUTED);
        self.button(Rect::new(1125.0, 730.0, 94.0, 39.0), "QUITTER", false)
    }

    fn draw_hud(&self, hit: Option<&RayHit>) {
        if self.hints {
            if let Some(i) = settlements::target_mob(&self.world, &self.mobs, &self.player) {
                let mob = &self.mobs[i];
                let help = match mob.kind {
                    MobKind::Villager => "Clic droit : échanger des émeraudes",
                    MobKind::Sheep | MobKind::Cow | MobKind::Pig => {
                        "Blé + clic droit : nourrir · deux adultes nourris : un petit"
                    }
                    MobKind::Golem => "Protège les villageois des zombies et des creepers",
                    MobKind::Creeper => "Éloignez-vous : il explose après 1,5 seconde près de vous",
                    MobKind::Zombie => "Hostile la nuit · brûle au soleil",
                };
                let label = format!("{} · {}", mob.kind.name(), help);
                text(
                    &label,
                    640. - measure_text(&label, None, 15, 1.).width * 0.5,
                    498.,
                    15.,
                    ACCENT,
                );
            }
        }
        if self
            .inventory
            .selected()
            .is_some_and(|s| weapons::spec(s.item).is_some_and(|w| w.scoped))
            && self.combat.aim > 0.85
        {
            let center = vec2(640., 400.);
            let radius = 300.;
            for i in 0..128 {
                let a = i as f32 * std::f32::consts::TAU / 128.;
                let b = (i + 1) as f32 * std::f32::consts::TAU / 128.;
                let (u, v) = (vec2(a.cos(), a.sin()), vec2(b.cos(), b.sin()));
                draw_triangle(
                    center + u * radius,
                    center + u * 1800.,
                    center + v * 1800.,
                    BLACK,
                );
                draw_triangle(
                    center + u * radius,
                    center + v * 1800.,
                    center + v * radius,
                    BLACK,
                );
            }
            draw_poly_lines(
                640.,
                400.,
                128,
                radius,
                0.,
                7.,
                Color::new(0.06, 0.07, 0.08, 1.),
            );
            draw_line(340., 400., 940., 400., 1., BLACK);
            draw_line(640., 100., 640., 700., 1., BLACK);
            for distance in [-180., -120., -60., 60., 120., 180.] {
                draw_line(634., 400. + distance, 646., 400. + distance, 1., BLACK);
                draw_line(640. + distance, 394., 640. + distance, 406., 1., BLACK);
            }
            draw_circle(640., 400., 2., Color::new(0.88, 0.13, 0.10, 1.));
            text(
                &format!(
                    "×{}",
                    weapons::magnification(self.inventory.selected().unwrap().optic)
                ),
                875.,
                664.,
                20.,
                PAPER,
            );
        }
        draw_rectangle(25.0, 24.0, 233.0, 69.0, Color::new(0.04, 0.08, 0.10, 0.70));
        draw_rectangle(
            1030.0,
            24.0,
            225.0,
            69.0,
            Color::new(0.04, 0.08, 0.10, 0.70),
        );
        text("VOXEL / 1.13", 41.0, 51.0, 21.0, PAPER);
        text(
            if self.mode == Mode::Creative {
                "CRÉATIF"
            } else {
                "SURVIE"
            },
            41.0,
            77.0,
            13.0,
            ACCENT,
        );
        let biome = self
            .world
            .biome_at(self.player.position.x as i32, self.player.position.z as i32);
        text(
            biome,
            1160.0 - measure_text(biome, None, 18, 1.0).width,
            46.0,
            18.0,
            PAPER,
        );
        let hours = ((self.time * 24.0 + 6.0) % 24.0) as i32;
        let clock = format!(
            "{hours:02} h  ·  JOUR {}",
            if self.time > 0.53 && self.time < 0.97 {
                "/ NUIT"
            } else {
                "/ LUMIÈRE"
            }
        );
        text(&clock, 1060.0, 71.0, 13.0, MUTED);
        if let Some(v) = self.village_hint {
            let delta = vec2(
                v.center[0] as f32 - self.player.position.x,
                v.center[2] as f32 - self.player.position.z,
            );
            let bearing = delta.x.atan2(-delta.y);
            let relative = (bearing - self.player.yaw + std::f32::consts::PI)
                .rem_euclid(std::f32::consts::TAU)
                - std::f32::consts::PI;
            let direction = if relative.abs() < 0.35 {
                "tout droit"
            } else if relative.abs() > 2.5 {
                "derrière"
            } else if relative > 0. {
                "à droite"
            } else {
                "à gauche"
            };
            let label = if delta.length() <= v.radius() as f32 {
                "Vous êtes dans un village".to_owned()
            } else {
                format!("Village · {} blocs · {direction}", delta.length() as i32)
            };
            draw_rectangle(400., 24., 480., 42., Color::new(0.04, 0.08, 0.10, 0.75));
            text(
                &label,
                640. - measure_text(&label, None, 17, 1.).width * 0.5,
                51.,
                17.,
                ACCENT,
            );
        }
        if selected_weapon(&self.inventory).is_some() {
            if self.combat.aim < 0.8 {
                let gap = 5. + self.combat.bloom * 3. + self.player.velocity.length().min(8.) * 0.7;
                for axis in [Vec2::X, -Vec2::X, Vec2::Y, -Vec2::Y] {
                    let a = vec2(640., 400.) + axis * gap;
                    let b = a + axis * 7.;
                    draw_line(a.x, a.y, b.x, b.y, 2., PAPER);
                }
            }
        } else {
            draw_line(633., 400., 647., 400., 2., PAPER);
            draw_line(640., 393., 640., 407., 2., PAPER);
        }
        if let Some(hit) = hit {
            let block = self.world.get(hit.block);
            let name = block.name();
            let width = measure_text(name, None, 18, 1.0).width;
            draw_rectangle(
                622.0 - width * 0.5,
                430.0,
                width + 36.0,
                32.0,
                Color::new(0.02, 0.04, 0.06, 0.65),
            );
            text(name, 640.0 - width * 0.5, 452.0, 18.0, PAPER);
            if self.mine_progress > 0.0 {
                draw_rectangle(589.0, 476.0, 102.0, 6.0, INK);
                draw_rectangle(
                    590.0,
                    477.0,
                    100.0 * self.mine_progress.min(1.0),
                    4.0,
                    ACCENT,
                );
            }
            if self.hints {
                let block = block.material();
                let hint = if selected_weapon(&self.inventory).is_some() {
                    "Clic gauche : tirer · les murs bloquent les tirs".into()
                } else if let Some(stage) = block.crop_stage() {
                    if stage == 3 {
                        "Blé mûr : clic gauche pour récolter".into()
                    } else if !self
                        .world
                        .irrigated([hit.block[0], hit.block[1] - 1, hit.block[2]])
                    {
                        "Croissance arrêtée : eau nécessaire à moins de 4 blocs".into()
                    } else if !self
                        .world
                        .crop_lit(hit.block, self.time < 0.5 || self.time > 0.98)
                    {
                        "Croissance arrêtée : lumière du jour ou torche nécessaire".into()
                    } else {
                        format!("Blé : stade {}/4 · irrigué et éclairé", stage + 1)
                    }
                } else if self
                    .inventory
                    .selected()
                    .is_some_and(|s| s.item == Item::WoodHoe)
                    && matches!(block, Block::Grass | Block::Dirt)
                {
                    "Clic droit : labourer cette terre".into()
                } else if block == Block::Farmland {
                    "Graines + clic droit : semer · eau à moins de 4 blocs".into()
                } else if matches!(block, Block::Workbench | Block::Furnace | Block::Chest) {
                    format!(
                        "Clic droit : ouvrir · {} + clic droit : placer",
                        self.key(Action::Sneak)
                    )
                } else if block == Block::Bedrock {
                    "Socle indestructible".into()
                } else if self.mode == Mode::Survival
                    && !can_harvest(self.inventory.selected().map(|s| s.item), block)
                {
                    "Outil adapté nécessaire pour récupérer ce bloc".into()
                } else {
                    format!(
                        "Maintenir gauche : miner · Droit : placer{}",
                        if self.mode == Mode::Creative {
                            " · Clic molette : copier"
                        } else {
                            ""
                        }
                    )
                };
                text(
                    &hint,
                    640.0 - measure_text(&hint, None, 13, 1.0).width * 0.5,
                    504.0,
                    13.0,
                    PAPER,
                );
            }
        }
        if self.player.protection() > 0. {
            let label = format!(
                "BALISTIQUE  {:.0} % · Plaques T{} · Casque T{}",
                self.player.protection() * 100.,
                self.player.plate.unwrap_or(0),
                self.player.helmet.unwrap_or(0)
            );
            text(
                &label,
                640. - measure_text(&label, None, 13, 1.).width * 0.5,
                651.,
                13.,
                ACCENT,
            );
        }
        if self.remote_available() && !self.extras.charges.is_empty() {
            text(
                &format!(
                    "{} : déclencher {} C4 · portée 128 blocs",
                    self.key(Action::Reload),
                    self.extras.charges.len()
                ),
                805.,
                620.,
                14.,
                ACCENT,
            );
        }
        if self
            .inventory
            .selected()
            .is_some_and(|s| matches!(s.item, Item::HandGrenade | Item::C4))
        {
            let help = if self.inventory.selected().unwrap().item == Item::C4 {
                "Droit : poser le C4 sur une surface"
            } else {
                "Droit : lancer · minuterie 3,2 s"
            };
            text(help, 805., 642., 14., PAPER);
        }
        if self.mode == Mode::Survival {
            text("VIE", 362.0, 669.0, 12.0, MUTED);
            text("FAIM", 754.0, 669.0, 12.0, MUTED);
            for i in 0..10 {
                draw_rectangle(
                    362.0 + i as f32 * 21.0,
                    679.0,
                    17.0,
                    8.0,
                    if self.player.health > i as f32 * 2.0 {
                        Color::from_rgba(229, 100, 97, 255)
                    } else {
                        INK
                    },
                );
                draw_rectangle(
                    714.0 + i as f32 * 21.0,
                    679.0,
                    17.0,
                    8.0,
                    if self.player.hunger > i as f32 * 2.0 {
                        Color::from_rgba(232, 186, 89, 255)
                    } else {
                        INK
                    },
                );
            }
        }
        if let Some(stack) = self.inventory.selected() {
            let label = stack.item.name();
            text(
                label,
                640.0 - measure_text(label, None, 18, 1.0).width * 0.5,
                710.0,
                18.0,
                PAPER,
            );
            if let Some(ammo) = weapon_ammo(stack.item) {
                let count = if self.mode == Mode::Creative {
                    "∞".into()
                } else {
                    self.inventory.count(ammo).to_string()
                };
                draw_rectangle(1030., 108., 225., 96., Color::new(0.04, 0.08, 0.10, 0.82));
                text(
                    &format!(
                        "{:02} / {:02}  ·  {count}",
                        stack.loaded,
                        stack.item.magazine_capacity()
                    ),
                    1046.,
                    139.,
                    25.,
                    PAPER,
                );
                let firing = match self.combat.fire_mode {
                    FireMode::Auto => "AUTO",
                    FireMode::Semi => {
                        if weapons::spec(stack.item).unwrap().bolt {
                            "VERROU"
                        } else {
                            "SEMI"
                        }
                    }
                    FireMode::Burst => "RAFALE 3",
                };
                text(
                    &format!("{firing}  ·  {} RECHARGER", self.key(Action::Reload)),
                    1046.,
                    165.,
                    13.,
                    ACCENT,
                );
                if let Some(progress) = self.combat.reload_progress() {
                    draw_rectangle(1046., 179., 193., 5., INK);
                    draw_rectangle(1046., 179., 193. * progress, 5., ACCENT);
                } else {
                    text(ammo.name(), 1046., 188., 12., MUTED);
                }
            }
        }
        let start = 640.0 - 9.0 * 62.0 * 0.5;
        for i in 0..9 {
            slot(
                Rect::new(start + i as f32 * 62.0, 723.0, 57.0, 57.0),
                self.inventory.slots[i],
                self.inventory.selected == i,
                Some(i + 1),
                self.mode,
            );
        }
        if self.hints {
            draw_rectangle(15.0, 700.0, 325.0, 91.0, Color::new(0.04, 0.08, 0.10, 0.72));
            draw_rectangle(
                1008.0,
                700.0,
                247.0,
                65.0,
                Color::new(0.04, 0.08, 0.10, 0.72),
            );
            text(
                &format!(
                    "{} / {} / {} / {}  déplacer",
                    self.key(Action::Forward),
                    self.key(Action::Left),
                    self.key(Action::Backward),
                    self.key(Action::Right)
                ),
                29.0,
                725.0,
                15.0,
                PAPER,
            );
            text(
                &format!(
                    "{} sac · {} créatif · Échap menu",
                    self.key(Action::Inventory),
                    self.key(Action::Creative)
                ),
                29.0,
                750.0,
                14.0,
                MUTED,
            );
            text(
                if selected_weapon(&self.inventory).is_some() {
                    "Clic gauche  tirer"
                } else {
                    "Clic gauche  miner"
                },
                1025.0,
                725.0,
                15.0,
                PAPER,
            );
            text(
                if selected_weapon(&self.inventory).is_some() {
                    "Clic droit  viser"
                } else {
                    "Clic droit  placer / utiliser"
                },
                1025.0,
                750.0,
                14.0,
                MUTED,
            );
            if self.mode == Mode::Creative {
                text(
                    &if self.player.flying {
                        format!(
                            "VOL · {} ↑ · {} ↓",
                            self.key(Action::Jump),
                            self.key(Action::Sneak)
                        )
                    } else {
                        format!(
                            "{}  vol · {}  prudence",
                            self.key(Action::Fly),
                            self.key(Action::Sneak)
                        )
                    },
                    29.0,
                    775.0,
                    14.0,
                    ACCENT,
                );
            } else {
                text(
                    &format!(
                        "{}  courir · {}  prudence",
                        self.key(Action::Sprint),
                        self.key(Action::Sneak)
                    ),
                    29.0,
                    775.0,
                    14.0,
                    ACCENT,
                );
            }
        }
        if self.debug {
            let (chunks, triangles) = self.renderer.stats();
            let p = self.player.position;
            let lines = [
                format!(
                    "{} FPS · {} chunks · {} triangles",
                    get_fps(),
                    chunks,
                    triangles
                ),
                format!(
                    "XYZ {:.1} / {:.1} / {:.1} · seed {}",
                    p.x, p.y, p.z, self.world.seed
                ),
                format!(
                    "{} créatures · rayon {} · F5 sauvegarder",
                    self.mobs.len(),
                    self.radius
                ),
            ];
            draw_rectangle(25.0, 108.0, 520.0, 89.0, PANEL);
            for (i, line) in lines.iter().enumerate() {
                text(line, 39.0, 134.0 + i as f32 * 24.0, 16.0, PAPER);
            }
        }
        if self.hints
            && self.mode == Mode::Creative
            && self
                .inventory
                .selected()
                .is_some_and(|s| matches!(s.item, Item::Block(Block::Water | Block::Lava)))
        {
            text(
                &format!(
                    "Clic droit : source · {} : retirer un liquide",
                    self.key(Action::RemoveFluid)
                ),
                470.0,
                646.0,
                17.0,
                PAPER,
            );
        }
    }

    fn draw_pause(&mut self) -> bool {
        shade();
        draw_rectangle(405.0, 85.0, 470.0, 644.0, PANEL);
        text("UNE PETITE PAUSE", 441.0, 132.0, 14.0, ACCENT);
        text("Votre monde attend.", 441.0, 179.0, 31.0, PAPER);
        if self.button(Rect::new(441.0, 211.0, 398.0, 57.0), "REPRENDRE  →", true) {
            self.enter(Screen::Playing);
        }
        if self.button(
            Rect::new(441.0, 282.0, 398.0, 46.0),
            "SAUVEGARDER  /  F5",
            false,
        ) {
            self.save();
        }
        text("RÉGLAGES", 441.0, 374.0, 13.0, MUTED);
        let layout = if self.azerty {
            "CLAVIER  AZERTY"
        } else {
            "CLAVIER  QWERTY"
        };
        if self.button(Rect::new(441.0, 392.0, 398.0, 41.0), layout, false) {
            self.azerty = !self.azerty;
            self.persist_preferences();
        }
        text(
            &format!("Distance d'affichage : {} chunks", self.radius),
            441.0,
            471.0,
            18.0,
            PAPER,
        );
        if self.button(Rect::new(732.0, 443.0, 48.0, 41.0), "−", false) {
            self.radius = (self.radius - 1).max(2);
            self.persist_preferences();
        }
        if self.button(Rect::new(791.0, 443.0, 48.0, 41.0), "+", false) {
            self.radius = (self.radius + 1).min(6);
            self.persist_preferences();
        }
        text(
            &format!("Souris : {:.1}", self.sensitivity * 1000.0),
            441.0,
            522.0,
            18.0,
            PAPER,
        );
        if self.button(Rect::new(732.0, 494.0, 48.0, 41.0), "−", false) {
            self.sensitivity = (self.sensitivity - 0.0005).max(0.0005);
            self.persist_preferences();
        }
        if self.button(Rect::new(791.0, 494.0, 48.0, 41.0), "+", false) {
            self.sensitivity = (self.sensitivity + 0.0005).min(0.006);
            self.persist_preferences();
        }
        if self.button(
            Rect::new(441.0, 545.0, 398.0, 35.0),
            if self.audio.enabled {
                "SONS  ACTIVÉS"
            } else {
                "SONS  COUPÉS"
            },
            false,
        ) {
            self.audio.enabled = !self.audio.enabled;
            self.persist_preferences();
        }
        if self.button(
            Rect::new(441.0, 586.0, 398.0, 29.0),
            "COMMANDES & SOURIS",
            false,
        ) {
            self.enter(Screen::Controls);
        }
        if self.button(
            Rect::new(441.0, 621.0, 398.0, 40.0),
            "SAUVEGARDER ET ACCUEIL",
            false,
        ) && self.save()
        {
            self.enter(Screen::Title);
        }
        self.button(
            Rect::new(441.0, 676.0, 398.0, 34.0),
            "SAUVEGARDER ET QUITTER",
            false,
        )
    }

    fn cycle_optic(&mut self) {
        if let Some(stack) = self.inventory.slots[self.inventory.selected].as_mut() {
            if weapons::spec(stack.item).is_some_and(|s| s.scoped) {
                stack.optic = (stack.optic + 1) % 3;
                let label = format!("Optique ×{}", weapons::magnification(stack.optic));
                self.message(label);
            }
        }
    }

    fn drop_stack(&mut self, whole: bool) {
        let cursor = self.held.is_some();
        let source = if cursor {
            self.held
        } else {
            self.inventory.selected()
        };
        let Some(mut stack) = source else {
            return;
        };
        if !whole {
            stack.count = 1;
        }
        if !self.extras.toss(stack, &self.player, &self.world) {
            self.message("Trop d'objets au sol : récupérez-en d'abord");
            return;
        }
        let slot = if cursor {
            &mut self.held
        } else {
            &mut self.inventory.slots[self.inventory.selected]
        };
        let old = slot.as_mut().unwrap();
        old.count -= stack.count;
        if old.count == 0 {
            *slot = None;
        }
        self.combat.cancel_trigger();
        self.message(format!("Jeté : {} ×{}", stack.item.name(), stack.count));
    }

    fn weapon_sound(&self, event: WeaponSound) {
        if let Some(item) = selected_weapon(&self.inventory) {
            self.audio.weapon(item, event);
        }
    }
    fn use_bucket(&mut self) -> bool {
        let Some(s) = self.inventory.selected() else {
            return false;
        };
        if !matches!(s.item, Item::Bucket | Item::WaterBucket | Item::LavaBucket) {
            return false;
        }
        if let Some(hit) =
            raycast_with_fluids(&self.world, self.player.eye(), self.player.direction(), 6.)
        {
            let fluid = self.world.get(hit.block);
            if s.item == Item::Bucket {
                if fluid.fluid() && self.world.is_fluid_source(hit.block) {
                    self.world.set(hit.block, Block::Air);
                    self.inventory.slots[self.inventory.selected] = Some(settlements::stack(
                        if fluid == Block::Water {
                            Item::WaterBucket
                        } else {
                            Item::LavaBucket
                        },
                        1,
                    ));
                    self.audio.play(Effect::Place);
                    self.message("Source recueillie dans le seau");
                } else {
                    self.message("Le seau ne recueille que les blocs source d’eau ou de lave");
                }
            } else if !self.world.get(hit.previous).solid()
                && !player_intersects_block(&self.player, hit.previous)
            {
                self.world.set(
                    hit.previous,
                    if s.item == Item::WaterBucket {
                        Block::Water
                    } else {
                        Block::Lava
                    },
                );
                self.inventory.slots[self.inventory.selected] =
                    Some(settlements::stack(Item::Bucket, 1));
                self.audio.play(Effect::Place);
            }
        }
        true
    }
    fn draw_storage(&mut self) {
        shade();
        draw_rectangle(64., 48., 1152., 726., PANEL);
        let chest = self
            .active_chest
            .map(|p| settlements::chest_index(&mut self.extras, &self.world, p));
        text(
            if chest.is_some() {
                "COFFRE · 27 CASES"
            } else {
                "ÉCHANGES DU VILLAGE"
            },
            94.,
            94.,
            28.,
            PAPER,
        );
        text(
            "Clic : pile · Droit : moitié / un objet · Maj-clic : transférer",
            94.,
            124.,
            16.,
            MUTED,
        );
        if self.button(Rect::new(1090., 74., 94., 40.), "FERMER", false) {
            self.close_inventory();
            return;
        }
        if let Some(c) = chest {
            for i in 0..27 {
                let rect = Rect::new(
                    94. + (i % 9) as f32 * 70.,
                    170. + (i / 9) as f32 * 63.,
                    62.,
                    57.,
                );
                let stack = self.extras.chests[c].inventory.slots[i];
                slot(rect, stack, false, None, Mode::Survival);
                if let Some(s) = stack {
                    self.tip(rect,s.item.name(),"Gauche : prendre ou poser · Droit : diviser\nMaj-clic : déplacer vers votre inventaire");
                }
                if rect.contains(self.ui.mouse) {
                    if self.ui.left {
                        if self.input_down(KeyCode::LeftShift) && self.held.is_none() {
                            settlements::move_stack(
                                &mut self.extras.chests[c].inventory,
                                &mut self.inventory,
                                i,
                            );
                        } else {
                            self.extras.chests[c]
                                .inventory
                                .left_click(i, &mut self.held);
                        }
                        self.ui.left = false;
                    }
                    if self.ui.right {
                        self.extras.chests[c]
                            .inventory
                            .right_click(i, &mut self.held);
                        self.ui.right = false;
                    }
                }
            }
            if self.button(Rect::new(800., 182., 320., 44.), "TOUT PRENDRE", false) {
                for i in 0..27 {
                    settlements::move_stack(
                        &mut self.extras.chests[c].inventory,
                        &mut self.inventory,
                        i,
                    );
                }
            }
            text("Le contenu reste dans ce monde.", 800., 258., 17., MUTED);
            text(
                "Un coffre vidé ne recrée pas son loot.",
                800.,
                287.,
                17.,
                MUTED,
            );
        } else {
            for (i, (input, count, output, quantity)) in settlements::TRADES.into_iter().enumerate()
            {
                let y = 163. + i as f32 * 57.;
                icon(input, 116., y + 23., 28.);
                icon(output, 650., y + 23., 28.);
                text(
                    &format!(
                        "{} ×{}  →  {} ×{}",
                        input.name(),
                        count,
                        output.name(),
                        quantity
                    ),
                    150.,
                    y + 27.,
                    17.,
                    PAPER,
                );
                if self.button(Rect::new(740., y, 230., 43.), "ÉCHANGER", false) {
                    if settlements::trade(&mut self.inventory, i) {
                        self.audio.play(Effect::Craft);
                        self.message("Échange effectué");
                    } else {
                        self.message("Il manque des objets ou une place dans l’inventaire");
                    }
                }
            }
        }
        text("VOTRE INVENTAIRE", 94., 425., 16., MUTED);
        for i in 0..36 {
            let rect = Rect::new(
                94. + (i % 9) as f32 * 70.,
                442. + (i / 9) as f32 * 61.,
                62.,
                55.,
            );
            let stack = self.inventory.slots[i];
            slot(
                rect,
                stack,
                self.inventory.selected == i,
                if i < 9 { Some(i + 1) } else { None },
                Mode::Survival,
            );
            if let Some(s) = stack {
                self.tip(
                    rect,
                    s.item.name(),
                    "Gauche : pile · Droit : moitié / un objet\nMaj-clic : ranger dans le coffre",
                );
            }
            if rect.contains(self.ui.mouse) {
                if self.ui.left {
                    if self.input_down(KeyCode::LeftShift) && self.held.is_none() {
                        if let Some(c) = chest {
                            settlements::move_stack(
                                &mut self.inventory,
                                &mut self.extras.chests[c].inventory,
                                i,
                            );
                        } else {
                            self.inventory.quick_transfer(i);
                        }
                    } else {
                        self.inventory.left_click(i, &mut self.held);
                    }
                    self.ui.left = false;
                }
                if self.ui.right {
                    self.inventory.right_click(i, &mut self.held);
                    self.ui.right = false;
                }
            }
        }
        if let Some(s) = self.held {
            icon(s.item, self.ui.mouse.x, self.ui.mouse.y, 36.);
            text(
                &s.count.to_string(),
                self.ui.mouse.x + 15.,
                self.ui.mouse.y + 20.,
                16.,
                PAPER,
            );
        }
    }

    fn draw_inventory(&mut self) {
        if self.active_chest.is_some() || self.trading {
            self.draw_storage();
            return;
        }
        self.select_hotbar();
        if self.pressed(Action::Drop) {
            self.drop_stack(self.input_down(KeyCode::LeftControl));
        }
        shade();
        draw_rectangle(64., 48., 1152., 726., PANEL);
        text(
            match self.screen {
                Screen::Creative => "MENU CRÉATIF",
                Screen::Crafting => "FABRICATION",
                _ => "INVENTAIRE",
            },
            94.,
            94.,
            30.,
            PAPER,
        );
        text(
            if self.screen == Screen::Creative {
                "CATALOGUE CRÉATIF · objets toujours disponibles"
            } else if self.screen == Screen::Crafting {
                "POSTE DE FABRICATION · recettes du bloc utilisé"
            } else {
                "SAC & ÉQUIPEMENT · table de craft fabriquée à la main"
            },
            94.,
            123.,
            15.,
            ACCENT,
        );
        if self.button(Rect::new(1106., 74., 78., 40.), "FERMER", false) {
            self.close_inventory();
            return;
        }
        if self.screen == Screen::Creative {
            for (index, title) in ["BLOCS", "NOURRITURE", "UTILITAIRE", "ARMES"]
                .iter()
                .enumerate()
            {
                if self.button(
                    Rect::new(94. + index as f32 * 160., 143., 150., 35.),
                    title,
                    self.inventory_tab == index,
                ) {
                    self.inventory_tab = index;
                    self.catalog_page = 0;
                }
            }
            let entries = if self.world.generation == 4 {
                city_catalog(self.inventory_tab)
            } else {
                catalog(self.inventory_tab)
            };
            let pages = entries.len().div_ceil(30);
            self.catalog_page = self.catalog_page.min(pages.saturating_sub(1));
            for (index, item) in entries
                .into_iter()
                .skip(self.catalog_page * 30)
                .take(30)
                .enumerate()
            {
                let rect = Rect::new(
                    94. + (index % 10) as f32 * 63.,
                    190. + (index / 10) as f32 * 58.,
                    62.,
                    52.,
                );
                draw_rectangle(rect.x, rect.y, rect.w, rect.h, INK);
                icon(item, rect.x + 31., rect.y + 23., 31.);
                let short = item.name().replace(" Intervention", "");
                let size = (56. / measure_text(&short, None, 12, 1.).width * 12.).min(12.);
                text(&short, rect.x + 3., rect.y + 50., size, PAPER);
                let caliber =
                    weapon_ammo(item).map_or(String::new(), |ammo| format!(" · {}", ammo.name()));
                self.tip(
                    rect,
                    &format!("{}{caliber}", item.name()),
                    &format!(
                        "{}\nCréatif : clic pour prendre · droit : un objet · Maj : ajouter au sac",
                        item_help(item)
                    ),
                );
                if self.mode == Mode::Creative
                    && rect.contains(self.ui.mouse)
                    && (self.ui.left || self.ui.right)
                {
                    let stack = Stack {
                        item,
                        count: if self.ui.right { 1 } else { item.stack_limit() },
                        loaded: item.magazine_capacity(),
                        optic: 0,
                    };
                    if self.input_down(KeyCode::LeftShift) {
                        if !self.inventory.return_stack(stack) {
                            self.message("Inventaire plein");
                        }
                    } else if self.held.is_none() {
                        self.held = Some(stack);
                    } else if let Some(held) = self.held.as_mut().filter(|s| s.item == item) {
                        held.count = held
                            .count
                            .saturating_add(stack.count)
                            .min(item.stack_limit());
                    } else {
                        self.message("Posez, jetez ou supprimez d'abord l'objet tenu");
                    }
                    self.ui.left = false;
                    self.ui.right = false;
                }
            }
            if pages > 1 {
                text(
                    &format!("Page {} / {}", self.catalog_page + 1, pages),
                    94.,
                    386.,
                    15.,
                    MUTED,
                );
                if self.button(Rect::new(530., 364., 82., 28.), "<", false) {
                    self.catalog_page = self.catalog_page.saturating_sub(1);
                }
                if self.button(Rect::new(624., 364., 82., 28.), ">", false) {
                    self.catalog_page = (self.catalog_page + 1).min(pages - 1);
                }
            }
        } else {
            self.draw_equipment();
        }
        text(
            "VOTRE INVENTAIRE · première rangée : barre rapide",
            94.,
            404.,
            14.,
            MUTED,
        );
        for i in 0..36 {
            let rect = Rect::new(
                94. + (i % 9) as f32 * 70.,
                418. + (i / 9) as f32 * 61.,
                62.,
                55.,
            );
            slot(
                rect,
                self.inventory.slots[i],
                self.inventory.selected == i,
                if i < 9 { Some(i + 1) } else { None },
                self.mode,
            );
            if rect.contains(self.ui.mouse) {
                draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 2., ACCENT);
                if let Some(stack) = self.inventory.slots[i] {
                    self.tip(rect, &format!("{} · {}", stack.item.name(), stack.count), &format!("{}\nGauche : pile · Droit : moitié / un objet · Maj : transfert\nSurvol + 1–9 : échanger avec la barre", item_help(stack.item)));
                }
                if self.ui.left {
                    if self.input_down(KeyCode::LeftShift) && self.held.is_none() {
                        self.inventory.quick_transfer(i);
                    } else {
                        self.inventory.left_click(i, &mut self.held);
                    }
                    self.ui.left = false;
                }
                if self.ui.right {
                    self.inventory.right_click(i, &mut self.held);
                    self.ui.right = false;
                }
                if self.held.is_none() && window_focused() {
                    if let Some(selected) =
                        get_keys_pressed().into_iter().filter_map(slot_key).min()
                    {
                        self.inventory.slots.swap(i, selected);
                    }
                }
            }
        }
        if self.button(Rect::new(94., 671., 165., 36.), "JETER LA PILE", false) {
            self.drop_stack(true);
        }
        self.tip(Rect::new(94., 671., 165., 36.), "Jeter au sol", "Objet tenu, sinon case rapide sélectionnée.\nApprochez-vous pour le ramasser après une seconde.");
        if self.button(Rect::new(270., 671., 165., 36.), "SUPPRIMER", false) {
            if self.held.take().is_none() {
                self.inventory.slots[self.inventory.selected] = None;
            }
            self.combat.cancel_trigger();
        }
        self.tip(Rect::new(270., 671., 165., 36.), "Supprimer l'objet", "Supprime la pile tenue, sinon la case rapide sélectionnée.\nLe catalogue créatif conserve toujours tous les objets.");
        if let Some(stack) = self
            .inventory
            .selected()
            .filter(|s| weapons::spec(s.item).is_some_and(|w| w.scoped))
        {
            if self.button(
                Rect::new(446., 671., 268., 36.),
                &format!("OPTIQUE ×{}", weapons::magnification(stack.optic)),
                true,
            ) {
                self.cycle_optic();
            }
        }
        text(
            &format!(
                "{} : jeter un objet · Ctrl + {} : pile entière",
                self.key(Action::Drop),
                self.key(Action::Drop)
            ),
            94.,
            739.,
            14.,
            MUTED,
        );
        draw_line(
            757.0,
            170.0,
            757.0,
            700.0,
            1.0,
            Color::new(0.25, 0.34, 0.35, 1.0),
        );
        text(
            if self.screen == Screen::Creative {
                "UTILISATION"
            } else {
                "RECETTES"
            },
            788.0,
            189.0,
            13.0,
            MUTED,
        );
        if self.screen == Screen::Creative {
            for (i, line) in [
                "Prenez un objet, puis posez-le dans le sac.",
                "Les objets supprimés restent au catalogue.",
                "Armures : équipez-les depuis votre sac.",
                "Grenade : clic droit pour lancer.",
                "C4 : clic droit pour poser · R pour déclencher.",
                "Établi / four : clic droit pour fabriquer.",
            ]
            .iter()
            .enumerate()
            {
                text(line, 788., 228. + i as f32 * 34., 15., PAPER);
            }
        }
        let station = if self.screen == Screen::Crafting {
            crafting_station(&self.world, &self.player, self.active_station)
        } else {
            None
        };
        let all: Vec<_> = recipes()
            .into_iter()
            .filter(|r| {
                self.screen != Screen::Creative
                    && (self.screen != Screen::Crafting || station.is_some())
                    && recipe_allowed(r, station)
            })
            .collect();
        let pages = all.len().div_ceil(6).max(1);
        self.recipe_page = self.recipe_page.min(pages.saturating_sub(1));

        for (row, recipe) in all.iter().skip(self.recipe_page * 6).take(6).enumerate() {
            let y = 209.0 + row as f32 * 70.0;
            let available = recipe
                .ingredients
                .iter()
                .all(|(item, n)| self.inventory.count(*item) >= *n);
            let station_ready = self.screen != Screen::Crafting || station.is_some();
            draw_rectangle(785.0, y, 399.0, 63.0, Color::new(0.095, 0.15, 0.18, 1.0));
            icon(recipe.result.0, 810.0, y + 25.0, 28.0);
            text(recipe.name, 838.0, y + 23.0, 18.0, PAPER);
            let ingredients = recipe
                .ingredients
                .iter()
                .map(|(i, n)| format!("{}× {}", n, i.name()))
                .collect::<Vec<_>>()
                .join(" · ");
            text(
                &ingredients,
                799.0,
                y + 52.0,
                12.0,
                if available {
                    MUTED
                } else {
                    Color::new(0.76, 0.52, 0.44, 1.0)
                },
            );
            if self.button(
                Rect::new(1111.0, y + 6.0, 63.0, 29.0),
                "+",
                available && station_ready,
            ) {
                if !station_ready {
                    self.message("Ce poste n’est plus disponible");
                } else {
                    match craft_at(
                        &mut self.inventory,
                        recipe,
                        &self.world,
                        &self.player,
                        if self.screen == Screen::Crafting {
                            self.active_station
                        } else {
                            None
                        },
                    ) {
                        Ok(()) => {
                            self.audio.play(Effect::Craft);
                            self.message(format!(
                                "Fabriqué : {} ×{}",
                                recipe.result.0.name(),
                                recipe.result.1
                            ))
                        }
                        Err(e) => self.message(e),
                    }
                }
            }
            let station_help = match recipe.station {
                Some(Block::Workbench) if !station_ready => "Cliquez droit sur une table de craft.",
                Some(Block::Furnace) if !station_ready => "Cliquez droit sur un four.",
                _ => "Poste de fabrication disponible.",
            };
            let missing = recipe
                .ingredients
                .iter()
                .filter_map(|(item, count)| {
                    let have = self.inventory.count(*item);
                    (have < *count).then(|| format!("{} : manque {}", item.name(), count - have))
                })
                .collect::<Vec<_>>()
                .join(" · ");
            self.tip(
                Rect::new(785.0, y, 399.0, 63.0),
                recipe.name,
                &format!(
                    "Résultat : {} × {}\n{}\n{}",
                    recipe.result.1,
                    recipe.result.0.name(),
                    station_help,
                    if missing.is_empty() {
                        "Clic sur + : fabriquer"
                    } else {
                        &missing
                    }
                ),
            );
        }
        if pages > 1 {
            text(
                &format!("{} / {}", self.recipe_page + 1, pages),
                955.0,
                666.0,
                16.0,
                MUTED,
            );
            if self.button(Rect::new(788.0, 644.0, 87.0, 36.0), "←", false) {
                self.recipe_page = self.recipe_page.saturating_sub(1);
            }
            if self.button(Rect::new(1097.0, 644.0, 87.0, 36.0), "→", false) {
                self.recipe_page = (self.recipe_page + 1).min(pages.saturating_sub(1));
            }
        }
        text(
            match station {
                Some(Block::Furnace) => "FOUR UTILISÉ",
                Some(Block::Workbench) => "TABLE DE CRAFT UTILISÉE",
                _ if self.screen == Screen::Creative => "C : catalogue · E : inventaire",
                _ => "À la main : table de craft avec 4 bois.",
            },
            788.0,
            706.0,
            13.0,
            ACCENT,
        );
        if let Some(stack) = self.held {
            let mouse = self.ui.mouse;
            icon(stack.item, mouse.x, mouse.y, 34.0);
            text(
                &stack.count.to_string(),
                mouse.x + 14.0,
                mouse.y + 21.0,
                16.0,
                PAPER,
            );
        }
    }

    fn draw_dead(&mut self) {
        shade();
        text("L'AVENTURE CONTINUE", 445.0, 303.0, 31.0, PAPER);
        text("Vous avez perdu connaissance.", 459.0, 346.0, 21.0, MUTED);
        text(
            "Votre construction et votre inventaire sont conservés.",
            399.0,
            380.0,
            18.0,
            MUTED,
        );
        if self.button(
            Rect::new(440.0, 421.0, 400.0, 59.0),
            "REVENIR AU POINT DE DÉPART",
            true,
        ) {
            let spawn = self
                .extras
                .spawn
                .filter(|p| {
                    self.world
                        .get([
                            p[0].floor() as i32,
                            p[1].floor() as i32 - 1,
                            p[2].floor() as i32,
                        ])
                        .material()
                        == Block::Bed
                })
                .unwrap_or_else(|| self.world.spawn());
            let gear = (self.player.plate, self.player.helmet);
            self.player = Player::new(spawn);
            self.player.plate = gear.0;
            self.player.helmet = gear.1;
            self.player.yaw = if self.world.generation == 4 {
                city::get().expect("City map").metadata.spawns[self.world.city_spawn as usize].yaw
            } else {
                -0.9
            };
            self.enter(Screen::Playing);
        }
    }

    fn button(&mut self, rect: Rect, label: &str, active: bool) -> bool {
        self.tip(rect, label, button_help(label, rect, self.screen));
        button(rect, label, active, self.ui)
    }

    fn key(&self, action: Action) -> String {
        key_name(self.bindings[action as usize], self.azerty)
    }

    fn down(&self, action: Action) -> bool {
        let key = self.bindings[action as usize];
        if self.input_down(key) {
            return true;
        }
        let arrow = match action {
            Action::Forward => Some(KeyCode::Up),
            Action::Backward => Some(KeyCode::Down),
            Action::Left => Some(KeyCode::Left),
            Action::Right => Some(KeyCode::Right),
            _ => None,
        };
        arrow.is_some_and(|k| !self.bindings.contains(&k) && self.input_down(k))
    }

    fn pressed(&self, action: Action) -> bool {
        key_variants(self.bindings[action as usize])
            .into_iter()
            .any(|k| self.focus_keys.allows(k) && is_key_pressed(k))
    }

    fn input_down(&self, key: KeyCode) -> bool {
        key_variants(key)
            .into_iter()
            .any(|k| self.focus_keys.allows(k) && is_key_down(k))
    }

    fn select_hotbar(&mut self) {
        if !window_focused() {
            return;
        }
        if let Some(index) = get_keys_pressed().into_iter().filter_map(slot_key).min() {
            self.inventory.selected = index;
        }
        let wheel = mouse_wheel().1;
        if wheel != 0.0 {
            self.inventory.selected =
                (self.inventory.selected as i32 - wheel.signum() as i32).rem_euclid(9) as usize;
        }
    }

    fn restore_preferences(&mut self) {
        if let Some(p) = Preferences::load(&self.preferences_path) {
            self.bindings = p.decode_bindings().unwrap();
            self.azerty = p.azerty;
            self.sensitivity = p.sensitivity;
            self.invert_y = p.invert_y;
            self.toggle_sprint = p.toggle_sprint;
            self.radius = p.radius;
            self.audio.enabled = p.sound;
            self.hints = p.hints;
            self.renderer.enhanced = p.shaders;
        }
    }

    fn persist_preferences(&mut self) {
        let p = Preferences {
            bindings: self.bindings.map(|k| format!("{k:?}")).to_vec(),
            azerty: self.azerty,
            sensitivity: self.sensitivity,
            invert_y: self.invert_y,
            toggle_sprint: self.toggle_sprint,
            radius: self.radius,
            sound: self.audio.enabled,
            hints: self.hints,
            shaders: self.renderer.enhanced,
        };
        if let Err(error) = p.save(&self.preferences_path) {
            self.message(format!("Réglages non enregistrés : {error}"));
        }
    }

    fn tip(&mut self, rect: Rect, title: &str, body: &str) {
        if rect.contains(self.ui.mouse) {
            self.tooltip = Some((title.into(), body.into()));
        }
    }

    fn draw_tooltip(&mut self, dt: f32) {
        let Some((title, body)) = &self.tooltip else {
            self.hover_key.clear();
            self.hover_time = 0.0;
            return;
        };
        let key = format!("{title}\n{body}");
        if self.hover_key != key {
            self.hover_key = key;
            self.hover_time = 0.0;
        } else {
            self.hover_time += dt;
        }
        if self.hover_time < 0.30 || self.screen == Screen::Playing {
            return;
        }
        let lines = wrap_text(body, 340.0, 15);
        let width = (measure_text(title, None, 18, 1.0).width.max(
            lines
                .iter()
                .map(|s| measure_text(s, None, 15, 1.0).width)
                .fold(0.0, f32::max),
        ) + 28.0)
            .clamp(180.0, 370.0);
        let rect = tooltip_bounds(self.ui.mouse, width, 46.0 + lines.len() as f32 * 21.0);
        draw_rectangle(
            rect.x + 4.0,
            rect.y + 4.0,
            rect.w,
            rect.h,
            Color::new(0.0, 0.0, 0.0, 0.4),
        );
        draw_rectangle(rect.x, rect.y, rect.w, rect.h, INK);
        draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 1.0, ACCENT);
        text(title, rect.x + 14.0, rect.y + 25.0, 18.0, ACCENT);
        for (i, line) in lines.iter().enumerate() {
            text(
                line,
                rect.x + 14.0,
                rect.y + 49.0 + i as f32 * 21.0,
                15.0,
                PAPER,
            );
        }
    }

    fn draw_controls(&mut self) {
        shade();
        draw_rectangle(64.0, 54.0, 1152.0, 692.0, PANEL);
        text("VOS COMMANDES", 94.0, 98.0, 29.0, PAPER);
        text(
            "Cliquez sur une touche, puis appuyez sur sa remplaçante. Échap annule.",
            94.0,
            128.0,
            16.0,
            MUTED,
        );
        if self.button(Rect::new(1086.0, 77.0, 98.0, 40.0), "RETOUR", false) {
            self.enter(Screen::Paused);
            return;
        }
        for (index, action) in ACTIONS.iter().enumerate() {
            let x = 94.0 + (index / 9) as f32 * 550.0;
            let y = 156.0 + (index % 9) as f32 * 34.0;
            text(action.name(), x, y + 26.0, 17.0, PAPER);
            let rect = Rect::new(x + 352.0, y, 153.0, 32.0);
            let waiting = self.rebinding == Some(*action);
            let label = if waiting {
                "APPUYEZ…".into()
            } else {
                self.key(*action)
            };
            if self.button(rect, &label, waiting) {
                self.rebinding = Some(*action);
            }
            self.tip(rect, action.name(), "Cliquer puis appuyer sur une touche.\nUne touche déjà utilisée échange les deux commandes.\nÉchap, 1–9, F1, F3, F5 et F11 restent réservés.");
        }
        text("SOURIS & CONFORT", 94.0, 493.0, 14.0, ACCENT);
        if self.button(
            Rect::new(94.0, 510.0, 340.0, 39.0),
            if self.invert_y {
                "AXE VERTICAL  INVERSÉ"
            } else {
                "AXE VERTICAL  NORMAL"
            },
            false,
        ) {
            self.invert_y = !self.invert_y;
            self.persist_preferences();
        }
        if self.button(
            Rect::new(449.0, 510.0, 360.0, 39.0),
            if self.toggle_sprint {
                "COURSE  APPUYER POUR BASCULER"
            } else {
                "COURSE  MAINTENIR LA TOUCHE"
            },
            false,
        ) {
            self.toggle_sprint = !self.toggle_sprint;
            self.sprint_latched = false;
            self.persist_preferences();
        }
        if self.button(
            Rect::new(824.0, 510.0, 360.0, 39.0),
            "RÉTABLIR LES TOUCHES",
            false,
        ) {
            self.bindings = default_bindings(self.azerty);
            self.persist_preferences();
        }
        if self.button(
            Rect::new(94., 561., 340., 34.),
            if self.renderer.enhanced {
                "SHADERS  ACTIVÉS"
            } else {
                "SHADERS  CLASSIQUES"
            },
            self.renderer.enhanced,
        ) {
            self.renderer.enhanced = !self.renderer.enhanced;
            self.persist_preferences();
        }
        text(
            "Reflets d'eau · vent · lumière solaire · halo lumineux",
            449.,
            584.,
            15.,
            MUTED,
        );
        text(
            "Gauche : miner / tirer · Droit : placer / viser · Molette : case rapide",
            94.0,
            627.0,
            17.0,
            PAPER,
        );
        text(
            "Armes : R recharger · B auto / semi · I inspecter · Échap : pause",
            94.0,
            652.0,
            17.0,
            MUTED,
        );
        text(
            "Inventaire : gauche fusionne · droit sépare / pose un objet · Maj-clic transfère",
            94.0,
            677.0,
            17.0,
            PAPER,
        );
        text("F1 aide · F3 infos · F5 sauvegarde · F11 plein écran · Réglages conservés automatiquement", 94.0, 717.0, 15.0, ACCENT);
    }

    fn handle_menu_keys(&mut self, escape: bool, inventory: bool) -> bool {
        if let Some(next) = menu_shortcut(self.screen, escape, inventory) {
            if matches!(
                self.screen,
                Screen::Inventory | Screen::Creative | Screen::Crafting
            ) {
                self.close_inventory();
            } else {
                self.enter(next);
            }
            true
        } else {
            false
        }
    }

    fn handle_creative_key(&mut self) {
        if self.screen == Screen::Creative {
            self.close_inventory();
        } else if self.mode == Mode::Creative
            && matches!(
                self.screen,
                Screen::Playing | Screen::Inventory | Screen::Crafting
            )
        {
            if self.screen == Screen::Playing || self.close_inventory() {
                self.enter(Screen::Creative);
            }
        } else if self.screen == Screen::Playing {
            self.message("Le catalogue est disponible en mode Créatif");
        }
    }
    fn draw_equipment(&mut self) {
        text("ÉQUIPEMENT BALISTIQUE", 94., 177., 17., ACCENT);
        for (index, helmet) in [false, true].into_iter().enumerate() {
            let tier = if helmet {
                self.player.helmet
            } else {
                self.player.plate
            };
            let item = tier.map(|t| {
                if helmet {
                    Item::BallisticHelmet(t)
                } else {
                    Item::PlateCarrier(t)
                }
            });
            let rect = Rect::new(94. + index as f32 * 315., 198., 290., 82.);
            draw_rectangle(rect.x, rect.y, rect.w, rect.h, INK);
            if let Some(i) = item {
                icon(i, rect.x + 40., rect.y + 40., 52.);
            }
            text(
                if helmet { "CASQUE" } else { "PORTE-PLAQUES" },
                rect.x + 80.,
                rect.y + 25.,
                15.,
                MUTED,
            );
            text(
                &item.map_or("Non équipé".to_string(), |i| i.name().to_string()),
                rect.x + 80.,
                rect.y + 52.,
                17.,
                PAPER,
            );
            self.tip(
                rect,
                if helmet {
                    "Casque balistique"
                } else {
                    "Porte-plaques"
                },
                "Clic : prendre l'équipement ou poser celui tenu.
Maj-clic : ranger dans le sac.
Un tier supérieur réduit davantage les dégâts de combat.",
            );
            if rect.contains(self.ui.mouse) && self.ui.left {
                if self.input_down(KeyCode::LeftShift) && self.held.is_none() {
                    if !self.player.unequip(&mut self.inventory, helmet) {
                        self.message("Libérez une case dans le sac");
                    }
                } else {
                    let compatible = self.held.is_none()
                        || self.held.is_some_and(|s| {
                            s.valid()
                                && match s.item {
                                    Item::PlateCarrier(_) => !helmet,
                                    Item::BallisticHelmet(_) => helmet,
                                    _ => false,
                                }
                        });
                    if compatible {
                        let old = tier.map(|t| Stack {
                            item: if helmet {
                                Item::BallisticHelmet(t)
                            } else {
                                Item::PlateCarrier(t)
                            },
                            count: 1,
                            loaded: 0,
                            optic: 0,
                        });
                        let next = self.held.and_then(|s| match s.item {
                            Item::PlateCarrier(t) | Item::BallisticHelmet(t) => Some(t),
                            _ => None,
                        });
                        if helmet {
                            self.player.helmet = next;
                        } else {
                            self.player.plate = next;
                        }
                        self.held = old;
                    } else {
                        self.message("Cet objet ne correspond pas à cet emplacement");
                    }
                }
                self.ui.left = false;
            }
        }
        text(
            &format!(
                "Protection combat : {:.0} %",
                self.player.protection() * 100.
            ),
            94.,
            310.,
            20.,
            PAPER,
        );
        if self.button(
            Rect::new(94., 328., 360., 35.),
            "ÉQUIPER LA CASE RAPIDE",
            false,
        ) && !self.player.equip_selected(&mut self.inventory)
        {
            self.message("Sélectionnez un casque ou un porte-plaques");
        }
        text(
            &format!(
                "{} : sac · {} : catalogue créatif · Clic droit sur un établi : fabriquer",
                self.key(Action::Inventory),
                self.key(Action::Creative)
            ),
            94.,
            388.,
            14.,
            MUTED,
        );
    }
    fn prepare_weapon_demo(&mut self) {
        for x in -9..=9 {
            for z in -12..=12 {
                self.world.set([x, 40, z], Block::Stone);
                for y in 41..=49 {
                    self.world.set([x, y, z], Block::Air);
                }
            }
        }
        for x in -4..=4 {
            for y in 41..=45 {
                self.world.set([x, y, -6], Block::Brick);
            }
        }
        self.world.set([0, 40, -6], Block::Bedrock);
        self.mode = Mode::Survival;
        self.inventory = Inventory::new(self.mode);
        self.inventory.slots.fill(None);
        for (index, (item, count)) in [
            (Item::Ak, 1),
            (Item::RocketLauncher, 1),
            (Item::Bullet, 64),
            (Item::RocketAmmo, 16),
        ]
        .into_iter()
        .enumerate()
        {
            self.inventory.slots[index] = Some(Stack {
                item,
                count,
                loaded: item.magazine_capacity(),
                optic: 0,
            });
        }
        self.player.position = vec3(0.5, 41., 10.5);
        self.player.yaw = 0.;
        self.player.pitch = -0.08;
        self.mobs = [
            vec3(0.5, 41., 0.5),
            vec3(3.5, 41., -2.5),
            vec3(0.5, 41., -10.5),
        ]
        .into_iter()
        .map(|position| Mob {
            position,
            kind: MobKind::Zombie,
            health: 20.,
            phase: 0.,
            attack_timer: 1.,
            ..Default::default()
        })
        .collect();
        self.combat
            .tick_weapon(&mut self.inventory, self.mode, 0.1, WeaponInput::default());
        self.combat.equip_time = 0.;
    }

    fn prepare_village_demo(&mut self, desert: bool) {
        let village = (-12..=12)
            .flat_map(|z| (-12..=12).map(move |x| (x, z)))
            .find_map(|(x, z)| {
                self.world
                    .village_at(
                        x * self.world.village_region_size(),
                        z * self.world.village_region_size(),
                    )
                    .filter(|v| {
                        v.desert == desert
                            && v.terrain.is_none_or(|t| {
                                t.floors.iter().max().unwrap() - t.floors.iter().min().unwrap() >= 4
                            })
                    })
            })
            .expect("Village fixture");
        let center = Vec3::from_array(village.center.map(|v| v as f32));
        self.mobs.clear();
        self.extras = Default::default();
        self.radius = 8;
        self.world.stream(
            [
                village.center[0].div_euclid(CHUNK),
                village.center[2].div_euclid(CHUNK),
            ],
            4,
            81,
        );
        self.village_hint = Some(village);
        self.player.position = center + vec3(56., 36., 64.);
        self.player.yaw = -0.719;
        self.player.pitch = -0.42;
        self.player.flying = true;
        settlements::spawn_villages(&mut self.extras, &mut self.mobs, &self.world, center);
        for p in village.chests() {
            assert_eq!(self.world.get(p), Block::Chest);
            settlements::chest_index(&mut self.extras, &self.world, p);
        }
        for p in village.residents() {
            assert_eq!(self.world.get(p), Block::Air);
            assert_eq!(self.world.get([p[0], p[1] + 1, p[2]]), Block::Air);
            assert!(self.world.get([p[0], p[1] - 1, p[2]]).solid());
        }
        for m in self.mobs.iter().filter(|m| m.kind == MobKind::Golem) {
            let p = m.position.floor().as_ivec3().to_array();
            assert_eq!(self.world.get(p), Block::Air);
            assert!(self.world.get([p[0], p[1] - 1, p[2]]).solid());
        }
        self.time = 0.18;
        self.inventory = Inventory::new(Mode::Creative);
    }
    fn prepare_building_demo(&mut self, kind: villages::Kind) {
        self.prepare_village_demo(false);
        let v = self.village_hint.unwrap();
        let (i, b) = villages::BUILDINGS
            .iter()
            .enumerate()
            .find(|(_, b)| b.kind == kind)
            .unwrap();
        let mut center = Vec3::from_array(v.center.map(|n| n as f32));
        center.y = v.floor(i) as f32;
        self.player.position =
            center + vec3((b.x + b.w / 2 + 5) as f32, 7., (b.z + b.d + 12) as f32);
        if kind == villages::Kind::Church {
            self.player.position =
                center + vec3((b.x - 18) as f32, 10., (b.z + b.d / 2 + 4) as f32);
        }
        let target = center
            + vec3(
                (b.x + b.w / 2) as f32,
                if kind == villages::Kind::Church {
                    8.
                } else {
                    3.
                },
                (b.z + b.d / 2) as f32,
            );
        let dir = target - self.player.eye();
        self.player.yaw = dir.x.atan2(-dir.z);
        self.player.pitch = (dir.y / dir.length()).asin();
    }

    fn scripted_village_defense_check(&mut self, frame: u32) {
        if frame == 0 {
            self.prepare_village_demo(false);
            let v = self.village_hint.unwrap();
            let home = Vec3::from_array(v.center.map(|n| n as f32)) + vec3(0.5, 1., 0.5);
            self.mobs = vec![
                Mob {
                    kind: MobKind::Villager,
                    position: home + Vec3::Z * 8.,
                    health: 20.,
                    ..Default::default()
                },
                Mob {
                    kind: MobKind::Zombie,
                    position: home + Vec3::Z * 12.5,
                    health: 20.,
                    ..Default::default()
                },
            ];
            for m in &mut self.mobs {
                m.position.y = self
                    .world
                    .height_at(m.position.x.floor() as i32, m.position.z.floor() as i32)
                    as f32
                    + 1.;
            }
            self.player.position = home + vec3(10., 6., 17.);
            self.player.yaw = -0.8;
            self.player.pitch = -0.32;
            self.time = 0.7;
        }
        if frame == 90 {
            assert!(
                self.mobs[0].health < 20.,
                "Zombie never attacked the villager in the town"
            );
            self.mobs.push(Mob {
                kind: MobKind::Golem,
                position: self.mobs[1].position + Vec3::Z * 6.,
                health: 100.,
                ..Default::default()
            });
        }
        update_mobs(
            &mut self.mobs,
            &self.world,
            &mut self.player,
            0.05,
            true,
            Mode::Creative,
        );
        if frame == 179 {
            assert!(
                self.mobs[1].health <= 0.,
                "Town golem failed to kill the zombie"
            );
            assert!(
                self.mobs[0].health > 0.,
                "The villager wasn't defended in time"
            );
        }
    }

    fn prepare_mob_demo(&mut self) {
        for x in -12..=12 {
            for z in -8..=14 {
                for y in 40..=49 {
                    self.world
                        .set([x, y, z], if y == 40 { Block::Grass } else { Block::Air });
                }
            }
        }
        self.mobs = [
            MobKind::Sheep,
            MobKind::Zombie,
            MobKind::Villager,
            MobKind::Golem,
            MobKind::Creeper,
            MobKind::Cow,
            MobKind::Pig,
        ]
        .into_iter()
        .enumerate()
        .map(|(i, kind)| Mob {
            position: vec3(-6. + i as f32 * 2., 41., 0.),
            kind,
            health: 100.,
            yaw: std::f32::consts::PI,
            walking: true,
            ..Default::default()
        })
        .collect();
        self.player.position = vec3(0., 42., 12.);
        self.player.yaw = 0.;
        self.player.pitch = -0.10;
        self.player.flying = true;
        self.time = 0.18;
    }

    fn prepare_station_demo(&mut self) {
        self.mobs.clear();
        self.village_hint = None;
        self.radius = 5;
        for x in -5..=5 {
            for z in -4..=5 {
                for y in 40..=49 {
                    self.world
                        .set([x, y, z], if y == 40 { Block::Stone } else { Block::Air });
                }
            }
        }
        for (x, block) in [
            (-3, Block::Workbench),
            (-1, Block::Furnace),
            (1, Block::Chest),
            (3, Block::Door),
        ] {
            self.world.set([x, 41, -1], block);
        }
        self.player = Player::new([0., 41., 4.]);
        self.player.pitch = -0.19;
        self.player.flying = true;
        self.inventory.slots.fill(None);
        for (i, item) in [
            Item::RawMutton,
            Item::CookedMutton,
            Item::RawBeef,
            Item::CookedBeef,
            Item::RawPork,
            Item::CookedPork,
        ]
        .into_iter()
        .enumerate()
        {
            self.extras.drops.push(drops::DroppedItem {
                stack: settlements::stack(item, 1),
                position: [i as f32 * 0.65 - 1.6, 41.4, 1.6],
                velocity: [0.; 3],
                age: 2.,
            });
        }
    }

    fn prepare_shore_demo(&mut self) {
        let (x, z) = (-80..=80)
            .flat_map(|z| (-80..=80).map(move |x| (x * 8, z * 8)))
            .find(|&(x, z)| {
                self.world.height_at(x, z) < world::SEA - 2
                    && (world::SEA + 4..world::SEA + 18)
                        .contains(&self.world.height_at(x - 48, z + 24))
            })
            .expect("A natural lake shore");
        self.radius = 8;
        self.mobs.clear();
        self.village_hint = None;
        self.player.position = vec3(x as f32, world::SEA as f32 + 0.4, z as f32);
        let direction = vec3(-48., 3., 24.).normalize();
        self.player.yaw = direction.x.atan2(-direction.z);
        self.player.pitch = direction.y.asin();
        self.player.flying = true;
    }

    fn prepare_landscape_demo(&mut self, cave: bool) {
        let position = if cave {
            (-16..=16)
                .flat_map(|z| (-16..=16).map(move |x| (x, z)))
                .find_map(|(x, z)| self.world.cave_entrance(x, z))
                .expect("Walkable cave entrance")
        } else {
            (-128..=128)
                .flat_map(|z| (-128..=128).map(move |x| (x * 24, z * 24)))
                .map(|(x, z)| [x, self.world.height_at(x, z), z])
                .max_by_key(|p| p[1])
                .unwrap()
        };
        let center = Vec3::from_array(position.map(|n| n as f32));
        self.radius = 8;
        self.mobs.clear();
        self.village_hint = None;
        if cave {
            let [x, z] = self.world.cave_direction(position);
            self.player.position = center + vec3(-x as f32 * 12., 3., -z as f32 * 12.);
        } else {
            self.player.position = center + vec3(38., 12., 45.);
        }
        let direction = center + Vec3::Y - self.player.eye();
        self.player.yaw = direction.x.atan2(-direction.z);
        self.player.pitch = (direction.y / direction.length()).asin();
        self.player.flying = true;
        self.time = 0.18;
    }

    fn scripted_gameplay_check(&mut self, frame: u32) {
        match frame {
            0 => {
                self.prepare_village_demo(false);
                self.has_game = true;
                self.active_chest = Some(self.extras.chests[0].position);
                self.enter(Screen::Inventory);
            }
            1 => {
                self.ui = UiInput {
                    mouse: vec2(900., 205.),
                    left: true,
                    right: false,
                };
            }
            2 => {
                assert!(self.extras.chests[0]
                    .inventory
                    .slots
                    .iter()
                    .all(Option::is_none));
                assert!(self.inventory.count(Item::Emerald) > 0);
                self.ui = UiInput {
                    mouse: vec2(124., 467.),
                    left: true,
                    right: false,
                };
            }
            3 => {
                assert!(self.held.is_some());
                self.ui = UiInput {
                    mouse: vec2(124., 195.),
                    left: true,
                    right: false,
                };
            }
            4 => {
                assert!(self.held.is_none());
                assert!(self.extras.chests[0].inventory.slots[0].is_some());
                assert!(self.close_inventory());
                self.trading = true;
                self.inventory.add(Item::Wheat, 20);
                self.enter(Screen::Inventory);
            }
            5 => {
                self.ui = UiInput {
                    mouse: vec2(800., 185.),
                    left: true,
                    right: false,
                };
            }
            6 => {
                assert_eq!(self.inventory.count(Item::Wheat), 0);
                assert!(self.close_inventory());
                let population = self.mobs.len();
                assert!(self.save());
                self.load();
                assert_eq!(self.mobs.len(), population);
                assert!(self.extras.chests[0].inventory.slots[0].is_some());
                self.enter(Screen::Paused);
            }
            10 => {
                for x in -1..=1 {
                    for z in -4..=4 {
                        for y in 68..=75 {
                            self.world
                                .set([x, y, z], if y == 68 { Block::Stone } else { Block::Air });
                        }
                    }
                }
                self.player = Player::new([0.5, 69., 2.5]);
                self.player.yaw = 0.;
                self.player.pitch = 0.;
                self.inventory.selected = 0;
                self.world.set([0, 70, -1], Block::Stone);
                self.world.set([0, 70, 0], Block::Water);
                self.inventory.slots[0] = Some(settlements::stack(Item::Bucket, 1));
                assert!(self.use_bucket());
                assert_eq!(self.world.get([0, 70, 0]), Block::Air);
                assert_eq!(self.inventory.slots[0].unwrap().item, Item::WaterBucket);
                assert!(self.use_bucket());
                assert_eq!(self.world.get([0, 70, 0]), Block::Water);
                assert!(self.world.is_fluid_source([0, 70, 0]));
                assert_eq!(self.inventory.slots[0].unwrap().item, Item::Bucket);
                self.world.set([0, 70, 0], Block::Air);
                self.world.set([0, 70, -1], Block::Air);
            }
            11 => {
                self.world.set([0, 69, 1], Block::Door);
                for _ in 0..40 {
                    self.player.update(
                        &self.world,
                        Movement {
                            forward: 1.,
                            ..Default::default()
                        },
                        1. / 60.,
                        Mode::Creative,
                    );
                }
                assert!(
                    self.player.position.z >= 2.29,
                    "Closed door let the player pass"
                );
                self.world.set([0, 69, 1], Block::OpenDoor);
                for _ in 0..40 {
                    self.player.update(
                        &self.world,
                        Movement {
                            forward: 1.,
                            ..Default::default()
                        },
                        1. / 60.,
                        Mode::Creative,
                    );
                }
                assert!(self.player.position.z < 1.0, "Open door blocked the player");
            }
            _ => {}
        }
        set_cursor_grab(false);
        show_mouse(true);
    }

    fn scripted_weapon_check(&mut self, frame: u32) {
        match frame {
            20 => {
                self.enter(Screen::Paused);
                self.fire_weapon(true);
                assert_eq!(self.combat.shots, 0, "Un menu ne doit pas tirer");
            }
            22 => {
                self.enter(Screen::Playing);
                let active = self.mining.held(true, true, true);
                self.fire_weapon(active);
                assert_eq!(
                    self.combat.shots, 0,
                    "Relâchez le clic du menu avant de tirer"
                );
            }
            24 => {
                self.mining.held(true, false, false);
            }
            26 | 34 => {
                let active = self.mining.held(true, true, true);
                self.fire_weapon(active);
                let shots = if frame == 26 { 1 } else { 2 };
                assert_eq!(self.combat.shots, shots);
                assert_eq!(self.inventory.count(Item::Bullet), 64);
                assert_eq!(self.inventory.selected().unwrap().loaded, 30 - shots as u16);
                assert_eq!(self.mobs[0].health, 20. - shots as f32 * 6.);
                assert_eq!(
                    self.mobs[2].health, 20.,
                    "La cible derrière le mur doit rester intacte"
                );
            }
            27 => {
                self.fire_weapon(true);
                assert_eq!(self.combat.shots, 1, "Respecter la cadence de tir");
            }
            36 => {
                let active = self.mining.held(true, true, false);
                self.fire_weapon(active);
                assert_eq!(
                    self.combat.shots, 2,
                    "Un relâchement physique doit arrêter les tirs"
                );
            }
            44 => {
                self.inventory.consume(Item::Bullet, 64);
                self.inventory.slots[0].as_mut().unwrap().loaded = 0;
                self.fire_weapon(true);
                assert_eq!(self.combat.shots, 2, "Pas de tir sans munitions");
            }
            64 => {
                self.inventory.add(Item::Bullet, 64);
                self.inventory.slots[0].as_mut().unwrap().loaded = 28;
            }
            96 => {
                self.inventory.selected = 1;
            }
            147 => {
                self.player.pitch = 0.;
                self.fire_weapon(false);
            }
            148 => {
                self.inventory.selected = 1;
                self.mobs[0].health = 0.;
                self.fire_weapon(true);
                assert_eq!(self.combat.shots, 3);
                assert_eq!(self.inventory.count(Item::RocketAmmo), 16);
                assert_eq!(self.inventory.selected().unwrap().loaded, 0);
                assert_eq!(self.combat.rockets.len(), 1);
            }
            150 => {
                self.enter(Screen::Paused);
            }
            151..=159 => {
                assert!(
                    (self.combat.rockets[0].age - 2. / 60.).abs() < 0.00001,
                    "La pause doit immobiliser les roquettes"
                );
            }
            160 => {
                self.enter(Screen::Playing);
            }
            179 => {
                assert_eq!(self.combat.explosions, 1);
                assert!(self.combat.rockets.is_empty());
                assert!(
                    !self.combat.blasts.is_empty(),
                    "L'explosion doit avoir un effet visible"
                );
                assert_eq!(self.world.get([0, 41, -6]), Block::Air);
                assert_eq!(self.world.get([0, 40, -6]), Block::Bedrock);
                assert_eq!(
                    self.player.health, 20.,
                    "Le joueur loin de l'explosion reste intact"
                );
                self.toast_timer = 0.;
            }
            _ => {}
        }
        if self.screen == Screen::Playing {
            self.combat.tick_weapon(
                &mut self.inventory,
                self.mode,
                1. / 60.,
                WeaponInput::default(),
            );
            self.combat.update(
                &mut self.world,
                &mut self.mobs,
                &mut self.player,
                self.mode,
                1. / 60.,
            );
        }
        set_cursor_grab(false);
        show_mouse(true);
    }

    fn scripted_reload_check(&mut self, frame: u32) {
        match frame {
            0 => {
                self.inventory.slots[0].as_mut().unwrap().loaded = 2;
                self.fire_weapon(true);
                assert_eq!(self.combat.shots, 1);
            }
            8 => {
                self.fire_weapon(true);
                assert_eq!(self.inventory.selected().unwrap().loaded, 0);
                assert_eq!(self.combat.shots, 2);
            }
            16 => {
                self.fire_weapon(true);
                assert!(self.combat.reload.is_some());
                assert_eq!(self.inventory.count(Item::Bullet), 64);
            }
            40 => self.enter(Screen::Paused),
            41..=49 => {
                assert!((self.combat.reload.as_ref().unwrap().elapsed - 24. / 60.).abs() < 0.0001)
            }
            50 => self.enter(Screen::Playing),
            178 => {
                assert!(self.combat.reload.is_none());
                assert_eq!(self.combat.reloads, 1);
                assert_eq!(self.inventory.selected().unwrap().loaded, 30);
                assert_eq!(self.inventory.count(Item::Bullet), 34);
                assert_eq!(self.combat.shots, 2);
                self.combat.aim = 1.;
            }
            _ => {}
        }
        if self.screen == Screen::Playing {
            self.combat.tick_weapon(
                &mut self.inventory,
                self.mode,
                1. / 60.,
                WeaponInput {
                    aiming: frame >= 178,
                    ..Default::default()
                },
            );
            self.combat.update(
                &mut self.world,
                &mut self.mobs,
                &mut self.player,
                self.mode,
                1. / 60.,
            );
        }
        set_cursor_grab(false);
        show_mouse(true);
    }

    fn scripted_input_check(&mut self, frame: u32) {
        self.ui.left = false;
        self.ui.right = false;
        match frame {
            0 | 4 | 22 | 24 | 30 | 32 | 38 | 40 | 44 | 46 | 56 | 57 | 66 => {
                self.handle_menu_keys(true, false);
            }
            6 => {
                self.handle_menu_keys(false, true);
            }
            10 => {
                self.ui = UiInput {
                    mouse: vec2(1140.0, 105.0),
                    left: true,
                    right: false,
                };
            }
            12 => {
                assert!(
                    !self.mining.held(true, true, true),
                    "Un clic du menu ne doit pas miner"
                );
                let hit = RayHit {
                    block: [4, 70, 4],
                    previous: [4, 70, 3],
                    distance: 2.0,
                };
                self.world.set(hit.block, Block::Stone);
                self.mine_progress = 0.99;
                let active = self.mining.held(true, true, false);
                self.mine(Some(&hit), 0.05, active);
                assert_eq!(
                    self.world.get(hit.block),
                    Block::Stone,
                    "Le relâchement doit annuler le minage même avec un événement périmé"
                );
                assert_eq!(self.mine_progress, 0.0);
                assert!(self.mining.held(true, true, true));
                self.world.set(hit.block, Block::Air);
            }
            18 | 34 | 48 | 54 | 58 | 64 => {
                self.handle_creative_key();
            }
            19 => {
                self.ui = UiInput {
                    mouse: vec2(125.0, 239.0),
                    left: true,
                    right: false,
                };
            }
            20 => {
                self.ui = UiInput {
                    mouse: vec2(685.0, 628.0),
                    left: true,
                    right: false,
                };
            }
            26 => {
                self.ui = UiInput {
                    mouse: vec2(610.0, 600.0),
                    left: true,
                    right: false,
                }
            }
            27 => {
                self.ui = UiInput {
                    mouse: vec2(520.0, 309.0),
                    left: true,
                    right: false,
                }
            }
            28 => {
                assert!(rebind(&mut self.bindings, Action::Jump, KeyCode::J));
                self.rebinding = None;
                // Exercise persistence beside the harness's isolated save path.
                self.persist_preferences();
                assert_eq!(
                    Preferences::load(&self.preferences_path)
                        .unwrap()
                        .decode_bindings()
                        .unwrap()[Action::Jump as usize],
                    KeyCode::J
                );
            }
            29 | 43 => {
                self.ui = UiInput {
                    mouse: vec2(260., 578.),
                    left: true,
                    right: false,
                }
            }
            31 => {
                self.restore_preferences();
                assert!(
                    !self.renderer.enhanced,
                    "Le réglage graphique doit être conservé"
                );
            }
            42 => {
                self.ui = UiInput {
                    mouse: vec2(610., 600.),
                    left: true,
                    right: false,
                }
            }
            35 => {
                self.ui = UiInput {
                    mouse: vec2(685.0, 628.0),
                    left: false,
                    right: true,
                }
            }
            36 | 37 => {
                self.ui = UiInput {
                    mouse: vec2(125., 445.),
                    left: frame == 37,
                    right: frame == 36,
                };
            }
            49 | 59 => {
                self.ui = UiInput {
                    mouse: vec2(640., 160.),
                    left: true,
                    right: false,
                }
            }
            50 | 60 => {
                self.ui = UiInput {
                    mouse: vec2(562., 219.),
                    left: true,
                    right: false,
                }
            }
            51 | 61 => {
                self.inventory.selected = 1;
                self.ui = UiInput {
                    mouse: vec2(195., 445.),
                    left: true,
                    right: false,
                };
            }
            52 | 62 => {
                self.ui = UiInput {
                    mouse: vec2(550., 689.),
                    left: true,
                    right: false,
                }
            }
            53 => {
                self.ui = UiInput {
                    mouse: vec2(350., 689.),
                    left: true,
                    right: false,
                }
            }
            63 => {
                self.inventory.slots[1].as_mut().unwrap().loaded = 3;
                self.ui = UiInput {
                    mouse: vec2(160., 689.),
                    left: true,
                    right: false,
                };
            }
            65 => {
                assert_eq!(self.extras.drops.len(), 1);
                let drop = &mut self.extras.drops[0];
                assert_eq!(drop.stack.loaded, 3);
                assert_eq!(drop.stack.optic, 1);
                drop.age = 2.;
                drop.position = (self.player.position + Vec3::Y * 0.8).to_array();
                let p = drop.position;
                self.world.set(
                    Vec3::from_array(p).floor().as_ivec3().to_array(),
                    Block::Air,
                );
                assert_eq!(
                    self.extras
                        .update(&self.world, &self.player, &mut self.inventory, 0.),
                    1
                );
                assert_eq!(self.inventory.slots[1].unwrap().loaded, 3);
                assert_eq!(self.inventory.slots[1].unwrap().optic, 1);
            }
            _ => {}
        }
        set_cursor_grab(false);
        show_mouse(true);
    }

    fn assert_input_check(&self, frame: u32) {
        match frame {
            0..=3 | 24..=25 | 30..=31 | 40..=41 | 44..=45 | 56 | 66.. => assert_eq!(
                self.screen,
                Screen::Paused,
                "Le menu pause doit rester ouvert"
            ),
            6..=9 => assert_eq!(
                self.screen,
                Screen::Inventory,
                "L'inventaire doit rester ouvert"
            ),
            18..=21 | 34..=37 | 48..=53 | 58..=63 => assert_eq!(self.screen, Screen::Creative),
            26..=29 | 42..=43 => assert_eq!(self.screen, Screen::Controls),
            _ => assert_eq!(self.screen, Screen::Playing),
        }
        if frame == 19 {
            assert!(self.held.is_some());
            assert!(self.inventory.slots[0].is_none());
        }
        if frame == 20 {
            assert!(self.held.is_none());
            assert_eq!(
                self.inventory.slots[35].unwrap().item,
                Item::Block(Block::Grass)
            );
            assert_eq!(self.inventory.count(Item::Block(Block::Grass)), 64);
        }
        if frame == 27 {
            assert_eq!(self.rebinding, Some(Action::Jump));
        }
        if frame == 28 {
            assert_eq!(self.key(Action::Jump), "J");
        }
        if frame == 29 {
            assert!(!self.renderer.enhanced);
        }
        if frame == 43 {
            assert!(self.renderer.enhanced);
        }
        if frame == 49 || frame == 59 {
            assert_eq!(self.inventory_tab, 3);
        }
        if frame == 50 || frame == 60 {
            assert_eq!(self.held.unwrap().item, Item::M200);
        }
        if frame == 51 || frame == 61 {
            assert_eq!(self.inventory.slots[1].unwrap().loaded, 7);
        }
        if frame == 52 || frame == 62 {
            assert_eq!(self.inventory.slots[1].unwrap().optic, 1);
        }
        if frame == 53 {
            assert!(self.inventory.slots[1].is_none());
            assert!(catalog(3).contains(&Item::M200));
        }
        if frame == 63 {
            assert!(self.inventory.slots[1].is_none());
            assert_eq!(self.extras.drops.len(), 1);
        }
        if frame == 35 {
            assert_eq!(self.held.unwrap().count, 32);
            assert_eq!(self.inventory.slots[35].unwrap().count, 32);
        }
        if frame == 36 {
            assert_eq!(self.held.unwrap().count, 31);
            assert_eq!(self.inventory.slots[0].unwrap().count, 1);
        }
        if frame == 37 {
            assert!(self.held.is_none());
            assert_eq!(self.inventory.slots[0].unwrap().count, 32);
            assert_eq!(self.inventory.count(Item::Block(Block::Grass)), 64);
        }
    }
}

fn text(label: &str, x: f32, y: f32, size: f32, color: Color) {
    draw_text(label, x, y, size, color);
}

fn key_variants(key: KeyCode) -> [KeyCode; 2] {
    [
        key,
        match key {
            KeyCode::LeftShift => KeyCode::RightShift,
            KeyCode::LeftControl => KeyCode::RightControl,
            KeyCode::LeftAlt => KeyCode::RightAlt,
            _ => key,
        },
    ]
}

fn camera_delta(
    previous: &mut Option<Vec2>,
    previous_size: &mut Vec2,
    mouse: Vec2,
    size: Vec2,
) -> Vec2 {
    let delta = if *previous_size == size {
        previous.map_or(Vec2::ZERO, |p| mouse - p)
    } else {
        Vec2::ZERO
    };
    *previous = Some(mouse);
    *previous_size = size;
    delta
}

fn tooltip_bounds(mouse: Vec2, width: f32, height: f32) -> Rect {
    let x = if mouse.x + 18.0 + width > 1270.0 {
        mouse.x - width - 18.0
    } else {
        mouse.x + 18.0
    };
    let y = if mouse.y + 24.0 + height > 790.0 {
        mouse.y - height - 12.0
    } else {
        mouse.y + 24.0
    };
    Rect::new(
        x.clamp(10.0, 1270.0 - width),
        y.clamp(10.0, 790.0 - height),
        width,
        height,
    )
}

fn wrap_text(body: &str, width: f32, size: u16) -> Vec<String> {
    let mut lines = Vec::new();
    for paragraph in body.split('\n') {
        let mut line = String::new();
        for word in paragraph.split_whitespace() {
            let candidate = if line.is_empty() {
                word.into()
            } else {
                format!("{line} {word}")
            };
            if !line.is_empty() && measure_text(&candidate, None, size, 1.0).width > width {
                lines.push(line);
                line = word.into();
            } else {
                line = candidate;
            }
        }
        lines.push(line);
    }
    lines
}

fn catalog(tab: usize) -> Vec<Item> {
    use Block::*;
    match tab {
        0 => [
            Grass, Dirt, Stone, Sand, Wood, Leaves, Water, Planks, Cobble, Glass, Brick, CoalOre,
            IronOre, Torch, Workbench, Furnace, Snow, Bedrock, Lava, Farmland, Wheat0, Wheat1,
            Wheat2, Wheat3, Bed, Sandstone, Path, Chest, Door,
        ]
        .into_iter()
        .map(Item::Block)
        .collect(),
        1 => vec![
            Item::Apple,
            Item::Bread,
            Item::RawMutton,
            Item::CookedMutton,
            Item::RawBeef,
            Item::CookedBeef,
            Item::RawPork,
            Item::CookedPork,
        ],
        2 => vec![
            Item::Block(Workbench),
            Item::Block(Furnace),
            Item::Block(Bed),
            Item::Block(Chest),
            Item::Block(Door),
            Item::Emerald,
            Item::Wool,
            Item::Gunpowder,
            Item::Bucket,
            Item::WaterBucket,
            Item::LavaBucket,
            Item::Block(Torch),
            Item::WoodPick,
            Item::StonePick,
            Item::IronPick,
            Item::WoodAxe,
            Item::StoneAxe,
            Item::WoodHoe,
            Item::Seeds,
            Item::Wheat,
            Item::Stick,
            Item::Coal,
            Item::IronIngot,
        ],
        _ => weapons::GUNS
            .into_iter()
            .chain(weapons::AMMO)
            .chain([Item::Sword, Item::HandGrenade, Item::C4])
            .chain((1..=5).flat_map(|t| [Item::PlateCarrier(t), Item::BallisticHelmet(t)]))
            .collect(),
    }
}

fn city_catalog(tab: usize) -> Vec<Item> {
    let mut entries = catalog(tab);
    if tab == 0 || tab == 2 {
        let city = city::get().expect("City map");
        let mut names = std::collections::HashSet::new();
        for (id, state) in city.metadata.states.iter().enumerate() {
            if matches!(
                state.material,
                Block::Air | Block::Water | Block::Lava | Block::Bedrock
            ) || state.properties.get("half").is_some_and(|p| p == "upper")
                || (tab == 2
                    && !matches!(
                        state.material,
                        Block::Workbench
                            | Block::Furnace
                            | Block::Chest
                            | Block::Bed
                            | Block::Door
                            | Block::Torch
                    ))
            {
                continue;
            }
            if names.insert(&state.source) {
                entries.push(Item::Block(Block::Map(id as u16)));
            }
        }
    }
    entries
}

fn weapon_ammo(item: Item) -> Option<Item> {
    weapons::spec(item).map(|s| s.ammo)
}

fn selected_weapon(inventory: &Inventory) -> Option<Item> {
    inventory
        .selected()
        .map(|s| s.item)
        .filter(|item| weapon_ammo(*item).is_some())
}

fn item_help(item: Item) -> &'static str {
    if let Some(spec) = weapons::spec(item) {
        return if matches!(item, Item::M200 | Item::Aw50) {
            "Gauche : tirer · Droit : lunette · R : recharger\nO : optique ×4 / ×8 / ×12 · I : inspecter\nCe calibre brise le verre et les blocs en bois."
        } else if spec.scoped {
            "Clic gauche : tirer · Droit : lunette · R : recharger
O : optique ×4 / ×8 / ×12 · I : inspecter"
        } else if item == Item::RocketLauncher {
            "Grenade de 40mm : trajectoire courbe, explosion à l'impact.
R : ouvrir la culasse et recharger · Droit : viser"
        } else {
            "Gauche : tirer · Droit : viser · R : recharger
B : changer de mode · I : inspecter · M16 : semi / rafale"
        };
    }
    if weapons::AMMO.contains(&item) {
        return "Munitions partagées par les armes de ce calibre.
Gardez-les dans le sac, puis rechargez avec R.";
    }
    match item {
        Item::Block(Block::Water) => "Source d'eau : tombe, s'étale et refroidit la lave.",
        Item::Block(Block::Lava) => {
            "Source de lave : éclaire et brûle en survie. L'eau la solidifie."
        }
        Item::Block(Block::Workbench) => {
            "Clic droit : ouvrir la fabrication avancée. À fabriquer à la main avec 4 bois."
        }
        Item::Block(Block::Furnace) => {
            "Clic droit : ouvrir les recettes de cuisson, avec du charbon."
        }
        Item::Block(Block::Bed) => "Clic droit : dormir et définir le point de réapparition.",
        Item::Block(Block::Torch) => {
            "Torche : éclaire les blocs voisins, utile sous terre et la nuit."
        }
        Item::Block(Block::Bedrock) => "Socle indestructible.",
        Item::Block(Block::IronOre) => "Minerai de fer : nécessite une pioche en pierre ou en fer.",
        Item::Block(Block::Farmland) => {
            "Terre labourée : semez du blé. Une eau à moins de 4 blocs irrigue la parcelle."
        }
        Item::Block(Block::Wheat0 | Block::Wheat1 | Block::Wheat2 | Block::Wheat3) => {
            "Culture : laissez-la mûrir puis récoltez au clic gauche."
        }
        Item::Block(Block::Chest)=>"Clic droit : ouvrir les 27 cases. Maj-clic transfère. Le contenu est sauvegardé.",
        Item::Block(Block::Door|Block::OpenDoor)=>"Clic droit : ouvrir ou fermer. Accroupi + droit : placer.",
        Item::Block(_) => "Bloc de construction : clic droit pour le placer.",
        Item::Apple => "Clic droit : manger si vous avez faim (+6 faim et +2 vie).",
        Item::WoodPick => "Pioche : récolte la pierre et le charbon.",
        Item::StonePick | Item::IronPick => {
            "Pioche : récolte les minerais, dont le fer, et accélère le minage."
        }
        Item::WoodAxe | Item::StoneAxe => "Hache : coupe le bois plus rapidement.",
        Item::Sword => "Épée : augmente les dégâts aux créatures.",
        Item::Coal => "Combustible pour le four et ingrédient des torches.",
        Item::IronIngot => "Métal pour fabriquer une pioche en fer et une épée.",
        Item::Stick => "Ingrédient pour les outils et les torches.",
        Item::WoodHoe => {
            "Clic droit sur terre / herbe libre pour labourer, puis semez des graines."
        }
        Item::Seeds => {
            "Clic droit sur une terre labourée. Eau à moins de 4 blocs et lumière pour pousser."
        }
        Item::Wheat => "Récolte du blé mûr. Trois blés permettent de fabriquer un pain.",
        Item::Bread => "Clic droit : manger si vous avez faim (+8 faim et +2 vie).",
        Item::RawMutton|Item::RawPork|Item::RawBeef=>"Viande : +3 faim. Cuisez-la avec du charbon près d’un four.",
        Item::CookedMutton=>"Mouton cuit : clic droit pour manger (+8 faim).",
        Item::CookedPork=>"Porc cuit : clic droit pour manger (+9 faim).",
        Item::CookedBeef=>"Viande cuite : clic droit pour manger (+10 faim).",
        Item::Emerald=>"Monnaie des villages. Clic droit sur un villageois pour échanger.",
        Item::Wool=>"Les moutons donnent de la laine. Trois laines et trois planches fabriquent un lit.",
        Item::Gunpowder=>"Les creepers lâchent de la poudre quand vous les éliminez.",
        Item::Bucket|Item::WaterBucket|Item::LavaBucket=>"Clic droit : recueillir une source ou verser le seau. Les écoulements continuent à se propager.",
        Item::Ak => "Gauche : tirer · Droit : viser · R : recharger (30 balles) · B : auto/semi · I : inspecter. Les murs arrêtent les balles.",
        Item::RocketLauncher => "Gauche : lancer · Droit : viser · R : recharger. Une roquette par chargeur. Explosion : dégâts de zone et destruction des blocs. Éloignez-vous en survie.",
        Item::Bullet => "Munitions pour l'AK. Gardez-les dans l'inventaire ; une balle est consommée par tir en survie.",
        Item::PlateCarrier(_)=>"Clic droit ou emplacement du sac pour équiper.
Chaque tier réduit de 10 % les dégâts de combat.",
        Item::BallisticHelmet(_)=>"Clic droit ou emplacement du sac pour équiper.
Chaque tier réduit de 5 % les dégâts de combat.",
        Item::HandGrenade=>"Clic droit : lancer. Rebondit et explose après 3,2 s.
Les murs réduisent les dégâts ; éloignez-vous en survie.",
        Item::C4=>"Clic droit sur une surface : poser. R : déclencher à 128 blocs.
R fonctionne aussi main vide. Les charges posées sont sauvegardées.",
        _ => "Équipement disponible dans le catalogue créatif.",
    }
}

fn button_help(label: &str, rect: Rect, screen: Screen) -> &'static str {
    match label {
        "CRÉATIF" => "Blocs illimités, vol et aucun dégât.",
        "SURVIE" => "Récolte, fabrication, vie et faim. Attention à la lave et aux zombies.",
        "R" => "Choisir une graine aléatoire.",
        "EXPLORER LA VILLE  →" => "Démarrer avec le mode et la graine choisis. Un seul emplacement de sauvegarde.",
        "REPRENDRE LA PARTIE" => "Continuer la partie en cours ou charger la sauvegarde.",
        "REPRENDRE  →" => "Reprendre le jeu. Raccourci : Échap. Relâchez le clic avant de miner.",
        "FERMER" => "Fermer l'inventaire. Échap ou la touche d'inventaire font aussi l'affaire.",
        "RETOUR" => "Revenir aux réglages. Raccourci : Échap.",
        "SAUVEGARDER  /  F5" => "Enregistrer le monde et l'inventaire. Raccourci : F5 en jeu ou en pause.",
        "SAUVEGARDER ET ACCUEIL" => "Enregistrer puis revenir au menu principal.",
        "SAUVEGARDER ET QUITTER" => "Enregistrer avant de fermer le jeu.",
        "COMMANDES & SOURIS" => "Modifier les touches, inverser la caméra et choisir le mode de course.",
        "RÉTABLIR LES TOUCHES" => "Restaurer les touches par défaut du clavier choisi.",
        "←" => "Page de recettes précédente.", "→" => "Page de recettes suivante.",
        "−" | "+" if screen == Screen::Paused && rect.y < 490.0 => "Ajuster la distance d'affichage : plus de chunks demandent plus de calcul.",
        "−" | "+" if screen == Screen::Paused => "Ajuster la sensibilité de la caméra. Une valeur faible facilite les mouvements précis.",
        "QUITTER" => "Fermer le jeu. La partie en cours sera sauvegardée.",
        "REVENIR AU POINT DE DÉPART" => "Réapparaître avec votre inventaire conservé.",
        _ if label.starts_with("CLAVIER") => "Changer les libellés AZERTY / QWERTY. Les touches personnalisées sont conservées.",
        _ if label.starts_with("SONS") => "Activer ou couper les effets sonores.",
        _ if label.starts_with("AXE VERTICAL") => "Choisir le sens de la caméra lorsque la souris monte ou descend.",
        _ if label.starts_with("COURSE") => "Maintenir la touche pour courir, ou appuyer une fois pour activer puis désactiver la course.",
        _ if label.starts_with("SHADERS") => "Activer les reflets de l'eau, la lumière solaire, le vent, le halo lumineux et les couleurs améliorées. Le mode classique réduit la charge graphique.",
        _ => "Cliquer pour utiliser cette commande.",
    }
}

#[cfg(target_os = "windows")]
#[link(name = "user32")]
extern "system" {
    fn GetAsyncKeyState(key: i32) -> i16;
    fn GetForegroundWindow() -> *mut std::ffi::c_void;
    fn GetWindowThreadProcessId(window: *mut std::ffi::c_void, process: *mut u32) -> u32;
    fn GetSystemMetrics(index: i32) -> i32;
}

fn window_focused() -> bool {
    #[cfg(target_os = "windows")]
    {
        // SAFETY: user32 functions accept a valid output pointer and do not retain it.
        unsafe {
            let window = GetForegroundWindow();
            if window.is_null() {
                return false;
            }
            let mut process = 0;
            GetWindowThreadProcessId(window, &mut process);
            process == std::process::id()
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        true
    }
}

fn physical_mouse_down(button: MouseButton) -> bool {
    #[cfg(target_os = "windows")]
    {
        // An up event can be lost outside the window. Poll the current physical state,
        // respecting Windows' swapped mouse buttons, rather than trusting stale events.
        if !window_focused() {
            return false;
        }
        // SAFETY: documented user32 scalar queries; no pointers or retained memory.
        unsafe {
            let swapped = GetSystemMetrics(23) != 0;
            let key = match button {
                MouseButton::Left => {
                    if swapped {
                        2
                    } else {
                        1
                    }
                }
                MouseButton::Right => {
                    if swapped {
                        1
                    } else {
                        2
                    }
                }
                MouseButton::Middle => 4,
                _ => return false,
            };
            GetAsyncKeyState(key) < 0
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        is_mouse_button_down(button)
    }
}
fn shade() {
    draw_rectangle(
        0.0,
        0.0,
        1280.0,
        800.0,
        Color::new(0.015, 0.03, 0.055, 0.70),
    );
}

fn ui_geometry() -> (f32, f32, f32) {
    let scale = (screen_width() / 1280.0).min(screen_height() / 800.0);
    (
        scale,
        (screen_width() - 1280.0 * scale) * 0.5,
        (screen_height() - 800.0 * scale) * 0.5,
    )
}
fn ui_mouse() -> Vec2 {
    let (scale, x, y) = ui_geometry();
    let p = mouse_position();
    vec2((p.0 - x) / scale, (p.1 - y) / scale)
}
fn begin_ui() {
    let (scale, x, y) = ui_geometry();
    set_camera(&Camera2D {
        target: vec2(640.0, 400.0),
        zoom: vec2(2.0 / 1280.0, 2.0 / 800.0),
        viewport: Some((
            x as i32,
            y as i32,
            (1280.0 * scale) as i32,
            (800.0 * scale) as i32,
        )),
        ..Default::default()
    });
}
fn button(rect: Rect, label: &str, active: bool, input: UiInput) -> bool {
    let hover = rect.contains(input.mouse);
    let bg = if active {
        ACCENT
    } else if hover {
        Color::new(0.18, 0.26, 0.29, 1.0)
    } else {
        Color::new(0.105, 0.16, 0.19, 1.0)
    };
    draw_rectangle(rect.x, rect.y, rect.w, rect.h, bg);
    if hover {
        draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 1.0, PAPER);
    }
    let size = if rect.w < 100.0 { 16 } else { 18 };
    let width = measure_text(label, None, size, 1.0).width;
    text(
        label,
        rect.x + (rect.w - width) * 0.5,
        rect.y + rect.h * 0.5 + size as f32 * 0.34,
        size as f32,
        if active { INK } else { PAPER },
    );
    hover && input.left
}
fn block_color(block: Block, face: usize) -> Color {
    let c = block.color(face);
    Color::from_rgba(c[0], c[1], c[2], 255)
}
fn icon(item: Item, x: f32, y: f32, size: f32) {
    if item_icons::draw(item, x, y, size) {
        return;
    }
    if let Some(index) = weapons::GUNS.iter().position(|&gun| gun == item) {
        thread_local! { static IMAGES: Vec<Texture2D> = vec![
            Texture2D::from_file_with_format(include_bytes!("../assets/weapons/real/ak74u-icon.png"),Some(ImageFormat::Png)),
            Texture2D::from_file_with_format(include_bytes!("../assets/weapons/real/ak74-icon.png"),Some(ImageFormat::Png)),
            Texture2D::from_file_with_format(include_bytes!("../assets/weapons/real/akm-icon.png"),Some(ImageFormat::Png)),
            Texture2D::from_file_with_format(include_bytes!("../assets/weapons/real/m4a1-icon.png"),Some(ImageFormat::Png)),
            Texture2D::from_file_with_format(include_bytes!("../assets/weapons/real/famas-icon.png"),Some(ImageFormat::Png)),
            Texture2D::from_file_with_format(include_bytes!("../assets/weapons/real/m16a4-icon.png"),Some(ImageFormat::Png)),
            Texture2D::from_file_with_format(include_bytes!("../assets/weapons/real/m79-icon.png"),Some(ImageFormat::Png)),
            Texture2D::from_file_with_format(include_bytes!("../assets/weapons/real/m200-icon.png"),Some(ImageFormat::Png)),
            Texture2D::from_file_with_format(include_bytes!("../assets/weapons/real/tundra-icon.png"),Some(ImageFormat::Png)),
            Texture2D::from_file_with_format(include_bytes!("../assets/weapons/real/aw50-icon.png"),Some(ImageFormat::Png)),
            Texture2D::from_file_with_format(include_bytes!("../assets/weapons/real/svd-icon.png"),Some(ImageFormat::Png)),
            Texture2D::from_file_with_format(include_bytes!("../assets/weapons/real/mp5k-icon.png"),Some(ImageFormat::Png)),
        ]; }
        IMAGES.with(|images| {
            draw_texture_ex(
                &images[index],
                x - size * 0.6,
                y - size * 0.3,
                WHITE,
                DrawTextureParams {
                    dest_size: Some(vec2(size * 1.2, size * 0.6)),
                    ..Default::default()
                },
            )
        });
        return;
    }

    if let Some(index) = weapons::AMMO.iter().position(|&ammo| ammo == item) {
        let brass = Color::from_rgba(203, 161, 71, 255);
        let tip = [
            Color::from_rgba(183, 108, 55, 255),
            Color::from_rgba(137, 173, 108, 255),
            Color::from_rgba(188, 185, 167, 255),
            Color::from_rgba(108, 126, 65, 255),
            Color::from_rgba(178, 110, 97, 255),
            Color::from_rgba(101, 146, 183, 255),
            Color::from_rgba(204, 204, 211, 255),
            Color::from_rgba(146, 112, 176, 255),
            Color::from_rgba(211, 98, 90, 255),
        ][index];
        if item == Item::RocketAmmo {
            draw_rectangle(
                x - size * 0.24,
                y - size * 0.14,
                size * 0.48,
                size * 0.47,
                brass,
            );
            draw_ellipse(x, y - size * 0.14, size * 0.24, size * 0.24, 0., tip);
            draw_rectangle(
                x - size * 0.28,
                y + size * 0.29,
                size * 0.56,
                size * 0.07,
                brass,
            );
        } else {
            for offset in [-0.23, 0.12] {
                let bx = x + size * offset;
                draw_rectangle(bx, y - size * 0.16, size * 0.15, size * 0.49, brass);
                draw_triangle(
                    vec2(bx, y - size * 0.16),
                    vec2(bx + size * 0.15, y - size * 0.16),
                    vec2(bx + size * 0.075, y - size * 0.39),
                    tip,
                );
                draw_rectangle(bx, y + size * 0.11, size * 0.15, size * 0.07, tip);
            }
        }
        return;
    }
    if item == Item::Block(Block::Bed) {
        let wood = Color::from_rgba(139, 93, 57, 255);
        draw_rectangle(
            x - size * 0.45,
            y - size * 0.16,
            size * 0.9,
            size * 0.47,
            wood,
        );
        draw_rectangle(
            x - size * 0.43,
            y - size * 0.27,
            size * 0.86,
            size * 0.35,
            Color::from_rgba(189, 55, 46, 255),
        );
        draw_rectangle(
            x - size * 0.42,
            y - size * 0.25,
            size * 0.26,
            size * 0.31,
            PAPER,
        );
        for dx in [-0.4, 0.3] {
            draw_rectangle(x + size * dx, y + size * 0.2, size * 0.12, size * 0.2, wood);
        }
        return;
    }
    if let Some(block) = item.block() {
        let s = size * 0.50;
        let top = vec2(x, y - s * 0.75);
        let left = vec2(x - s, y - s * 0.25);
        let mid = vec2(x, y + s * 0.25);
        let right = vec2(x + s, y - s * 0.25);
        let bottom = vec2(x, y + s * 1.25);
        let bl = vec2(x - s, y + s * 0.75);
        let br = vec2(x + s, y + s * 0.75);
        draw_triangle(top, left, mid, block_color(block, 2));
        draw_triangle(top, mid, right, block_color(block, 2));
        draw_triangle(left, bl, bottom, block_color(block, 0));
        draw_triangle(left, bottom, mid, block_color(block, 0));
        let c = block_color(block, 4);
        let dark = Color::new(c.r * 0.75, c.g * 0.75, c.b * 0.75, 1.0);
        draw_triangle(mid, bottom, br, dark);
        draw_triangle(mid, br, right, dark);
    } else {
        let metal = match item {
            Item::WoodPick | Item::WoodAxe | Item::WoodHoe => Color::from_rgba(181, 131, 77, 255),
            Item::StonePick | Item::StoneAxe => Color::from_rgba(163, 170, 163, 255),
            _ => PAPER,
        };
        match item {
            Item::Seeds => {
                for (a, b) in [(-0.18, 0.10), (0.16, 0.0), (0.0, -0.20)] {
                    draw_ellipse(
                        x + size * a,
                        y + size * b,
                        size * 0.13,
                        size * 0.18,
                        0.,
                        ACCENT,
                    );
                }
            }
            Item::Wheat => {
                draw_line(
                    x - 3.,
                    y + size * 0.35,
                    x + 3.,
                    y - size * 0.30,
                    3.,
                    Color::from_rgba(218, 182, 64, 255),
                );
                for offset in 0..4 {
                    let top = y - size * 0.30 + offset as f32 * size * 0.13;
                    draw_line(x + 2., top, x - size * 0.17, top - size * 0.10, 5., YELLOW);
                    draw_line(x + 2., top, x + size * 0.21, top - size * 0.12, 5., YELLOW);
                }
            }
            Item::Emerald => {
                draw_poly(
                    x,
                    y,
                    4,
                    size * 0.42,
                    0.,
                    Color::from_rgba(48, 212, 115, 255),
                );
                draw_line(x - 6., y - 3., x + 2., y - 10., 3., PAPER);
            }
            Item::Wool => {
                draw_rectangle(
                    x - size * 0.34,
                    y - size * 0.30,
                    size * 0.68,
                    size * 0.60,
                    PAPER,
                );
                draw_rectangle_lines(
                    x - size * 0.34,
                    y - size * 0.30,
                    size * 0.68,
                    size * 0.60,
                    2.,
                    MUTED,
                );
            }
            Item::RawBeef | Item::CookedBeef => {
                draw_ellipse(
                    x,
                    y,
                    size * 0.40,
                    size * 0.25,
                    -25.,
                    if item == Item::RawBeef {
                        Color::from_rgba(192, 69, 67, 255)
                    } else {
                        Color::from_rgba(135, 79, 42, 255)
                    },
                );
                draw_circle(x + size * 0.2, y, size * 0.10, PAPER);
            }
            Item::Gunpowder => {
                draw_poly(x, y, 5, size * 0.36, 20., Color::from_rgba(96, 99, 94, 255));
            }
            Item::Bucket | Item::WaterBucket | Item::LavaBucket => {
                draw_rectangle(
                    x - size * 0.3,
                    y - size * 0.15,
                    size * 0.6,
                    size * 0.48,
                    MUTED,
                );
                draw_ellipse(
                    x,
                    y - size * 0.15,
                    size * 0.30,
                    size * 0.12,
                    0.,
                    match item {
                        Item::WaterBucket => BLUE,
                        Item::LavaBucket => ORANGE,
                        _ => INK,
                    },
                );
                draw_ellipse_lines(x, y - size * 0.2, size * 0.27, size * 0.33, 0., 2., PAPER);
            }
            Item::Bread => {
                draw_ellipse(
                    x,
                    y,
                    size * 0.43,
                    size * 0.28,
                    -12.,
                    Color::from_rgba(192, 123, 46, 255),
                );
                for offset in [-0.18, 0., 0.18] {
                    draw_line(
                        x + offset * size,
                        y - size * 0.10,
                        x + offset * size + 2.,
                        y + size * 0.06,
                        3.,
                        Color::from_rgba(246, 209, 138, 255),
                    );
                }
            }
            Item::Apple => {
                draw_circle(x - 4.0, y, size * 0.30, Color::from_rgba(220, 81, 63, 255));
                draw_circle(x + 4.0, y, size * 0.30, Color::from_rgba(239, 99, 64, 255));
                draw_line(x, y - size * 0.2, x + 3.0, y - size * 0.5, 3.0, ACCENT);
            }
            Item::Coal => {
                draw_poly(
                    x,
                    y,
                    5,
                    size * 0.40,
                    20.0,
                    Color::from_rgba(63, 70, 73, 255),
                );
                draw_line(x - 5.0, y - 3.0, x + 3.0, y - 8.0, 3.0, MUTED);
            }
            Item::IronIngot => {
                draw_rectangle(
                    x - size * 0.43,
                    y - size * 0.23,
                    size * 0.86,
                    size * 0.46,
                    metal,
                );
                draw_line(
                    x - size * 0.35,
                    y - size * 0.18,
                    x + size * 0.32,
                    y - size * 0.18,
                    3.0,
                    WHITE,
                );
            }
            Item::Stick => draw_line(
                x - size * 0.25,
                y + size * 0.35,
                x + size * 0.25,
                y - size * 0.35,
                5.0,
                Color::from_rgba(163, 114, 68, 255),
            ),
            _ => {
                draw_line(
                    x - size * 0.25,
                    y + size * 0.37,
                    x + size * 0.22,
                    y - size * 0.25,
                    5.0,
                    Color::from_rgba(155, 109, 60, 255),
                );
                if item == Item::WoodHoe {
                    draw_rectangle(
                        x + size * 0.10,
                        y - size * 0.36,
                        size * 0.33,
                        size * 0.19,
                        metal,
                    );
                } else if item == Item::Sword {
                    draw_line(x, y, x + size * 0.38, y - size * 0.47, 7.0, metal);
                    draw_line(
                        x - size * 0.15,
                        y - size * 0.1,
                        x + size * 0.15,
                        y + size * 0.1,
                        4.0,
                        ACCENT,
                    );
                } else {
                    draw_line(
                        x - size * 0.27,
                        y - size * 0.34,
                        x + size * 0.34,
                        y - size * 0.19,
                        8.0,
                        metal,
                    );
                }
            }
        }
    }
}
fn slot(rect: Rect, stack: Option<Stack>, selected: bool, number: Option<usize>, mode: Mode) {
    draw_rectangle(
        rect.x,
        rect.y,
        rect.w,
        rect.h,
        if selected {
            Color::new(0.27, 0.36, 0.25, 0.96)
        } else {
            Color::new(0.055, 0.09, 0.12, 0.90)
        },
    );
    draw_rectangle_lines(
        rect.x,
        rect.y,
        rect.w,
        rect.h,
        if selected { 2.0 } else { 1.0 },
        if selected {
            ACCENT
        } else {
            Color::new(0.24, 0.31, 0.33, 1.0)
        },
    );
    if let Some(stack) = stack {
        icon(
            stack.item,
            rect.x + rect.w * 0.5,
            rect.y + rect.h * 0.43,
            rect.w * 0.53,
        );
        let count = if mode == Mode::Creative && stack.item.block().is_some() {
            "+".into()
        } else {
            stack.count.to_string()
        };
        text(
            &count,
            rect.x + rect.w - 19.0,
            rect.y + rect.h - 6.0,
            14.0,
            PAPER,
        );
    }
    if let Some(n) = number {
        text(&n.to_string(), rect.x + 5.0, rect.y + 15.0, 12.0, MUTED);
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    prevent_quit();
    if let Ok(font) = load_ttf_font_from_bytes(include_bytes!("../assets/Lato-Regular.ttf")) {
        macroquad::text::set_default_font(font);
    }
    let args: Vec<String> = std::env::args().collect();
    let arg_path = |name: &str| {
        args.iter()
            .position(|s| s == name)
            .and_then(|i| args.get(i + 1))
            .map(PathBuf::from)
    };
    let smoke = args.iter().any(|s| s == "--smoke");
    let input_checks = smoke && args.iter().any(|s| s == "--input-checks");
    let occlusion_checks = smoke && args.iter().any(|s| s == "--occlusion-checks");
    let shader_checks = smoke && args.iter().any(|s| s == "--shader-checks");
    let sound_checks = smoke && args.iter().any(|s| s == "--sound-checks");
    let mob_checks = smoke && args.iter().any(|s| s == "--mob-checks");
    let npc_checks = smoke && args.iter().any(|s| s == "--npc-checks");
    let v110_checks = smoke && args.iter().any(|s| s == "--v110-checks");
    let grenade_checks = smoke && args.iter().any(|s| s == "--grenade-checks");
    let gameplay_checks = smoke && args.iter().any(|s| s == "--gameplay-checks");
    let mob_demo = smoke
        && args
            .windows(2)
            .any(|pair| pair[0] == "--screen" && pair[1] == "mobs");
    let weapon_checks = smoke && args.iter().any(|s| s == "--weapon-checks");
    let reload_checks = smoke && args.iter().any(|s| s == "--reload-checks");
    let fluid_demo = smoke && args.iter().any(|s| s == "fluids");
    let capture = arg_path("--capture");
    let hover = args
        .iter()
        .position(|s| s == "--hover")
        .and_then(|i| args.get(i + 1))
        .and_then(|s| s.split_once(','))
        .and_then(|(x, y)| Some(vec2(x.parse().ok()?, y.parse().ok()?)));
    let report = arg_path("--report");
    let save_path = arg_path("--save-path").unwrap_or_else(|| {
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.join("saves/apocalypse-city.json")))
            .unwrap_or_else(|| PathBuf::from("saves/apocalypse-city.json"))
    });
    if let Err(error) = city::get() {
        loop {
            clear_background(BLACK);
            draw_text(
                "Apocalypse City : chargement impossible",
                30.,
                70.,
                30.,
                WHITE,
            );
            for (i, line) in error.as_bytes().chunks(90).enumerate() {
                draw_text(
                    String::from_utf8_lossy(line),
                    30.,
                    120. + i as f32 * 28.,
                    20.,
                    WHITE,
                );
            }
            if is_key_pressed(KeyCode::Escape) {
                return;
            }
            next_frame().await;
        }
    }
    let city_checks = smoke && args.iter().any(|s| s == "--city-checks");
    let mut app = App::new(save_path, Audio::new().await);
    if let Some(spawn) = args
        .iter()
        .position(|s| s == "--spawn")
        .and_then(|i| args.get(i + 1))
        .and_then(|s| s.parse::<u8>().ok())
        .filter(|&s| s < 6)
    {
        app.selected_spawn = spawn;
    }
    if sound_checks {
        app.audio.assert_weapon_bank();
    }
    if mob_checks {
        app.renderer.assert_creature_opacity();
    }
    if input_checks {
        app.preferences_path = std::env::temp_dir().join(format!(
            "voxel-rust-smoke-{}-settings.json",
            std::process::id()
        ));
    }
    if !smoke {
        app.restore_preferences();
    }
    if occlusion_checks {
        app.renderer.assert_depth_occlusion();
        app.renderer.assert_partial_geometry();
    }
    if shader_checks {
        app.renderer.assert_shader_effects();
        explosives::assert_render_effects();
    }
    if args.iter().any(|s| s == "--no-shaders") {
        app.renderer.enhanced = false;
    }
    if smoke {
        app.new_game();
        if !city_checks && args.iter().any(|s| s == "--legacy-fixture") {
            app.world = World::new(64195485);
            app.player = Player::new(app.world.spawn());
            app.player.yaw = -0.9;
            app.player.pitch = -0.16;
            app.mobs = initial_mobs(&app.world);
        }
        app.audio.enabled = false;
        if let Some(i) = args.iter().position(|s| s == "--screen") {
            match args.get(i + 1).map(String::as_str) {
                Some("title") => app.screen = Screen::Title,
                Some("village") => app.prepare_village_demo(false),
                Some("desert") => app.prepare_village_demo(true),
                Some("church") => app.prepare_building_demo(villages::Kind::Church),
                Some("forge") => app.prepare_building_demo(villages::Kind::Forge),
                Some("inn") => app.prepare_building_demo(villages::Kind::Inn),
                Some("mobs") => app.prepare_mob_demo(),
                Some("mountain") => app.prepare_landscape_demo(false),
                Some("cave") => app.prepare_landscape_demo(true),
                Some("stations") | Some("grenade") | Some("c4") | Some("crafting")
                | Some("armor") => {
                    app.prepare_station_demo();
                    match args.get(i + 1).map(String::as_str) {
                        Some("grenade") => {
                            app.inventory.add(Item::HandGrenade, 4);
                        }
                        Some("c4") => {
                            app.inventory.add(Item::C4, 4);
                        }
                        Some("crafting") => {
                            app.player.position.z = 2.;
                            app.active_station = Some([-3, 41, -1]);
                            app.screen = Screen::Crafting;
                            app.inventory.add(Item::Block(Block::Wood), 8);
                        }
                        Some("armor") => {
                            app.player.plate = Some(5);
                            app.player.helmet = Some(5);
                            app.screen = Screen::Inventory;
                        }
                        _ => {}
                    }
                }
                Some("shore") | Some("sunset") | Some("shore-night") => {
                    app.prepare_shore_demo();
                    app.time = match args.get(i + 1).map(String::as_str) {
                        Some("sunset") => 0.474,
                        Some("shore-night") => 0.75,
                        _ => 0.18,
                    };
                }
                Some("chest") => {
                    app.prepare_village_demo(false);
                    app.active_chest = Some(app.extras.chests[0].position);
                    app.screen = Screen::Inventory;
                }
                Some("trade") => {
                    app.prepare_village_demo(false);
                    app.inventory.add(Item::Wheat, 20);
                    app.inventory.add(Item::Emerald, 12);
                    app.trading = true;
                    app.screen = Screen::Inventory;
                }
                Some("inventory") => app.screen = Screen::Inventory,
                Some("creative") => app.screen = Screen::Creative,
                Some("pause") => app.screen = Screen::Paused,
                Some("controls") => app.screen = Screen::Controls,
                Some("dead") => app.screen = Screen::Dead,
                Some("night") => app.time = 0.75,
                Some("weapons") => app.prepare_weapon_demo(),
                Some("firing") => {
                    app.prepare_weapon_demo();
                    app.fire_weapon(true);
                }
                Some("sprint") => {
                    app.prepare_weapon_demo();
                    app.combat.sprint = 1.;
                }
                Some("aim") => {
                    app.prepare_weapon_demo();
                    app.combat.aim = 1.;
                }
                Some("reload") => {
                    app.prepare_weapon_demo();
                    app.inventory.slots[0].as_mut().unwrap().loaded = 12;
                    assert!(app.combat.start_reload(&app.inventory, app.mode));
                    app.combat.reload.as_mut().unwrap().elapsed = 0.8;
                }
                Some("inspect") => {
                    app.prepare_weapon_demo();
                    app.combat.tick_weapon(
                        &mut app.inventory,
                        app.mode,
                        1. / 60.,
                        WeaponInput::default(),
                    );
                    app.combat.inspect();
                    app.combat.inspect_time = 1.3;
                }
                Some("rocket-reload") => {
                    app.prepare_weapon_demo();
                    app.inventory.selected = 1;
                    app.inventory.slots[1].as_mut().unwrap().loaded = 0;
                    app.combat.tick_weapon(
                        &mut app.inventory,
                        app.mode,
                        0.1,
                        WeaponInput::default(),
                    );
                    app.combat.equip_time = 0.;
                    assert!(app.combat.start_reload(&app.inventory, app.mode));
                    app.combat.reload.as_mut().unwrap().elapsed = 1.3;
                }
                Some("rocket") => {
                    app.prepare_weapon_demo();
                    app.inventory.selected = 1;
                }
                Some("torch") => {
                    app.time = 0.75;
                    let p = app.player.position.floor();
                    for (x, z) in [(-3, -2), (-5, -4), (-1, -5)] {
                        let x = p.x as i32 + x;
                        let z = p.z as i32 + z;
                        let y = app.world.height_at(x, z) + 1;
                        app.world.set([x, y, z], Block::Torch);
                    }
                }
                Some("fluids") => {
                    let base = 40;
                    for x in -7..=8 {
                        for z in -6..=5 {
                            app.world.set([x, base, z], Block::Stone);
                            for y in base + 1..=base + 7 {
                                app.world.set([x, y, z], Block::Air);
                            }
                        }
                    }
                    app.world.set([-3, base + 4, -2], Block::Water);
                    app.world.set([4, base + 3, -2], Block::Lava);
                    for x in -4..=-2 {
                        app.world.set([x, base + 3, -2], Block::Cobble);
                    }
                    app.world.set([4, base + 2, -2], Block::Brick);
                    app.player.position = vec3(1.0, base as f32 + 4.0, 8.0);
                    app.player.yaw = 0.0;
                    app.player.pitch = -0.24;
                }
                Some("farm") => {
                    let base = 40;
                    for x in -6..=6 {
                        for z in -7..=5 {
                            app.world.set([x, base - 1, z], Block::Stone);
                            app.world.set([x, base, z], Block::Grass);
                            for y in base + 1..=base + 8 {
                                app.world.set([x, y, z], Block::Air);
                            }
                        }
                    }
                    for z in -6..=4 {
                        app.world.set([0, base, z], Block::Water);
                    }
                    for x in -4i32..=4 {
                        if x == 0 {
                            continue;
                        }
                        for z in -4i32..=2 {
                            app.world.set([x, base, z], Block::Farmland);
                            app.world
                                .set([x, base + 1, z], Block::wheat((z + 4).rem_euclid(4) as u8));
                        }
                    }
                    for y in base + 1..=base + 4 {
                        app.world.set([-5, y, 3], Block::Wood);
                    }
                    for x in -6..=-4 {
                        for z in 2..=4 {
                            for y in base + 4..=base + 5 {
                                app.world.set([x, y, z], Block::Leaves);
                            }
                        }
                    }
                    app.world.set([-5, base + 1, -5], Block::Workbench);
                    app.world.set([5, base + 1, -5], Block::Furnace);
                    for x in [-5, 5] {
                        app.world.set([x, base + 1, 0], Block::Torch);
                    }
                    app.player.position = vec3(6.5, 43.5, 11.0);
                    app.player.yaw = -0.49;
                    app.player.pitch = -0.35;
                    app.time = 0.18;
                    app.inventory.slots[0] = Some(Stack {
                        item: Item::Seeds,
                        count: 64,
                        loaded: 0,
                        optic: 0,
                    });
                    app.inventory.slots[1] = Some(Stack {
                        item: Item::WoodHoe,
                        count: 1,
                        loaded: 0,
                        optic: 0,
                    });
                    app.inventory.slots[2] = Some(Stack {
                        item: Item::Bread,
                        count: 64,
                        loaded: 0,
                        optic: 0,
                    });
                    app.inventory.selected = 0;
                }
                _ => {}
            }
        }
        if let Some(i) = args.iter().position(|s| s == "--gun") {
            app.prepare_weapon_demo();
            let index: usize = args.get(i + 1).unwrap().parse().unwrap();
            let gun = weapons::GUNS[index];
            let optic = args
                .iter()
                .position(|s| s == "--optic")
                .map_or(0, |i| args[i + 1].parse::<u8>().unwrap());
            app.inventory.slots[0] = Some(Stack {
                item: gun,
                count: 1,
                loaded: gun.magazine_capacity(),
                optic,
            });
            app.combat
                .tick_weapon(&mut app.inventory, app.mode, 0.1, WeaponInput::default());
            app.combat.equip_time = 0.;
            if args.iter().any(|s| s == "--aim") {
                app.combat.aim = 1.;
            }
            if args.iter().any(|s| s == "--reload-pose") {
                app.inventory.slots[0].as_mut().unwrap().loaded = 0;
                app.inventory.add(weapons::spec(gun).unwrap().ammo, 64);
                assert!(app.combat.start_reload(&app.inventory, app.mode));
                let reload = app.combat.reload.as_mut().unwrap();
                let progress = args
                    .iter()
                    .position(|s| s == "--reload-progress")
                    .map_or(0.42, |i| args[i + 1].parse::<f32>().unwrap().clamp(0., 1.));
                reload.elapsed = reload.duration * progress;
            }
            if args.iter().any(|s| s == "--bolt-pose") {
                app.combat.bolt_time = weapons::spec(gun).unwrap().interval * 0.5;
            }
        }
        if let Some(i) = args.iter().position(|s| s == "--inventory-tab") {
            app.inventory_tab = args[i + 1].parse().unwrap();
            app.screen = Screen::Creative;
        }
        if let Some(i) = args.iter().position(|s| s == "--inventory-page") {
            app.catalog_page = args[i + 1].parse().unwrap();
        }
        if args.iter().any(|s| s == "--drops") {
            app.prepare_weapon_demo();
            app.inventory.slots.fill(None);
            let center = app.player.position + app.player.direction() * 2.5;
            for (i, gun) in weapons::GUNS.into_iter().enumerate() {
                let position =
                    center + vec3((i % 4) as f32 * 0.5 - 0.75, 0.2, (i / 4) as f32 * 0.5);
                app.extras.drops.push(drops::DroppedItem {
                    stack: Stack {
                        item: gun,
                        count: 1,
                        loaded: 3.min(gun.magazine_capacity()),
                        optic: 0,
                    },
                    position: position.to_array(),
                    velocity: [0.; 3],
                    age: 2.,
                });
            }
            app.player.pitch = -0.6;
        }
        if weapon_checks || reload_checks {
            app.prepare_weapon_demo();
        }
        app.toast_timer = 0.0;
        set_cursor_grab(false);
        show_mouse(true);
        if let Some(mouse) = hover {
            app.ui.mouse = mouse;
        }
    }
    let mut frames = 0u32;
    let mut smoke_elapsed = 0.0f32;
    let mut smoke_fps = Vec::new();
    loop {
        let dt = get_frame_time().min(0.05);
        if !smoke {
            app.update(dt);
        } else {
            smoke_elapsed += get_frame_time();
            if npc_checks {
                app.scripted_village_defense_check(frames);
            }
            if v110_checks {
                app.v110_check(frames);
            }
            if grenade_checks {
                app.v111_grenade_check(frames);
            }
            if gameplay_checks {
                app.scripted_gameplay_check(frames);
            }
            if mob_demo {
                for mob in &mut app.mobs {
                    mob.phase = frames as f32 / 60.;
                }
            }
            if input_checks {
                app.scripted_input_check(frames);
            }
            if city_checks {
                app.scripted_city_check(frames);
            }
            if weapon_checks {
                app.scripted_weapon_check(frames);
            }
            if reload_checks {
                app.scripted_reload_check(frames);
            }
        }
        let p = app.player.position;
        let center = [
            (p.x.floor() as i32).div_euclid(CHUNK),
            (p.z.floor() as i32).div_euclid(CHUNK),
        ];
        app.world.stream(center, app.radius, 2);
        app.update_fluids(if fluid_demo { 0.10 } else { dt });
        app.renderer.sync(&mut app.world, center, 3);
        let hit = if app.screen == Screen::Playing {
            raycast(&app.world, app.player.eye(), app.player.direction(), 6.0)
        } else {
            None
        };
        if app.screen == Screen::Title {
            let saved_yaw = app.player.yaw;
            let saved_pitch = app.player.pitch;
            app.player.yaw = -0.9 + get_time() as f32 * 0.015;
            app.player.pitch = -0.12;
            app.renderer.draw(
                &app.world,
                &app.player,
                &app.mobs,
                None,
                0.22,
                app.radius,
                0.0,
                &app.combat,
                &app.extras.drops,
                &app.viewmodel,
                &app.extras.charges,
            );
            app.player.yaw = saved_yaw;
            app.player.pitch = saved_pitch;
        } else {
            app.renderer.draw(
                &app.world,
                &app.player,
                &app.mobs,
                hit.as_ref(),
                app.time,
                app.radius,
                app.mine_progress,
                &app.combat,
                &app.extras.drops,
                &app.viewmodel,
                &app.extras.charges,
            );
        }
        if shader_checks && frames > 20 {
            set_default_camera();
            let pixels = [(0.45, 0.25), (0.75, 0.65), (0.65, 0.85)]
                .map(|(x, y)| graphics::screen_pixel(x, y));
            assert!(
                pixels
                    .iter()
                    .filter(|p| p[..3].iter().map(|&c| c as u32).sum::<u32>() > 30)
                    .count()
                    >= 2,
                "The composed world scene is missing"
            );
        }
        if app.screen == Screen::Playing {
            if let Some(weapon) = selected_weapon(&app.inventory) {
                let daylight =
                    ((app.time * std::f32::consts::TAU).sin() * 1.7 + 0.18).clamp(0., 1.);
                if !weapons::spec(weapon).unwrap().scoped || app.combat.aim < 0.85 {
                    app.viewmodel.draw(
                        weapon,
                        app.inventory.selected().unwrap().optic,
                        &app.combat,
                        get_time() as f32,
                        daylight,
                    );
                }
            }
        }
        if app.screen == Screen::Playing {
            if let Some(item) = app
                .inventory
                .selected()
                .map(|s| s.item)
                .or(app.combat.throw_item.filter(|_| app.combat.throw_time > 0.))
            {
                explosives::draw_held(item, &app.combat);
            }
        }
        begin_ui();
        app.tooltip = None;
        let quit = match app.screen {
            Screen::Title => app.draw_title(),
            Screen::Playing => {
                app.draw_hud(hit.as_ref());
                if app.combat.hit_marker > 0.0 {
                    for (x, y) in [(-1., -1.), (1., -1.), (-1., 1.), (1., 1.)] {
                        draw_line(
                            640. + x * 10.,
                            400. + y * 10.,
                            640. + x * 17.,
                            400. + y * 17.,
                            2.,
                            ACCENT,
                        );
                    }
                }
                false
            }
            Screen::Paused => app.draw_pause(),
            Screen::Controls => {
                app.draw_controls();
                false
            }
            Screen::Inventory | Screen::Creative | Screen::Crafting => {
                app.draw_inventory();
                false
            }
            Screen::Dead => {
                app.draw_dead();
                false
            }
        };
        if app.toast_timer > 0.0 {
            let width = measure_text(&app.toast, None, 18, 1.0).width.min(1080.0);
            draw_rectangle(620.0 - width * 0.5, 110.0, width + 40.0, 44.0, PANEL);
            text(&app.toast, 640.0 - width * 0.5, 139.0, 18.0, ACCENT);
        }
        app.draw_tooltip(if smoke { 1.0 / 60.0 } else { dt });
        set_default_camera();
        if input_checks {
            app.assert_input_check(frames);
        }
        if (quit || is_quit_requested()) && (app.held.is_none() || app.close_inventory()) {
            if smoke {
                break;
            }
            if app.save() {
                break;
            }
            app.enter(Screen::Paused);
        }
        frames += 1;
        if smoke && frames > 100 {
            smoke_fps.push(1.0 / get_frame_time().max(0.000001));
        }
        if smoke && frames >= 180 {
            if let Some(ref path) = capture {
                get_screen_data().export_png(&path.to_string_lossy());
            }
            if let Some(ref path) = report {
                let (chunks, triangles) = app.renderer.stats();
                let avg = smoke_fps.iter().sum::<f32>() / smoke_fps.len().max(1) as f32;
                let data = serde_json::json!({"frames":frames,"elapsed_seconds":smoke_elapsed,"average_fps":avg,"chunks":chunks,"triangles":triangles,"seed":app.world.seed,"player":[p.x,p.y,p.z],"size":[screen_width(),screen_height()],"status":"rendered","input_checks":if input_checks {"passed"} else {"not_requested"},"occlusion_checks":if occlusion_checks {"passed"} else {"not_requested"},"shader_checks":if shader_checks {"passed"} else {"not_requested"},"weapon_checks":if weapon_checks {"passed"} else {"not_requested"},"reload_checks":if reload_checks {"passed"} else {"not_requested"},"reloads":app.combat.reloads,"magazine":app.inventory.selected().map(|s|s.loaded),"shots":app.combat.shots,"explosions":app.combat.explosions,"shaders":app.renderer.enhanced,"crops":app.world.crop_count(),"flowing_cells":app.world.fluid_flows().len(),"mobs":app.mobs.len(),"chests":app.extras.chests.len(),"villages":app.extras.visited_villages.len(),"sound_checks":if sound_checks {"passed"}else{"not_requested"},"gameplay_checks":if gameplay_checks {"passed"}else{"not_requested"},"mob_checks":if mob_checks {"passed"}else{"not_requested"},"generation":app.world.generation,"city_spawn":app.world.city_spawn,"city_checks":if city_checks {"passed"}else{"not_requested"},"v110_checks":if v110_checks {"passed"}else{"not_requested"},"npc_checks":if npc_checks {"passed"}else{"not_requested"},"grenade_checks":if grenade_checks {"passed"}else{"not_requested"},"village_floor_span":app.village_hint.and_then(|v|v.terrain.map(|t|t.floors.iter().max().unwrap()-t.floors.iter().min().unwrap()))});
                let _ = std::fs::write(path, serde_json::to_string_pretty(&data).unwrap());
            }
            break;
        }
        next_frame().await;
    }
    set_cursor_grab(false);
    show_mouse(true);
    if input_checks {
        let _ = std::fs::remove_file(&app.preferences_path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn daylight_lasts_twice_as_long_and_nights_keep_their_duration() {
        for time in [0.01, 0.22, 0.52, 0.98] {
            assert!((advance_day_cycle(time, 0.05) - time - 0.05 / 1200.).abs() < 0.0000001);
        }
        for time in [0.54, 0.75, 0.96] {
            assert!((advance_day_cycle(time, 0.05) - time - 0.05 / 600.).abs() < 0.0000001);
        }
        assert_eq!(advance_day_cycle(0.22, 0.), 0.22);
        assert!(advance_day_cycle(0.99998, 0.05) < 0.00003);
        let mut time = 0.97;
        let mut day_frames = 0;
        while time >= 0.97 || time <= 0.53 {
            time = advance_day_cycle(time, 0.05);
            day_frames += 1;
            assert!(day_frames < 14_000);
        }
        assert!((day_frames as f32 * 0.05 - 672.).abs() < 2.);
        let mut night_frames = 0;
        while time < 0.97 {
            time = advance_day_cycle(time, 0.05);
            night_frames += 1;
            assert!(night_frames < 5500);
        }
        assert!((night_frames as f32 * 0.05 - 264.).abs() < 2.);
    }

    #[test]
    fn creative_tabs_keep_every_block_weapon_ammo_food_and_station() {
        assert_eq!(catalog(0).len(), 29);
        assert!(!catalog(0).contains(&Item::Block(Block::Air)));
        assert_eq!(
            catalog(1),
            vec![
                Item::Apple,
                Item::Bread,
                Item::RawMutton,
                Item::CookedMutton,
                Item::RawBeef,
                Item::CookedBeef,
                Item::RawPork,
                Item::CookedPork
            ]
        );
        for station in [Block::Workbench, Block::Furnace, Block::Bed] {
            assert!(catalog(0).contains(&Item::Block(station)));
            assert!(catalog(2).contains(&Item::Block(station)));
        }
        assert_eq!(catalog(3).len(), 34);
        for item in weapons::GUNS.into_iter().chain(weapons::AMMO) {
            assert!(catalog(3).contains(&item));
        }
    }
    #[test]
    fn camera_regrab_resize_and_tooltip_edges_are_stable() {
        let mut previous = None;
        let mut size = Vec2::ZERO;
        assert_eq!(
            camera_delta(
                &mut previous,
                &mut size,
                vec2(500.0, 400.0),
                vec2(1280.0, 800.0)
            ),
            Vec2::ZERO
        );
        assert_eq!(
            camera_delta(
                &mut previous,
                &mut size,
                vec2(510.0, 405.0),
                vec2(1280.0, 800.0)
            ),
            vec2(10.0, 5.0)
        );
        assert_eq!(
            camera_delta(&mut previous, &mut size, vec2(0.0, 0.0), vec2(960.0, 600.0)),
            Vec2::ZERO
        );
        previous = None;
        assert_eq!(
            camera_delta(
                &mut previous,
                &mut size,
                vec2(900.0, 700.0),
                vec2(960.0, 600.0)
            ),
            Vec2::ZERO
        );
        for mouse in [vec2(0.0, 0.0), vec2(1279.0, 799.0), vec2(30.0, 780.0)] {
            let bounds = tooltip_bounds(mouse, 370.0, 230.0);
            assert!(bounds.x >= 10.0 && bounds.y >= 10.0);
            assert!(bounds.right() <= 1270.0 && bounds.bottom() <= 790.0);
        }
    }

    #[test]
    fn menu_keys_apply_one_transition_per_frame() {
        assert_eq!(
            menu_shortcut(Screen::Controls, true, false),
            Some(Screen::Paused)
        );
        for (key_escape, key_inventory, opened) in [
            (true, false, Screen::Paused),
            (false, true, Screen::Inventory),
        ] {
            let screen = menu_shortcut(Screen::Playing, key_escape, key_inventory).unwrap();
            assert_eq!(screen, opened);
            assert_eq!(menu_shortcut(screen, false, false), None);
            assert_eq!(
                menu_shortcut(screen, key_escape, key_inventory),
                Some(Screen::Playing)
            );
        }
    }

    #[test]
    fn mining_requires_release_after_menus_and_stops_on_physical_release() {
        let mut latch = MiningLatch::default();
        assert!(!latch.held(true, true, true));
        assert!(!latch.held(true, false, false));
        assert!(latch.held(true, true, true));
        assert!(!latch.held(true, true, false));
        assert!(!latch.held(false, true, true));
        assert!(!latch.held(true, true, true));
        assert!(!latch.held(true, false, false));
        assert!(latch.held(true, true, true));
    }
}
