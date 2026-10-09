#[cfg(test)]
use crate::world::HEIGHT;
use crate::world::{Block, Flow, Pos, World};
use macroquad::prelude::{vec3, Vec3};
use serde::{Deserialize, Serialize};
use std::{fs, io::Write, path::Path};
pub const PLAYER_RADIUS: f32 = 0.30;
pub const PLAYER_HEIGHT: f32 = 1.80;
pub const STACK_LIMIT: u16 = 64;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    Creative,
    Survival,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Item {
    Block(Block),
    Stick,
    Coal,
    IronIngot,
    Apple,
    WoodPick,
    StonePick,
    IronPick,
    WoodAxe,
    StoneAxe,
    Sword,
    WoodHoe,
    Seeds,
    Wheat,
    Bread,
    Ak,
    RocketLauncher,
    Bullet,
    RocketAmmo,
    Ak74,
    Akm,
    M4A1,
    Famas,
    M16A4,
    M200,
    Tundra,
    Aw50,
    Svd,
    #[serde(alias = "Vpo139")]
    Mp5k,
    Ammo762,
    Ammo556,
    Ammo408,
    Ammo308,
    Ammo50,
    Ammo754,
    Ammo9,
    Emerald,
    #[serde(alias = "RawMeat")]
    RawBeef,
    #[serde(alias = "CookedMeat")]
    CookedBeef,
    RawMutton,
    CookedMutton,
    RawPork,
    CookedPork,
    PlateCarrier(u8),
    BallisticHelmet(u8),
    HandGrenade,
    C4,
    Wool,
    Gunpowder,
    Bucket,
    WaterBucket,
    LavaBucket,
}
impl Item {
    fn supplies(self, requested: Item) -> bool {
        self == requested
            || matches!((self,requested),(Item::Block(Block::Map(id)),Item::Block(block)) if !matches!(block,Block::Map(_)) && crate::city::state(id).is_some_and(|s|s.material==block))
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Block(block) => block.name(),
            Self::Stick => "Bâton",
            Self::Coal => "Charbon",
            Self::IronIngot => "Lingot de fer",
            Self::Apple => "Pomme",
            Self::WoodPick => "Pioche en bois",
            Self::StonePick => "Pioche en pierre",
            Self::IronPick => "Pioche en fer",
            Self::WoodAxe => "Hache en bois",
            Self::StoneAxe => "Hache en pierre",
            Self::Sword => "Épée en fer",
            Self::WoodHoe => "Houe en bois",
            Self::Seeds => "Graines de blé",
            Self::Wheat => "Blé",
            Self::Bread => "Pain",
            Self::Ak => "AK-74U",
            Self::RocketLauncher => "M79",
            Self::Bullet => "5.45x39mm",
            Self::RocketAmmo => "40mm",
            Self::Ak74 => "AK-74",
            Self::Akm => "AKM",
            Self::M4A1 => "M4-A1",
            Self::Famas => "FAMAS",
            Self::M16A4 => "M16-A4",
            Self::M200 => "M200 Intervention",
            Self::Tundra => "Tundra",
            Self::Aw50 => "AW 50",
            Self::Svd => "SVD",
            Self::Mp5k => "MP5K",
            Self::Ammo762 => "7.62x39mm",
            Self::Ammo556 => "5.56x45mm",
            Self::Ammo408 => ".408 CheyTac",
            Self::Ammo308 => ".308 Winchester",
            Self::Ammo50 => ".50 BMG",
            Self::Ammo754 => "7.62x54mmR",
            Self::Ammo9 => "9x19mm",
            Self::Emerald => "Émeraude",
            Self::RawBeef => "Bœuf cru",
            Self::CookedBeef => "Steak cuit",
            Self::RawMutton => "Mouton cru",
            Self::CookedMutton => "Mouton cuit",
            Self::RawPork => "Porc cru",
            Self::CookedPork => "Côte de porc cuite",
            Self::PlateCarrier(tier) => match tier {
                1 => "Porte-plaques T1",
                2 => "Porte-plaques T2",
                3 => "Porte-plaques T3",
                4 => "Porte-plaques T4",
                5 => "Porte-plaques T5",
                _ => "Armure invalide",
            },
            Self::BallisticHelmet(tier) => match tier {
                1 => "Casque balistique T1",
                2 => "Casque balistique T2",
                3 => "Casque balistique T3",
                4 => "Casque balistique T4",
                5 => "Casque balistique T5",
                _ => "Casque invalide",
            },
            Self::HandGrenade => "Grenade à main",
            Self::C4 => "Charge C4",
            Self::Wool => "Laine",
            Self::Gunpowder => "Poudre",
            Self::Bucket => "Seau vide",
            Self::WaterBucket => "Seau d’eau",
            Self::LavaBucket => "Seau de lave",
        }
    }
    pub fn block(self) -> Option<Block> {
        if let Self::Block(block) = self {
            Some(block)
        } else {
            None
        }
    }
    pub fn magazine_capacity(self) -> u16 {
        crate::weapons::spec(self).map_or(0, |s| s.capacity)
    }
    pub fn stack_limit(self) -> u16 {
        if self.magazine_capacity() > 0 {
            return 1;
        }
        match self {
            Self::PlateCarrier(_) | Self::BallisticHelmet(_) => 1,
            Self::HandGrenade | Self::C4 => 16,
            Self::WoodPick
            | Self::StonePick
            | Self::IronPick
            | Self::WoodAxe
            | Self::StoneAxe
            | Self::Sword => 1,
            Self::WoodHoe
            | Self::Ak
            | Self::RocketLauncher
            | Self::Bucket
            | Self::WaterBucket
            | Self::LavaBucket => 1,
            _ => STACK_LIMIT,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stack {
    pub item: Item,
    pub count: u16,
    #[serde(default)]
    pub loaded: u16,
    #[serde(default)]
    pub optic: u8,
}
impl Stack {
    pub fn valid(&self) -> bool {
        self.count > 0
            && self.count <= self.item.stack_limit()
            && self.loaded <= self.item.magazine_capacity()
            && self.optic <= 2
            && (self.optic == 0 || crate::weapons::spec(self.item).is_some_and(|w| w.scoped))
            && self.item != Item::Block(Block::Air)
            && match self.item {
                Item::PlateCarrier(t) | Item::BallisticHelmet(t) => (1..=5).contains(&t),
                Item::Block(block) => block.valid(),
                _ => true,
            }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Inventory {
    pub slots: Vec<Option<Stack>>,
    pub selected: usize,
}
impl Inventory {
    pub fn new(_mode: Mode) -> Self {
        Self {
            slots: vec![None; 36],
            selected: 0,
        }
    }
    /// All-or-nothing: a full inventory never destroys the excess items.
    pub fn add(&mut self, item: Item, count: u16) -> bool {
        let capacity: u32 = self
            .slots
            .iter()
            .map(|slot| match slot {
                Some(stack) if stack.item == item => (item.stack_limit() - stack.count) as u32,
                None => item.stack_limit() as u32,
                _ => 0,
            })
            .sum();
        if capacity < count as u32 {
            return false;
        }
        let mut remaining = count;
        for stack in self.slots.iter_mut().flatten() {
            if stack.item == item {
                let amount = remaining.min(item.stack_limit() - stack.count);
                stack.count += amount;
                remaining -= amount;
            }
        }
        for slot in &mut self.slots {
            if remaining == 0 {
                break;
            }
            if slot.is_none() {
                let amount = remaining.min(item.stack_limit());
                *slot = Some(Stack {
                    item,
                    count: amount,
                    loaded: 0,
                    optic: 0,
                });
                remaining -= amount;
            }
        }
        true
    }
    pub fn count(&self, item: Item) -> u16 {
        self.slots
            .iter()
            .flatten()
            .filter(|stack| stack.item.supplies(item))
            .map(|stack| stack.count)
            .sum()
    }
    pub fn return_stack(&mut self, stack: Stack) -> bool {
        if stack.item.magazine_capacity() == 0 {
            return self.add(stack.item, stack.count);
        }
        if let Some(slot) = self.slots.iter_mut().find(|slot| slot.is_none()) {
            *slot = Some(stack);
            true
        } else {
            false
        }
    }
    pub fn consume(&mut self, item: Item, count: u16) -> bool {
        if self.count(item) < count {
            return false;
        }
        let mut remaining = count;
        for slot in &mut self.slots {
            if let Some(stack) = slot {
                if stack.item.supplies(item) {
                    let amount = stack.count.min(remaining);
                    stack.count -= amount;
                    remaining -= amount;
                    if stack.count == 0 {
                        *slot = None;
                    }
                }
            }
            if remaining == 0 {
                break;
            }
        }
        true
    }
    pub fn selected(&self) -> Option<Stack> {
        self.slots.get(self.selected).copied().flatten()
    }
    pub fn take_selected(&mut self) -> Option<Item> {
        let stack = self.slots.get_mut(self.selected)?.as_mut()?;
        let item = stack.item;
        stack.count -= 1;
        if stack.count == 0 {
            self.slots[self.selected] = None;
        }
        Some(item)
    }
    /// Pick up, place, merge up to the stack limit, or swap different items.
    pub fn left_click(&mut self, index: usize, cursor: &mut Option<Stack>) -> bool {
        let Some(slot) = self.slots.get_mut(index) else {
            return false;
        };
        match (slot.as_mut(), cursor.as_mut()) {
            (Some(target), Some(held))
                if target.item == held.item && target.item.magazine_capacity() == 0 =>
            {
                let moved = held.count.min(target.item.stack_limit() - target.count);
                if moved == 0 {
                    return false;
                }
                target.count += moved;
                held.count -= moved;
                if held.count == 0 {
                    *cursor = None;
                }
            }
            _ => {
                if slot.is_none() && cursor.is_none() {
                    return false;
                }
                std::mem::swap(slot, cursor);
            }
        }
        true
    }
    /// Pick up the larger half of a stack, or place one item from the cursor.
    pub fn right_click(&mut self, index: usize, cursor: &mut Option<Stack>) -> bool {
        let Some(slot) = self.slots.get_mut(index) else {
            return false;
        };
        if let Some(held) = cursor.as_mut() {
            match slot {
                Some(target)
                    if target.item == held.item && target.count < target.item.stack_limit() =>
                {
                    target.count += 1;
                }
                None => {
                    *slot = Some(Stack {
                        item: held.item,
                        count: 1,
                        loaded: held.loaded,
                        optic: held.optic,
                    });
                }
                _ => return false,
            }
            held.count -= 1;
            if held.count == 0 {
                *cursor = None;
            }
        } else if let Some(stack) = slot.as_mut() {
            let count = stack.count.div_ceil(2);
            *cursor = Some(Stack {
                item: stack.item,
                count,
                loaded: stack.loaded,
                optic: stack.optic,
            });
            stack.count -= count;
            if stack.count == 0 {
                *slot = None;
            }
        } else {
            return false;
        }
        true
    }
    /// Move between the hotbar and bag, preserving any remainder when full.
    pub fn quick_transfer(&mut self, index: usize) -> bool {
        let Some(stack) = self.slots.get(index).copied().flatten() else {
            return false;
        };
        let bag_start = 9.min(self.slots.len());
        let destination = if index < 9 {
            bag_start..self.slots.len()
        } else {
            0..bag_start
        };
        let mut remaining = stack.count;
        for target in self.slots[destination.clone()].iter_mut().flatten() {
            if target.item == stack.item {
                let moved = remaining.min(stack.item.stack_limit() - target.count);
                target.count += moved;
                remaining -= moved;
            }
        }
        for slot in &mut self.slots[destination] {
            if remaining == 0 {
                break;
            }
            if slot.is_none() {
                let moved = remaining.min(stack.item.stack_limit());
                *slot = Some(Stack {
                    item: stack.item,
                    count: moved,
                    loaded: stack.loaded,
                    optic: stack.optic,
                });
                remaining -= moved;
            }
        }
        self.slots[index] = (remaining > 0).then_some(Stack {
            count: remaining,
            ..stack
        });
        remaining != stack.count
    }
}
pub struct Recipe {
    pub name: &'static str,
    pub ingredients: Vec<(Item, u16)>,
    pub result: (Item, u16),
    pub station: Option<Block>,
}
pub fn recipes() -> Vec<Recipe> {
    use Block::*;
    use Item::{
        Apple, Coal, IronIngot, IronPick, Stick, StoneAxe, StonePick, Sword, WoodAxe, WoodPick,
    };
    let b = Item::Block;
    let mut all = vec![
        Recipe {
            name: "4 planches",
            ingredients: vec![(b(Wood), 1)],
            result: (b(Planks), 4),
            station: None,
        },
        Recipe {
            name: "4 bâtons",
            ingredients: vec![(b(Planks), 2)],
            result: (Stick, 4),
            station: None,
        },
        Recipe {
            name: "Établi",
            ingredients: vec![(b(Wood), 4)],
            result: (b(Workbench), 1),
            station: None,
        },
        Recipe {
            name: "4 torches",
            ingredients: vec![(Coal, 1), (Stick, 1)],
            result: (b(Torch), 4),
            station: None,
        },
        Recipe {
            name: "Pioche en bois",
            ingredients: vec![(b(Planks), 3), (Stick, 2)],
            result: (WoodPick, 1),
            station: Some(Workbench),
        },
        Recipe {
            name: "Pioche en pierre",
            ingredients: vec![(b(Cobble), 3), (Stick, 2)],
            result: (StonePick, 1),
            station: Some(Workbench),
        },
        Recipe {
            name: "Pioche en fer",
            ingredients: vec![(IronIngot, 3), (Stick, 2)],
            result: (IronPick, 1),
            station: Some(Workbench),
        },
        Recipe {
            name: "Hache en bois",
            ingredients: vec![(b(Planks), 3), (Stick, 2)],
            result: (WoodAxe, 1),
            station: Some(Workbench),
        },
        Recipe {
            name: "Hache en pierre",
            ingredients: vec![(b(Cobble), 3), (Stick, 2)],
            result: (StoneAxe, 1),
            station: Some(Workbench),
        },
        Recipe {
            name: "Épée en fer",
            ingredients: vec![(IronIngot, 2), (Stick, 1)],
            result: (Sword, 1),
            station: Some(Workbench),
        },
        Recipe {
            name: "Four",
            ingredients: vec![(b(Cobble), 8)],
            result: (b(Furnace), 1),
            station: Some(Workbench),
        },
        Recipe {
            name: "Fondre le fer",
            ingredients: vec![(b(IronOre), 1), (Coal, 1)],
            result: (IronIngot, 1),
            station: Some(Furnace),
        },
        Recipe {
            name: "Cuire le verre",
            ingredients: vec![(b(Sand), 4), (Coal, 1)],
            result: (b(Glass), 4),
            station: Some(Furnace),
        },
        Recipe {
            name: "Cuire la pierre",
            ingredients: vec![(b(Cobble), 4), (Coal, 1)],
            result: (b(Stone), 4),
            station: Some(Furnace),
        },
        Recipe {
            name: "4 briques",
            ingredients: vec![(b(Stone), 4)],
            result: (b(Brick), 4),
            station: Some(Workbench),
        },
        Recipe {
            name: "Charbon de bois",
            ingredients: vec![(b(Wood), 2)],
            result: (Coal, 1),
            station: Some(Furnace),
        },
        Recipe {
            name: "Récolter une pomme",
            ingredients: vec![(b(Leaves), 6)],
            result: (Apple, 1),
            station: None,
        },
        Recipe {
            name: "Houe en bois",
            ingredients: vec![(b(Planks), 2), (Stick, 2)],
            result: (Item::WoodHoe, 1),
            station: Some(Workbench),
        },
        Recipe {
            name: "Pain",
            ingredients: vec![(Item::Wheat, 3)],
            result: (Item::Bread, 1),
            station: None,
        },
        Recipe {
            name: "AK-74U",
            ingredients: vec![(IronIngot, 6), (b(Planks), 2)],
            result: (Item::Ak, 1),
            station: Some(Workbench),
        },
        Recipe {
            name: "M79",
            ingredients: vec![(IronIngot, 8), (Coal, 2)],
            result: (Item::RocketLauncher, 1),
            station: Some(Workbench),
        },
        Recipe {
            name: "16 × 5.45x39mm",
            ingredients: vec![(IronIngot, 1), (Coal, 1)],
            result: (Item::Bullet, 16),
            station: Some(Workbench),
        },
        Recipe {
            name: "4 grenades 40mm",
            ingredients: vec![(IronIngot, 2), (Coal, 2)],
            result: (Item::RocketAmmo, 4),
            station: Some(Workbench),
        },
    ];
    all.extend([
        Recipe {
            name: "Coffre",
            ingredients: vec![(b(Planks), 8)],
            result: (b(Chest), 1),
            station: Some(Workbench),
        },
        Recipe {
            name: "Porte",
            ingredients: vec![(b(Planks), 6)],
            result: (b(Door), 1),
            station: Some(Workbench),
        },
        Recipe {
            name: "Seau",
            ingredients: vec![(IronIngot, 3)],
            result: (Item::Bucket, 1),
            station: Some(Workbench),
        },
        Recipe {
            name: "Steak cuit",
            ingredients: vec![(Item::RawBeef, 3), (Coal, 1)],
            result: (Item::CookedBeef, 3),
            station: Some(Furnace),
        },
        Recipe {
            name: "Lit en laine",
            ingredients: vec![(Item::Wool, 3), (b(Planks), 3)],
            result: (b(Bed), 1),
            station: Some(Workbench),
        },
        Recipe {
            name: "Grès",
            ingredients: vec![(b(Sand), 4)],
            result: (b(Sandstone), 1),
            station: None,
        },
    ]);
    for (raw, cooked, name) in [
        (Item::RawMutton, Item::CookedMutton, "Mouton cuit"),
        (Item::RawPork, Item::CookedPork, "Porc cuit"),
    ] {
        all.push(Recipe {
            name,
            ingredients: vec![(raw, 3), (Coal, 1)],
            result: (cooked, 3),
            station: Some(Furnace),
        });
    }
    all.push(Recipe {
        name: "40mm avec poudre",
        ingredients: vec![(IronIngot, 2), (Item::Gunpowder, 2)],
        result: (Item::RocketAmmo, 4),
        station: Some(Workbench),
    });
    all.push(Recipe {
        name: "Lit",
        ingredients: vec![(b(Planks), 4), (b(Leaves), 3)],
        result: (b(Bed), 1),
        station: Some(Workbench),
    });
    for item in crate::weapons::GUNS
        .into_iter()
        .filter(|i| !matches!(i, Item::Ak | Item::RocketLauncher))
    {
        all.push(Recipe {
            name: item.name(),
            ingredients: vec![(IronIngot, 8), (b(Planks), 2)],
            result: (item, 1),
            station: Some(Workbench),
        });
    }
    for item in crate::weapons::AMMO
        .into_iter()
        .filter(|i| !matches!(i, Item::Bullet | Item::RocketAmmo))
    {
        all.push(Recipe {
            name: item.name(),
            ingredients: vec![(IronIngot, 2), (Coal, 1)],
            result: (item, 16),
            station: Some(Workbench),
        });
    }
    for tier in 1..=5 {
        for item in [Item::PlateCarrier(tier), Item::BallisticHelmet(tier)] {
            all.push(Recipe {
                name: item.name(),
                ingredients: vec![(Item::Wool, tier as u16), (IronIngot, tier as u16 * 2)],
                result: (item, 1),
                station: Some(Workbench),
            });
        }
    }
    for item in [Item::HandGrenade, Item::C4] {
        all.push(Recipe {
            name: item.name(),
            ingredients: vec![
                (IronIngot, 2),
                (Item::Gunpowder, if item == Item::C4 { 4 } else { 2 }),
            ],
            result: (item, 1),
            station: Some(Workbench),
        });
    }
    for recipe in &mut all {
        if recipe.result.0 != Item::Block(Workbench) && recipe.station.is_none() {
            recipe.station = Some(Workbench);
        }
    }
    all
}
pub fn recipe_allowed(recipe: &Recipe, station: Option<Block>) -> bool {
    match station.map(Block::material) {
        None => recipe.result.0 == Item::Block(Block::Workbench) && recipe.station.is_none(),
        Some(Block::Workbench) => recipe.station != Some(Block::Furnace),
        Some(Block::Furnace) => recipe.station == Some(Block::Furnace),
        _ => false,
    }
}
pub fn crafting_station(world: &World, player: &Player, position: Option<Pos>) -> Option<Block> {
    let p = position?;
    let center = Vec3::from_array(p.map(|n| n as f32)) + Vec3::splat(0.5);
    let block = world.get(p).material();
    (center.distance_squared(player.eye()) <= 6.5_f32.powi(2)
        && matches!(block, Block::Workbench | Block::Furnace))
    .then_some(block)
}
pub fn craft_at(
    inventory: &mut Inventory,
    recipe: &Recipe,
    world: &World,
    player: &Player,
    position: Option<Pos>,
) -> Result<(), String> {
    let station = crafting_station(world, player, position);
    if position.is_some() && station.is_none() || !recipe_allowed(recipe, station) {
        return Err("Utilisez la table de craft ou le four correspondant".into());
    }
    craft(inventory, recipe).map_err(str::to_string)
}
fn craft(inventory: &mut Inventory, recipe: &Recipe) -> Result<(), &'static str> {
    let mut next = inventory.clone();
    for &(item, count) in &recipe.ingredients {
        if !next.consume(item, count) {
            return Err("Matériaux insuffisants");
        }
    }
    if !next.add(recipe.result.0, recipe.result.1) {
        return Err("Inventaire plein");
    }
    *inventory = next;
    Ok(())
}
/// Return None when the selected item has no farming action.
pub fn farm_use(
    world: &mut World,
    inventory: &mut Inventory,
    hit: &RayHit,
    mode: Mode,
) -> Option<Result<&'static str, &'static str>> {
    let item = inventory.selected()?.item;
    let above = [hit.block[0], hit.block[1] + 1, hit.block[2]];
    match item {
        Item::WoodHoe => Some(
            if matches!(world.get(hit.block).material(), Block::Grass | Block::Dirt)
                && world.get(above) == Block::Air
                && world.set(hit.block, Block::Farmland)
            {
                Ok("Terre labourée : semez avec un clic droit")
            } else {
                Err("Labourez de la terre ou de l'herbe avec de l'air au-dessus")
            },
        ),
        Item::Seeds => Some(
            if world.get(hit.block).material() == Block::Farmland
                && world.get(above) == Block::Air
                && above[1] < world.height()
                && world.set(above, Block::Wheat0)
            {
                if mode == Mode::Survival {
                    inventory.take_selected();
                }
                Ok("Blé semé : eau à moins de 4 blocs et lumière nécessaires")
            } else {
                Err("Semez sur une terre labourée libre")
            },
        ),
        _ => None,
    }
}
pub fn harvest_crop(
    world: &mut World,
    inventory: &mut Inventory,
    pos: Pos,
    mode: Mode,
) -> Result<bool, &'static str> {
    let Some(stage) = world.get(pos).crop_stage() else {
        return Ok(false);
    };
    if mode == Mode::Survival {
        let mut next = inventory.clone();
        if !next.add(Item::Seeds, if stage == 3 { 2 } else { 1 })
            || (stage == 3 && !next.add(Item::Wheat, 1))
        {
            return Err("Inventaire plein : récolte conservée");
        }
        *inventory = next;
    }
    world.set(pos, Block::Air);
    Ok(true)
}
/// Collect compound drops atomically, including seeds from grass and a plant above soil.
pub fn collect_block(inventory: &mut Inventory, world: &World, pos: Pos, drop: Item) -> bool {
    let mut next = inventory.clone();
    if !next.add(drop, 1) {
        return false;
    }
    if world.get(pos).material() == Block::Grass && !next.add(Item::Seeds, 1) {
        return false;
    }
    if let Some(stage) = world.get([pos[0], pos[1] + 1, pos[2]]).crop_stage() {
        if !next.add(Item::Seeds, if stage == 3 { 2 } else { 1 })
            || (stage == 3 && !next.add(Item::Wheat, 1))
        {
            return false;
        }
    }
    *inventory = next;
    true
}
pub fn eat_selected(inventory: &mut Inventory, player: &mut Player, mode: Mode) -> bool {
    let Some(stack) = inventory.selected() else {
        return false;
    };
    let food = match stack.item {
        Item::Apple => 6.0,
        Item::Bread => 8.0,
        Item::RawBeef | Item::RawMutton | Item::RawPork => 3.,
        Item::CookedBeef => 10.,
        Item::CookedMutton => 8.,
        Item::CookedPork => 9.,
        _ => return false,
    };
    if player.hunger >= 20.0 {
        return false;
    }
    if mode == Mode::Survival {
        inventory.take_selected();
    }
    player.hunger = (player.hunger + food).min(20.0);
    player.health = (player.health + 2.0).min(20.0);
    true
}
pub struct Player {
    /// Feet, with blocks occupying [x, x + 1] on each axis.
    pub position: Vec3,
    pub velocity: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub grounded: bool,
    pub flying: bool,
    pub health: f32,
    pub hunger: f32,
    pub plate: Option<u8>,
    pub helmet: Option<u8>,
    jump_buffer: f32,
    coyote_time: f32,
}
#[derive(Default, Clone, Copy)]
pub struct Movement {
    pub forward: f32,
    pub right: f32,
    pub jump: bool,
    pub jump_pressed: bool,
    pub sprint: bool,
    pub descend: bool,
    pub sneak: bool,
}
impl Player {
    pub fn new(position: [f32; 3]) -> Self {
        Self {
            position: Vec3::from_array(position),
            velocity: Vec3::ZERO,
            yaw: 0.0,
            pitch: 0.0,
            grounded: false,
            flying: false,
            health: 20.0,
            hunger: 20.0,
            plate: None,
            helmet: None,
            jump_buffer: 0.0,
            coyote_time: 0.0,
        }
    }
    pub fn protection(&self) -> f32 {
        (self.plate.unwrap_or(0) as f32 * 0.10 + self.helmet.unwrap_or(0) as f32 * 0.05)
            .clamp(0., 0.75)
    }
    pub fn combat_damage(&mut self, amount: f32) {
        if amount.is_finite() && amount > 0. {
            self.health = (self.health - amount * (1. - self.protection())).max(0.);
        }
    }
    pub fn equip_selected(&mut self, inventory: &mut Inventory) -> bool {
        let Some(stack) = inventory.selected() else {
            return false;
        };
        if !stack.valid() {
            return false;
        }
        let old = match stack.item {
            Item::PlateCarrier(_) => self.plate.map(Item::PlateCarrier),
            Item::BallisticHelmet(_) => self.helmet.map(Item::BallisticHelmet),
            _ => return false,
        };
        let mut next = inventory.clone();
        next.take_selected();
        if let Some(item) = old {
            if !next.add(item, 1) {
                return false;
            }
        }
        match stack.item {
            Item::PlateCarrier(t) => self.plate = Some(t),
            Item::BallisticHelmet(t) => self.helmet = Some(t),
            _ => unreachable!(),
        }
        *inventory = next;
        true
    }
    pub fn unequip(&mut self, inventory: &mut Inventory, helmet: bool) -> bool {
        let slot = if helmet {
            &mut self.helmet
        } else {
            &mut self.plate
        };
        let Some(t) = *slot else {
            return false;
        };
        if !inventory.add(
            if helmet {
                Item::BallisticHelmet(t)
            } else {
                Item::PlateCarrier(t)
            },
            1,
        ) {
            return false;
        }
        *slot = None;
        true
    }
    pub fn eye(&self) -> Vec3 {
        self.position + vec3(0.0, 1.62, 0.0)
    }
    pub fn direction(&self) -> Vec3 {
        vec3(
            self.yaw.sin() * self.pitch.cos(),
            self.pitch.sin(),
            -self.yaw.cos() * self.pitch.cos(),
        )
    }
    pub fn update(&mut self, world: &World, input: Movement, dt: f32, mode: Mode) {
        if !dt.is_finite() || dt <= 0.0 {
            return;
        }
        let dt = dt.min(0.10);
        if mode == Mode::Survival {
            self.flying = false;
        }
        let in_water = touches_fluid(world, self.position, Block::Water);
        let in_lava = touches_fluid(world, self.position, Block::Lava);
        let in_fluid = in_water || in_lava;
        let climbing = world.generation == 4
            && !self.flying
            && (-1..=1).any(|dy| {
                let p = self.position + Vec3::Y * dy as f32;
                if let Block::Map(id) = world.get(p.floor().as_ivec3().to_array()) {
                    crate::city::state(id).is_some_and(|s| s.climb)
                } else {
                    false
                }
            });
        if input.jump_pressed && !self.flying && !in_fluid {
            self.jump_buffer = 0.12;
        }
        if self.flying || in_fluid {
            self.jump_buffer = 0.0;
            self.coyote_time = 0.0;
        }
        let moving = input.forward.abs() + input.right.abs() > 0.01;
        let sprint = input.sprint
            && (!input.sneak || self.flying)
            && (mode == Mode::Creative || self.hunger > 3.0);
        let speed = if self.flying {
            if sprint {
                18.0
            } else {
                10.0
            }
        } else if in_lava {
            1.4
        } else if in_water {
            2.8
        } else if input.sneak {
            1.8
        } else if sprint {
            7.2
        } else {
            4.6
        };
        let mut wish = vec3(self.yaw.sin(), 0.0, -self.yaw.cos()) * input.forward
            + vec3(self.yaw.cos(), 0.0, self.yaw.sin()) * input.right;
        if wish.length_squared() > 1.0 {
            wish = wish.normalize();
        }
        self.velocity.x = wish.x * speed;
        self.velocity.z = wish.z * speed;
        if self.flying {
            self.velocity.y = (input.jump as i32 - input.descend as i32) as f32 * speed;
        }
        let steps = (dt / 0.016).ceil() as usize;
        let step = dt / steps as f32;
        for _ in 0..steps {
            let supported = self.velocity.y <= 0.0 && has_support(world, self.position);
            if !self.flying && !in_fluid {
                self.coyote_time = if supported {
                    0.10
                } else {
                    (self.coyote_time - step).max(0.0)
                };
                if self.jump_buffer > 0.0 && self.coyote_time > 0.0 {
                    self.velocity.y = 7.8;
                    self.grounded = false;
                    self.coyote_time = 0.0;
                    self.jump_buffer = 0.0;
                }
                self.jump_buffer = (self.jump_buffer - step).max(0.0);
            }
            let guard_edge =
                input.sneak && !self.flying && !in_fluid && supported && self.velocity.y <= 0.0;
            if !self.flying {
                if climbing {
                    self.velocity.y = if input.descend {
                        -2.6
                    } else if input.jump || input.forward > 0. {
                        2.6
                    } else {
                        self.velocity.y.max(-1.5)
                    };
                } else if in_fluid {
                    self.velocity.y =
                        (self.velocity.y - 5.0 * step).max(if in_lava { -1.5 } else { -3.0 });
                    if input.jump {
                        self.velocity.y = if in_lava { 2.0 } else { 3.8 };
                    }
                } else {
                    self.velocity.y = (self.velocity.y - 23.0 * step).max(-45.0);
                }
            }
            let fall_speed = self.velocity.y;
            let mut grounded = false;
            for axis in [0, 2, 1] {
                let delta = self.velocity[axis] * step;
                if delta == 0.0 {
                    continue;
                }
                let candidate = self.position + axis_vector(axis, delta);
                if collides(world, candidate, PLAYER_RADIUS, PLAYER_HEIGHT)
                    || (axis != 1 && guard_edge && !has_support(world, candidate))
                {
                    if axis != 1
                        && world.generation == 4
                        && supported
                        && !self.flying
                        && !in_fluid
                        && self.velocity.y <= 0.
                    {
                        let raised = candidate + Vec3::Y * 0.6;
                        if !collides(world, raised, PLAYER_RADIUS, PLAYER_HEIGHT)
                            && collides(world, candidate, PLAYER_RADIUS, PLAYER_HEIGHT)
                        {
                            let mut blocked = 0.;
                            let mut clear = 0.6;
                            for _ in 0..12 {
                                let rise = (clear + blocked) * 0.5;
                                if collides(
                                    world,
                                    candidate + Vec3::Y * rise,
                                    PLAYER_RADIUS,
                                    PLAYER_HEIGHT,
                                ) {
                                    blocked = rise;
                                } else {
                                    clear = rise;
                                }
                            }
                            self.position = candidate + Vec3::Y * clear;
                            self.velocity.y = 0.;
                            grounded = true;
                            continue;
                        }
                    }
                    // Bisect the move to reach the surface without embedding or hovering.
                    let mut clear = 0.0;
                    let mut blocked = 1.0;
                    for _ in 0..10 {
                        let fraction = (clear + blocked) * 0.5;
                        let tested = self.position + axis_vector(axis, delta * fraction);
                        if collides(world, tested, PLAYER_RADIUS, PLAYER_HEIGHT)
                            || (axis != 1 && guard_edge && !has_support(world, tested))
                        {
                            blocked = fraction;
                        } else {
                            clear = fraction;
                        }
                    }
                    self.position[axis] += delta * clear;
                    self.velocity[axis] = 0.0;
                    if axis == 1 && delta < 0.0 {
                        grounded = true;
                        if mode == Mode::Survival && !in_fluid && fall_speed < -12.0 {
                            self.health = (self.health - (-fall_speed - 12.0) * 0.55).max(0.0);
                        }
                    }
                } else {
                    self.position = candidate;
                }
            }
            self.grounded = grounded
                || (!self.flying
                    && collides(
                        world,
                        self.position - vec3(0.0, 0.025, 0.0),
                        PLAYER_RADIUS,
                        PLAYER_HEIGHT,
                    ));
        }
        if mode == Mode::Survival {
            let in_lava = in_lava || touches_fluid(world, self.position, Block::Lava);
            if in_lava {
                self.health = (self.health - dt * 4.0).max(0.0);
            }
            self.hunger = (self.hunger
                - dt * if sprint && moving {
                    0.05
                } else if moving {
                    0.012
                } else {
                    0.004
                })
            .max(0.0);
            if !in_lava && self.hunger >= 16.0 && self.health < 20.0 {
                self.health = (self.health + dt * 0.25).min(20.0);
                self.hunger = (self.hunger - dt * 0.03).max(0.0);
            } else if self.hunger <= 0.0 {
                self.health = (self.health - dt * 0.25).max(0.0);
            }
            if self.position.y < -20.0 {
                self.health = 0.0;
            }
        }
    }
}
/// Fluid immersion follows the rendered surface, including shallow flow.
pub fn fluid_at(world: &World, point: Vec3) -> Option<Block> {
    let cell = point.floor().as_ivec3().to_array();
    let height = world.fluid_height(cell)?;
    (point.y < cell[1] as f32 + height).then(|| world.get(cell))
}
fn touches_fluid(world: &World, feet: Vec3, fluid: Block) -> bool {
    let low = (feet - vec3(PLAYER_RADIUS, 0.0, PLAYER_RADIUS) + Vec3::splat(0.0001))
        .floor()
        .as_ivec3();
    let high = (feet + vec3(PLAYER_RADIUS, PLAYER_HEIGHT, PLAYER_RADIUS) - Vec3::splat(0.0001))
        .floor()
        .as_ivec3();
    for x in low.x..=high.x {
        for y in low.y..=high.y {
            for z in low.z..=high.z {
                let cell = [x, y, z];
                if world.get(cell) == fluid
                    && world
                        .fluid_height(cell)
                        .is_some_and(|height| feet.y + 0.0001 < y as f32 + height)
                {
                    return true;
                }
            }
        }
    }
    false
}
fn axis_vector(axis: usize, value: f32) -> Vec3 {
    let mut result = Vec3::ZERO;
    result[axis] = value;
    result
}
fn has_support(world: &World, feet: Vec3) -> bool {
    collides(world, feet - vec3(0.0, 0.025, 0.0), PLAYER_RADIUS, 0.025)
}
pub fn collides(world: &World, feet: Vec3, radius: f32, height: f32) -> bool {
    let low = (feet - vec3(radius, 0.0, radius) + Vec3::splat(0.0001))
        .floor()
        .as_ivec3();
    let high = (feet + vec3(radius, height, radius) - Vec3::splat(0.0001))
        .floor()
        .as_ivec3();
    let min = feet - vec3(radius, 0., radius) + Vec3::splat(0.0001);
    let max = feet + vec3(radius, height, radius) - Vec3::splat(0.0001);
    for x in low.x..=high.x {
        for y in low.y - 1..=high.y {
            for z in low.z..=high.z {
                if !world.contains(x, z) {
                    return true;
                }
                let block = world.get([x, y, z]);
                let offset = vec3(x as f32, y as f32, z as f32);
                let boxes = if block == Block::Door {
                    &[[0., 0., 0., 1., 2., 1.]][..]
                } else {
                    block.boxes()
                };
                if boxes.iter().any(|b| {
                    let block_min = offset + vec3(b[0], b[1], b[2]);
                    let block_max = offset + vec3(b[3], b[4], b[5]);
                    min.cmplt(block_max).all() && max.cmpgt(block_min).all()
                }) {
                    return true;
                }
            }
        }
    }
    false
}
pub fn player_intersects_block(player: &Player, block: Pos) -> bool {
    let p = player.position;
    p.x + PLAYER_RADIUS > block[0] as f32
        && p.x - PLAYER_RADIUS < block[0] as f32 + 1.0
        && p.y + PLAYER_HEIGHT > block[1] as f32
        && p.y < block[1] as f32 + 1.0
        && p.z + PLAYER_RADIUS > block[2] as f32
        && p.z - PLAYER_RADIUS < block[2] as f32 + 1.0
}
#[derive(Clone, Copy, Debug)]
pub struct RayHit {
    pub block: Pos,
    pub previous: Pos,
    pub distance: f32,
}
/// Exact voxel traversal, including negative coordinates and axis-aligned rays.
pub fn raycast(world: &World, origin: Vec3, direction: Vec3, max: f32) -> Option<RayHit> {
    raycast_voxels(world, origin, direction, max, false)
}
pub fn raycast_with_fluids(
    world: &World,
    origin: Vec3,
    direction: Vec3,
    max: f32,
) -> Option<RayHit> {
    raycast_voxels(world, origin, direction, max, true)
}
fn raycast_voxels(
    world: &World,
    origin: Vec3,
    direction: Vec3,
    max: f32,
    include_fluids: bool,
) -> Option<RayHit> {
    if !origin.is_finite()
        || !direction.is_finite()
        || direction.length_squared() < 0.000001
        || !max.is_finite()
        || max < 0.0
    {
        return None;
    }
    let direction = direction.normalize();
    let mut cell = origin.floor().as_ivec3().to_array();
    let mut previous = cell;
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
        let block = world.get(cell);
        if block != Block::Air && (include_fluids || !matches!(block, Block::Water | Block::Lava)) {
            if let Block::Map(id) = block {
                if let Some(hit) = crate::city::state(id).and_then(|state| {
                    state
                        .boxes
                        .iter()
                        .filter_map(|b| ray_box(origin, direction, cell, *b, max))
                        .min_by(|a, b| a.0.total_cmp(&b.0))
                }) {
                    if hit.0 <= distances.iter().copied().fold(max, f32::min) + 0.0001 {
                        return Some(RayHit {
                            block: cell,
                            previous: hit.1,
                            distance: hit.0,
                        });
                    }
                }
            } else {
                return Some(RayHit {
                    block: cell,
                    previous,
                    distance,
                });
            }
        }
        let below = [cell[0], cell[1] - 1, cell[2]];
        let overhang = match world.get(below) {
            Block::Map(id) => crate::city::state(id).and_then(|state| {
                state
                    .boxes
                    .iter()
                    .filter(|b| b[4] > 1.)
                    .filter_map(|b| ray_box(origin, direction, below, *b, max))
                    .min_by(|a, b| a.0.total_cmp(&b.0))
            }),
            Block::Door => ray_box(origin, direction, below, [0., 0., 0., 1., 2., 1.], max),
            _ => None,
        };
        if let Some(hit) = overhang.filter(|h| {
            h.0 >= distance - 0.0001
                && h.0 <= distances.iter().copied().fold(max, f32::min) + 0.0001
        }) {
            return Some(RayHit {
                block: below,
                previous: hit.1,
                distance: hit.0,
            });
        }
        let axis = if distances[0] <= distances[1] && distances[0] <= distances[2] {
            0
        } else if distances[1] <= distances[2] {
            1
        } else {
            2
        };
        previous = cell;
        distance = distances[axis];
        cell[axis] += steps[axis];
        distances[axis] += increments[axis];
    }
    None
}
fn ray_box(
    origin: Vec3,
    direction: Vec3,
    cell: Pos,
    b: crate::city::Box3,
    max: f32,
) -> Option<(f32, Pos)> {
    let mut enter = 0.;
    let mut exit = max;
    let mut previous = cell;
    for axis in 0..3 {
        let low = cell[axis] as f32 + b[axis];
        let high = cell[axis] as f32 + b[axis + 3];
        if direction[axis].abs() < 0.000001 {
            if origin[axis] < low || origin[axis] > high {
                return None;
            }
            continue;
        }
        let a = (low - origin[axis]) / direction[axis];
        let z = (high - origin[axis]) / direction[axis];
        let near = a.min(z);
        if near > enter {
            enter = near;
            previous = cell;
            previous[axis] += if direction[axis] > 0. { -1 } else { 1 };
        }
        exit = exit.min(a.max(z));
        if enter > exit {
            return None;
        }
    }
    (exit >= 0. && enter <= max).then_some((enter, previous))
}
pub fn can_harvest(tool: Option<Item>, block: Block) -> bool {
    use Block::*;
    match block.material() {
        Air | Water | Lava | Bedrock => false,
        IronOre => matches!(tool, Some(Item::StonePick | Item::IronPick)),
        Stone | Cobble | CoalOre | Brick | Furnace => matches!(
            tool,
            Some(Item::WoodPick | Item::StonePick | Item::IronPick)
        ),
        _ => true,
    }
}
pub fn drop_for(block: Block) -> Item {
    match block.material() {
        Block::Grass | Block::Farmland => Item::Block(Block::Dirt),
        Block::Wheat0 | Block::Wheat1 | Block::Wheat2 | Block::Wheat3 => Item::Seeds,
        Block::Stone => Item::Block(Block::Cobble),
        Block::CoalOre => Item::Coal,
        Block::OpenDoor => Item::Block(Block::Door),
        _ => Item::Block(block.material()),
    }
}
pub fn mining_speed(tool: Option<Item>, block: Block) -> f32 {
    let block = block.material();
    if matches!(
        block,
        Block::Air | Block::Water | Block::Lava | Block::Bedrock
    ) {
        return 0.0;
    }
    let stone = matches!(
        block,
        Block::Stone
            | Block::Cobble
            | Block::CoalOre
            | Block::IronOre
            | Block::Brick
            | Block::Furnace
    );
    let wood = matches!(block, Block::Wood | Block::Planks | Block::Workbench);
    match tool {
        Some(Item::WoodPick) if stone => 3.0,
        Some(Item::StonePick) if stone => 5.0,
        Some(Item::IronPick) if stone => 8.0,
        Some(Item::WoodAxe) if wood => 3.0,
        Some(Item::StoneAxe) if wood => 5.0,
        _ => 1.0,
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum MobKind {
    #[default]
    Sheep,
    Zombie,
    Villager,
    Golem,
    Creeper,
    Cow,
    Pig,
}
impl MobKind {
    pub fn height(self) -> f32 {
        match self {
            Self::Sheep => 1.3,
            Self::Zombie => 1.9,
            Self::Villager => 1.95,
            Self::Golem => 2.9,
            Self::Creeper => 1.7,
            Self::Cow => 1.4,
            Self::Pig => 0.95,
        }
    }
    pub fn animal(self) -> bool {
        matches!(self, Self::Sheep | Self::Cow | Self::Pig)
    }
    pub fn hostile(self) -> bool {
        matches!(self, Self::Zombie | Self::Creeper)
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Sheep => "Mouton",
            Self::Zombie => "Zombie",
            Self::Villager => "Villageois",
            Self::Golem => "Golem de fer",
            Self::Creeper => "Creeper",
            Self::Cow => "Vache",
            Self::Pig => "Cochon",
        }
    }
}
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Mob {
    #[serde(with = "mob_position")]
    pub position: Vec3,
    pub kind: MobKind,
    pub health: f32,
    pub phase: f32,
    pub attack_timer: f32,
    #[serde(default)]
    pub yaw: f32,
    #[serde(default)]
    pub walking: bool,
    #[serde(default)]
    pub love: f32,
    #[serde(default)]
    pub baby: f32,
    #[serde(default)]
    pub breed_cooldown: f32,
    #[serde(default)]
    pub fuse: f32,
    #[serde(default)]
    pub home: Option<[f32; 3]>,
}
mod mob_position {
    use super::*;
    pub fn serialize<S: serde::Serializer>(v: &Vec3, s: S) -> Result<S::Ok, S::Error> {
        v.to_array().serialize(s)
    }
    pub fn deserialize<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec3, D::Error> {
        Ok(Vec3::from_array(<[f32; 3]>::deserialize(d)?))
    }
}
pub fn initial_mobs(world: &World) -> Vec<Mob> {
    if world.generation == 4 {
        return Vec::new();
    }
    let spawn = world.spawn();
    (0..10)
        .map(|i| {
            let mut position = Vec3::from_array(spawn);
            for attempt in 0..32 {
                let angle = i as f32 * 2.39996 + attempt as f32 * 0.53;
                let distance = 12.0 + i as f32 * 2.0 + attempt as f32 * 0.35;
                let x = (spawn[0] + angle.cos() * distance).floor() + 0.5;
                let z = (spawn[2] + angle.sin() * distance).floor() + 0.5;
                let y = if world.generation == 4 {
                    let Some(y) =
                        world.walkable(x.floor() as i32, z.floor() as i32, spawn[1], 1.95)
                    else {
                        continue;
                    };
                    y
                } else {
                    world.height_at(x.floor() as i32, z.floor() as i32) as f32 + 1.0
                };
                let feet = vec3(x, y, z);
                if !collides(world, feet, 0.32, 1.35)
                    && !matches!(
                        world.get(feet.floor().as_ivec3().to_array()),
                        Block::Water | Block::Lava
                    )
                {
                    position = feet;
                    break;
                }
            }
            Mob {
                position,
                kind: if i < 7 {
                    match i % 3 {
                        0 => MobKind::Sheep,
                        1 => MobKind::Cow,
                        _ => MobKind::Pig,
                    }
                } else {
                    if i == 9 {
                        MobKind::Creeper
                    } else {
                        MobKind::Zombie
                    }
                },
                health: if i < 7 { 10.0 } else { 20.0 },
                phase: i as f32 * 1.87,
                attack_timer: 0.0,
                ..Default::default()
            }
        })
        .collect()
}
pub fn update_mobs(
    mobs: &mut Vec<Mob>,
    world: &World,
    player: &mut Player,
    dt: f32,
    night: bool,
    mode: Mode,
) -> Vec<Vec3> {
    if !dt.is_finite() || dt < 0. {
        return Vec::new();
    }
    let dt = dt.min(0.1);
    let positions: Vec<_> = mobs
        .iter()
        .enumerate()
        .filter(|(_, m)| m.health > 0.)
        .map(|(i, m)| (i, m.kind, m.position))
        .collect();
    let partners: Vec<_> = mobs
        .iter()
        .filter(|m| m.love > 0. && m.health > 0.)
        .map(|m| (m.kind, m.position))
        .collect();
    let mut damage = Vec::new();
    let mut explosions = Vec::new();
    for mob in mobs.iter_mut() {
        if mob.health <= 0. || mob.position.distance_squared(player.position) > 100_f32.powi(2) {
            continue;
        }
        mob.phase += dt;
        mob.attack_timer = (mob.attack_timer - dt).max(0.);
        mob.love = (mob.love - dt).max(0.);
        mob.baby = (mob.baby - dt).max(0.);
        mob.breed_cooldown = (mob.breed_cooldown - dt).max(0.);
        mob.walking = false;
        let height = mob.kind.height() * if mob.baby > 0. { 0.55 } else { 1. };
        let radius = if mob.kind == MobKind::Golem {
            0.55
        } else {
            0.3
        };
        let old = mob.position;
        let cell = old.floor().as_ivec3().to_array();
        let delta = Vec3::Y * -8. * dt;
        if collides(world, old + delta, radius, height) {
            let mut clear = 0.;
            let mut blocked = 1.;
            for _ in 0..10 {
                let fraction = (clear + blocked) * 0.5;
                if collides(world, old + delta * fraction, radius, height) {
                    blocked = fraction;
                } else {
                    clear = fraction;
                }
            }
            mob.position = old + delta * clear;
        } else {
            mob.position = old + delta;
        }
        if world.get(cell) == Block::Lava {
            mob.health -= 8. * dt;
        }
        if !night
            && mob.kind == MobKind::Zombie
            && (cell[1] + 2..world.height()).all(|y| !world.get([cell[0], y, cell[2]]).solid())
        {
            mob.health -= 1.5 * dt;
        }
        let to_player = player.position - mob.position;
        let distance = to_player.length();
        let nearest = |kind: fn(MobKind) -> bool, range: f32| {
            positions
                .iter()
                .filter(|(_, k, p)| kind(*k) && p.distance_squared(mob.position) < range * range)
                .min_by(|a, b| {
                    a.2.distance_squared(mob.position)
                        .total_cmp(&b.2.distance_squared(mob.position))
                })
                .copied()
        };
        let enemy = if mob.kind == MobKind::Golem {
            nearest(MobKind::hostile, 48.)
        } else if mob.kind == MobKind::Zombie {
            nearest(|k| k == MobKind::Villager, 32.)
        } else {
            None
        };
        let mut direction = Vec3::ZERO;
        let mut speed = 0.8;
        if let Some((i, _, target)) = enemy {
            direction = target - mob.position;
            speed = if mob.kind == MobKind::Golem { 3.4 } else { 3.0 };
            let delta = target + Vec3::Y - mob.position - Vec3::Y;
            if delta.length() < 2.3
                && mob.attack_timer == 0.
                && raycast(world, mob.position + Vec3::Y, delta, delta.length()).is_none()
            {
                damage.push((i, if mob.kind == MobKind::Golem { 18. } else { 3. }));
                mob.attack_timer = 1.1;
            }
        } else if mob.kind.hostile() && mode == Mode::Survival && night && distance < 26. {
            direction = to_player;
            speed = if mob.kind == MobKind::Creeper {
                1.65
            } else {
                1.8
            };
        } else if mob.kind.animal() && mob.love > 0. {
            if let Some((_, partner)) = partners
                .iter()
                .filter(|(kind, p)| {
                    *kind == mob.kind
                        && p.distance_squared(mob.position) > 0.001
                        && p.distance_squared(mob.position) < 12_f32.powi(2)
                })
                .min_by(|a, b| {
                    a.1.distance_squared(mob.position)
                        .total_cmp(&b.1.distance_squared(mob.position))
                })
            {
                direction = *partner - mob.position;
            }
        } else if mob.kind == MobKind::Villager && nearest(MobKind::hostile, 9.).is_some() {
            direction = mob.position - nearest(MobKind::hostile, 9.).unwrap().2;
            speed = 2.2;
        } else if let Some(home) = mob.home {
            let home = Vec3::from_array(home);
            if home.distance_squared(mob.position)
                > (if mob.kind == MobKind::Golem {
                    44_f32
                } else {
                    10_f32
                })
                .powi(2)
            {
                direction = home - mob.position;
            } else if (mob.phase * 0.4).sin() > -0.25 {
                direction = vec3((mob.phase * 0.23).sin(), 0., (mob.phase * 0.29).cos());
            }
        } else if mob.kind.animal() && (mob.phase * 0.37).sin() > -0.15 {
            direction = vec3((mob.phase * 0.23).sin(), 0., (mob.phase * 0.19).cos());
        }
        let attack = to_player - Vec3::Y * to_player.y;
        let visible = distance < 3.
            && raycast(
                world,
                mob.position + Vec3::Y,
                player.position + Vec3::Y - mob.position - Vec3::Y,
                distance,
            )
            .is_none();
        if mob.kind == MobKind::Creeper {
            mob.fuse = if mode == Mode::Survival && visible {
                mob.fuse + dt
            } else {
                (mob.fuse - dt * 2.).max(0.)
            };
            if mob.fuse > 0. {
                direction = Vec3::ZERO;
            }
            if mob.fuse >= 1.5 {
                explosions.push(mob.position + Vec3::Y * 0.5);
                mob.health = 0.;
                continue;
            }
        }
        if mob.kind == MobKind::Zombie
            && mode == Mode::Survival
            && night
            && visible
            && distance < 1.5
            && mob.attack_timer == 0.
        {
            player.combat_damage(2.);
            mob.attack_timer = 1.2;
        }
        direction.y = 0.;
        direction = direction.normalize_or_zero();
        if direction.length_squared() > 0. {
            let mut next = mob.position + direction * speed * dt;
            let x = next.x.floor() as i32;
            let z = next.z.floor() as i32;
            if let Some(floor) = world.walkable(x, z, mob.position.y, height) {
                if floor - mob.position.y <= 1.05 && mob.position.y - floor <= 2. {
                    next.y = floor;
                    if !collides(world, next, radius, height)
                        && !world.get(next.floor().as_ivec3().to_array()).fluid()
                    {
                        mob.position = next;
                        mob.walking = true;
                        mob.yaw = direction.x.atan2(-direction.z);
                    }
                }
            }
        } else if mob.kind == MobKind::Villager && distance < 6. {
            mob.yaw = attack.x.atan2(-attack.z);
        }
    }
    for (i, amount) in damage {
        mobs[i].health -= amount;
    }
    crate::settlements::breed(mobs);
    explosions
}
#[derive(Serialize, Deserialize)]
struct SavedPlayer {
    position: [f32; 3],
    velocity: [f32; 3],
    yaw: f32,
    pitch: f32,
    grounded: bool,
    flying: bool,
    health: f32,
    hunger: f32,
    #[serde(default)]
    plate: Option<u8>,
    #[serde(default)]
    helmet: Option<u8>,
}
fn legacy_generation() -> u8 {
    1
}

#[derive(Serialize, Deserialize)]
struct Save {
    version: u32,
    seed: u64,
    #[serde(default = "legacy_generation")]
    generation: u8,
    #[serde(default)]
    map_id: Option<String>,
    #[serde(default)]
    city_spawn: u8,
    edits: Vec<(Pos, Block)>,
    #[serde(default)]
    flows: Vec<(Pos, Flow)>,
    #[serde(default)]
    extras: crate::drops::WorldExtras,
    player: SavedPlayer,
    inventory: Inventory,
    time: f32,
    mode: Mode,
}
#[allow(clippy::too_many_arguments)]
pub fn save_game(
    path: &Path,
    world: &World,
    player: &Player,
    inventory: &Inventory,
    time: f32,
    mode: Mode,
    extras: &crate::drops::WorldExtras,
) -> Result<(), String> {
    let save = Save {
        version: 1,
        seed: world.seed,
        generation: world.generation,
        map_id: if world.generation == 4 {
            Some(crate::city::get()?.metadata.id.clone())
        } else {
            None
        },
        city_spawn: world.city_spawn,
        edits: world.edits(),
        flows: world.fluid_flows(),
        extras: extras.clone(),
        player: SavedPlayer {
            position: player.position.to_array(),
            velocity: player.velocity.to_array(),
            yaw: player.yaw,
            pitch: player.pitch,
            grounded: player.grounded,
            flying: player.flying,
            health: player.health,
            hunger: player.hunger,
            plate: player.plate,
            helmet: player.helmet,
        },
        inventory: inventory.clone(),
        time,
        mode,
    };
    validate_save(&save)?;
    let bytes = serde_json::to_vec(&save).map_err(|error| error.to_string())?;
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let temporary = path.with_extension("tmp");
    let backup = path.with_extension("bak");
    let mut file = fs::File::create(&temporary).map_err(|error| error.to_string())?;
    file.write_all(&bytes)
        .and_then(|_| file.sync_all())
        .map_err(|error| error.to_string())?;
    drop(file);
    // Windows rename cannot overwrite an existing destination. Keep a recovery
    // copy until the complete new file is in place.
    if path.exists() {
        if backup.exists() {
            fs::remove_file(&backup).map_err(|error| error.to_string())?;
        }
        fs::rename(path, &backup).map_err(|error| error.to_string())?;
    }
    if let Err(error) = fs::rename(&temporary, path) {
        if backup.exists() {
            let _ = fs::rename(&backup, path);
        }
        return Err(error.to_string());
    }
    Ok(())
}
pub fn load_game(
    path: &Path,
) -> Result<
    (
        World,
        Player,
        Inventory,
        f32,
        Mode,
        crate::drops::WorldExtras,
    ),
    String,
> {
    let file = fs::File::open(path).map_err(|error| error.to_string())?;
    let mut save: Save = serde_json::from_reader(file).map_err(|error| error.to_string())?;
    // The retired VPO becomes an unscoped MP5K, including dropped copies.
    for stack in save
        .inventory
        .slots
        .iter_mut()
        .flatten()
        .chain(save.extras.drops.iter_mut().map(|d| &mut d.stack))
    {
        if stack.item == Item::Mp5k && stack.optic <= 2 {
            stack.optic = 0;
        }
    }
    validate_save(&save)?;
    let mut world = save_world(&save)?;
    world.restore_edits(save.edits);
    world.restore_fluid_flows(save.flows);
    let player = Player {
        position: Vec3::from_array(save.player.position),
        velocity: Vec3::from_array(save.player.velocity),
        yaw: save.player.yaw,
        pitch: save.player.pitch,
        grounded: save.player.grounded,
        flying: save.player.flying && save.mode == Mode::Creative,
        health: save.player.health,
        hunger: save.player.hunger,
        plate: save.player.plate,
        helmet: save.player.helmet,
        jump_buffer: 0.0,
        coyote_time: 0.0,
    };
    Ok((
        world,
        player,
        save.inventory,
        save.time,
        save.mode,
        save.extras,
    ))
}
fn save_world(save: &Save) -> Result<World, String> {
    if save.version != 1 || !(1..=4).contains(&save.generation) {
        return Err("Version de sauvegarde incompatible".into());
    }
    if save.generation == 4 {
        let city = crate::city::get()?;
        if save.map_id.as_deref() != Some(city.metadata.id.as_str()) || save.city_spawn >= 6 {
            return Err("Map ou point de départ incompatible".into());
        }
        Ok(World::city(save.city_spawn))
    } else {
        if save.map_id.is_some() || save.city_spawn != 0 {
            return Err("Map incompatible avec cette génération".into());
        }
        let mut world = World::new(save.seed);
        world.generation = save.generation;
        Ok(world)
    }
}
fn validate_save(save: &Save) -> Result<(), String> {
    let mut world = save_world(save)?;
    if save.player.plate.is_some_and(|t| !(1..=5).contains(&t))
        || save.player.helmet.is_some_and(|t| !(1..=5).contains(&t))
    {
        return Err("Équipement balistique invalide".into());
    }
    if !save.extras.valid() {
        return Err("Objets au sol invalides".into());
    }
    if save.inventory.slots.len() != 36 || save.inventory.selected >= 9 {
        return Err("Inventaire de sauvegarde invalide".into());
    }
    for stack in save.inventory.slots.iter().flatten() {
        if !stack.valid() {
            return Err("Pile d'objets invalide".into());
        }
    }
    if !save
        .player
        .position
        .iter()
        .chain(&save.player.velocity)
        .all(|value| value.is_finite() && value.abs() <= 1_000_000.0)
        || !save.player.yaw.is_finite()
        || !save.player.pitch.is_finite()
        || save.player.pitch.abs() > std::f32::consts::FRAC_PI_2 + 0.01
        || !save.player.health.is_finite()
        || !(0.0..=20.0).contains(&save.player.health)
        || !save.player.hunger.is_finite()
        || !(0.0..=20.0).contains(&save.player.hunger)
        || !save.time.is_finite()
        || save.time < 0.0
    {
        return Err("État du joueur invalide".into());
    }
    if save.edits.len() > 500_000
        || save.edits.iter().any(|(position, block)| {
            position[0].unsigned_abs() > 1_000_000
                || position[2].unsigned_abs() > 1_000_000
                || position[1] < 0
                || position[1] >= world.height()
                || !world.contains(position[0], position[2])
                || !block.valid()
                || save.generation != 4 && matches!(block, Block::Map(_))
        })
    {
        return Err("Modifications du monde invalides".into());
    }
    if save.flows.len() > 500_000
        || save.flows.iter().any(|(position, flow)| {
            !(1..=8).contains(&flow.level)
                || position[0].unsigned_abs() > 1_000_000
                || position[2].unsigned_abs() > 1_000_000
                || position[1] < 0
                || position[1] >= world.height()
                || !world.contains(position[0], position[2])
        })
    {
        return Err("Écoulements de sauvegarde invalides".into());
    }
    if !save.flows.is_empty() {
        world.restore_edits(save.edits.clone());
        let mut positions = std::collections::HashSet::new();
        if save.flows.iter().any(|(position, _)| {
            !positions.insert(*position)
                || !matches!(world.get(*position), Block::Water | Block::Lava)
        }) {
            return Err("Écoulements de sauvegarde invalides".into());
        }
    }
    if save
        .extras
        .chests
        .iter()
        .any(|c| c.position[1] >= world.height() || !world.contains(c.position[0], c.position[2]))
        || save.extras.charges.iter().any(|c| {
            c.position[1] >= world.height() as f32
                || !world.contains(c.position[0].floor() as i32, c.position[2].floor() as i32)
        })
    {
        return Err("Objets du monde hors des limites".into());
    }
    if save.generation == 4 {
        let within = |p: &[f32; 3]| world.contains(p[0].floor() as i32, p[2].floor() as i32);
        if !within(&save.player.position)
            || save.extras.spawn.as_ref().is_some_and(|p| !within(p))
            || save.extras.drops.iter().any(|d| !within(&d.position))
            || save
                .extras
                .grenades
                .iter()
                .any(|g| !within(&g.position.to_array()))
            || save.extras.mobs.as_ref().is_some_and(|mobs| {
                mobs.iter().any(|m| {
                    !within(&m.position.to_array()) || m.home.as_ref().is_some_and(|p| !within(p))
                })
            })
            || save.extras.city_populated.iter().any(|p| {
                p[0].unsigned_abs() > 6251
                    || p[1].unsigned_abs() > 6251
                    || !world.contains(p[0] * 16, p[1] * 16)
            })
        {
            return Err("Position extérieure à Apocalypse City".into());
        }
    } else if save
        .inventory
        .slots
        .iter()
        .flatten()
        .chain(save.extras.drops.iter().map(|d| &d.stack))
        .chain(
            save.extras
                .chests
                .iter()
                .flat_map(|c| c.inventory.slots.iter().flatten()),
        )
        .any(|s| matches!(s.item, Item::Block(Block::Map(_))))
        || !save.extras.city_populated.is_empty()
    {
        return Err("Objet de la ville dans une sauvegarde procédurale".into());
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn weapon_recipes_consume_materials_and_respect_stack_limits() {
        for (item, count, ingredients) in [
            (
                Item::Ak,
                1,
                vec![(Item::IronIngot, 6), (Item::Block(Block::Planks), 2)],
            ),
            (
                Item::RocketLauncher,
                1,
                vec![(Item::IronIngot, 8), (Item::Coal, 2)],
            ),
            (
                Item::Bullet,
                16,
                vec![(Item::IronIngot, 1), (Item::Coal, 1)],
            ),
            (
                Item::RocketAmmo,
                4,
                vec![(Item::IronIngot, 2), (Item::Coal, 2)],
            ),
        ] {
            let recipe = recipes().into_iter().find(|r| r.result.0 == item).unwrap();
            assert_eq!(recipe.ingredients, ingredients);
            assert_eq!(recipe.result.1, count);
            assert_eq!(recipe.station, Some(Block::Workbench));
            let mut inventory = empty_inventory();
            for &(ingredient, amount) in &ingredients {
                assert!(inventory.add(ingredient, amount));
            }
            craft(&mut inventory, &recipe).unwrap();
            assert_eq!(inventory.count(item), count);
            for &(ingredient, _) in &ingredients {
                assert_eq!(inventory.count(ingredient), 0);
            }
            let before = inventory.slots.clone();
            assert!(craft(&mut inventory, &recipe).is_err());
            assert_eq!(inventory.slots, before);
        }
        let mut inventory = empty_inventory();
        for weapon in [Item::Ak, Item::RocketLauncher] {
            assert!(inventory.add(weapon, 2));
            assert_eq!(
                inventory
                    .slots
                    .iter()
                    .flatten()
                    .filter(|s| s.item == weapon)
                    .count(),
                2
            );
            assert!(inventory
                .slots
                .iter()
                .flatten()
                .filter(|s| s.item == weapon)
                .all(|s| s.count == 1));
        }
        for ammo in [Item::Bullet, Item::RocketAmmo] {
            assert!(inventory.add(ammo, 65));
            let mut stacks: Vec<_> = inventory
                .slots
                .iter()
                .flatten()
                .filter(|s| s.item == ammo)
                .map(|s| s.count)
                .collect();
            stacks.sort_unstable();
            assert_eq!(stacks, [1, 64]);
        }
        // Keep creative inventory's free slots for clicks and transfers; weapons use the palette.
        assert!(Inventory::new(Mode::Creative).slots[35].is_none());
    }
    #[test]
    fn weapons_and_ammunition_roundtrip_without_changing_legacy_save_format() {
        let world = World::new(42);
        let player = Player::new(world.spawn());
        let mut inventory = empty_inventory();
        for item in [
            Item::Ak,
            Item::RocketLauncher,
            Item::Bullet,
            Item::RocketAmmo,
        ] {
            assert!(inventory.add(item, item.stack_limit()));
        }
        inventory.slots[0].as_mut().unwrap().loaded = 17;
        inventory.slots[1].as_mut().unwrap().loaded = 1;
        let path = std::env::temp_dir().join(format!(
            "voxel-rust-weapon-save-{}.json",
            std::process::id()
        ));
        save_game(
            &path,
            &world,
            &player,
            &inventory,
            0.2,
            Mode::Survival,
            &Default::default(),
        )
        .unwrap();
        let (_, _, restored, _, _, _) = load_game(&path).unwrap();
        assert_eq!(restored.slots, inventory.slots);
        let mut legacy: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(legacy["version"], 1);
        legacy.as_object_mut().unwrap().remove("flows");
        let old_inventory = Inventory::new(Mode::Survival);
        legacy["inventory"] = serde_json::to_value(&old_inventory).unwrap();
        for slot in legacy["inventory"]["slots"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .filter(|s| !s.is_null())
        {
            slot.as_object_mut().unwrap().remove("loaded");
        }
        fs::write(&path, serde_json::to_vec(&legacy).unwrap()).unwrap();
        let (_, _, restored, _, _, _) = load_game(&path).unwrap();
        assert_eq!(restored.slots, old_inventory.slots);
        fs::remove_file(path).unwrap();
    }
    #[test]
    fn moving_and_swapping_identical_weapons_preserves_each_magazine() {
        let mut inventory = empty_inventory();
        inventory.slots[0] = Some(Stack {
            item: Item::Ak,
            count: 1,
            loaded: 17,
            optic: 0,
        });
        inventory.slots[1] = Some(Stack {
            item: Item::Ak,
            count: 1,
            loaded: 3,
            optic: 0,
        });
        let mut cursor = None;
        inventory.left_click(0, &mut cursor);
        inventory.left_click(1, &mut cursor);
        assert_eq!(inventory.slots[1].unwrap().loaded, 17);
        assert_eq!(cursor.unwrap().loaded, 3);
        inventory.right_click(2, &mut cursor);
        assert!(cursor.is_none());
        inventory.quick_transfer(2);
        assert_eq!(inventory.slots[9].unwrap().loaded, 3);
        inventory.right_click(9, &mut cursor);
        assert!(inventory.return_stack(cursor.take().unwrap()));
        assert_eq!(inventory.slots[0].unwrap().loaded, 3);
        assert_eq!(
            inventory
                .slots
                .iter()
                .flatten()
                .map(|s| s.loaded)
                .sum::<u16>(),
            20
        );
    }
    #[test]
    fn farming_loop_crafts_harvests_and_eats_without_losing_items() {
        let mut world = World::new(42);
        world.stream([0, 0], 0, 1);
        let soil = [0, 65, 0];
        world.set(soil, Block::Grass);
        let hit = RayHit {
            block: soil,
            previous: [0, 66, 0],
            distance: 2.,
        };
        let mut inventory = empty_inventory();
        inventory.add(Item::WoodHoe, 1);
        inventory.add(Item::Seeds, 3);
        assert!(farm_use(&mut world, &mut inventory, &hit, Mode::Survival)
            .unwrap()
            .is_ok());
        inventory.selected = 1;
        assert!(farm_use(&mut world, &mut inventory, &hit, Mode::Survival)
            .unwrap()
            .is_ok());
        assert_eq!(inventory.count(Item::Seeds), 2);
        assert!(farm_use(&mut world, &mut inventory, &hit, Mode::Survival)
            .unwrap()
            .is_err());
        assert_eq!(inventory.count(Item::Seeds), 2);
        let plant = [0, 66, 0];
        world.set([4, 65, 0], Block::Water);
        for _ in 0..3 {
            assert_eq!(world.grow_crops(true), 1);
        }
        harvest_crop(&mut world, &mut inventory, plant, Mode::Survival).unwrap();
        assert_eq!(inventory.count(Item::Wheat), 1);
        assert_eq!(inventory.count(Item::Seeds), 4);
        inventory.add(Item::Wheat, 2);
        let bread = recipes()
            .into_iter()
            .find(|r| r.result.0 == Item::Bread)
            .unwrap();
        craft(&mut inventory, &bread).unwrap();
        assert_eq!(inventory.count(Item::Wheat), 0);
        inventory.selected = inventory
            .slots
            .iter()
            .position(|s| s.is_some_and(|s| s.item == Item::Bread))
            .unwrap();
        let mut player = Player::new(world.spawn());
        player.hunger = 10.;
        assert!(eat_selected(&mut inventory, &mut player, Mode::Survival));
        assert_eq!(player.hunger, 18.);
        assert_eq!(inventory.count(Item::Bread), 0);
        // Atomic compound harvest, including an immature plant's returned seed.
        world.set(plant, Block::Wheat3);
        inventory.slots.fill(Some(Stack {
            item: Item::Block(Block::Stone),
            count: 64,
            loaded: 0,
            optic: 0,
        }));
        assert!(harvest_crop(&mut world, &mut inventory, plant, Mode::Survival).is_err());
        assert_eq!(world.get(plant), Block::Wheat3);
        assert!(!collect_block(
            &mut inventory,
            &world,
            soil,
            Item::Block(Block::Dirt)
        ));
        assert_eq!(world.get(soil), Block::Farmland);
        let mut inventory = empty_inventory();
        assert!(collect_block(
            &mut inventory,
            &world,
            soil,
            Item::Block(Block::Dirt)
        ));
        assert_eq!(inventory.count(Item::Seeds), 2);
        assert_eq!(inventory.count(Item::Wheat), 1);
        world.set(soil, Block::Air);
        assert_eq!(world.get(plant), Block::Air);
    }
    #[test]
    fn farm_growth_and_new_items_roundtrip_in_world_save() {
        let mut world = World::new(42);
        world.set([0, 65, 0], Block::Farmland);
        world.set([0, 66, 0], Block::Wheat2);
        let mut inventory = empty_inventory();
        for item in [Item::WoodHoe, Item::Seeds, Item::Wheat, Item::Bread] {
            inventory.add(item, 1);
        }
        let player = Player::new(world.spawn());
        let path =
            std::env::temp_dir().join(format!("voxel-rust-crop-save-{}.json", std::process::id()));
        save_game(
            &path,
            &world,
            &player,
            &inventory,
            0.2,
            Mode::Survival,
            &Default::default(),
        )
        .unwrap();
        let (restored, _, items, _, _, _) = load_game(&path).unwrap();
        assert_eq!(restored.get([0, 66, 0]), Block::Wheat2);
        assert_eq!(restored.crop_count(), 1);
        assert_eq!(items.slots, inventory.slots);
        fs::remove_file(path).unwrap();
    }
    fn empty_inventory() -> Inventory {
        Inventory {
            slots: vec![None; 36],
            selected: 0,
        }
    }
    fn platform_world() -> (World, i32) {
        let mut world = World::new(42);
        let floor = HEIGHT - 10;
        for x in -2..=6 {
            for z in -2..=2 {
                for y in floor..HEIGHT {
                    world.set([x, y, z], Block::Air);
                }
            }
        }
        world.set([0, floor, 0], Block::Stone);
        (world, floor)
    }
    #[test]
    fn slot_clicks_split_merge_swap_and_preserve_stack_limits() {
        let mut inventory = empty_inventory();
        inventory.slots[0] = Some(Stack {
            item: Item::Coal,
            count: 61,
            loaded: 0,
            optic: 0,
        });
        inventory.slots[1] = Some(Stack {
            item: Item::StonePick,
            count: 1,
            loaded: 0,
            optic: 0,
        });
        let mut cursor = Some(Stack {
            item: Item::Coal,
            count: 7,
            loaded: 0,
            optic: 0,
        });
        assert!(inventory.left_click(0, &mut cursor));
        assert_eq!(inventory.slots[0].unwrap().count, 64);
        assert_eq!(cursor.unwrap().count, 4);
        assert!(!inventory.right_click(0, &mut cursor));
        assert!(inventory.right_click(2, &mut cursor));
        assert_eq!(inventory.slots[2].unwrap().count, 1);
        assert_eq!(cursor.unwrap().count, 3);
        assert!(!inventory.right_click(1, &mut cursor));
        assert!(inventory.left_click(1, &mut cursor));
        assert_eq!(cursor.unwrap().item, Item::StonePick);
        assert!(inventory.left_click(3, &mut cursor));
        assert!(cursor.is_none());
        assert!(inventory.right_click(1, &mut cursor));
        assert_eq!(cursor.unwrap().count, 2);
        assert_eq!(inventory.slots[1].unwrap().count, 1);
        assert!(inventory.right_click(1, &mut cursor));
        assert_eq!(cursor.unwrap().count, 1);
        assert!(inventory.left_click(1, &mut cursor));
        assert!(cursor.is_none());
        assert_eq!(inventory.count(Item::Coal), 68);
        assert_eq!(inventory.count(Item::StonePick), 1);
        for stack in inventory.slots.iter().flatten() {
            assert!(stack.count > 0 && stack.count <= stack.item.stack_limit());
        }
        let before = inventory.slots.clone();
        assert!(!inventory.left_click(36, &mut cursor));
        assert!(!inventory.right_click(36, &mut cursor));
        assert_eq!(inventory.slots, before);
    }
    #[test]
    fn quick_transfer_merges_first_and_retains_items_when_full() {
        let mut inventory = empty_inventory();
        for slot in &mut inventory.slots[9..] {
            *slot = Some(Stack {
                item: Item::Apple,
                count: 64,
                loaded: 0,
                optic: 0,
            });
        }
        inventory.slots[0] = Some(Stack {
            item: Item::Coal,
            count: 10,
            loaded: 0,
            optic: 0,
        });
        inventory.slots[1] = Some(Stack {
            item: Item::Coal,
            count: 60,
            loaded: 0,
            optic: 0,
        });
        inventory.slots[9] = Some(Stack {
            item: Item::Coal,
            count: 62,
            loaded: 0,
            optic: 0,
        });
        assert!(inventory.quick_transfer(0));
        assert_eq!(inventory.slots[0].unwrap().count, 8);
        assert_eq!(inventory.slots[9].unwrap().count, 64);
        let before = inventory.slots.clone();
        assert!(!inventory.quick_transfer(0));
        assert_eq!(inventory.slots, before);
        inventory.slots[10] = None;
        assert!(inventory.quick_transfer(0));
        assert!(inventory.slots[0].is_none());
        assert_eq!(inventory.slots[10].unwrap().count, 8);
        assert!(inventory.quick_transfer(9));
        assert_eq!(inventory.slots[1].unwrap().count, 64);
        assert_eq!(inventory.slots[0].unwrap().count, 60);
        assert!(inventory.slots[9].is_none());
        assert_eq!(inventory.count(Item::Coal), 132);
        for slot in &mut inventory.slots[..9] {
            *slot = Some(Stack {
                item: Item::Sword,
                count: 1,
                loaded: 0,
                optic: 0,
            });
        }
        inventory.slots[35] = Some(Stack {
            item: Item::IronPick,
            count: 1,
            loaded: 0,
            optic: 0,
        });
        let before = inventory.slots.clone();
        assert!(!inventory.quick_transfer(35));
        assert_eq!(inventory.slots, before);
    }
    #[test]
    fn ground_jump_is_pressed_once_with_landing_buffer_and_edge_grace() {
        let (world, floor) = platform_world();
        let mut player = Player::new([0.5, floor as f32 + 1.0, 0.5]);
        let held = Movement {
            jump: true,
            ..Movement::default()
        };
        player.update(&world, held, 0.016, Mode::Survival);
        assert!((player.position.y - (floor as f32 + 1.0)).abs() < 0.001);
        player.update(
            &world,
            Movement {
                jump_pressed: true,
                ..held
            },
            0.016,
            Mode::Survival,
        );
        assert!(player.velocity.y > 0.0);
        for _ in 0..100 {
            player.update(&world, held, 0.016, Mode::Survival);
        }
        assert!(player.grounded);
        assert!((player.position.y - (floor as f32 + 1.0)).abs() < 0.001);
        let mut buffered = Player::new([0.5, floor as f32 + 1.10, 0.5]);
        buffered.velocity.y = -2.0;
        buffered.update(
            &world,
            Movement {
                jump_pressed: true,
                ..held
            },
            0.016,
            Mode::Survival,
        );
        for _ in 0..6 {
            buffered.update(&world, held, 0.016, Mode::Survival);
        }
        assert!(buffered.velocity.y > 0.0);
        assert!(buffered.position.y > floor as f32 + 1.2);
        let mut grace = Player::new([0.9, floor as f32 + 1.0, 0.5]);
        grace.update(
            &world,
            Movement {
                right: 1.0,
                ..Movement::default()
            },
            0.10,
            Mode::Survival,
        );
        assert!(!has_support(&world, grace.position));
        grace.update(
            &world,
            Movement {
                jump_pressed: true,
                ..held
            },
            0.016,
            Mode::Survival,
        );
        assert!(grace.velocity.y > 0.0);
    }
    #[test]
    fn sneaking_guards_ledge_and_flight_uses_held_jump_and_sprint() {
        let (world, floor) = platform_world();
        let mut sneaking = Player::new([0.5, floor as f32 + 1.0, 0.5]);
        let input = Movement {
            right: 1.0,
            sneak: true,
            sprint: true,
            ..Movement::default()
        };
        sneaking.update(&world, input, 0.016, Mode::Creative);
        assert_eq!(sneaking.velocity.x, 1.8);
        for _ in 0..100 {
            sneaking.update(&world, input, 0.016, Mode::Creative);
        }
        assert!(sneaking.position.x < 1.3001);
        assert!((sneaking.position.y - (floor as f32 + 1.0)).abs() < 0.001);
        assert!(sneaking.grounded);
        let mut walking = Player::new([0.5, floor as f32 + 1.0, 0.5]);
        for _ in 0..30 {
            walking.update(
                &world,
                Movement {
                    right: 1.0,
                    ..Movement::default()
                },
                0.016,
                Mode::Creative,
            );
        }
        assert!(walking.position.x > 1.3);
        assert!(walking.position.y < floor as f32 + 1.0);
        let mut flying = Player::new([0.5, floor as f32 + 2.0, 0.5]);
        flying.flying = true;
        flying.update(
            &world,
            Movement {
                right: 1.0,
                jump: true,
                sprint: true,
                ..Movement::default()
            },
            0.016,
            Mode::Creative,
        );
        assert_eq!(flying.velocity.x, 18.0);
        assert_eq!(flying.velocity.y, 18.0);
        flying.update(
            &world,
            Movement {
                descend: true,
                ..Movement::default()
            },
            0.016,
            Mode::Creative,
        );
        assert_eq!(flying.velocity.y, -10.0);
    }
    #[test]
    fn inventory_add_and_consume_are_atomic() {
        let mut inventory = empty_inventory();
        assert!(inventory.add(Item::Coal, 100));
        assert_eq!(inventory.count(Item::Coal), 100);
        assert!(!inventory.consume(Item::Coal, 101));
        assert_eq!(inventory.count(Item::Coal), 100);
        assert!(inventory.consume(Item::Coal, 70));
        assert_eq!(inventory.count(Item::Coal), 30);
        inventory.slots = vec![
            Some(Stack {
                item: Item::Apple,
                count: 64,
                loaded: 0,
                optic: 0,
            });
            36
        ];
        let before = inventory.slots.clone();
        assert!(!inventory.add(Item::Coal, 1));
        assert_eq!(inventory.slots, before);
    }
    #[test]
    fn craft_can_use_capacity_freed_by_ingredients_and_rolls_back() {
        let mut inventory = empty_inventory();
        inventory.slots = vec![
            Some(Stack {
                item: Item::Apple,
                count: 64,
                loaded: 0,
                optic: 0,
            });
            36
        ];
        inventory.slots[0] = Some(Stack {
            item: Item::Block(Block::Wood),
            count: 1,
            loaded: 0,
            optic: 0,
        });
        let recipe = recipes()
            .into_iter()
            .find(|r| r.result.0 == Item::Block(Block::Planks))
            .unwrap();
        assert!(craft(&mut inventory, &recipe).is_ok());
        assert_eq!(
            inventory.slots[0],
            Some(Stack {
                item: Item::Block(Block::Planks),
                count: 4,
                loaded: 0,
                optic: 0,
            })
        );
        inventory.slots[0] = Some(Stack {
            item: Item::Block(Block::Wood),
            count: 2,
            loaded: 0,
            optic: 0,
        });
        let before = inventory.slots.clone();
        assert!(craft(&mut inventory, &recipe).is_err());
        assert_eq!(inventory.slots, before);
    }
    #[test]
    fn crafting_requires_the_used_station_and_only_the_table_can_be_made_by_hand() {
        let (mut world, floor) = platform_world();
        let table = [0, floor + 1, 0];
        let furnace = [2, floor + 1, 0];
        world.set(table, Block::Workbench);
        world.set(furnace, Block::Furnace);
        let player = Player::new([0.5, (floor + 1) as f32, 2.5]);
        let all = recipes();
        let bootstrap = all
            .iter()
            .find(|r| r.result.0 == Item::Block(Block::Workbench))
            .unwrap();
        assert_eq!(bootstrap.ingredients, [(Item::Block(Block::Wood), 4)]);
        assert_eq!(bootstrap.station, None);
        for recipe in &all {
            assert_eq!(
                recipe.station.is_none(),
                recipe.result.0 == Item::Block(Block::Workbench)
            );
            for position in [None, Some(table), Some(furnace)] {
                let mut inventory = empty_inventory();
                for &(item, count) in &recipe.ingredients {
                    assert!(inventory.add(item, count));
                }
                let before = inventory.slots.clone();
                let expected = match recipe.station {
                    None => position != Some(furnace),
                    Some(Block::Workbench) => position == Some(table),
                    Some(Block::Furnace) => position == Some(furnace),
                    _ => panic!("Unexpected recipe station"),
                };
                let result = craft_at(&mut inventory, recipe, &world, &player, position);
                assert_eq!(result.is_ok(), expected, "{} at {position:?}", recipe.name);
                if expected {
                    assert_eq!(inventory.count(recipe.result.0), recipe.result.1);
                    for &(ingredient, _) in &recipe.ingredients {
                        assert_eq!(inventory.count(ingredient), 0);
                    }
                } else {
                    assert_eq!(inventory.slots, before);
                }
            }
        }
    }
    #[test]
    fn stale_far_and_wrong_stations_and_failed_crafts_leave_materials_untouched() {
        let (mut world, floor) = platform_world();
        let table = [0, floor + 1, 0];
        let far = [20, floor + 1, 0];
        let player = Player::new([0.5, (floor + 1) as f32, 2.5]);
        let planks = recipes()
            .into_iter()
            .find(|r| r.result.0 == Item::Block(Block::Planks))
            .unwrap();
        let mut inventory = empty_inventory();
        assert!(inventory.add(Item::Block(Block::Wood), 1));
        let before = inventory.slots.clone();
        for block in [Block::Air, Block::Chest, Block::Furnace] {
            world.set(table, block);
            assert!(craft_at(&mut inventory, &planks, &world, &player, Some(table)).is_err());
            assert_eq!(inventory.slots, before);
        }
        world.set(far, Block::Workbench);
        assert!(craft_at(&mut inventory, &planks, &world, &player, Some(far)).is_err());
        assert_eq!(inventory.slots, before);
        world.set(table, Block::Workbench);
        inventory.slots.fill(Some(Stack {
            item: Item::Coal,
            count: 64,
            loaded: 0,
            optic: 0,
        }));
        let before = inventory.slots.clone();
        assert!(craft_at(&mut inventory, &planks, &world, &player, Some(table)).is_err());
        assert_eq!(inventory.slots, before);
        inventory.slots[0] = Some(Stack {
            item: Item::Block(Block::Wood),
            count: 2,
            loaded: 0,
            optic: 0,
        });
        let before = inventory.slots.clone();
        assert!(craft_at(&mut inventory, &planks, &world, &player, Some(table)).is_err());
        assert_eq!(inventory.slots, before);
        inventory.slots[0].as_mut().unwrap().count = 1;
        assert!(craft_at(&mut inventory, &planks, &world, &player, Some(table)).is_ok());
        assert_eq!(inventory.count(Item::Block(Block::Wood)), 0);
        assert_eq!(inventory.count(Item::Block(Block::Planks)), 4);
        assert_eq!(inventory.count(Item::Coal), 35 * 64);
    }
    #[test]
    fn dda_hits_negative_coordinates_and_respects_reach() {
        let mut world = World::new(42);
        for x in -5..=1 {
            world.set([x, HEIGHT - 3, 0], Block::Air);
        }
        world.set([-3, HEIGHT - 3, 0], Block::Stone);
        let origin = vec3(0.5, (HEIGHT - 3) as f32 + 0.5, 0.5);
        let hit = raycast(&world, origin, vec3(-1.0, 0.0, 0.0), 5.0).unwrap();
        assert_eq!(hit.block, [-3, HEIGHT - 3, 0]);
        assert_eq!(hit.previous, [-2, HEIGHT - 3, 0]);
        assert!((hit.distance - 2.5).abs() < 0.001);
        assert!(raycast(&world, origin, vec3(-1.0, 0.0, 0.0), 2.0).is_none());
        assert!(raycast(&world, origin, Vec3::ZERO, 5.0).is_none());
    }
    #[test]
    fn targeting_can_remove_fluids_without_blocking_normal_mining() {
        let mut world = World::new(42);
        let y = HEIGHT - 3;
        world.set([0, y, 0], Block::Air);
        world.set([1, y, 0], Block::Water);
        world.set([2, y, 0], Block::Lava);
        world.set([3, y, 0], Block::Stone);
        let origin = vec3(0.5, y as f32 + 0.5, 0.5);
        let direction = vec3(1.0, 0.0, 0.0);
        assert_eq!(
            raycast(&world, origin, direction, 5.0).unwrap().block,
            [3, y, 0]
        );
        assert_eq!(
            raycast_with_fluids(&world, origin, direction, 5.0)
                .unwrap()
                .block,
            [1, y, 0]
        );
        for fluid in [Block::Water, Block::Lava] {
            assert!(!can_harvest(Some(Item::IronPick), fluid));
            assert_eq!(mining_speed(Some(Item::IronPick), fluid), 0.0);
        }
        assert_eq!(
            Inventory::new(Mode::Creative).count(Item::Block(Block::Lava)),
            0
        );
    }
    #[test]
    fn fluid_contact_uses_surface_height_and_lava_only_hurts_survival() {
        let mut world = World::new(42);
        let y = HEIGHT - 5;
        for x in -1..=1 {
            for z in -1..=1 {
                for block_y in y..HEIGHT {
                    world.set([x, block_y, z], Block::Air);
                }
            }
        }
        let cell = [0, y, 0];
        world.set(cell, Block::Water);
        world.restore_fluid_flows(vec![(
            cell,
            Flow {
                level: 2,
                falling: false,
            },
        )]);
        assert_eq!(
            fluid_at(&world, vec3(0.5, y as f32 + 0.1, 0.5)),
            Some(Block::Water)
        );
        assert_eq!(fluid_at(&world, vec3(0.5, y as f32 + 0.3, 0.5)), None);
        let movement = Movement {
            right: 1.0,
            ..Movement::default()
        };
        let mut above = Player::new([0.5, y as f32 + 0.3, 0.5]);
        above.update(&world, movement, 0.016, Mode::Survival);
        assert_eq!(above.velocity.x, 4.6);
        let mut immersed = Player::new([0.5, y as f32 + 0.1, 0.5]);
        immersed.update(&world, movement, 0.016, Mode::Survival);
        assert_eq!(immersed.velocity.x, 2.8);
        assert_eq!(immersed.health, 20.0);
        world.set(cell, Block::Lava);
        let mut survival = Player::new([0.5, y as f32 + 0.1, 0.5]);
        survival.update(&world, movement, 0.1, Mode::Survival);
        assert!((survival.health - 19.6).abs() < 0.001);
        assert_eq!(survival.velocity.x, 1.4);
        let mut creative = Player::new([0.5, y as f32 + 0.1, 0.5]);
        creative.update(&world, movement, 0.1, Mode::Creative);
        assert_eq!(creative.health, 20.0);
        assert_eq!(creative.velocity.x, 1.4);
    }
    #[test]
    fn collision_prevents_walking_through_wall_and_lands_on_floor() {
        let mut world = World::new(42);
        let floor = HEIGHT - 8;
        for x in -3..=3 {
            for z in -3..=3 {
                for y in floor..HEIGHT {
                    world.set(
                        [x, y, z],
                        if y == floor { Block::Stone } else { Block::Air },
                    );
                }
            }
        }
        world.set([1, floor + 1, 0], Block::Stone);
        world.set([1, floor + 2, 0], Block::Stone);
        let mut player = Player::new([0.5, floor as f32 + 3.0, 0.5]);
        for _ in 0..100 {
            player.update(&world, Movement::default(), 0.016, Mode::Survival);
        }
        assert!((player.position.y - (floor as f32 + 1.0)).abs() < 0.03);
        assert!(player.grounded);
        for _ in 0..100 {
            player.update(
                &world,
                Movement {
                    right: 1.0,
                    ..Movement::default()
                },
                0.016,
                Mode::Survival,
            );
        }
        assert!(player.position.x <= 0.701);
        assert!(!collides(
            &world,
            player.position,
            PLAYER_RADIUS,
            PLAYER_HEIGHT
        ));
    }
    #[test]
    fn save_roundtrip_and_replacement_preserve_world_and_inventory() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "voxel-rust-save-{}-{nonce}.json",
            std::process::id()
        ));
        let mut world = World::new(42);
        world.set([2, HEIGHT - 3, 3], Block::Glass);
        let source = [3, HEIGHT - 3, 3];
        let flowing = [4, HEIGHT - 3, 3];
        let lava = [5, HEIGHT - 3, 3];
        world.set(source, Block::Water);
        world.set(flowing, Block::Water);
        world.set(lava, Block::Lava);
        let flow = Flow {
            level: 4,
            falling: false,
        };
        let lava_flow = Flow {
            level: 8,
            falling: true,
        };
        world.restore_fluid_flows(vec![(flowing, flow), (lava, lava_flow)]);
        let player = Player::new(world.spawn());
        let inventory = Inventory::new(Mode::Survival);
        save_game(
            &path,
            &world,
            &player,
            &inventory,
            120.0,
            Mode::Survival,
            &Default::default(),
        )
        .unwrap();
        save_game(
            &path,
            &world,
            &player,
            &inventory,
            121.0,
            Mode::Survival,
            &Default::default(),
        )
        .unwrap();
        let (loaded, loaded_player, loaded_inventory, time, mode, _) = load_game(&path).unwrap();
        assert_eq!(loaded.seed, 42);
        assert_eq!(loaded.get([2, HEIGHT - 3, 3]), Block::Glass);
        assert!(loaded.is_fluid_source(source));
        assert!(!loaded.is_fluid_source(flowing));
        assert_eq!(loaded.fluid_state(flowing), Some(flow));
        assert_eq!(loaded.fluid_state(lava), Some(lava_flow));
        assert_eq!(loaded_player.position, player.position);
        assert_eq!(loaded_inventory.slots, inventory.slots);
        assert_eq!(time, 121.0);
        assert_eq!(mode, Mode::Survival);
        // Version 1 saves from before fluids omit metadata; their water remains a source.
        let mut legacy: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        legacy.as_object_mut().unwrap().remove("flows");
        fs::write(&path, serde_json::to_vec(&legacy).unwrap()).unwrap();
        let (legacy_world, _, _, _, _, _) = load_game(&path).unwrap();
        assert!(legacy_world.is_fluid_source(flowing));
        legacy["flows"] = serde_json::json!([[flowing, {"level": 0, "falling": false}]]);
        fs::write(&path, serde_json::to_vec(&legacy).unwrap()).unwrap();
        assert!(load_game(&path).is_err());
        legacy["flows"] = serde_json::json!([[[2, HEIGHT - 3, 3], {"level": 4, "falling": false}]]);
        fs::write(&path, serde_json::to_vec(&legacy).unwrap()).unwrap();
        assert!(load_game(&path).is_err());
        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(path.with_extension("bak"));
    }
    #[test]
    fn idle_mob_falls_when_ground_is_removed() {
        let mut world = World::new(42);
        let floor = HEIGHT - 12;
        for x in -1..=1 {
            for z in -1..=1 {
                for y in floor..HEIGHT {
                    world.set(
                        [x, y, z],
                        if y == floor { Block::Stone } else { Block::Air },
                    );
                }
            }
        }
        let mut player = Player::new([20.0, floor as f32 + 1.0, 20.0]);
        let mut mobs = vec![Mob {
            position: vec3(0.5, floor as f32 + 6.0, 0.5),
            kind: MobKind::Zombie,
            health: 20.0,
            phase: 0.0,
            attack_timer: 0.0,
            ..Default::default()
        }];
        for _ in 0..100 {
            update_mobs(&mut mobs, &world, &mut player, 0.016, false, Mode::Creative);
        }
        assert!((mobs[0].position.y - (floor as f32 + 1.0)).abs() < 0.025);
        assert!(!collides(&world, mobs[0].position, 0.32, 1.35));
    }

    #[test]
    fn new_games_are_empty_and_arsenal_drops_survive_save_and_legacy_loading() {
        use crate::drops::{DroppedItem, WorldExtras};
        for mode in [Mode::Creative, Mode::Survival] {
            assert!(Inventory::new(mode).slots.iter().all(Option::is_none));
        }
        let path = std::env::temp_dir().join(format!("voxel-arsenal-{}.json", std::process::id()));
        let world = World::new(987);
        let player = Player::new(world.spawn());
        let mut inventory = Inventory::new(Mode::Creative);
        for (index, gun) in crate::weapons::GUNS.into_iter().enumerate() {
            inventory.slots[index] = Some(Stack {
                item: gun,
                count: 1,
                loaded: gun.magazine_capacity() - 1,
                optic: if crate::weapons::spec(gun).unwrap().scoped {
                    2
                } else {
                    0
                },
            });
        }
        let extras = WorldExtras {
            spawn: Some(world.spawn()),
            drops: vec![DroppedItem {
                stack: inventory.slots[7].unwrap(),
                position: player.eye().to_array(),
                velocity: [1., 2., 3.],
                age: 2.,
            }],
            ..Default::default()
        };
        save_game(
            &path,
            &world,
            &player,
            &inventory,
            0.25,
            Mode::Creative,
            &extras,
        )
        .unwrap();
        let (_, _, loaded, _, _, loose) = load_game(&path).unwrap();
        assert_eq!(loaded.slots, inventory.slots);
        assert_eq!(loose.spawn, extras.spawn);
        assert_eq!(loose.drops[0].stack, extras.drops[0].stack);
        let mut legacy: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        legacy["inventory"]["slots"][11]["item"] = "Vpo139".into();
        legacy["inventory"]["slots"][11]["loaded"] = 9.into();
        legacy["inventory"]["slots"][11]["optic"] = 2.into();
        legacy["extras"]["drops"][0]["stack"] = legacy["inventory"]["slots"][11].clone();
        fs::write(&path, serde_json::to_vec(&legacy).unwrap()).unwrap();
        let (_, _, converted, _, _, loose) = load_game(&path).unwrap();
        assert_eq!(
            converted.slots[11],
            Some(Stack {
                item: Item::Mp5k,
                count: 1,
                loaded: 9,
                optic: 0
            })
        );
        assert_eq!(loose.drops[0].stack, converted.slots[11].unwrap());
        assert_eq!(converted.slots[7], inventory.slots[7]);
        legacy["inventory"]["slots"][11]["optic"] = 3.into();
        fs::write(&path, serde_json::to_vec(&legacy).unwrap()).unwrap();
        assert!(load_game(&path).is_err());
        legacy["inventory"]["slots"][11]["optic"] = 0.into();
        legacy.as_object_mut().unwrap().remove("extras");
        for stack in legacy["inventory"]["slots"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .filter(|s| !s.is_null())
        {
            stack.as_object_mut().unwrap().remove("optic");
        }
        fs::write(&path, serde_json::to_vec(&legacy).unwrap()).unwrap();
        let (_, _, loaded, _, _, loose) = load_game(&path).unwrap();
        assert!(loose.drops.is_empty());
        for stack in loaded.slots.into_iter().flatten() {
            assert_eq!(stack.optic, 0);
        }
        let _ = fs::remove_file(path);
    }
}

#[cfg(test)]
mod generation_save_tests {
    use super::*;
    #[test]
    fn old_saves_keep_their_terrain_and_new_saves_keep_their_generation() {
        let mut world = World::new(64195485);
        world.set([3, 70, 5], Block::Brick);
        let inventory = Inventory::new(Mode::Creative);
        let player = Player::new(world.spawn());
        let path =
            std::env::temp_dir().join(format!("voxel-generation-{}.json", std::process::id()));
        save_game(
            &path,
            &world,
            &player,
            &inventory,
            0.2,
            Mode::Creative,
            &Default::default(),
        )
        .unwrap();
        let (restored, _, _, _, _, _) = load_game(&path).unwrap();
        assert_eq!(restored.generation, 3);
        assert_eq!(restored.edits(), world.edits());
        let mut value: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        value.as_object_mut().unwrap().remove("generation");
        fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        let (legacy, _, _, _, _, _) = load_game(&path).unwrap();
        assert_eq!(legacy.generation, 1);
        assert_eq!(legacy.edits(), world.edits());
        assert_eq!(legacy.get([3, 70, 5]), Block::Brick);
        value["generation"] = 2.into();
        fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        let (previous, _, _, _, _, _) = load_game(&path).unwrap();
        assert_eq!(previous.generation, 2);
        assert_eq!(previous.edits(), world.edits());
        assert_eq!(previous.get([3, 70, 5]), Block::Brick);
        assert!(previous
            .nearest_village([0, 0], 1000)
            .unwrap()
            .terrain
            .is_none());
        value["generation"] = 99.into();
        fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        assert!(load_game(&path).is_err());
        let _ = fs::remove_file(path);
    }
}

#[cfg(test)]
mod city_gameplay_tests {
    use super::*;
    #[test]
    fn imported_chest_materials_can_craft_without_merging_their_block_variants() {
        let wood = mapped("oak_log", None);
        let mut inventory = Inventory::new(Mode::Survival);
        inventory.add(Item::Block(wood), 4);
        assert_eq!(inventory.slots[0].unwrap().item, Item::Block(wood));
        let recipe = recipes()
            .into_iter()
            .find(|r| r.result.0 == Item::Block(Block::Workbench))
            .unwrap();
        craft(&mut inventory, &recipe).unwrap();
        assert_eq!(inventory.count(Item::Block(wood)), 0);
        assert_eq!(inventory.count(Item::Block(Block::Workbench)), 1);
        let slab = mapped("oak_slab", Some(("type", "bottom")));
        inventory.add(Item::Block(slab), 2);
        assert!(!inventory.consume(Item::Block(Block::Planks), 3));
        assert_eq!(inventory.count(Item::Block(slab)), 2);
        assert!(inventory.consume(Item::Block(Block::Planks), 1));
        assert_eq!(inventory.count(Item::Block(slab)), 1);
    }
    #[test]
    fn city_loot_and_grenades_land_on_slabs_and_ladders_can_be_climbed() {
        let mut world = cleared();
        world.stream([29, -13], 1, 9);
        world.set(
            [476, 300, -200],
            mapped("oak_slab", Some(("type", "bottom"))),
        );
        let player = Player::new([480.5, 300., -199.5]);
        let mut inventory = Inventory::new(Mode::Creative);
        let mut extras = crate::drops::WorldExtras::default();
        extras.drops.push(crate::drops::DroppedItem {
            stack: crate::settlements::stack(Item::Coal, 1),
            position: [476.5, 301.2, -199.5],
            velocity: [0.; 3],
            age: 0.,
        });
        let mut combat = crate::combat::Combat::default();
        combat.grenades.push(crate::explosives::Grenade {
            position: vec3(476.5, 301.2, -199.5),
            velocity: Vec3::ZERO,
            age: 0.,
        });
        let mut actor = Player::new(player.position.to_array());
        for _ in 0..110 {
            extras.update(&world, &player, &mut inventory, 0.016);
            combat.update_grenades(&mut world, &mut [], &mut actor, Mode::Creative, 0.016);
        }
        assert!((extras.drops[0].position[1] - 300.62).abs() < 0.03);
        assert!((combat.grenades[0].position.y - 300.60).abs() < 0.05);
        let ladder = mapped("ladder", None);
        for y in 300..=303 {
            world.set([478, y, -200], ladder);
        }
        let mut climber = Player::new([478.5, 300., -199.5]);
        for _ in 0..30 {
            climber.update(
                &world,
                Movement {
                    jump: true,
                    ..Default::default()
                },
                0.016,
                Mode::Survival,
            );
        }
        assert!(climber.position.y > 301.0);
    }
    fn mapped(name: &str, property: Option<(&str, &str)>) -> Block {
        let states = &crate::city::get().unwrap().metadata.states;
        Block::Map(
            states
                .iter()
                .position(|s| {
                    s.source == name
                        && property.is_none_or(|(key, value)| {
                            s.properties.get(key).map(String::as_str) == Some(value)
                        })
                })
                .unwrap() as u16,
        )
    }
    fn cleared() -> World {
        let mut world = World::city(0);
        for x in 474..=480 {
            for z in -202..=-198 {
                for y in 299..=305 {
                    world.set([x, y, z], if y == 299 { Block::Stone } else { Block::Air });
                }
            }
        }
        world
    }
    #[test]
    fn city_partial_blocks_match_collision_rays_steps_and_mob_feet() {
        let mut world = cleared();
        let slab = mapped("oak_slab", Some(("type", "bottom")));
        world.set([476, 300, -200], slab);
        world.set([478, 300, -200], Block::Stone);
        assert!(collides(&world, vec3(476.5, 300.1, -199.5), 0.2, 1.8));
        assert!(!collides(&world, vec3(476.5, 300.501, -199.5), 0.2, 1.8));
        assert_eq!(
            raycast(&world, vec3(475.2, 300.75, -199.5), Vec3::X, 4.)
                .unwrap()
                .block,
            [478, 300, -200]
        );
        let hit = raycast(&world, vec3(475.2, 300.25, -199.5), Vec3::X, 4.).unwrap();
        assert_eq!(hit.block, [476, 300, -200]);
        assert_eq!(hit.previous, [475, 300, -200]);
        assert!((hit.distance - 0.8).abs() < 0.0001);
        world.set(
            [477, 300, -198],
            mapped("oak_fence", Some(("east", "false"))),
        );
        assert!(collides(&world, vec3(477.5, 301.25, -197.5), 0.2, 1.));
        let fence_hit = raycast(&world, vec3(476.5, 301.25, -197.5), Vec3::X, 2.).unwrap();
        assert_eq!(fence_hit.block, [477, 300, -198]);
        let mut player = Player::new([475.3, 300., -199.5]);
        let mut peak: f32 = player.position.y;
        for _ in 0..20 {
            player.update(
                &world,
                Movement {
                    right: 1.,
                    ..Default::default()
                },
                0.016,
                Mode::Survival,
            );
            peak = peak.max(player.position.y);
            assert!(!collides(
                &world,
                player.position,
                PLAYER_RADIUS,
                PLAYER_HEIGHT
            ));
        }
        assert!((peak - 300.5).abs() < 0.001);
        let mut mobs = vec![Mob {
            position: vec3(476.5, 300.5, -199.5),
            kind: MobKind::Zombie,
            health: 20.,
            ..Default::default()
        }];
        player.position = vec3(490., 300., -199.5);
        for _ in 0..120 {
            update_mobs(&mut mobs, &world, &mut player, 0.016, false, Mode::Creative);
        }
        assert!((mobs[0].position.y - 300.5).abs() < 0.001);
    }
    #[test]
    fn city_materials_supply_existing_crafting_farming_and_tools() {
        for (name, material, drop) in [
            ("oak_log", Block::Wood, Item::Block(Block::Wood)),
            ("iron_ore", Block::IronOre, Item::Block(Block::IronOre)),
            ("stone", Block::Stone, Item::Block(Block::Cobble)),
        ] {
            let block = mapped(name, None);
            assert_eq!(block.material(), material);
            assert_eq!(drop_for(block), drop);
            assert_eq!(
                can_harvest(Some(Item::IronPick), block),
                can_harvest(Some(Item::IronPick), material)
            );
            assert_eq!(
                mining_speed(Some(Item::WoodAxe), block),
                mining_speed(Some(Item::WoodAxe), material)
            );
        }
        let mut world = cleared();
        let pos = [476, 300, -200];
        world.set(pos, mapped("crafting_table", None));
        let player = Player::new([475.5, 300., -199.5]);
        assert_eq!(
            crafting_station(&world, &player, Some(pos)),
            Some(Block::Workbench)
        );
        world.set(pos, mapped("furnace", None));
        assert_eq!(
            crafting_station(&world, &player, Some(pos)),
            Some(Block::Furnace)
        );
        world.set(pos, mapped("grass_block", None));
        let mut inventory = Inventory::new(Mode::Survival);
        inventory.add(Item::WoodHoe, 1);
        assert!(farm_use(
            &mut world,
            &mut inventory,
            &RayHit {
                block: pos,
                previous: pos,
                distance: 1.
            },
            Mode::Survival
        )
        .unwrap()
        .is_ok());
        assert_eq!(world.get(pos), Block::Farmland);
    }
    #[test]
    fn city_saves_keep_selected_spawn_height_edits_items_and_identity() {
        let path =
            std::env::temp_dir().join(format!("voxel-city-save-{}.json", std::process::id()));
        let mut world = World::city(4);
        let block = mapped("oak_slab", Some(("type", "bottom")));
        let pos = [476, 300, -200];
        world.set(pos, block);
        world.set([477, 300, -200], Block::Water);
        let flow = Flow {
            level: 3,
            falling: false,
        };
        world.restore_fluid_flows(vec![([477, 300, -200], flow)]);
        let player = Player::new(world.spawn());
        let mut inventory = Inventory::new(Mode::Creative);
        inventory.add(Item::Block(block), 7);
        let extras = crate::drops::WorldExtras {
            city_populated: vec![[29, -13]],
            ..Default::default()
        };
        save_game(
            &path,
            &world,
            &player,
            &inventory,
            0.2,
            Mode::Creative,
            &extras,
        )
        .unwrap();
        let (loaded, player2, inventory2, _, _, extras2) = load_game(&path).unwrap();
        assert_eq!(loaded.generation, 4);
        assert_eq!(loaded.city_spawn, 4);
        assert_eq!(loaded.spawn(), world.spawn());
        assert_eq!(loaded.get(pos), block);
        assert_eq!(loaded.fluid_state([477, 300, -200]), Some(flow));
        assert_eq!(player2.position, player.position);
        assert_eq!(inventory2.slots, inventory.slots);
        assert_eq!(extras2.city_populated, extras.city_populated);
        let original: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        for (field, value) in [
            ("city_spawn", serde_json::json!(6)),
            ("map_id", serde_json::json!("other-map")),
        ] {
            let mut bad = original.clone();
            bad[field] = value;
            fs::write(&path, serde_json::to_vec(&bad).unwrap()).unwrap();
            assert!(load_game(&path).is_err());
        }
        let mut bad = original.clone();
        bad["inventory"]["slots"][0]["item"] = serde_json::json!({"Block":{"Map":65535}});
        fs::write(&path, serde_json::to_vec(&bad).unwrap()).unwrap();
        assert!(load_game(&path).is_err());
        bad = original;
        bad["edits"][0][0][1] = serde_json::json!(384);
        fs::write(&path, serde_json::to_vec(&bad).unwrap()).unwrap();
        assert!(load_game(&path).is_err());
        let _ = fs::remove_file(path);
    }
}
