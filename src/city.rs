//! The supplied, immutable city is the base; gameplay edits remain in the normal save.
use crate::{
    game::{Item, MobKind, Stack},
    world::{Block, ChunkKey, Flow, Pos},
};
use serde::Deserialize;
use std::{
    collections::HashMap,
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::PathBuf,
    sync::{Mutex, OnceLock},
};

pub const HEIGHT: i32 = 384;
pub const SEA: i32 = 127;
pub type Box3 = [f32; 6];
pub const FULL: [Box3; 1] = [[0., 0., 0., 1., 1., 1.]];

#[derive(Deserialize)]
pub struct State {
    pub name: String,
    pub source: String,
    pub properties: HashMap<String, String>,
    pub material: Block,
    pub boxes: Vec<Box3>,
    pub colliders: Vec<Box3>,
    pub tiles: [usize; 6],
    pub full: bool,
    pub emission: f32,
    pub flow: Option<Flow>,
    pub open_variant: Option<u16>,
    pub partner: i32,
    pub partner_state: Option<u16>,
    pub climb: bool,
    pub color: [u8; 3],
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        game::{collides, Inventory, Mode},
        settlements,
    };
    #[test]
    fn all_real_city_spawns_and_imported_storage_survive_chunk_eviction() {
        let city = get().unwrap();
        assert_eq!(city.entries.len(), 69344);
        assert_eq!(city.metadata.containers.len(), 1311);
        let mut positions = std::collections::HashSet::new();
        for i in 0..6 {
            let mut world = crate::world::World::city(i);
            let p = world.spawn();
            assert!(positions.insert(p.map(f32::to_bits)));
            assert!(p[1] > 80.);
            assert!(!collides(
                &world,
                macroquad::prelude::Vec3::from_array(p),
                0.3,
                1.8
            ));
            let floor = [
                p[0].floor() as i32,
                p[1].floor() as i32 - 1,
                p[2].floor() as i32,
            ];
            let original = world.get(floor);
            assert!(original.solid());
            let key = [floor[0].div_euclid(16), floor[2].div_euclid(16)];
            world.stream(key, 0, 1);
            assert_eq!(world.get(floor), original);
            world.set(floor, Block::Glass);
            world.stream([key[0] + 4, key[1]], 0, 1);
            assert!(!world.chunks.contains_key(&key));
            assert_eq!(world.get(floor), Block::Glass);
            world.stream(key, 0, 1);
            assert_eq!(world.get(floor), Block::Glass);
            assert!(Inventory::new(Mode::Survival)
                .slots
                .iter()
                .all(Option::is_none));
            assert!(world.village_at(floor[0], floor[2]).is_none());
        }
        let world = crate::world::World::city(0);
        let c = city
            .metadata
            .containers
            .iter()
            .find(|c| !c.items.is_empty())
            .unwrap();
        assert_eq!(world.get(c.position).material(), Block::Chest);
        let mut extras = crate::drops::WorldExtras::default();
        let index = settlements::chest_index(&mut extras, &world, c.position);
        for &(slot, stack) in &c.items {
            assert_eq!(extras.chests[index].inventory.slots[slot], Some(stack));
        }
        extras.chests[index].inventory.slots.fill(None);
        assert_eq!(
            settlements::chest_index(&mut extras, &world, c.position),
            index
        );
        assert!(extras.chests[index]
            .inventory
            .slots
            .iter()
            .all(Option::is_none));
        let mut world = crate::world::World::city(0);
        let door = city
            .metadata
            .states
            .iter()
            .enumerate()
            .find(|(_, s)| s.source == "oak_door" && s.partner == 1 && s.open_variant.is_some())
            .unwrap();
        let pos = [476, 300, -200];
        world.set(pos, Block::Map(door.0 as u16));
        world.set(
            [pos[0], pos[1] + 1, pos[2]],
            Block::Map(door.1.partner_state.unwrap()),
        );
        world.set(pos, Block::Air);
        assert_eq!(world.get([pos[0], pos[1] + 1, pos[2]]), Block::Air);
    }
}
#[derive(Deserialize)]
pub struct Spawn {
    pub name: String,
    pub description: String,
    pub position: [f32; 3],
    pub yaw: f32,
}
#[derive(Deserialize)]
pub struct Container {
    pub position: Pos,
    pub items: Vec<(usize, Stack)>,
    pub loot: bool,
}
#[derive(Deserialize)]
pub struct Resident {
    pub position: [f32; 3],
    pub kind: MobKind,
}
#[derive(Deserialize)]
pub struct Metadata {
    pub id: String,
    pub source_sha256: String,
    pub data_sha256: String,
    pub bounds: [i32; 4],
    pub states: Vec<State>,
    pub region_palettes: Vec<Vec<u16>>,
    pub spawns: Vec<Spawn>,
    pub containers: Vec<Container>,
    pub residents: Vec<Resident>,
    pub atlas_height: usize,
}
struct Entry {
    offset: u64,
    length: usize,
    crc: u32,
    region: usize,
    height: usize,
}
pub struct City {
    pub metadata: Metadata,
    entries: HashMap<ChunkKey, Entry>,
    containers: HashMap<Pos, usize>,
    pub residents: HashMap<ChunkKey, Vec<usize>>,
    file: Mutex<File>,
}
static CITY: OnceLock<Result<City, String>> = OnceLock::new();
pub fn get() -> Result<&'static City, String> {
    CITY.get_or_init(City::load).as_ref().map_err(Clone::clone)
}
pub fn state(id: u16) -> Option<&'static State> {
    get().ok()?.metadata.states.get(id as usize)
}
impl City {
    fn load() -> Result<Self, String> {
        let executable = std::env::current_exe().map_err(|e| e.to_string())?;
        let installed = executable.parent().unwrap().join("assets/maps");
        let development = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/maps");
        let directory = if installed.join("city.json").is_file() {
            installed
        } else {
            development
        };
        let metadata: Metadata =
            serde_json::from_slice(&std::fs::read(directory.join("city.json")).map_err(|e| {
                format!("Map Apocalypse City introuvable : {e}. Extrayez l’archive complète.")
            })?)
            .map_err(|e| e.to_string())?;
        if metadata.states.is_empty()
            || metadata.states.len() > u16::MAX as usize
            || metadata.spawns.len() != 6
            || metadata.atlas_height > 4096
            || !metadata.atlas_height.is_multiple_of(16)
        {
            return Err("Catalogue ou points de départ de la map invalides".into());
        }
        for s in &metadata.states {
            if matches!(s.material, Block::Map(_))
                || s.boxes.len() > 24
                || s.colliders.len() > 24
                || !s.emission.is_finite()
                || s.boxes.iter().chain(&s.colliders).any(|b| {
                    b.iter().any(|v| !v.is_finite())
                        || (0..3).any(|a| b[a] < 0. || b[a + 3] > 1.5 || b[a] >= b[a + 3])
                })
                || s.open_variant
                    .is_some_and(|i| i as usize >= metadata.states.len())
                || s.partner_state
                    .is_some_and(|i| i as usize >= metadata.states.len())
                || s.tiles.iter().any(|&i| i >= metadata.atlas_height / 16 * 8)
                || !(-1..=1).contains(&s.partner)
            {
                return Err("État de bloc de la map invalide".into());
            }
        }
        let mut file = File::open(directory.join("city.bin")).map_err(|e| e.to_string())?;
        let length = file.metadata().map_err(|e| e.to_string())?.len();
        let mut header = [0; 16];
        file.read_exact(&mut header).map_err(|e| e.to_string())?;
        if &header[..8] != b"VXCTY001" {
            return Err("Format de map inconnu".into());
        }
        let count = u32::from_le_bytes(header[8..12].try_into().unwrap()) as usize;
        if count != 69344
            || u32::from_le_bytes(header[12..].try_into().unwrap()) as usize
                != metadata.region_palettes.len()
        {
            return Err("Index de map incomplet".into());
        }
        let mut entries = HashMap::with_capacity(count);
        for _ in 0..count {
            let mut b = [0; 28];
            file.read_exact(&mut b).map_err(|e| e.to_string())?;
            let key = [
                i32::from_le_bytes(b[0..4].try_into().unwrap()),
                i32::from_le_bytes(b[4..8].try_into().unwrap()),
            ];
            let entry = Entry {
                offset: u64::from_le_bytes(b[8..16].try_into().unwrap()),
                length: u32::from_le_bytes(b[16..20].try_into().unwrap()) as usize,
                crc: u32::from_le_bytes(b[20..24].try_into().unwrap()),
                region: u16::from_le_bytes(b[24..26].try_into().unwrap()) as usize,
                height: u16::from_le_bytes(b[26..28].try_into().unwrap()) as usize,
            };
            if entry.height == 0
                || entry.height > HEIGHT as usize
                || entry.region >= metadata.region_palettes.len()
                || entry.offset < (16 + count * 28) as u64
                || entry
                    .offset
                    .checked_add(entry.length as u64)
                    .is_none_or(|end| end > length)
                || entries.insert(key, entry).is_some()
            {
                return Err("Entrée de chunk de la map invalide".into());
            }
        }
        if metadata
            .region_palettes
            .iter()
            .flatten()
            .any(|&i| i as usize >= metadata.states.len())
        {
            return Err("Palette de région invalide".into());
        }
        let in_bounds = |p: Pos| {
            (0..HEIGHT).contains(&p[1])
                && entries.contains_key(&[p[0].div_euclid(16), p[2].div_euclid(16)])
        };
        if metadata.id != "apocalypse-city-v1.32-import-1"
            || [&metadata.source_sha256, &metadata.data_sha256]
                .iter()
                .any(|s| s.len() != 64 || !s.bytes().all(|b| b.is_ascii_hexdigit()))
            || metadata.bounds != [-2112, 2207, -2176, 2335]
            || metadata.spawns.iter().any(|s| {
                !s.yaw.is_finite()
                    || s.position.iter().any(|v| !v.is_finite())
                    || !in_bounds(s.position.map(|v| v.floor() as i32))
            })
            || metadata.residents.iter().any(|r| {
                r.position.iter().any(|v| !v.is_finite())
                    || !in_bounds(r.position.map(|v| v.floor() as i32))
            })
            || metadata.containers.iter().any(|c| {
                !in_bounds(c.position)
                    || c.items.iter().any(|(slot, stack)| {
                        let mut checked = *stack;
                        if let Item::Block(Block::Map(id)) = checked.item {
                            if id as usize >= metadata.states.len()
                                || metadata.states[id as usize].material == Block::Air
                            {
                                return true;
                            }
                            checked.item = Item::Block(Block::Stone);
                        }
                        *slot >= 27 || !checked.valid()
                    })
            })
        {
            return Err("Identité, positions ou objets de la map invalides".into());
        }
        let containers = metadata
            .containers
            .iter()
            .enumerate()
            .map(|(i, c)| (c.position, i))
            .collect();
        let mut residents: HashMap<ChunkKey, Vec<usize>> = HashMap::new();
        for (i, r) in metadata.residents.iter().enumerate() {
            residents
                .entry([
                    (r.position[0].floor() as i32).div_euclid(16),
                    (r.position[2].floor() as i32).div_euclid(16),
                ])
                .or_default()
                .push(i);
        }
        Ok(Self {
            metadata,
            entries,
            containers,
            residents,
            file: Mutex::new(file),
        })
    }
    pub fn contains(&self, x: i32, z: i32) -> bool {
        self.entries
            .contains_key(&[x.div_euclid(16), z.div_euclid(16)])
    }
    pub fn chunk(&self, key: ChunkKey) -> Option<Vec<u16>> {
        let e = self.entries.get(&key)?;
        let mut bytes = vec![0; e.length];
        let mut file = self.file.lock().expect("City file lock");
        file.seek(SeekFrom::Start(e.offset))
            .and_then(|_| file.read_exact(&mut bytes))
            .expect("City chunk read");
        drop(file);
        let raw = miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&bytes, e.height * 512)
            .expect("City chunk decompression");
        assert_eq!(raw.len(), e.height * 512, "City chunk length");
        assert_eq!(crc32fast::hash(&raw), e.crc, "City chunk checksum");
        let palette = &self.metadata.region_palettes[e.region];
        Some(
            raw.as_chunks::<2>()
                .0
                .iter()
                .map(|b| {
                    let i = u16::from_le_bytes([b[0], b[1]]) as usize;
                    *palette.get(i).expect("City palette index")
                })
                .collect(),
        )
    }
    pub fn block(&self, id: u16) -> Block {
        let material = self.metadata.states[id as usize].material;
        if matches!(
            material,
            Block::Air | Block::Water | Block::Lava | Block::Bedrock
        ) {
            material
        } else {
            Block::Map(id)
        }
    }
    pub fn container(&self, pos: Pos) -> Option<&Container> {
        self.containers
            .get(&pos)
            .map(|&i| &self.metadata.containers[i])
    }
}
