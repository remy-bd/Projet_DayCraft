use super::Village;
use crate::world::{Block, Pos};
const HOUSES: [[i32; 2]; 4] = [[-17, -16], [6, -17], [-17, 7], [7, 8]];
impl Village {
    pub(super) fn legacy_block(self, [x, y, z]: Pos) -> Option<Block> {
        use Block::*;
        let [cx, ground, cz] = self.center;
        let (x, z, h) = (x - cx, z - cz, y - ground);
        if x.abs() > 24 || z.abs() > 24 || !(-3..=16).contains(&h) {
            return None;
        }
        let wall = if self.desert { Sandstone } else { Planks };
        let floor = if self.desert { Sandstone } else { Cobble };
        if h < 0 {
            return Some(if self.desert { Sandstone } else { Dirt });
        }
        let mut block = if h == 0 {
            if x.abs() <= 2 || z.abs() <= 2 {
                Path
            } else if self.desert {
                Sand
            } else {
                Grass
            }
        } else {
            Air
        };
        for (i, [hx, hz]) in HOUSES.into_iter().enumerate() {
            let (u, v) = (x - hx, z - hz);
            let width = if i == 3 { 9 } else { 7 };
            let depth = if i == 1 { 9 } else { 7 };
            if (-1..=width).contains(&u) && (-1..=depth).contains(&v) {
                if (0..width).contains(&u) && (0..depth).contains(&v) {
                    if h == 0 {
                        block = floor;
                    }
                    if (1..=4).contains(&h) {
                        block = if u == 0 || u == width - 1 || v == 0 || v == depth - 1 {
                            wall
                        } else {
                            Air
                        };
                        if (h == 2 || h == 3)
                            && (u == 3 && v == 0 || v == 3 && (u == 0 || u == width - 1))
                        {
                            block = Glass;
                        }
                        if u == width / 2 && v == depth - 1 && h <= 2 {
                            block = if h == 1 { Door } else { Air };
                        }
                        if h == 1 && u == 1 && v == 1 {
                            block = Bed;
                        }
                        if h == 1 && u == width - 2 && v == 1 && i % 2 == 0 {
                            block = Chest;
                        }
                        if h == 1 && u == 1 && v == depth - 2 {
                            block = if i == 1 { Furnace } else { Workbench };
                        }
                        if h == 2 && u == width - 2 && v == depth - 2 {
                            block = Torch;
                        }
                    }
                }
                if self.desert {
                    if h == 5 {
                        block = Sandstone;
                    }
                    if h == 6 && (u == -1 || u == width || v == -1 || v == depth) {
                        block = Sandstone;
                    }
                } else if (5..=7).contains(&h) && u >= h - 6 && u <= width + 5 - h {
                    block = if h == 5 { Wood } else { Planks };
                }
            }
        }
        if x.abs() <= 2 && z.abs() <= 2 {
            if h == 0 {
                block = if x.abs() < 2 && z.abs() < 2 {
                    Water
                } else {
                    floor
                };
            }
            if (1..=3).contains(&h) && x.abs() == 2 && z.abs() == 2 {
                block = wall;
            }
            if h == 4 {
                block = floor;
            }
        }
        if (-6..=5).contains(&x) && (14..=20).contains(&z) {
            if h == 0 {
                block = if x == 0 { Water } else { Farmland };
            }
            if h == 1 && x != 0 {
                block = Wheat3;
            }
        }
        if h == 1 && [(3, -5), (-5, 3), (3, 12), (12, 3)].contains(&(x, z)) {
            block = Torch;
        }
        Some(block)
    }
    pub(super) fn legacy_chests(self) -> [Pos; 2] {
        [0, 2].map(|i| {
            [
                self.center[0] + HOUSES[i][0] + 5,
                self.center[1] + 1,
                self.center[2] + HOUSES[i][1] + 1,
            ]
        })
    }
}
