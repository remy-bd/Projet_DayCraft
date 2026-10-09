//! Seeded towns, with shared voxel geometry for rendering and collision.
mod legacy;
use crate::world::{Block, Pos};
pub const REGION: i32 = 192;
pub const TERRAIN_REGION: i32 = 640;
pub const RADIUS: i32 = 52;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Village {
    pub region: [i32; 2],
    pub center: Pos,
    pub desert: bool,
    pub legacy: bool,
    pub terrain: Option<Terrain>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Terrain {
    pub floors: [i32; 14],
    pub fields: [i32; 2],
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    House,
    Church,
    Forge,
    Inn,
    Barn,
    Market,
}
#[derive(Clone, Copy)]
pub struct Building {
    pub kind: Kind,
    pub x: i32,
    pub z: i32,
    pub w: i32,
    pub d: i32,
}
pub const BUILDINGS: [Building; 14] = [
    Building {
        kind: Kind::Church,
        x: -43,
        z: -44,
        w: 13,
        d: 19,
    },
    Building {
        kind: Kind::Inn,
        x: -19,
        z: -43,
        w: 15,
        d: 14,
    },
    Building {
        kind: Kind::House,
        x: 7,
        z: -42,
        w: 9,
        d: 10,
    },
    Building {
        kind: Kind::Forge,
        x: 28,
        z: -43,
        w: 16,
        d: 14,
    },
    Building {
        kind: Kind::House,
        x: -43,
        z: -18,
        w: 11,
        d: 12,
    },
    Building {
        kind: Kind::House,
        x: -18,
        z: -18,
        w: 9,
        d: 10,
    },
    Building {
        kind: Kind::House,
        x: 7,
        z: -18,
        w: 10,
        d: 9,
    },
    Building {
        kind: Kind::Barn,
        x: 28,
        z: -19,
        w: 16,
        d: 14,
    },
    Building {
        kind: Kind::House,
        x: -43,
        z: 7,
        w: 10,
        d: 11,
    },
    Building {
        kind: Kind::Market,
        x: -19,
        z: 6,
        w: 15,
        d: 13,
    },
    Building {
        kind: Kind::House,
        x: 7,
        z: 7,
        w: 9,
        d: 10,
    },
    Building {
        kind: Kind::House,
        x: 29,
        z: 7,
        w: 12,
        d: 10,
    },
    Building {
        kind: Kind::House,
        x: -42,
        z: 29,
        w: 10,
        d: 12,
    },
    Building {
        kind: Kind::House,
        x: 7,
        z: 29,
        w: 12,
        d: 14,
    },
];
impl Building {
    fn north(self) -> bool {
        self.z > 0
    }
    pub fn door(self) -> [i32; 2] {
        [
            self.x + self.w / 2,
            self.z + if self.north() { 0 } else { self.d - 1 },
        ]
    }
    fn block(self, x: i32, h: i32, z: i32, desert: bool) -> Option<Block> {
        use Block::*;
        let (u, v) = (x - self.x, z - self.z);
        if !(-1..=self.w).contains(&u) || !(-1..=self.d).contains(&v) {
            return None;
        }
        let inside = (0..self.w).contains(&u) && (0..self.d).contains(&v);
        if h == 0 && !inside {
            return None;
        }
        let wall = if desert {
            Sandstone
        } else if self.kind == Kind::Church {
            Cobble
        } else {
            Planks
        };
        let floor = if desert { Sandstone } else { Cobble };
        let door_v = if self.north() { 0 } else { self.d - 1 };
        let wall_height = match self.kind {
            Kind::Church => 8,
            Kind::Inn => 8,
            Kind::Barn => 6,
            _ => 4,
        };
        let mut result = if inside && h == 0 { floor } else { Air };
        if self.kind == Kind::Market {
            if (1..=4).contains(&h) && [1, 6, 12].contains(&u) && [1, self.d - 2].contains(&v) {
                result = Wood;
            }
            if h == 5 && u >= 0 && u < self.w && (v <= 3 || v >= self.d - 4) {
                result = if desert { Sandstone } else { Planks };
            }
            if h == 1 && (2..self.w - 2).contains(&u) && (v == 2 || v == self.d - 3) {
                result = Planks;
            }
            if h == 2 && u == 2 && v == 2 {
                result = Chest;
            }
            if h == 2 && u == self.w - 3 && v == self.d - 3 {
                result = Torch;
            }
            return Some(result);
        }
        if inside && (1..=wall_height).contains(&h) {
            let outer = u == 0 || u == self.w - 1 || v == 0 || v == self.d - 1;
            result = if outer { wall } else { Air };
            if outer
                && (h == 2 || h == 3 || self.kind == Kind::Inn && (h == 6 || h == 7))
                && (u % 4 == 2 && (v == 0 || v == self.d - 1)
                    || v % 4 == 2 && (u == 0 || u == self.w - 1))
            {
                result = Glass;
            }
            if (u == self.w / 2 || self.kind == Kind::Barn && u == self.w / 2 + 1)
                && v == door_v
                && h <= 2
            {
                result = if h == 1 { Door } else { Air };
            }
            if !outer {
                if h == 1 && u == 1 && v == 1 {
                    result = match self.kind {
                        Kind::Forge => Furnace,
                        Kind::Church => Torch,
                        Kind::Barn => Wood,
                        _ => Bed,
                    };
                }
                if h == 1 && u == self.w - 2 && v == 1 {
                    result = Chest;
                }
                if h == 1 && u == 1 && v == self.d - 2 {
                    result = Workbench;
                }
                if h == 2 && u == self.w - 2 && v == self.d - 2 {
                    result = Torch;
                }
                if self.kind == Kind::Inn {
                    if h == 5 {
                        result = Planks;
                    }
                    if h == 6 && [2, self.w - 3].contains(&u) && [2, self.d - 3].contains(&v) {
                        result = Bed;
                    }
                    // A real staircase reaches the upper floor through its own opening.
                    if v == self.d - 3 && (2..=6).contains(&u) && h < u {
                        result = Planks;
                    }
                    if h == 5 && v >= self.d - 4 && u <= 5 {
                        result = Air;
                    }
                }
                if self.kind == Kind::Barn {
                    if h <= 2 && (u == 2 || u == self.w - 3) && v < self.d - 3 {
                        result = if h == 1 { Wood } else { Air };
                    }
                    if h == 1 && u == self.w / 2 && v == 2 {
                        result = Water;
                    }
                }
                if self.kind == Kind::Church {
                    if h == 1 && (u == 3 || u == self.w - 4) && v > 5 && v % 3 == 0 {
                        result = Planks;
                    }
                    if h == 1 && (4..self.w - 4).contains(&u) && v == 3 {
                        result = Brick;
                    }
                    if h == 2 && u == self.w / 2 && v == 3 {
                        result = Torch;
                    }
                }
            }
        }
        if self.kind == Kind::Forge {
            if inside && u >= self.w - 6 && v <= 5 {
                if h == 1 {
                    result = Cobble;
                }
                if h == 2 && u == self.w - 4 && v == 3 {
                    result = Lava;
                }
                if h == 2 && ((u - self.w + 4).abs() + (v - 3).abs() == 1) {
                    result = Cobble;
                }
                if (3..=12).contains(&h)
                    && (u == self.w - 4 && v == 3
                        || h >= 5 && (u - self.w + 4).abs() + (v - 3).abs() == 1)
                {
                    result = Brick;
                }
            }
            if h == 1 && u == 3 && v == 3 {
                result = Furnace;
            }
            if h == 1 && u == 3 && v == 4 {
                result = Workbench;
            }
        }
        if desert {
            if h == wall_height + 1 {
                result = Sandstone;
            }
            if h == wall_height + 2 && (u == -1 || u == self.w || v == -1 || v == self.d) {
                result = Sandstone;
            }
        } else {
            let roof_h = h - wall_height;
            if (1..=4).contains(&roof_h) && u >= roof_h - 2 && u <= self.w + 1 - roof_h {
                result = if roof_h == 1 { Wood } else { Planks };
            }
        }
        if self.kind == Kind::Church && (1..=6).contains(&u) && (1..=6).contains(&v) {
            if (9..=16).contains(&h) {
                result = if u == 1 || u == 6 || v == 1 || v == 6 {
                    wall
                } else {
                    Air
                };
            }
            if (12..=14).contains(&h) && (u == 3 || u == 4) && (v == 1 || v == 6) {
                result = Glass;
            }
            if h == 17 {
                result = floor;
            }
            if h == 18 && (u == 1 || u == 6 || v == 1 || v == 6) {
                result = wall;
            }
        }
        // The forge chimney continues through the roof.
        if self.kind == Kind::Forge {
            if (5..=12).contains(&h) && u == self.w - 4 && v == 3 {
                result = Brick;
            }
            if h == 1 && u == 2 && v == 1 {
                result = Chest;
            }
        }
        Some(result)
    }
}
impl Village {
    pub fn floor(self, index: usize) -> i32 {
        self.terrain.map_or(self.center[1], |t| t.floors[index])
    }
    pub fn surface_height(self, x: i32, z: i32, ground: i32) -> i32 {
        let Some(terrain) = self.terrain else {
            return self.center[1];
        };
        let (x, z) = (x - self.center[0], z - self.center[2]);
        for (i, b) in BUILDINGS.iter().enumerate() {
            if (b.x..b.x + b.w).contains(&x) && (b.z..b.z + b.d).contains(&z) {
                return self.floor(i);
            }
        }
        if x.abs() <= 2 && z.abs() <= 2 {
            return self.center[1];
        }
        for (i, (fx, fz)) in [(-19, 29), (28, 29)].into_iter().enumerate() {
            if (fx..fx + 15).contains(&x) && (fz..fz + 15).contains(&z) {
                return terrain.fields[i];
            }
        }
        if !(road(x) || road(z) || entrance(x, z)) {
            return ground;
        }
        let mut height = ground.max(self.center[1] - (x.abs() - 2).max(0) - (z.abs() - 2).max(0));
        for (i, b) in BUILDINGS.iter().enumerate() {
            let [dx, dz] = b.door();
            // Blend each landing into the street too, avoiding a cliff at short entrances.
            let distance = ((x - dx).abs() - 1).max(0) + ((z - dz).abs() - 2).max(0);
            height = height.max(self.floor(i) - distance);
        }
        height
    }
    pub fn block_at(self, pos: Pos, ground: i32) -> Option<Block> {
        let Some(terrain) = self.terrain else {
            return self.block(pos);
        };
        use Block::*;
        let [wx, y, wz] = pos;
        let (x, z) = (wx - self.center[0], wz - self.center[2]);
        if x.abs() > RADIUS || z.abs() > RADIUS {
            return None;
        }
        let foundation = if self.desert { Sandstone } else { Cobble };
        for (i, b) in BUILDINGS.iter().enumerate() {
            let floor = terrain.floors[i];
            if (b.x..b.x + b.w).contains(&x)
                && (b.z..b.z + b.d).contains(&z)
                && (ground..floor).contains(&y)
            {
                return Some(foundation);
            }
            if (0..=20).contains(&(y - floor)) {
                if let Some(block) = b.block(x, y - floor, z, self.desert) {
                    return Some(block);
                }
            }
        }
        if x.abs() <= 2 && z.abs() <= 2 {
            let h = y - self.center[1];
            return if (ground..self.center[1]).contains(&y) {
                Some(foundation)
            } else if h == 0 {
                Some(if x.abs() < 2 && z.abs() < 2 {
                    Water
                } else {
                    Cobble
                })
            } else if h == 4 {
                Some(if self.desert { Sandstone } else { Planks })
            } else if (1..=3).contains(&h) && x.abs() == 2 && z.abs() == 2 {
                Some(Wood)
            } else if (1..=8).contains(&h) {
                Some(Air)
            } else {
                None
            };
        }
        for (i, (fx, fz)) in [(-19, 29), (28, 29)].into_iter().enumerate() {
            let (u, v) = (x - fx, z - fz);
            if (0..15).contains(&u) && (0..15).contains(&v) {
                let h = y - terrain.fields[i];
                return if (ground..terrain.fields[i]).contains(&y) {
                    Some(foundation)
                } else if h == 0 {
                    Some(if u == 7 && (1..14).contains(&v) {
                        Water
                    } else {
                        Farmland
                    })
                } else if h == 1 {
                    Some(if u == 0 || u == 14 || v == 0 || v == 14 {
                        Wood
                    } else if u != 7 {
                        Wheat3
                    } else {
                        Air
                    })
                } else if (2..=8).contains(&h) {
                    Some(Air)
                } else {
                    None
                };
            }
        }
        if road(x) || road(z) || entrance(x, z) {
            let surface = self.surface_height(wx, wz, ground);
            let h = y - surface;
            if (ground..surface).contains(&y) {
                return Some(foundation);
            }
            if h == 0 {
                return Some(Path);
            }
            if [-23, 23].contains(&x) && [-23, 0, 23].contains(&z) && (1..=4).contains(&h) {
                return Some(if h == 4 { Torch } else { Wood });
            }
            if (1..=8).contains(&h) {
                return Some(Air);
            }
        }
        // Unoccupied ground, vegetation and underground layers remain natural.
        None
    }
    pub fn radius(self) -> i32 {
        if self.legacy {
            24
        } else {
            RADIUS
        }
    }
    pub fn foundation_depth(self) -> i32 {
        if self.legacy {
            3
        } else {
            12
        }
    }
    pub fn block(self, [x, y, z]: Pos) -> Option<Block> {
        if self.legacy {
            return self.legacy_block([x, y, z]);
        }
        use Block::*;
        let (x, z, h) = (x - self.center[0], z - self.center[2], y - self.center[1]);
        if x.abs() > RADIUS || z.abs() > RADIUS || h < -12 {
            return None;
        }
        if h < 0 {
            return Some(if self.desert { Sandstone } else { Dirt });
        }
        let road = |n: i32| n.abs() <= 2 || (n.abs() - 23).abs() <= 1;
        let mut block = if h == 0 {
            if road(x) || road(z) {
                Path
            } else if self.desert {
                Sand
            } else {
                Grass
            }
        } else {
            Air
        };
        // Every entrance is joined to the closest east/west street.
        for b in BUILDINGS {
            let [dx, dz] = b.door();
            let target = if dz < -23 {
                -23
            } else if dz < 23 {
                0
            } else {
                23
            };
            if h == 0 && (x - dx).abs() <= 1 && (dz.min(target)..=dz.max(target)).contains(&z) {
                block = Path;
            }
            if let Some(b) = b.block(x, h, z, self.desert) {
                if h == 0 || b != Air {
                    block = b;
                } else {
                    block = Air;
                }
            }
        }
        if x.abs() <= 2 && z.abs() <= 2 {
            if h == 0 {
                block = if x.abs() < 2 && z.abs() < 2 {
                    Water
                } else {
                    Cobble
                };
            }
            if (1..=3).contains(&h) && x.abs() == 2 && z.abs() == 2 {
                block = Wood;
            }
            if h == 4 {
                block = if self.desert { Sandstone } else { Planks };
            }
        }
        for (fx, fz) in [(-19, 29), (28, 29)] {
            let (u, v) = (x - fx, z - fz);
            if (0..15).contains(&u) && (0..15).contains(&v) {
                if h == 0 {
                    block = if u == 7 { Water } else { Farmland };
                }
                if h == 1 && u != 7 {
                    block = Wheat3;
                }
                if h == 1 && (u == 0 || u == 14 || v == 0 || v == 14) {
                    block = Wood;
                }
            }
        }
        if h <= 4 && h > 0 && [-23, 23].contains(&x) && [-23, 0, 23].contains(&z) {
            block = if h == 4 { Torch } else { Wood };
        }
        Some(block)
    }
    pub fn chests(self) -> Vec<Pos> {
        if self.legacy {
            return self.legacy_chests().to_vec();
        }
        BUILDINGS
            .iter()
            .enumerate()
            .map(|(i, b)| {
                let (x, z, h) = if b.kind == Kind::Market {
                    (b.x + 2, b.z + 2, 2)
                } else if b.kind == Kind::Forge {
                    (b.x + 2, b.z + 1, 1)
                } else {
                    (b.x + b.w - 2, b.z + 1, 1)
                };
                [self.center[0] + x, self.floor(i) + h, self.center[2] + z]
            })
            .collect()
    }
    pub fn residents(self) -> Vec<Pos> {
        if self.legacy {
            return [(-4, -5), (4, -7), (-7, 5), (5, 6), (-4, 10)]
                .map(|(x, z)| [self.center[0] + x, self.center[1] + 1, self.center[2] + z])
                .to_vec();
        }
        BUILDINGS
            .iter()
            .enumerate()
            .map(|(i, b)| {
                let [x, z] = b.door();
                [
                    self.center[0] + x,
                    self.floor(i) + 1,
                    self.center[2] + z + if b.north() { -2 } else { 2 },
                ]
            })
            .collect()
    }
}

fn street(z: i32) -> i32 {
    if z < -23 {
        -23
    } else if z < 23 {
        0
    } else {
        23
    }
}

fn road(n: i32) -> bool {
    n.abs() <= 2 || (n.abs() - 23).abs() <= 1
}
fn entrance(x: i32, z: i32) -> bool {
    BUILDINGS.iter().any(|b| {
        let [dx, dz] = b.door();
        let target = street(dz);
        (x - dx).abs() <= 1 && (dz.min(target)..=dz.max(target)).contains(&z)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::World;
    #[test]
    fn towns_are_large_varied_and_all_loot_positions_are_real_chests() {
        let world = World::new(64195485);
        let region_size = world.village_region_size();
        let mut styles = [false; 2];
        let mut count = 0;
        for z in -12..=12 {
            for x in -12..=12 {
                if let Some(v) = world.village_at(x * region_size, z * region_size) {
                    assert_eq!(world.village_at(x * region_size, z * region_size), Some(v));
                    styles[usize::from(v.desert)] = true;
                    count += 1;
                    for dz in (-RADIUS..=RADIUS).step_by(4) {
                        for dx in (-RADIUS..=RADIUS).step_by(4) {
                            assert_eq!(
                                world.biome_at(v.center[0] + dx, v.center[2] + dz),
                                if v.desert { "Désert" } else { "Prairie" }
                            );
                        }
                    }
                    for p in v.chests() {
                        assert_eq!(world.get(p), Block::Chest, "{p:?}");
                    }
                    for p in v.residents() {
                        assert_eq!(world.get(p), Block::Air);
                        assert!(world.get([p[0], p[1] - 1, p[2]]).solid());
                    }
                    assert_eq!(v.residents().len(), 14);
                    assert_eq!(v.radius(), 52);
                }
            }
        }
        assert!(styles.into_iter().all(|s| s));
        assert!(
            count > 5 && count < 200,
            "Unexpected density: {count} towns"
        );
        for k in [
            Kind::Church,
            Kind::Forge,
            Kind::Inn,
            Kind::Barn,
            Kind::Market,
        ] {
            assert!(BUILDINGS.iter().any(|b| b.kind == k));
        }
        let v = Village {
            region: [0, 0],
            center: [96, 35, 96],
            desert: false,
            legacy: false,
            terrain: None,
        };
        assert_eq!(v.block([96 - 40, 35 + 17, 96 - 41]), Some(Block::Cobble));
        assert_eq!(v.block([96 + 40, 35 + 2, 96 - 40]), Some(Block::Lava));
    }
}
