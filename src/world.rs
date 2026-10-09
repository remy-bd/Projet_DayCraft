use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};

pub const CHUNK: i32 = 16;
pub const HEIGHT: i32 = 80;
pub const SEA: i32 = 23;
pub type Pos = [i32; 3];
pub type ChunkKey = [i32; 2];
const MAX_FLUID_QUEUE: usize = 32_768;
const NEIGHBORS: [Pos; 6] = [
    [1, 0, 0],
    [-1, 0, 0],
    [0, 1, 0],
    [0, -1, 0],
    [0, 0, 1],
    [0, 0, -1],
];
const SIDES: [Pos; 4] = [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1]];

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Flow {
    pub level: u8,
    pub falling: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum Block {
    Air,
    Grass,
    Dirt,
    Stone,
    Sand,
    Wood,
    Leaves,
    Water,
    Planks,
    Cobble,
    Glass,
    Brick,
    CoalOre,
    IronOre,
    Torch,
    Workbench,
    Furnace,
    Snow,
    Bedrock,
    Lava,
    Farmland,
    Wheat0,
    Wheat1,
    Wheat2,
    Wheat3,
    Bed,
    Sandstone,
    Path,
    Chest,
    Door,
    OpenDoor,
    Map(u16),
}

impl Block {
    pub fn material(self) -> Self {
        if let Self::Map(id) = self {
            crate::city::state(id).map_or(Self::Air, |s| s.material)
        } else {
            self
        }
    }
    pub fn boxes(self) -> &'static [crate::city::Box3] {
        if let Self::Map(id) = self {
            crate::city::state(id).map_or(&[], |s| s.colliders.as_slice())
        } else if self.solid() {
            &crate::city::FULL
        } else {
            &[]
        }
    }
    pub fn valid(self) -> bool {
        !matches!(self,Self::Map(id) if crate::city::state(id).is_none())
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Map(id) => crate::city::state(id).map_or("Bloc invalide", |s| s.name.as_str()),
            Self::Air => "Air",
            Self::Grass => "Herbe",
            Self::Dirt => "Terre",
            Self::Stone => "Pierre",
            Self::Sand => "Sable",
            Self::Wood => "Bois",
            Self::Leaves => "Feuillage",
            Self::Water => "Eau",
            Self::Planks => "Planches",
            Self::Cobble => "Pavés",
            Self::Glass => "Verre",
            Self::Brick => "Briques",
            Self::CoalOre => "Minerai de charbon",
            Self::IronOre => "Minerai de fer",
            Self::Torch => "Torche",
            Self::Workbench => "Établi",
            Self::Furnace => "Four",
            Self::Snow => "Neige",
            Self::Bedrock => "Roche mère",
            Self::Lava => "Lave",
            Self::Farmland => "Terre labourée",
            Self::Wheat0 => "Pousses de blé",
            Self::Wheat1 => "Blé en croissance",
            Self::Wheat2 => "Blé presque mûr",
            Self::Wheat3 => "Blé mûr",
            Self::Bed => "Lit",
            Self::Sandstone => "Grès",
            Self::Path => "Chemin",
            Self::Chest => "Coffre",
            Self::Door | Self::OpenDoor => "Porte en bois",
        }
    }

    pub fn solid(self) -> bool {
        if let Self::Map(id) = self {
            return crate::city::state(id).is_some_and(|s| !s.colliders.is_empty());
        }
        !matches!(
            self,
            Self::Air | Self::Water | Self::Lava | Self::Torch | Self::OpenDoor
        ) && self.crop_stage().is_none()
    }

    pub fn crop_stage(self) -> Option<u8> {
        match self {
            Self::Map(_) => self.material().crop_stage(),
            Self::Wheat0 => Some(0),
            Self::Wheat1 => Some(1),
            Self::Wheat2 => Some(2),
            Self::Wheat3 => Some(3),
            _ => None,
        }
    }

    pub fn wheat(stage: u8) -> Self {
        match stage {
            0 => Self::Wheat0,
            1 => Self::Wheat1,
            2 => Self::Wheat2,
            _ => Self::Wheat3,
        }
    }

    pub fn fluid(self) -> bool {
        matches!(self, Self::Water | Self::Lava)
    }

    pub fn hardness(self) -> f32 {
        match self {
            Self::Map(_) => self.material().hardness(),
            Self::Air | Self::Water | Self::Lava => 0.0,
            Self::Torch
            | Self::Leaves
            | Self::Wheat0
            | Self::Wheat1
            | Self::Wheat2
            | Self::Wheat3 => 0.2,
            Self::Glass => 0.3,
            Self::Sand | Self::Snow => 0.45,
            Self::Dirt | Self::Grass | Self::Farmland => 0.55,
            Self::Wood
            | Self::Planks
            | Self::Workbench
            | Self::Bed
            | Self::Chest
            | Self::Door
            | Self::OpenDoor => 1.15,
            Self::Sandstone => 1.5,
            Self::Path => 0.65,
            Self::Stone | Self::Cobble | Self::Brick => 1.7,
            Self::CoalOre => 2.1,
            Self::IronOre | Self::Furnace => 2.6,
            Self::Bedrock => f32::INFINITY,
        }
    }

    /// Face order: +X, -X, +Y (top), -Y (bottom), +Z, -Z.
    pub fn color(self, face: usize) -> [u8; 3] {
        match self {
            Self::Map(id) => crate::city::state(id).map_or([0, 0, 0], |s| s.color),
            Self::Air => [0, 0, 0],
            Self::Grass => match face {
                2 => [102, 172, 65],
                3 => [123, 85, 55],
                _ => [114, 123, 61],
            },
            Self::Dirt => [130, 91, 61],
            Self::Stone => [128, 134, 140],
            Self::Sand => [224, 206, 144],
            Self::Wood => {
                if face == 2 || face == 3 {
                    [168, 128, 75]
                } else {
                    [116, 82, 47]
                }
            }
            Self::Leaves => [68, 140, 60],
            Self::Water => [58, 133, 205],
            Self::Planks => [190, 146, 89],
            Self::Cobble => [116, 123, 128],
            Self::Glass => [179, 220, 230],
            Self::Brick => [181, 90, 71],
            Self::CoalOre => [92, 96, 103],
            Self::IronOre => [157, 133, 116],
            Self::Torch => {
                if face == 2 {
                    [255, 211, 83]
                } else {
                    [230, 159, 52]
                }
            }
            Self::Workbench => {
                if face == 2 {
                    [165, 110, 58]
                } else {
                    [135, 86, 42]
                }
            }
            Self::Furnace => [95, 102, 111],
            Self::Snow => {
                if face == 2 {
                    [242, 248, 251]
                } else {
                    [210, 226, 233]
                }
            }
            Self::Bedrock => [51, 56, 63],
            Self::Lava => [245, 104, 30],
            Self::Farmland => [108, 72, 42],
            Self::Wheat0 | Self::Wheat1 => [83, 150, 43],
            Self::Wheat2 => [166, 175, 55],
            Self::Wheat3 => [225, 189, 76],
            Self::Bed => [255, 255, 255],
            Self::Sandstone => [220, 204, 145],
            Self::Path => [163, 133, 75],
            Self::Chest => [156, 99, 43],
            Self::Door | Self::OpenDoor => [149, 104, 53],
        }
    }
}

pub struct Chunk {
    pub blocks: Vec<Block>,
    pub dirty: bool,
}

pub struct World {
    pub seed: u64,
    pub generation: u8,
    pub city_spawn: u8,
    pub chunks: HashMap<ChunkKey, Chunk>,
    edits: HashMap<Pos, Block>,
    flows: HashMap<Pos, Flow>,
    fluid_queue: VecDeque<Pos>,
    fluid_queued: HashSet<Pos>,
    fluid_deferred: HashMap<ChunkKey, VecDeque<Pos>>,
    fluid_waiting: HashSet<Pos>,
    fluid_resume: VecDeque<ChunkKey>,
    fluid_resuming: HashSet<ChunkKey>,
    crops: HashSet<Pos>,
    crop_cursor: usize,
    villages: RefCell<HashMap<[i32; 2], Option<crate::villages::Village>>>,
    entrances: RefCell<HashMap<[i32; 2], Option<Pos>>>,
    city_cache: RefCell<HashMap<ChunkKey, Vec<u16>>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Biome {
    Forest,
    Plains,
    Desert,
    Snow,
}

#[derive(Clone, Copy)]
struct Column {
    height: i32,
    biome: Biome,
}

#[derive(Clone, Copy)]
struct Tree {
    x: i32,
    z: i32,
    ground: i32,
    height: i32,
}

impl World {
    pub fn new(seed: u64) -> Self {
        Self {
            seed,
            generation: 3,
            city_spawn: 0,
            chunks: HashMap::new(),
            edits: HashMap::new(),
            flows: HashMap::new(),
            fluid_queue: VecDeque::new(),
            fluid_queued: HashSet::new(),
            fluid_deferred: HashMap::new(),
            fluid_waiting: HashSet::new(),
            fluid_resume: VecDeque::new(),
            fluid_resuming: HashSet::new(),
            crops: HashSet::new(),
            crop_cursor: 0,
            villages: RefCell::new(HashMap::new()),
            entrances: RefCell::new(HashMap::new()),
            city_cache: RefCell::new(HashMap::new()),
        }
    }

