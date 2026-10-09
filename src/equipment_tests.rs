use crate::game::*;
use crate::world::{Block, World};

#[test]
fn tiers_reduce_combat_damage_and_swaps_never_lose_equipment() {
    let mut previous = 0.;
    for tier in 1..=5 {
        let mut player = Player::new([0., 70., 0.]);
        let mut bag = Inventory::new(Mode::Survival);
        assert!(bag.add(Item::PlateCarrier(tier), 1));
        assert!(player.equip_selected(&mut bag));
        assert!(bag.add(Item::BallisticHelmet(tier), 1));
        assert!(player.equip_selected(&mut bag));
        player.combat_damage(8.);
        assert!(player.health > previous);
        previous = player.health;
        assert!(player.health < 20.);
        player.combat_damage(f32::NAN);
        assert_eq!(player.health, previous);
        bag.slots.fill(Some(Stack {
            item: Item::Coal,
            count: 64,
            loaded: 0,
            optic: 0,
        }));
        assert!(!player.unequip(&mut bag, true));
        assert_eq!(player.helmet, Some(tier));
        bag.slots[0] = Some(Stack {
            item: Item::PlateCarrier(1),
            count: 1,
            loaded: 0,
            optic: 0,
        });
        assert!(player.equip_selected(&mut bag));
        assert_eq!(player.plate, Some(1));
        assert_eq!(bag.count(Item::PlateCarrier(tier)), 1);
    }
    for tier in [0, 6, 255] {
        assert!(!Stack {
            item: Item::PlateCarrier(tier),
            count: 1,
            loaded: 0,
            optic: 0
        }
        .valid());
        assert!(!Stack {
            item: Item::BallisticHelmet(tier),
            count: 1,
            loaded: 0,
            optic: 0
        }
        .valid());
    }
}

#[test]
fn equipment_and_foods_survive_save_and_old_foods_keep_their_count() {
    let path = std::env::temp_dir().join(format!("voxel-equipment-{}.json", std::process::id()));
    let world = World::new(7);
    let mut player = Player::new([0., 70., 0.]);
    assert!(player.plate.is_none() && player.helmet.is_none());
    player.plate = Some(5);
    player.helmet = Some(4);
    let mut bag = Inventory::new(Mode::Survival);
    for item in [
        Item::RawBeef,
        Item::CookedBeef,
        Item::RawMutton,
        Item::CookedMutton,
        Item::RawPork,
        Item::CookedPork,
    ] {
        assert!(bag.add(item, 3));
    }
    save_game(
        &path,
        &world,
        &player,
        &bag,
        0.2,
        Mode::Survival,
        &Default::default(),
    )
    .unwrap();
    let (_, restored, items, _, _, _) = load_game(&path).unwrap();
    assert_eq!((restored.plate, restored.helmet), (Some(5), Some(4)));
    assert_eq!(items.slots, bag.slots);
    let mut value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    value["player"].as_object_mut().unwrap().remove("plate");
    value["player"].as_object_mut().unwrap().remove("helmet");
    value["inventory"]["slots"][0]["item"] = "RawMeat".into();
    value["inventory"]["slots"][1]["item"] = "CookedMeat".into();
    std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
    let (_, restored, items, _, _, _) = load_game(&path).unwrap();
    assert!(restored.plate.is_none() && restored.helmet.is_none());
    assert_eq!(items.count(Item::RawBeef), 3);
    assert_eq!(items.count(Item::CookedBeef), 3);
    value["player"]["plate"] = 6.into();
    std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
    assert!(load_game(&path).is_err());
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(path.with_extension("bak"));
    assert!(crate::game::recipes()
        .iter()
        .any(|r| r.result.0 == Item::CookedMutton && r.station == Some(Block::Furnace)));
}

#[test]
fn each_animal_drops_its_own_meat_and_each_cooked_food_can_be_eaten() {
    for (kind, raw, cooked) in [
        (MobKind::Sheep, Item::RawMutton, Item::CookedMutton),
        (MobKind::Cow, Item::RawBeef, Item::CookedBeef),
        (MobKind::Pig, Item::RawPork, Item::CookedPork),
    ] {
        let mut mobs = vec![Mob {
            kind,
            health: 0.,
            ..Default::default()
        }];
        let mut extras = Default::default();
        crate::settlements::loot_dead(&mut extras, &mut mobs);
        assert!(mobs.is_empty());
        assert!(extras.drops.iter().any(|d| d.stack.item == raw));
        let mut bag = Inventory::new(Mode::Survival);
        assert!(bag.add(cooked, 1));
        let mut player = Player::new([0., 70., 0.]);
        player.hunger = 1.;
        assert!(eat_selected(&mut bag, &mut player, Mode::Survival));
        assert!(bag.slots[0].is_none());
        assert!(player.hunger >= 9.);
    }
}
