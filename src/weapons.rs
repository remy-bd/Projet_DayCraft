//! Gameplay tuning: these values balance the voxel sandbox, not real ballistics.
use crate::combat::FireMode;
use crate::game::Item;

pub const GUNS: [Item; 12] = [
    Item::Ak,
    Item::Ak74,
    Item::Akm,
    Item::M4A1,
    Item::Famas,
    Item::M16A4,
    Item::RocketLauncher,
    Item::M200,
    Item::Tundra,
    Item::Aw50,
    Item::Svd,
    Item::Mp5k,
];
pub const AMMO: [Item; 9] = [
    Item::Bullet,
    Item::Ammo762,
    Item::Ammo556,
    Item::RocketAmmo,
    Item::Ammo408,
    Item::Ammo308,
    Item::Ammo50,
    Item::Ammo754,
    Item::Ammo9,
];
#[derive(Clone, Copy)]
pub struct Spec {
    pub ammo: Item,
    pub capacity: u16,
    pub interval: f32,
    pub damage: f32,
    pub recoil: f32,
    pub reload: f32,
    pub scoped: bool,
    pub bolt: bool,
}
pub fn spec(item: Item) -> Option<Spec> {
    use Item::*;
    let (ammo, capacity, interval, damage, recoil, reload, scoped, bolt) = match item {
        Ak => (Bullet, 30, 0.11, 6., 0.025, 2.5, false, false),
        Ak74 => (Bullet, 30, 0.10, 6., 0.021, 2.6, false, false),
        Akm => (Ammo762, 30, 0.12, 8., 0.034, 2.7, false, false),
        M4A1 => (Ammo556, 30, 0.085, 6., 0.019, 2.4, false, false),
        Famas => (Ammo556, 25, 0.075, 6., 0.022, 2.8, false, false),
        M16A4 => (Ammo556, 30, 0.095, 7., 0.022, 2.5, false, false),
        RocketLauncher => (RocketAmmo, 1, 0.9, 24., 0.055, 2.8, false, false),
        M200 => (Ammo408, 7, 1.45, 24., 0.067, 3.5, true, true),
        Tundra => (Ammo308, 5, 1.25, 20., 0.052, 3.2, true, true),
        Aw50 => (Ammo50, 5, 1.65, 30., 0.085, 3.8, true, true),
        Svd => (Ammo754, 10, 0.30, 13., 0.045, 2.9, true, false),
        Mp5k => (Ammo9, 30, 0.075, 4., 0.016, 2.2, false, false),
        _ => return None,
    };
    Some(Spec {
        ammo,
        capacity,
        interval,
        damage,
        recoil,
        reload,
        scoped,
        bolt,
    })
}
pub fn modes(item: Item) -> &'static [FireMode] {
    use FireMode::*;
    match item {
        Item::M16A4 => &[Semi, Burst],
        Item::Ak | Item::Ak74 | Item::Akm | Item::M4A1 | Item::Famas | Item::Mp5k => {
            &[Auto, Semi, Burst]
        }
        _ => &[Semi],
    }
}
pub fn magnification(optic: u8) -> f32 {
    [4., 8., 12.][(optic as usize).min(2)]
}
pub fn field_of_view(aim: f32, zoom: f32) -> f32 {
    let magnification = 1. + aim.clamp(0., 1.) * (zoom.max(1.) - 1.);
    2. * (36_f32.to_radians().tan() / magnification).atan()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn optics_use_actual_angular_magnification() {
        for optic in 0..3 {
            let zoom = magnification(optic);
            let hip = (field_of_view(0., zoom) * 0.5).tan();
            let aim = (field_of_view(1., zoom) * 0.5).tan();
            assert!((hip / aim - zoom).abs() < 0.001);
        }
    }
    #[test]
    fn arsenal_calibers_and_modes() {
        for gun in GUNS {
            let s = spec(gun).unwrap();
            assert!(AMMO.contains(&s.ammo));
            assert!(s.capacity > 0);
        }
        assert_eq!(spec(Item::Ak).unwrap().ammo, spec(Item::Ak74).unwrap().ammo);
        assert_eq!(spec(Item::Mp5k).unwrap().ammo, Item::Ammo9);
        assert!(!modes(Item::M16A4).contains(&FireMode::Auto));
        assert_eq!(GUNS.iter().filter(|&&g| spec(g).unwrap().scoped).count(), 4);
    }
}