    pub fn city(spawn: u8) -> Self {
        assert!(spawn < 6);
        crate::city::get().expect("Apocalypse City map");
        let mut world = Self::new(64195485);
        world.generation = 4;
        world.city_spawn = spawn;
        world
    }
    pub fn height(&self) -> i32 {
        if self.generation == 4 {
            crate::city::HEIGHT
        } else {
            HEIGHT
        }
    }
    pub fn sea(&self) -> i32 {
        if self.generation == 4 {
            crate::city::SEA
        } else {
            SEA
        }
    }
    pub fn contains(&self, x: i32, z: i32) -> bool {
        self.generation != 4 || crate::city::get().expect("City map").contains(x, z)
    }
    pub fn city_state(&self, pos: Pos) -> Option<u16> {
        if self.generation != 4 || pos[1] < 0 || pos[1] >= self.height() {
            return None;
        }
        let key = chunk_key(pos);
        if !self.city_cache.borrow().contains_key(&key) {
            let data = crate::city::get().expect("City map").chunk(key)?;
            let mut cache = self.city_cache.borrow_mut();
            if cache.len() >= 64 {
                cache.clear();
            }
            cache.insert(key, data);
        }
        let cache = self.city_cache.borrow();
        cache
            .get(&key)?
            .get(index(
                pos[0].rem_euclid(CHUNK),
                pos[1],
                pos[2].rem_euclid(CHUNK),
            ))
            .copied()
    }

    pub fn get(&self, pos: Pos) -> Block {
        if pos[1] < 0 {
            return Block::Bedrock;
        }
        if pos[1] >= self.height() {
            return Block::Air;
        }
        if !self.contains(pos[0], pos[2]) {
            return Block::Bedrock;
        }
        let key = [pos[0].div_euclid(CHUNK), pos[2].div_euclid(CHUNK)];
        if let Some(chunk) = self.chunks.get(&key) {
            return chunk
                .blocks
                .get(index(
                    pos[0].rem_euclid(CHUNK),
                    pos[1],
                    pos[2].rem_euclid(CHUNK),
                ))
                .copied()
                .unwrap_or(Block::Air);
        }
        self.edits
            .get(&pos)
            .copied()
            .unwrap_or_else(|| self.generated(pos))
    }

    pub fn set(&mut self, pos: Pos, block: Block) -> bool {
        // A player's placement makes a source, including when replacing a flowing cell.
        let paired = if block == Block::Air {
            if let Block::Map(id) = self.get(pos) {
                crate::city::state(id)
                    .filter(|s| s.partner != 0)
                    .map(|s| ([pos[0], pos[1] + s.partner, pos[2]], s.source.as_str()))
            } else {
                None
            }
        } else {
            None
        };
        let changed = self.write_cell(pos, block, None);
        if changed {
            if let Some((other, source)) = paired {
                if let Block::Map(id) = self.get(other) {
                    if crate::city::state(id).is_some_and(|s| s.source == source) {
                        self.write_cell(other, Block::Air, None);
                    }
                }
            }
        }
        changed
    }
    pub fn edited(&self, pos: Pos) -> bool {
        self.edits.contains_key(&pos)
    }

    fn write_cell(&mut self, pos: Pos, block: Block, flow: Option<Flow>) -> bool {
        if pos[1] <= 0
            || pos[1] >= self.height()
            || !self.contains(pos[0], pos[2])
            || !block.valid()
            || (self.get(pos) == block && self.flow_override(pos) == flow)
        {
            return false;
        }
        if let Some(flow) = flow.filter(|_| block.fluid()) {
            self.flows.insert(pos, flow);
        } else {
            self.flows.remove(&pos);
        }
        if block.crop_stage().is_some() {
            self.crops.insert(pos);
        } else {
            self.crops.remove(&pos);
        }
        if self.generated(pos) == block
            && !(block.fluid() && flow.is_none() && self.imported_flow(pos).is_some())
        {
            self.edits.remove(&pos);
        } else {
            self.edits.insert(pos, block);
        }
        let x = pos[0].rem_euclid(CHUNK);
        let z = pos[2].rem_euclid(CHUNK);
        let key = [pos[0].div_euclid(CHUNK), pos[2].div_euclid(CHUNK)];
        if let Some(chunk) = self.chunks.get_mut(&key) {
            let needed = (pos[1] + 1) as usize * (CHUNK * CHUNK) as usize;
            if chunk.blocks.len() < needed {
                chunk.blocks.resize(needed, Block::Air);
            }
            chunk.blocks[index(x, pos[1], z)] = block;
            chunk.dirty = true;
        }
        for (boundary, neighbor) in [
            (x == 0, [key[0] - 1, key[1]]),
            (x == CHUNK - 1, [key[0] + 1, key[1]]),
            (z == 0, [key[0], key[1] - 1]),
            (z == CHUNK - 1, [key[0], key[1] + 1]),
        ] {
            if boundary {
                if let Some(chunk) = self.chunks.get_mut(&neighbor) {
                    chunk.dirty = true;
                }
            }
        }
        self.queue_fluid_neighbors(pos);
        if block.material() != Block::Farmland
            && self
                .get([pos[0], pos[1] + 1, pos[2]])
                .crop_stage()
                .is_some()
        {
            self.set([pos[0], pos[1] + 1, pos[2]], Block::Air);
        }
        true
    }

    pub fn crop_count(&self) -> usize {
        self.crops.len()
    }

    /// Water at soil height or just above irrigates a four-block horizontal radius.
    pub fn irrigated(&self, soil: Pos) -> bool {
        (-4..=4).any(|x| {
            (-4..=4).any(|z| {
                (0..=1).any(|y| self.get([soil[0] + x, soil[1] + y, soil[2] + z]) == Block::Water)
            })
        })
    }

    pub fn crop_lit(&self, pos: Pos, daylight: bool) -> bool {
        if daylight && (pos[1] + 1..self.height()).all(|y| !self.get([pos[0], y, pos[2]]).solid()) {
            return true;
        }
        (-3..=3).any(|x| {
            (-3..=3).any(|z| {
                (-2..=2).any(|y| {
                    self.get([pos[0] + x, pos[1] + y, pos[2] + z]).material() == Block::Torch
                })
            })
        })
    }

    /// Advance loaded, irrigated, lit plants by one stage on each farming tick.
    pub fn grow_crops(&mut self, daylight: bool) -> usize {
        let mut plants: Vec<_> = self.crops.iter().copied().collect();
        plants.sort_unstable();
        if plants.is_empty() {
            return 0;
        }
        let mut grown = 0;
        // ponytail: at most 512 plants per growth tick; enormous farms grow in rounds.
        for offset in 0..plants.len().min(512) {
            let pos = plants[(self.crop_cursor + offset) % plants.len()];
            if !self.chunks.contains_key(&chunk_key(pos)) {
                continue;
            }
            let soil = [pos[0], pos[1] - 1, pos[2]];
            if self.get(soil).material() != Block::Farmland {
                self.set(pos, Block::Air);
            } else if let Some(stage @ 0..=2) = self.get(pos).crop_stage() {
                if self.irrigated(soil)
                    && self.crop_lit(pos, daylight)
                    && self.set(pos, Block::wheat(stage + 1))
                {
                    grown += 1;
                }
            }
        }
        self.crop_cursor = (self.crop_cursor + plants.len().min(512)) % plants.len();
        grown
    }

    pub fn fluid_state(&self, pos: Pos) -> Option<Flow> {
        self.get(pos).fluid().then(|| {
            self.flow_override(pos).unwrap_or(Flow {
                level: 8,
                falling: false,
            })
        })
    }

    pub fn is_fluid_source(&self, pos: Pos) -> bool {
        self.get(pos).fluid() && self.flow_override(pos).is_none()
    }

    fn imported_flow(&self, pos: Pos) -> Option<Flow> {
        crate::city::state(self.city_state(pos)?)
            .and_then(|s| s.flow)
            .filter(|f| f.level != 8 || f.falling)
    }
    fn flow_override(&self, pos: Pos) -> Option<Flow> {
        self.flows.get(&pos).copied().or_else(|| {
            (!self.edits.contains_key(&pos))
                .then(|| self.imported_flow(pos))
                .flatten()
        })
    }

    /// Shared surface height for drawing, swimming and fluid selection.
    pub fn fluid_height(&self, pos: Pos) -> Option<f32> {
        self.fluid_state(pos).map(|flow| {
            if flow.falling || self.get([pos[0], pos[1] + 1, pos[2]]) == self.get(pos) {
                1.0
            } else {
                flow.level as f32 / 8.0 * 0.88
            }
        })
    }

    pub fn fluid_flows(&self) -> Vec<(Pos, Flow)> {
        let mut flows: Vec<_> = self.flows.iter().map(|(&pos, &flow)| (pos, flow)).collect();
        flows.sort_unstable_by_key(|(pos, _)| *pos);
        flows
    }

    /// Call after restore_edits, before advancing the simulation.
    pub fn restore_fluid_flows(&mut self, flows: Vec<(Pos, Flow)>) {
        self.flows.clear();
        for (pos, flow) in flows {
            if pos[1] > 0
                && pos[1] < self.height()
                && (1..=8).contains(&flow.level)
                && self.get(pos).fluid()
            {
                self.flows.insert(pos, flow);
                self.queue_fluid_neighbors(pos);
            }
        }
    }

    fn queue_fluid_neighbors(&mut self, pos: Pos) {
        self.queue_fluid(pos);
        for d in NEIGHBORS {
            self.queue_fluid(add(pos, d));
        }
    }

    fn queue_fluid(&mut self, pos: Pos) {
        if pos[1] <= 0
            || pos[1] >= self.height()
            || !self.contains(pos[0], pos[2])
            || self.fluid_queued.contains(&pos)
            || self.fluid_waiting.contains(&pos)
        {
            return;
        }
        let key = chunk_key(pos);
        let loaded = self.chunks.contains_key(&key);
        if loaded && self.fluid_queue.len() < MAX_FLUID_QUEUE {
            self.fluid_queued.insert(pos);
            self.fluid_queue.push_back(pos);
        } else {
            self.fluid_waiting.insert(pos);
            self.fluid_deferred.entry(key).or_default().push_back(pos);
            if loaded && self.fluid_resuming.insert(key) {
                self.fluid_resume.push_back(key);
            }
        }
    }

    /// Fixed voxel levels, gravity and local relaxation; no whole-world scan per tick.
    /// The budget bounds processed cells, while unloaded chunks suspend their updates.
    // ponytail: eight voxel levels; a volumetric solver is only needed for conserved volume/pressure.
    pub fn tick_fluids(&mut self, budget: usize) -> usize {
        let mut processed = 0;
        for _ in 0..budget {
            if self.fluid_queue.len() < MAX_FLUID_QUEUE {
                if let Some(key) = self.fluid_resume.pop_front() {
                    self.fluid_resuming.remove(&key);
                    if self.chunks.contains_key(&key) {
                        let pos = self
                            .fluid_deferred
                            .get_mut(&key)
                            .and_then(VecDeque::pop_front);
                        if let Some(pos) = pos {
                            self.fluid_waiting.remove(&pos);
                            self.queue_fluid(pos);
                        }
                        if self
                            .fluid_deferred
                            .get(&key)
                            .is_some_and(|pending| !pending.is_empty())
                            && self.fluid_resuming.insert(key)
                        {
                            self.fluid_resume.push_back(key);
                        }
                    }
                }
            }
            let Some(pos) = self.fluid_queue.pop_front() else {
                if self.fluid_resume.is_empty() {
                    break;
                }
                continue;
            };
            self.fluid_queued.remove(&pos);
            processed += 1;
            if self.chunks.contains_key(&chunk_key(pos)) {
                self.update_fluid(pos);
            } else {
                self.queue_fluid(pos);
            }
        }
        processed
    }

    fn update_fluid(&mut self, pos: Pos) {
        let current = self.get(pos);
        if current == Block::Lava
            && NEIGHBORS
                .into_iter()
                .any(|d| self.get(add(pos, d)) == Block::Water)
        {
            let stone = if self.is_fluid_source(pos) {
                Block::Stone
            } else {
                Block::Cobble
            };
            self.write_cell(pos, stone, None);
            return;
        }
        if current == Block::Water {
            for d in NEIGHBORS {
                let other = add(pos, d);
                if self.chunks.contains_key(&chunk_key(other)) && self.get(other) == Block::Lava {
                    let stone = if self.is_fluid_source(other) {
                        Block::Stone
                    } else {
                        Block::Cobble
                    };
                    self.write_cell(other, stone, None);
                }
            }
        }
        if current.solid() || self.is_fluid_source(pos) {
            return;
        }

        let above = [pos[0], pos[1] + 1, pos[2]];
        let mut desired = self.fluid_state(above).map(|_| {
            (
                self.get(above),
                Flow {
                    level: 8,
                    falling: true,
                },
            )
        });
        if desired.is_none() {
            for d in SIDES {
                let neighbor = add(pos, d);
                let Some(flow) = self.fluid_state(neighbor) else {
                    continue;
                };
                let block = self.get(neighbor);
                let below = [neighbor[0], neighbor[1] - 1, neighbor[2]];
                // A waterfall feeds sideways only where it lands or meets settled liquid.
                let supported = self.get(below).solid()
                    || (self.get(below) == block
                        && self.fluid_state(below).is_some_and(|f| !f.falling));
                let level = flow
                    .level
                    .saturating_sub(if block == Block::Lava { 2 } else { 1 });
                if supported && level > 0 && desired.is_none_or(|(_, best)| level > best.level) {
                    desired = Some((
                        block,
                        Flow {
                            level,
                            falling: false,
                        },
                    ));
                }
            }
        }
        if let Some((block, flow)) = desired {
            self.write_cell(pos, block, Some(flow));
        } else if current.fluid() {
            self.write_cell(pos, Block::Air, None);
        }
    }

    fn activate_fluids(&mut self, key: ChunkKey) {
        if let Some(pending) = self.fluid_deferred.remove(&key) {
            for pos in pending {
                self.fluid_waiting.remove(&pos);
                self.queue_fluid(pos);
            }
        }
        let mut exposed = Vec::new();
        let chunk = &self.chunks[&key];
        for y in 1..(chunk.blocks.len() / 256) as i32 {
            for z in 0..CHUNK {
                for x in 0..CHUNK {
                    let block = chunk.blocks[index(x, y, z)];
                    if !block.fluid() {
                        continue;
                    }
                    let pos = [key[0] * CHUNK + x, y, key[1] * CHUNK + z];
                    // Lake interiors and their air above need no work. Activate only outlets,
                    // mixing boundaries, and restored flowing cells.
                    let outlet = [[0, -1, 0], [1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1]]
                        .into_iter()
                        .any(|d| {
                            let next = self.get(add(pos, d));
                            matches!(next, Block::Air | Block::Torch)
                                || (next.fluid() && next != block)
                        });
                    if outlet || self.flows.contains_key(&pos) {
                        exposed.push(pos);
                    }
                }
            }
        }
        for pos in exposed {
            self.queue_fluid_neighbors(pos);
        }
    }

    /// Load a bounded number of chunks, starting with the player's nearest chunks.
    pub fn stream(&mut self, center: ChunkKey, radius: i32, budget: usize) -> usize {
        let radius = radius.max(0);
        self.chunks.retain(|key, _| {
            (key[0] - center[0]).abs() <= radius + 1 && (key[1] - center[1]).abs() <= radius + 1
        });
        let mut missing = Vec::new();
        for z in -radius..=radius {
            for x in -radius..=radius {
                let key = [center[0] + x, center[1] + z];
                if !self.chunks.contains_key(&key) && self.contains(key[0] * CHUNK, key[1] * CHUNK)
                {
                    missing.push((x * x + z * z, key));
                }
            }
        }
        missing.sort_unstable();
        let count = missing.len().min(budget);
        for (_, key) in missing.into_iter().take(count) {
            let chunk = self.generate_chunk(key);
            for (i, block) in chunk.blocks.iter().enumerate() {
                if block.crop_stage().is_some() {
                    self.crops.insert([
                        key[0] * CHUNK + (i % 16) as i32,
                        (i / 256) as i32,
                        key[1] * CHUNK + ((i / 16) % 16) as i32,
                    ]);
                }
            }
            self.chunks.insert(key, chunk);
            self.activate_fluids(key);
        }
        count
    }

    pub fn village_at(&self, x: i32, z: i32) -> Option<crate::villages::Village> {
        if self.generation == 4 {
            return None;
        }
        if self.generation == 1 {
            return self.legacy_village_at(x, z);
        }
        use crate::villages::{Terrain, Village, BUILDINGS, RADIUS};
        let region_size = self.village_region_size();
        let region = [x.div_euclid(region_size), z.div_euclid(region_size)];
        if let Some(v) = self.villages.borrow().get(&region) {
            return *v;
        }
        let n = hash(self.seed ^ 0x0711_1a9e, region[0], 0, region[1]);
        let mut found = None;
        let attempts = if self.generation >= 3 && n % 100 >= 55 {
            0
        } else {
            5
        };
        let span = if self.generation == 2 { 78 } else { 256 };
        let margin = if self.generation == 2 {
            RADIUS + 2
        } else {
            192
        };
        // Sparse candidate regions; local foundations never reshape the whole town.
        for attempt in 0..attempts {
            let a = hash(n, attempt, 0, 17);
            let cx = region[0] * region_size + margin + ((a >> 12) % span as u64) as i32;
            let cz = region[1] * region_size + margin + ((a >> 24) % span as u64) as i32;
            let center = self.column(cx, cz);
            if !matches!(center.biome, Biome::Plains | Biome::Desert)
                || center.height <= SEA + 3
                || center.height > HEIGHT - 22
            {
                continue;
            }
            let suitable = [-RADIUS, 0, RADIUS].into_iter().all(|dz| {
                [-RADIUS, 0, RADIUS].into_iter().all(|dx| {
                    let c = self.column(cx + dx, cz + dz);
                    c.biome == center.biome
                        && c.height > SEA + 2
                        && (c.height - center.height).abs()
                            <= if self.generation == 2 { 8 } else { 14 }
                })
            }) && (-RADIUS..=RADIUS).all(|dz| {
                (-RADIUS..=RADIUS).all(|dx| {
                    let c = self.column(cx + dx, cz + dz);
                    c.biome == center.biome
                        && c.height > SEA + 2
                        && (c.height - center.height).abs()
                            <= if self.generation == 2 { 10 } else { 14 }
                        && (self.generation == 2 || c.height <= HEIGHT - 22)
                })
            });
            if suitable {
                let mut terrain = None;
                let mut ground = center.height;
                if self.generation >= 3 {
                    let plot = |x: i32, z: i32, w: i32, d: i32| {
                        let (mut low, mut high) = (HEIGHT, 0);
                        for dz in z..z + d {
                            for dx in x..x + w {
                                let h = self.column(cx + dx, cz + dz).height;
                                low = low.min(h);
                                high = high.max(h);
                            }
                        }
                        (low, high)
                    };
                    let mut levels = Terrain {
                        floors: [0; 14],
                        fields: [0; 2],
                    };
                    let mut gentle = true;
                    for (i, b) in BUILDINGS.iter().enumerate() {
                        let (low, high) = plot(b.x - 1, b.z - 2, b.w + 2, b.d + 4);
                        levels.floors[i] = high;
                        gentle &= high - low <= 4;
                    }
                    for (i, (x, z)) in [(-19, 29), (28, 29)].into_iter().enumerate() {
                        let (low, high) = plot(x, z, 15, 15);
                        levels.fields[i] = high;
                        gentle &= high - low <= 3;
                    }
                    if !gentle {
                        continue;
                    }
                    ground = plot(-2, -2, 5, 5).1;
                    terrain = Some(levels);
                }
                found = Some(Village {
                    region,
                    center: [cx, ground, cz],
                    desert: center.biome == Biome::Desert,
                    legacy: false,
                    terrain,
                });
                break;
            }
        }
        let mut cache = self.villages.borrow_mut();
        if cache.len() > 4096 {
            cache.clear();
        }
        cache.insert(region, found);
        found
    }

    fn legacy_village_at(&self, x: i32, z: i32) -> Option<crate::villages::Village> {
        use crate::villages::Village;
        const REGION: i32 = 160;
        let region = [x.div_euclid(REGION), z.div_euclid(REGION)];
        if let Some(v) = self.villages.borrow().get(&region) {
            return *v;
        }
        let n = hash(self.seed ^ 0x0711_1a9e, region[0], 0, region[1]);
        let cx = region[0] * REGION + 50 + ((n >> 12) % 60) as i32;
        let cz = region[1] * REGION + 50 + ((n >> 24) % 60) as i32;
        let center = self.column(cx, cz);
        let suitable = n % 100 < 72
            && matches!(center.biome, Biome::Plains | Biome::Desert)
            && [(-24, -24), (-24, 24), (24, -24), (24, 24)]
                .into_iter()
                .all(|(dx, dz)| {
                    let c = self.column(cx + dx, cz + dz);
                    matches!(
                        (center.biome, c.biome),
                        (Biome::Plains, Biome::Plains) | (Biome::Desert, Biome::Desert)
                    ) && c.height > SEA + 2
                        && (c.height - center.height).abs() <= 6
                })
            && center.height > SEA + 2
            && (-24..=24).all(|z| {
                (-24..=24).all(|x| {
                    let c = self.column(cx + x, cz + z);
                    matches!(
                        (center.biome, c.biome),
                        (Biome::Plains, Biome::Plains) | (Biome::Desert, Biome::Desert)
                    ) && c.height > SEA + 2
                })
            });
        let v = suitable.then_some(Village {
            region,
            legacy: true,
            terrain: None,
            center: [cx, center.height, cz],
            desert: matches!(center.biome, Biome::Desert),
        });
        let mut cache = self.villages.borrow_mut();
        if cache.len() > 4096 {
            cache.clear();
        }
        cache.insert(region, v);
        v
    }

    pub fn nearby_villages(&self, center: [i32; 2], radius: i32) -> Vec<crate::villages::Village> {
        let region_size = self.village_region_size();
        let mut found = Vec::new();
        for z in (center[1] - radius).div_euclid(region_size)
            ..=(center[1] + radius).div_euclid(region_size)
        {
            for x in (center[0] - radius).div_euclid(region_size)
                ..=(center[0] + radius).div_euclid(region_size)
            {
                if let Some(v) = self.village_at(x * region_size, z * region_size) {
                    found.push(v);
                }
            }
        }
        found
    }

    pub fn height_at(&self, x: i32, z: i32) -> i32 {
        if self.generation == 4 {
            return (0..self.height())
                .rev()
                .find(|&y| self.get([x, y, z]).solid())
                .unwrap_or(0);
        }
        if let Some(v) = self.village_at(x, z) {
            if (x - v.center[0]).abs() <= v.radius() && (z - v.center[2]).abs() <= v.radius() {
                return v.surface_height(x, z, self.column(x, z).height);
            }
        }
        self.column(x, z).height
    }

    pub fn village_region_size(&self) -> i32 {
        match self.generation {
            1 => 160,
            2 => crate::villages::REGION,
            _ => crate::villages::TERRAIN_REGION,
        }
    }

    pub fn nearest_village(
        &self,
        center: [i32; 2],
        radius: i32,
    ) -> Option<crate::villages::Village> {
        self.nearby_villages(center, radius)
            .into_iter()
            .filter(|v| {
                (v.center[0] - center[0]).pow(2) + (v.center[2] - center[1]).pow(2)
                    <= radius * radius
            })
            .min_by_key(|v| (v.center[0] - center[0]).pow(2) + (v.center[2] - center[1]).pow(2))
    }

    pub fn biome_at(&self, x: i32, z: i32) -> &'static str {
        if self.generation == 4 {
            return "Apocalypse City";
        }
        match self.column(x, z).biome {
            Biome::Forest => "Forêt",
            Biome::Plains => "Prairie",
            Biome::Desert => "Désert",
            Biome::Snow => "Toundra",
        }
    }

    pub fn edits(&self) -> Vec<(Pos, Block)> {
        let mut changes: Vec<_> = self
            .edits
            .iter()
            .map(|(&pos, &block)| (pos, block))
            .collect();
        changes.sort_unstable_by_key(|(pos, _)| *pos);
        changes
    }

    pub fn restore_edits(&mut self, changes: Vec<(Pos, Block)>) {
        self.chunks.clear();
        self.edits.clear();
        self.flows.clear();
        self.fluid_queue.clear();
        self.fluid_queued.clear();
        self.fluid_deferred.clear();
        self.fluid_waiting.clear();
        self.fluid_resume.clear();
        self.fluid_resuming.clear();
        self.crops.clear();
        self.crop_cursor = 0;
        for (pos, block) in changes {
            self.set(pos, block);
        }
    }

    /// Player feet, above dry ground with room for a standing player.
    pub fn spawn(&self) -> [f32; 3] {
        if self.generation == 4 {
            return crate::city::get().expect("City map").metadata.spawns[self.city_spawn as usize]
                .position;
        }
        if self.generation >= 2 {
            for radius in [384, 768, 1536, 3072] {
                if let Some(v) = self.nearest_village([0, 0], radius) {
                    let [x, _, z] = v.center;
                    let z = z + crate::villages::RADIUS - 2;
                    let h = self.height_at(x, z);
                    return [x as f32 + 0.5, h as f32 + 1.05, z as f32 + 0.5];
                }
            }
        }
        for radius in 0i32..=32 {
            for z in -radius..=radius {
                for x in -radius..=radius {
                    if x.abs().max(z.abs()) != radius {
                        continue;
                    }
                    let x = x * 4;
                    let z = z * 4;
                    let h = self.height_at(x, z);
                    if h > SEA && (1..=3).all(|dy| self.get([x, h + dy, z]) == Block::Air) {
                        return [x as f32 + 0.5, h as f32 + 1.05, z as f32 + 0.5];
                    }
                }
            }
        }
        [0.5, (self.height_at(0, 0).max(SEA) + 10) as f32, 0.5]
    }

    /// Find nearby support on the same storey, including half slabs and stairs.
    pub fn walkable(&self, x: i32, z: i32, near_y: f32, height: f32) -> Option<f32> {
        if !near_y.is_finite() || !height.is_finite() || !self.contains(x, z) {
            return None;
        }
        let mut tops = Vec::new();
        for y in
            (near_y.floor() as i32 - 3).max(0)..=(near_y.ceil() as i32 + 1).min(self.height() - 1)
        {
            for b in self.get([x, y, z]).boxes() {
                if b[0] < 0.8 && b[3] > 0.2 && b[2] < 0.8 && b[5] > 0.2 {
                    let top = y as f32 + b[4];
                    if top >= near_y - 2.05 && top <= near_y + 1.05 {
                        tops.push(top + 0.001);
                    }
                }
            }
        }
        tops.sort_by(|a, b| (a - near_y).abs().total_cmp(&(b - near_y).abs()));
        tops.into_iter().find(|&feet| {
            (feet.floor() as i32 - 1..=(feet + height).floor() as i32).all(|y| {
                let block = self.get([x, y, z]);
                !block.fluid()
                    && block.boxes().iter().all(|b| {
                        b[0] >= 0.8
                            || b[3] <= 0.2
                            || b[2] >= 0.8
                            || b[5] <= 0.2
                            || y as f32 + b[4] <= feet
                            || y as f32 + b[1] >= feet + height
                    })
            })
        })
    }

    fn column(&self, x: i32, z: i32) -> Column {
        if self.generation == 1 {
            return self.legacy_column(x, z);
        }
        let (xf, zf) = (x as f64, z as f64);
        // Temperature and humidity vary over kilometres, never with terrain altitude.
        let climate = noise2(self.seed ^ 0x7a19, xf / 1100., zf / 1100.);
        let moisture = noise2(self.seed ^ 0x192c, xf / 900., zf / 900.);
        let biome = if climate < -0.35 {
            Biome::Snow
        } else if climate > 0.35 && moisture < 0.45 {
            Biome::Desert
        } else if moisture > 0.10 {
            Biome::Forest
        } else {
            Biome::Plains
        };
        let continental = noise2(self.seed, xf / 450., zf / 450.);
        let hills = noise2(self.seed ^ 0x53a9, xf / 130., zf / 130.);
        let detail = noise2(self.seed ^ 0xba21, xf / 35., zf / 35.);
        let mass = ((noise2(self.seed ^ 0x4d07, xf / 620., zf / 620.) - 0.08) / 0.48).clamp(0., 1.);
        let ridge = 1. - noise2(self.seed ^ 0x71d9, xf / 310., zf / 310.).abs();
        let mountains = mass * ridge.powi(3) * 35.;
        let height =
            (29. + continental * 13. + hills * 4. + detail * 1.5 + mountains).round() as i32;
        Column {
            height: height.clamp(8, HEIGHT - 12),
            biome,
        }
    }

    fn legacy_column(&self, x: i32, z: i32) -> Column {
        let xf = x as f64;
        let zf = z as f64;
        let continental = noise2(self.seed, xf / 210.0, zf / 210.0);
        let hills = noise2(self.seed ^ 0x53a9, xf / 58.0, zf / 58.0);
        let detail = noise2(self.seed ^ 0xba21, xf / 19.0, zf / 19.0);
        let height = (27.0 + continental * 14.0 + hills * 6.0 + detail * 2.0).round() as i32;
        let climate = noise2(self.seed ^ 0x7a19, xf / 135.0, zf / 135.0);
        let moisture = noise2(self.seed ^ 0x192c, xf / 110.0, zf / 110.0);
        let biome = if climate < -0.35 || height >= 44 {
            Biome::Snow
        } else if climate > 0.2 && moisture < 0.15 {
            Biome::Desert
        } else if moisture > 0.02 {
            Biome::Forest
        } else {
            Biome::Plains
        };
        Column {
            height: height.clamp(8, HEIGHT - 12),
            biome,
        }
    }

    pub fn cave_entrance(&self, cell_x: i32, cell_z: i32) -> Option<Pos> {
        if self.generation == 1 {
            return None;
        }
        let key = [cell_x, cell_z];
        if let Some(entrance) = self.entrances.borrow().get(&key) {
            return *entrance;
        }
        let n = hash(self.seed ^ 0xca7e_9a7e, cell_x, 0, cell_z);
        let x = cell_x * 96 + 20 + ((n >> 12) % 55) as i32;
        let z = cell_z * 96 + 20 + ((n >> 24) % 55) as i32;
        let c = self.column(x, z);
        let entrance = (n % 100 < 62
            && c.height > SEA + 6
            && c.height <= HEIGHT - 12
            && !self.nearby_villages([x, z], 32).into_iter().any(|v| {
                (x - v.center[0]).abs() <= v.radius() + 28
                    && (z - v.center[2]).abs() <= v.radius() + 28
            }))
        .then_some([x, c.height, z]);
        let mut cache = self.entrances.borrow_mut();
        if cache.len() > 4096 {
            cache.clear();
        }
        cache.insert(key, entrance);
        entrance
    }

    pub fn cave_direction(&self, [x, _, z]: Pos) -> [i32; 2] {
        let n = hash(
            self.seed ^ 0xca7e_9a7e,
            x.div_euclid(96),
            0,
            z.div_euclid(96),
        );
        let sign = if n & 2 == 0 { 1 } else { -1 };
        if n & 1 == 0 {
            [sign, 0]
        } else {
            [0, sign]
        }
    }

    fn entrance_block(&self, [x, y, z]: Pos) -> Option<Block> {
        if y < 9 {
            return None;
        }
        for cz in (z - 24).div_euclid(96)..=(z + 24).div_euclid(96) {
            for cx in (x - 24).div_euclid(96)..=(x + 24).div_euclid(96) {
                let Some([ex, ey, ez]) = self.cave_entrance(cx, cz) else {
                    continue;
                };
                let n = hash(self.seed ^ 0xca7e_9a7e, cx, 0, cz);
                let (u, v) = if n & 1 == 0 {
                    (x - ex, z - ez)
                } else {
                    (z - ez, x - ex)
                };
                let u = if n & 2 == 0 { u } else { -u };
                if (-4..=24).contains(&u) && v.abs() <= 3 {
                    let floor = ey - 1 - (u.max(0) / 2);
                    let arch = if v.abs() == 3 { 2 } else { 4 };
                    if y == floor {
                        return Some(Block::Stone);
                    }
                    if y > floor && y <= floor + arch {
                        return Some(Block::Air);
                    }
                }
                // The entrance tunnel opens into a walkable chamber below the hillside.
                let (u, v) = if n & 1 == 0 {
                    (x - ex, z - ez)
                } else {
                    (z - ez, x - ex)
                };
                let u = if n & 2 == 0 { u } else { -u };
                if (u - 21).pow(2) + v.pow(2) <= 36 && (ey - 13..=ey - 7).contains(&y) {
                    return Some(Block::Air);
                }
                if (u - 21).pow(2) + v.pow(2) <= 36 && y == ey - 14 {
                    return Some(Block::Stone);
                }
            }
        }
        None
    }

    fn terrain(&self, pos: Pos, column: Column) -> Block {
        let [x, y, z] = pos;
        if y <= 0 {
            return Block::Bedrock;
        }
        if y >= HEIGHT {
            return Block::Air;
        }
        if self.generation >= 2 {
            if let Some(block) = self.entrance_block(pos) {
                return block;
            }
        }
        if y > column.height {
            return if y <= SEA { Block::Water } else { Block::Air };
        }
        if y == column.height {
            return if column.height <= SEA + 1 || matches!(column.biome, Biome::Desert) {
                Block::Sand
            } else if matches!(column.biome, Biome::Snow) {
                Block::Snow
            } else if self.generation >= 2 && column.height >= 52 {
                Block::Stone
            } else {
                Block::Grass
            };
        }
        if y > column.height - 4 {
            return if self.generation >= 2 && column.height >= 52 {
                if matches!(column.biome, Biome::Desert) {
                    Block::Sandstone
                } else {
                    Block::Stone
                }
            } else if matches!(column.biome, Biome::Desert) || column.height <= SEA + 1 {
                Block::Sand
            } else {
                Block::Dirt
            };
        }
        if y >= 4
            && y < column.height - 4
            && noise3(
                self.seed ^ 0xca7e,
                x as f64 / 14.0,
                y as f64 / 9.0,
                z as f64 / 14.0,
            ) > 0.39
        {
            return if y <= 7 { Block::Lava } else { Block::Air };
        }
        let ore = hash(self.seed ^ 0x0ae5, x, y, z) % 1000;
        if y < 25 && ore < 27 {
            Block::IronOre
        } else if ore < 75 {
            Block::CoalOre
        } else {
            Block::Stone
        }
    }

    fn tree(&self, cell_x: i32, cell_z: i32) -> Option<Tree> {
        let n = hash(self.seed ^ 0x7ee5, cell_x, 0, cell_z);
        let x = cell_x * 8 + 2 + ((n >> 8) % 4) as i32;
        let z = cell_z * 8 + 2 + ((n >> 16) % 4) as i32;
        let column = self.column(x, z);
        let chance = match column.biome {
            Biome::Forest => 74,
            Biome::Plains => 9,
            _ => 0,
        };
        if column.height <= SEA + 1
            || n % 100 >= chance
            || self.generation >= 2 && column.height >= 52
            || self.generation >= 2
                && self.entrance_block([x, column.height, z]) == Some(Block::Air)
        {
            return None;
        }
        Some(Tree {
            x,
            z,
            ground: column.height,
            height: 4 + ((n >> 24) % 3) as i32,
        })
    }

    fn generated(&self, pos: Pos) -> Block {
        if self.generation == 4 {
            if !self.contains(pos[0], pos[2]) {
                return Block::Bedrock;
            }
            return self.city_state(pos).map_or(Block::Air, |i| {
                crate::city::get().expect("City map").block(i)
            });
        }
        let column = self.column(pos[0], pos[2]);
        if let Some(block) = self
            .village_at(pos[0], pos[2])
            .and_then(|v| v.block_at(pos, column.height))
        {
            return block;
        }
        let terrain = self.terrain(pos, column);
        if terrain != Block::Air || pos[1] <= column.height || pos[1] > column.height + 12 {
            return terrain;
        }
        if self.generation >= 2 && self.entrance_block(pos).is_some() {
            return terrain;
        }
        let mut leaves = false;
        for cz in (pos[2] - 2).div_euclid(8)..=(pos[2] + 2).div_euclid(8) {
            for cx in (pos[0] - 2).div_euclid(8)..=(pos[0] + 2).div_euclid(8) {
                if let Some(tree) = self.tree(cx, cz) {
                    match tree.block(pos) {
                        Block::Wood => return Block::Wood,
                        Block::Leaves => leaves = true,
                        _ => {}
                    }
                }
            }
        }
        if leaves {
            Block::Leaves
        } else {
            Block::Air
        }
    }

    fn generate_chunk(&self, key: ChunkKey) -> Chunk {
        if self.generation == 4 {
            let city = crate::city::get().expect("City map");
            let mut blocks: Vec<Block> = city
                .chunk(key)
                .unwrap_or_default()
                .into_iter()
                .map(|i| city.block(i))
                .collect();
            for (&[x, y, z], &block) in &self.edits {
                if x.div_euclid(CHUNK) == key[0] && z.div_euclid(CHUNK) == key[1] {
                    blocks.resize(blocks.len().max((y + 1) as usize * 256), Block::Air);
                    blocks[index(x.rem_euclid(CHUNK), y, z.rem_euclid(CHUNK))] = block;
                }
            }
            return Chunk {
                blocks,
                dirty: true,
            };
        }
        let mut blocks = vec![Block::Air; (CHUNK * CHUNK * HEIGHT) as usize];
        let mut heights = [0; (CHUNK * CHUNK) as usize];
        let ox = key[0] * CHUNK;
        let oz = key[1] * CHUNK;
        for z in 0..CHUNK {
            for x in 0..CHUNK {
                let column = self.column(ox + x, oz + z);
                heights[(x + CHUNK * z) as usize] = column.height;
                // A cave's supported entrance can sit a few blocks above a sloping column.
                let mut top = column.height.max(SEA);
                if self.generation >= 2 {
                    for cz in (oz + z - 28).div_euclid(96)..=(oz + z + 28).div_euclid(96) {
                        for cx in (ox + x - 28).div_euclid(96)..=(ox + x + 28).div_euclid(96) {
                            if let Some([ex, ey, ez]) = self.cave_entrance(cx, cz) {
                                if (ox + x - ex).abs() <= 28 && (oz + z - ez).abs() <= 28 {
                                    top = top.max(ey + 4);
                                }
                            }
                        }
                    }
                }
                for y in 0..=top.min(HEIGHT - 1) {
                    blocks[index(x, y, z)] = self.terrain([ox + x, y, oz + z], column);
                }
            }
        }
        // Cell roots are evaluated once; leaves never overwrite terrain or trunks.
        for cz in (oz - 2).div_euclid(8)..=(oz + CHUNK + 1).div_euclid(8) {
            for cx in (ox - 2).div_euclid(8)..=(ox + CHUNK + 1).div_euclid(8) {
                if let Some(tree) = self.tree(cx, cz) {
                    for z in (tree.z - 2).max(oz)..=(tree.z + 2).min(oz + CHUNK - 1) {
                        for x in (tree.x - 2).max(ox)..=(tree.x + 2).min(ox + CHUNK - 1) {
                            for y in tree.ground + 1..=tree.ground + tree.height + 1 {
                                if y >= HEIGHT || y <= heights[(x - ox + CHUNK * (z - oz)) as usize]
                                {
                                    continue;
                                }
                                let b = tree.block([x, y, z]);
                                if b == Block::Air {
                                    continue;
                                }
                                if self.generation >= 2 && self.entrance_block([x, y, z]).is_some()
                                {
                                    continue;
                                }
                                let dst = &mut blocks[index(x - ox, y, z - oz)];
                                if *dst == Block::Air || (*dst == Block::Leaves && b == Block::Wood)
                                {
                                    *dst = b;
                                }
                            }
                        }
                    }
                }
            }
        }
        for village in self.nearby_villages([ox + 8, oz + 8], crate::villages::RADIUS + 16) {
            for z in oz..oz + CHUNK {
                for x in ox..ox + CHUNK {
                    let ground = heights[(x - ox + CHUNK * (z - oz)) as usize];
                    let bottom = if village.terrain.is_some() {
                        ground
                    } else {
                        village.center[1] - village.foundation_depth()
                    };
                    for y in bottom.max(1)..=HEIGHT - 1 {
                        if let Some(block) = village.block_at([x, y, z], ground) {
                            blocks[index(x - ox, y, z - oz)] = block;
                        }
                    }
                }
            }
        }
        for (&[x, y, z], &block) in &self.edits {
            if x.div_euclid(CHUNK) == key[0] && z.div_euclid(CHUNK) == key[1] {
                blocks[index(x.rem_euclid(CHUNK), y, z.rem_euclid(CHUNK))] = block;
            }
        }
        Chunk {
            blocks,
            dirty: true,
        }
    }
}

impl Tree {
    fn block(self, [x, y, z]: Pos) -> Block {
        let top = self.ground + self.height;
        if x == self.x && z == self.z && y > self.ground && y <= top {
            return Block::Wood;
        }
        let dx = (x - self.x).abs();
        let dz = (z - self.z).abs();
        let radius = if y >= top - 2 && y < top {
            2
        } else if y == top || y == top + 1 {
            1
        } else {
            return Block::Air;
        };
        if dx <= radius && dz <= radius && !(radius == 2 && dx == 2 && dz == 2) {
            Block::Leaves
        } else {
            Block::Air
        }
    }
}

fn index(x: i32, y: i32, z: i32) -> usize {
    (x + CHUNK * (z + CHUNK * y)) as usize
}

fn chunk_key(pos: Pos) -> ChunkKey {
    [pos[0].div_euclid(CHUNK), pos[2].div_euclid(CHUNK)]
}

fn add(pos: Pos, offset: Pos) -> Pos {
    [pos[0] + offset[0], pos[1] + offset[1], pos[2] + offset[2]]
}

fn hash(seed: u64, x: i32, y: i32, z: i32) -> u64 {
    let mut n = seed
        ^ (x as u64).wrapping_mul(0x9e3779b185ebca87)
        ^ (y as u64).wrapping_mul(0xc2b2ae3d27d4eb4f)
        ^ (z as u64).wrapping_mul(0x165667b19e3779f9);
    n ^= n >> 30;
    n = n.wrapping_mul(0xbf58476d1ce4e5b9);
    n ^= n >> 27;
    n = n.wrapping_mul(0x94d049bb133111eb);
    n ^ (n >> 31)
}

fn value(seed: u64, x: i32, y: i32, z: i32) -> f64 {
    (hash(seed, x, y, z) >> 11) as f64 / ((1u64 << 53) as f64) * 2.0 - 1.0
}

fn fade(x: f64) -> f64 {
    x * x * (3.0 - 2.0 * x)
}
fn mix(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

fn noise2(seed: u64, x: f64, z: f64) -> f64 {
    let ix = x.floor() as i32;
    let iz = z.floor() as i32;
    let tx = fade(x - ix as f64);
    let tz = fade(z - iz as f64);
    mix(
        mix(value(seed, ix, 0, iz), value(seed, ix + 1, 0, iz), tx),
        mix(
            value(seed, ix, 0, iz + 1),
            value(seed, ix + 1, 0, iz + 1),
            tx,
        ),
        tz,
    )
}

fn noise3(seed: u64, x: f64, y: f64, z: f64) -> f64 {
    let ix = x.floor() as i32;
    let iy = y.floor() as i32;
    let iz = z.floor() as i32;
    let tx = fade(x - ix as f64);
    let ty = fade(y - iy as f64);
    let tz = fade(z - iz as f64);
    let layer = |dz| {
        mix(
            mix(
                value(seed, ix, iy, iz + dz),
                value(seed, ix + 1, iy, iz + dz),
                tx,
            ),
            mix(
                value(seed, ix, iy + 1, iz + dz),
                value(seed, ix + 1, iy + 1, iz + dz),
                tx,
            ),
            ty,
        )
    };
    mix(layer(0), layer(1), tz)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn crops_need_irrigation_light_and_loaded_soil_and_survive_restore() {
        let mut world = World::new(42);
        let soil = [0, 65, 0];
        let plant = [0, 66, 0];
        world.set(soil, Block::Farmland);
        world.set(plant, Block::Wheat0);
        assert_eq!(world.grow_crops(true), 0); // unloaded
        world.stream([0, 0], 0, 1);
        assert_eq!(world.grow_crops(true), 0); // dry
        world.set([4, 65, 0], Block::Water);
        assert!(world.irrigated(soil));
        assert_eq!(world.grow_crops(true), 1);
        assert_eq!(world.get(plant), Block::Wheat1);
        assert_eq!(world.grow_crops(false), 0); // night
        world.set([0, 67, 0], Block::Stone);
        assert_eq!(world.grow_crops(true), 0); // covered
        world.set([2, 66, 0], Block::Torch);
        assert_eq!(world.grow_crops(false), 1);
        assert_eq!(world.grow_crops(false), 1);
        assert_eq!(world.get(plant), Block::Wheat3);
        assert_eq!(world.grow_crops(true), 0); // mature
        let mut restored = World::new(world.seed);
        restored.restore_edits(world.edits());
        assert_eq!(restored.crop_count(), 1);
        assert_eq!(restored.get(plant), Block::Wheat3);
        restored.set(soil, Block::Dirt);
        assert_eq!(restored.get(plant), Block::Air);
        assert_eq!(restored.crop_count(), 0);
        assert!(!Block::Wheat3.solid());
    }

    fn empty_world(radius: i32) -> World {
        let mut world = World::new(123);
        for z in -radius..=radius {
            for x in -radius..=radius {
                let mut blocks = vec![Block::Air; (CHUNK * CHUNK * HEIGHT) as usize];
                for z in 0..CHUNK {
                    for x in 0..CHUNK {
                        blocks[index(x, 0, z)] = Block::Bedrock;
                    }
                }
                world.chunks.insert(
                    [x, z],
                    Chunk {
                        blocks,
                        dirty: true,
                    },
                );
            }
        }
        world
    }

    fn platform(world: &mut World) {
        for z in -9..=9 {
            for x in -9..=9 {
                world.set([x, 60, z], Block::Stone);
            }
        }
    }

    fn settle(world: &mut World) {
        for _ in 0..1000 {
            if world.tick_fluids(512) == 0 {
                return;
            }
        }
        panic!("fluid simulation failed to settle within its bounded test budget");
    }

    #[test]
    fn fluid_gravity_surface_height_and_lateral_distance() {
        let mut world = empty_world(1);
        platform(&mut world);
        world.set([0, 64, 0], Block::Water);
        settle(&mut world);
        assert!(world.is_fluid_source([0, 64, 0]));
        for y in 61..64 {
            assert_eq!(
                world.fluid_state([0, y, 0]),
                Some(Flow {
                    level: 8,
                    falling: true
                })
            );
            assert_eq!(world.fluid_height([0, y, 0]), Some(1.0));
        }
        assert_eq!(
            world.get([1, 63, 0]),
            Block::Air,
            "gravity precedes side flow"
        );
        assert_eq!(
            world.fluid_state([7, 61, 0]),
            Some(Flow {
                level: 1,
                falling: false
            })
        );
        assert_eq!(world.get([8, 61, 0]), Block::Air);
        assert!((world.fluid_height([7, 61, 0]).unwrap() - 0.11).abs() < 0.001);
        assert_eq!(world.get([0, 60, 0]), Block::Stone);
        world.set([0, 64, 0], Block::Air);
        settle(&mut world);
        assert!(
            world.flows.is_empty(),
            "removing a waterfall source drains all of its flow"
        );

        world.set([0, 61, 0], Block::Lava);
        settle(&mut world);
        assert_eq!(
            world.fluid_state([3, 61, 0]),
            Some(Flow {
                level: 2,
                falling: false
            })
        );
        assert_eq!(world.get([4, 61, 0]), Block::Air);
    }

    #[test]
    fn fluid_obstacles_source_promotion_and_recession() {
        let mut world = empty_world(1);
        platform(&mut world);
        for x in -9..=9 {
            for z in [-1, 1] {
                world.set([x, 61, z], Block::Glass);
            }
        }
        world.set([2, 61, 0], Block::Stone);
        world.set([0, 61, 0], Block::Water);
        settle(&mut world);
        assert_eq!(world.get([1, 61, 0]), Block::Water);
        assert_eq!(world.get([2, 61, 0]), Block::Stone);
        assert_eq!(
            world.get([3, 61, 0]),
            Block::Air,
            "liquid cannot cross a sealed wall"
        );
        assert!(!world.is_fluid_source([1, 61, 0]));
        assert!(
            world.set([1, 61, 0], Block::Water),
            "placing water on flow promotes it to a source"
        );
        assert!(world.is_fluid_source([1, 61, 0]));
        world.set([0, 61, 0], Block::Air);
        settle(&mut world);
        assert_eq!(
            world.get([0, 61, 0]),
            Block::Water,
            "the second source remains effective"
        );
        world.set([1, 61, 0], Block::Air);
        settle(&mut world);
        assert!(world.flows.is_empty());
        assert_eq!(world.get([0, 61, 0]), Block::Air);
    }

    #[test]
    fn water_lava_contact_solidifies_sources_and_flow() {
        let mut world = empty_world(1);
        platform(&mut world);
        world.set([0, 61, 0], Block::Water);
        world.set([1, 61, 0], Block::Lava);
        world.write_cell(
            [-1, 61, 0],
            Block::Lava,
            Some(Flow {
                level: 6,
                falling: false,
            }),
        );
        settle(&mut world);
        assert_eq!(world.get([1, 61, 0]), Block::Stone);
        assert_eq!(world.get([-1, 61, 0]), Block::Cobble);
        assert_eq!(world.get([0, 61, 0]), Block::Water);
        assert!(!world.flows.contains_key(&[1, 61, 0]));
        assert!(!world.flows.contains_key(&[-1, 61, 0]));
    }

    #[test]
    fn flow_crosses_negative_chunk_boundaries_and_level_changes_dirty_neighbors() {
        let mut world = empty_world(1);
        platform(&mut world);
        world.set([-1, 61, -1], Block::Water);
        settle(&mut world);
        let pos = [0, 61, -1];
        assert_eq!(
            world.fluid_state(pos),
            Some(Flow {
                level: 7,
                falling: false
            })
        );
        for chunk in world.chunks.values_mut() {
            chunk.dirty = false;
        }
        assert!(world.write_cell(
            pos,
            Block::Water,
            Some(Flow {
                level: 4,
                falling: false
            })
        ));
        assert!(world.chunks[&[0, -1]].dirty);
        assert!(
            world.chunks[&[-1, -1]].dirty,
            "same-kind level changes affect neighboring mesh faces"
        );
        assert!(world.chunks[&[0, 0]].dirty);
        assert!(!world.chunks[&[1, 1]].dirty);
    }

    #[test]
    fn fluid_updates_wait_for_loading_and_metadata_survives_restore() {
        let mut world = empty_world(0);
        for z in 0..16 {
            for x in 0..25 {
                world.set([x, 60, z], Block::Stone);
            }
        }
        world.set([15, 61, 4], Block::Water);
        settle(&mut world);
        assert_eq!(
            world.get([16, 61, 4]),
            Block::Air,
            "simulation cannot write outside loaded chunks"
        );
        assert!(!world.flows.keys().any(|pos| pos[0] >= 16));
        assert!(world.fluid_deferred.contains_key(&[1, 0]));
        world.stream([1, 0], 0, 1);
        settle(&mut world);
        let flow = world.fluid_state([16, 61, 4]).unwrap();
        assert_eq!(
            flow,
            Flow {
                level: 7,
                falling: false
            }
        );
        assert!(!world.is_fluid_source([16, 61, 4]));

        let mut restored = World::new(world.seed);
        restored.restore_edits(world.edits());
        restored.restore_fluid_flows(world.fluid_flows());
        assert_eq!(restored.fluid_state([16, 61, 4]), Some(flow));
        assert!(!restored.is_fluid_source([16, 61, 4]));
        assert!(restored.is_fluid_source([15, 61, 4]));
        restored.stream([1, 0], 0, 1);
        assert_eq!(restored.fluid_state([16, 61, 4]), Some(flow));
        restored.stream([50, 50], 0, 1);
        assert_eq!(
            restored.fluid_state([16, 61, 4]),
            Some(flow),
            "chunk eviction preserves flow levels"
        );
    }

    #[test]
    fn fluid_budget_queue_cap_and_idle_lakes() {
        let mut world = empty_world(1);
        for chunk in world.chunks.values_mut() {
            for y in 1..=SEA {
                for z in 0..CHUNK {
                    for x in 0..CHUNK {
                        chunk.blocks[index(x, y, z)] = Block::Water;
                    }
                }
            }
        }
        world.activate_fluids([0, 0]);
        assert!(
            world.fluid_queue.is_empty(),
            "a lake's interior and top surface stay idle"
        );
        assert_eq!(world.tick_fluids(1024), 0);
        for y in 40..60 {
            for z in -16..32 {
                for x in -16..32 {
                    world.queue_fluid([x, y, z]);
                }
            }
        }
        assert_eq!(world.fluid_queue.len(), MAX_FLUID_QUEUE);
        assert_eq!(world.fluid_queued.len(), MAX_FLUID_QUEUE);
        assert!(world
            .fluid_deferred
            .values()
            .any(|pending| !pending.is_empty()));
        assert_eq!(world.tick_fluids(0), 0);
        assert_eq!(world.tick_fluids(17), 17);
        assert!(world.fluid_queue.len() <= MAX_FLUID_QUEUE);
        assert_eq!(world.chunks.len(), 9);
        assert_eq!(world.tick_fluids(MAX_FLUID_QUEUE * 2), 48 * 48 * 20 - 17);
        assert!(world.fluid_queue.is_empty());
        assert!(
            world.fluid_waiting.is_empty(),
            "queue overflow is resumed without dropped updates"
        );
        assert!(world.fluid_deferred.values().all(VecDeque::is_empty));
    }

    #[test]
    fn loaded_chunks_match_uncached_generation_and_seed() {
        let plain = World::new(731);
        let mut loaded = World::new(731);
        assert_eq!(loaded.stream([-1, 0], 1, 9), 9);
        for z in -16..32 {
            for x in -32..16 {
                for y in [0, 5, 13, 22, 29, 36, 43, 50, 79] {
                    assert_eq!(
                        loaded.get([x, y, z]),
                        plain.get([x, y, z]),
                        "at {x},{y},{z}"
                    );
                }
            }
        }
        let other = World::new(732);
        assert!((-100..100).any(|x| plain.height_at(x, 21) != other.height_at(x, 21)));
    }

    #[test]
    fn negative_coordinates_and_neighbor_dirty_flags() {
        let mut world = World::new(91);
        world.stream([0, 0], 1, 9);
        for chunk in world.chunks.values_mut() {
            chunk.dirty = false;
        }
        assert!(world.set([-1, 60, 0], Block::Brick));
        assert_eq!(
            world.chunks[&[-1, 0]].blocks[index(15, 60, 0)],
            Block::Brick
        );
        assert_eq!(world.get([-1, 60, 0]), Block::Brick);
        assert!(world.chunks[&[-1, 0]].dirty);
        assert!(world.chunks[&[0, 0]].dirty);
        assert!(world.chunks[&[-1, -1]].dirty);
        assert!(!world.chunks[&[1, 1]].dirty);
    }

    #[test]
    fn changes_survive_eviction_restore_and_reversion() {
        let mut world = World::new(37);
        let pos = [-17, 65, -1];
        assert!(world.set(pos, Block::Glass));
        world.stream([-2, -1], 0, 1);
        assert_eq!(world.get(pos), Block::Glass);
        world.stream([20, 20], 0, 1);
        assert_eq!(world.get(pos), Block::Glass);
        let mut restored = World::new(37);
        restored.restore_edits(world.edits());
        restored.stream([-2, -1], 0, 1);
        assert_eq!(restored.get(pos), Block::Glass);
        assert!(restored.set(pos, Block::Air));
        assert!(restored.edits().is_empty());
        assert!(!restored.set(pos, Block::Air));
    }

    #[test]
    fn bedrock_sky_budget_and_spawn() {
        let mut world = World::new(123);
        assert_eq!(world.get([3, -40, -2]), Block::Bedrock);
        assert_eq!(world.get([3, 0, -2]), Block::Bedrock);
        assert_eq!(world.get([3, HEIGHT, -2]), Block::Air);
        assert!(!world.set([3, 0, -2], Block::Air));
        assert!(!world.set([3, HEIGHT, -2], Block::Stone));
        assert_eq!(world.stream([0, 0], 2, 1), 1);
        assert!(world.chunks.contains_key(&[0, 0]));
        let spawn = world.spawn();
        let [x, y, z] = spawn.map(|v| v.floor() as i32);
        assert_eq!(world.get([x, y, z]), Block::Air);
        assert!(world.get([x, y - 1, z]).solid());
    }

    #[test]
    fn tree_canopies_match_across_chunk_boundaries() {
        let plain = (0..64)
            .map(World::new)
            .find(|w| w.column(0, 0).biome == Biome::Forest)
            .unwrap();
        let mut checked = 0;
        for cz in -12..12 {
            for cx in -12..12 {
                let Some(tree) = plain.tree(cx, cz) else {
                    continue;
                };
                let mut loaded = World::new(plain.seed);
                loaded.stream([tree.x.div_euclid(CHUNK), tree.z.div_euclid(CHUNK)], 1, 9);
                for z in tree.z - 2..=tree.z + 2 {
                    for x in tree.x - 2..=tree.x + 2 {
                        for y in tree.ground + 1..=tree.ground + tree.height + 1 {
                            assert_eq!(
                                plain.get([x, y, z]),
                                loaded.get([x, y, z]),
                                "canopy {x},{y},{z}"
                            );
                        }
                    }
                }
                checked += 1;
                if checked == 4 {
                    return;
                }
            }
        }
        assert!(checked > 0, "test seed must have trees");
    }
}

#[cfg(test)]
mod village_chunk_tests {
    use super::*;
    #[test]
    fn short_entrances_blend_into_streets_without_a_foundation_cliff() {
        let world = World::new(0);
        let v = world
            .village_at(
                -3 * crate::villages::TERRAIN_REGION,
                crate::villages::TERRAIN_REGION,
            )
            .unwrap();
        assert_eq!([v.center[0], v.center[2]], [-1634, 1024]);
        let mut checked = 0;
        for z in v.center[2] - v.radius()..=v.center[2] + v.radius() {
            for x in v.center[0] - v.radius()..=v.center[0] + v.radius() {
                let ground = world.column(x, z).height;
                let h = world.height_at(x, z);
                if v.block_at([x, h, z], ground) != Some(Block::Path) {
                    continue;
                }
                for (dx, dz) in [(1, 0), (0, 1)] {
                    let other_ground = world.column(x + dx, z + dz).height;
                    let other = world.height_at(x + dx, z + dz);
                    if v.block_at([x + dx, other, z + dz], other_ground) == Some(Block::Path) {
                        assert!(
                            (h - other).abs() <= (ground - other_ground).abs().max(1),
                            "road cliff at {x},{z}: {h}->{other}"
                        );
                        checked += 1;
                    }
                }
            }
        }
        assert!(checked > 1000);
        assert!(
            (world.height_at(v.center[0] - 37, v.center[2] - 23)
                - world.height_at(v.center[0] - 38, v.center[2] - 23))
            .abs()
                <= 1
        );
    }
    #[test]
    fn towns_are_sparse_separated_and_follow_natural_ground() {
        let world = World::new(64195485);
        let mut previous = World::new(world.seed);
        previous.generation = 2;
        let towns = world.nearby_villages([0, 0], 3200);
        let old = previous.nearby_villages([0, 0], 3200);
        assert!(!towns.is_empty());
        assert!(
            towns.len() * 5 < old.len(),
            "{} new versus {} old villages",
            towns.len(),
            old.len()
        );
        println!(
            "Density sample: {} new versus {} old villages",
            towns.len(),
            old.len()
        );
        let mut varied = 0;
        let mut preserved = 0;
        for (i, v) in towns.iter().enumerate() {
            for other in &towns[i + 1..] {
                assert!(
                    (v.center[0] - other.center[0])
                        .abs()
                        .max((v.center[2] - other.center[2]).abs())
                        >= 385
                );
            }
            let levels = v.terrain.unwrap();
            for (i, (x, z)) in [(-19, 29), (28, 29)].into_iter().enumerate() {
                for end in [0, 14] {
                    assert_eq!(
                        world.get([v.center[0] + x + 7, levels.fields[i], v.center[2] + z + end]),
                        Block::Farmland
                    );
                }
                assert_eq!(
                    world.get([v.center[0] + x + 7, levels.fields[i], v.center[2] + z + 7]),
                    Block::Water
                );
            }
            varied += usize::from(levels.floors.iter().max() != levels.floors.iter().min());
            for dz in (-crate::villages::RADIUS..=crate::villages::RADIUS).step_by(3) {
                for dx in (-crate::villages::RADIUS..=crate::villages::RADIUS).step_by(3) {
                    let (x, z) = (v.center[0] + dx, v.center[2] + dz);
                    let c = world.column(x, z);
                    // No village foundation replaces or fills the underground landscape.
                    assert_eq!(v.block_at([x, c.height - 1, z], c.height), None);
                    if v.block_at([x, c.height, z], c.height).is_none() {
                        assert_eq!(world.height_at(x, z), c.height);
                        assert_eq!(
                            world.get([x, c.height, z]),
                            world.terrain([x, c.height, z], c)
                        );
                        preserved += 1;
                    }
                }
            }
            for (i, b) in crate::villages::BUILDINGS.iter().enumerate() {
                let [x, z] = b.door();
                let x = v.center[0] + x;
                let z = v.center[2] + z + if b.z > 0 { -2 } else { 2 };
                assert_eq!(world.height_at(x, z), v.floor(i));
                assert_eq!(world.get([x, v.floor(i) + 1, z]), Block::Air);
                assert_eq!(world.get([x, v.floor(i) + 2, z]), Block::Air);
                assert!(world.get([x, v.floor(i), z]).solid());
            }
        }
        assert!(
            varied * 2 > towns.len(),
            "Terrain following must be visible in most towns"
        );
        assert!(
            preserved > towns.len() * 500,
            "Open areas must retain their natural surface"
        );
        println!(
            "{} varied towns; {} unchanged surface samples",
            varied, preserved
        );
    }

    #[test]
    fn village_overlay_and_edits_match_across_chunk_loads_and_negative_coordinates() {
        let mut world = World::new(64195485);
        let villages = world.nearby_villages([0, 0], 1800);
        for desert in [false, true] {
            let v = *villages
                .iter()
                .find(|v| v.desert == desert && v.center[0] < 0)
                .unwrap();
            for pos in v.chests() {
                let key = chunk_key(pos);
                let chunk = world.generate_chunk(key);
                for z in 0..CHUNK {
                    for x in 0..CHUNK {
                        for y in v.center[1] - 3..=v.center[1] + 8 {
                            let p = [key[0] * CHUNK + x, y, key[1] * CHUNK + z];
                            assert_eq!(
                                chunk.blocks[index(x, y, z)],
                                world.get(p),
                                "village mismatch {p:?}"
                            );
                        }
                    }
                }
                world.chunks.insert(key, chunk);
                assert_eq!(world.get(pos), Block::Chest);
                world.set(pos, Block::Air);
                world.chunks.clear();
                assert_eq!(world.get(pos), Block::Air);
                let rebuilt = world.generate_chunk(key);
                assert_eq!(
                    rebuilt.blocks
                        [index(pos[0].rem_euclid(CHUNK), pos[1], pos[2].rem_euclid(CHUNK))],
                    Block::Air
                );
            }
        }
    }
}

#[cfg(test)]
mod landscape_tests {
    use super::*;
    #[test]
    fn wide_climate_regions_keep_snow_and_desert_separated() {
        let (mut large, mut old) = (0, 0);
        let mut seen = [false; 4];
        for seed in 0..6 {
            let world = World::new(seed);
            for z in [-2400, 0, 2400] {
                let (mut a, mut b) = (None, None);
                for x in (-6000..=6000).step_by(20) {
                    let c = world.column(x, z);
                    seen[c.biome as usize] = true;
                    large += usize::from(a.is_some_and(|v| v != c.biome));
                    a = Some(c.biome);
                    let legacy = world.legacy_column(x, z).biome;
                    old += usize::from(b.is_some_and(|v| v != legacy));
                    b = Some(legacy);
                    if c.biome == Biome::Desert {
                        for (dx, dz) in [(-192, 0), (192, 0), (0, -192), (0, 192)] {
                            assert_ne!(world.column(x + dx, z + dz).biome, Biome::Snow);
                        }
                    }
                }
            }
        }
        assert!(seen.into_iter().all(|s| s));
        assert!(
            large * 4 < old,
            "{large} new boundaries versus {old} old boundaries"
        );
    }
    #[test]
    fn ranges_have_high_rocky_peaks_without_changing_warm_climate_to_snow() {
        let (mut low, mut high, mut warm_peak) = (HEIGHT, 0, false);
        for seed in 0..4 {
            let world = World::new(seed);
            for z in (-4000..=4000).step_by(64) {
                for x in (-4000..=4000).step_by(64) {
                    let c = world.column(x, z);
                    low = low.min(c.height);
                    high = high.max(c.height);
                    assert!((c.height - world.column(x + 1, z).height).abs() <= 2);
                    if c.height >= 58 && c.biome == Biome::Desert {
                        warm_peak = true;
                    }
                }
            }
        }
        assert!(high >= 64 && high - low >= 40);
        assert!(warm_peak);
    }
    #[test]
    fn cave_mouths_lead_down_into_a_walkable_chamber_across_chunk_edges() {
        let mut world = World::new(64195485);
        let mut checked = 0;
        for z in -12..=12 {
            for x in -12..=12 {
                let Some(p) = world.cave_entrance(x, z) else {
                    continue;
                };
                let [dx, dz] = world.cave_direction(p);
                let cells: Vec<_> = (-4..=24)
                    .map(|u| [p[0] + u * dx, p[1] - (u.max(0) / 2), p[2] + u * dz])
                    .collect();
                for &feet in &cells {
                    assert!(world.get([feet[0], feet[1] - 1, feet[2]]).solid());
                    assert_eq!(world.get(feet), Block::Air);
                    assert_eq!(world.get([feet[0], feet[1] + 1, feet[2]]), Block::Air);
                }
                world.stream(chunk_key(p), 2, 25);
                for feet in &cells {
                    assert_eq!(world.get(*feet), Block::Air);
                    assert!(world.get([feet[0], feet[1] - 1, feet[2]]).solid());
                }
                let mut player = crate::game::Player::new([
                    p[0] as f32 - 3. * dx as f32 + 0.5,
                    p[1] as f32 + 0.01,
                    p[2] as f32 - 3. * dz as f32 + 0.5,
                ]);
                player.yaw = (dx as f32).atan2(-dz as f32);
                for _ in 0..155 {
                    player.update(
                        &world,
                        crate::game::Movement {
                            forward: 1.,
                            ..Default::default()
                        },
                        0.05,
                        crate::game::Mode::Survival,
                    );
                }
                let advance = (player.position.x - p[0] as f32) * dx as f32
                    + (player.position.z - p[2] as f32) * dz as f32;
                assert!(
                    advance > 18. && player.position.y < p[1] as f32 - 7.,
                    "Cave could not be entered: {player_pos:?}, mouth {p:?}, axis {dx},{dz}, advance {advance}",
                    player_pos = player.position
                );
                checked += 1;
                if checked == 8 {
                    return;
                }
            }
        }
        panic!("Not enough cave fixtures: {checked}");
    }
    #[test]
    fn new_world_spawn_has_a_town_nearby_for_many_seeds() {
        for seed in 0..32 {
            let world = World::new(seed);
            let p = world.spawn();
            let v = world
                .nearest_village([p[0] as i32, p[2] as i32], 160)
                .expect("Spawn should help discover a village");
            assert!((p[0] - v.center[0] as f32).hypot(p[2] - v.center[2] as f32) <= 53.);
            assert_eq!(world.get(p.map(|n| n.floor() as i32)), Block::Air);
        }
    }
}
