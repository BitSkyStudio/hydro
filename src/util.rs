use immutable_string::ImmutableString;

use crate::lua::Position;

pub const CHUNK_SIZE: i32 = 32;

#[derive(Copy, Clone, Hash, Eq, PartialEq)]
pub struct ChunkPosition {
    pub x: i16,
    pub y: i16,
}
#[derive(Copy, Clone, Hash, Eq, PartialEq)]
pub struct ChunkOffset {
    pub x: u8,
    pub y: u8,
}
#[derive(Copy, Clone)]
pub struct TilePosition {
    pub x: i32,
    pub y: i32,
}
impl TilePosition {
    pub fn to_chunk_position(self) -> (ChunkPosition, ChunkOffset) {
        (
            ChunkPosition {
                x: self.x.div_floor(CHUNK_SIZE) as i16,
                y: self.y.div_floor(CHUNK_SIZE) as i16,
            },
            ChunkOffset {
                x: self.x.rem_euclid(CHUNK_SIZE) as u8,
                y: self.y.rem_euclid(CHUNK_SIZE) as u8,
            }
        )
    }
}

#[derive(Copy, Clone)]
pub struct AABB {
    pub(crate) x: f64,
    pub(crate) y: f64,
    pub(crate) w: f64,
    pub(crate) h: f64,
}
impl AABB {
    pub fn offset(&self, position: Position) -> Self {
        AABB {
            x: self.x + position.x,
            y: self.y + position.y,
            w: self.w,
            h: self.h,
        }
    }
    pub fn get_position(&self, world: ImmutableString) -> Position {
        Position {
            x: self.x,
            y: self.y,
            world,
        }
    }
    pub fn collides(&self, other: &AABB) -> bool {
        self.x < other.x + other.w &&
            self.x + self.w > other.x &&
            self.y < other.y + other.h &&
            self.y + self.h > other.y
    }
    pub fn tiles_overlapping(&self) -> AABBTileIterator {
        let x = self.x.floor() as i32;
        AABBTileIterator {
            x,
            y: self.y.floor() as i32,
            x_start: x,
            x_end: (self.x + self.w).ceil() as i32,
            y_end: (self.y + self.h).ceil() as i32,
        }
    }
    pub fn sweep(&self, other: &AABB, target_position: Position) -> (AABB, f64) {
        //https://www.gamedev.net/tutorials/programming/general-and-gameplay-programming/swept-aabb-collision-detection-and-response-r3084/

        let (vx, vy) = (target_position.x - self.x, target_position.y - self.y);

        // find the distance between the objects on the near and far sides for both x and y
        let (x_inv_entry, x_inv_exit) = if vx > 0.0
        {
            (other.x - (self.x + self.w),
             (other.x + other.w) - self.x)
        } else {
            ((other.x + other.w) - self.x,
             other.x - (self.x + self.w))
        };

        let (y_inv_entry, y_inv_exit) = if vy > 0.0
        {
            (other.y - (self.y + self.h),
             (other.y + other.h) - self.y)
        } else {
            ((other.y + other.h) - self.y,
             other.y - (self.y + self.h))
        };

        let (x_entry, x_exit) = if vx == 0.0
        {
            (-f64::INFINITY, f64::INFINITY)
        } else {
            (x_inv_entry / vx, x_inv_exit / vx)
        };

        let (y_entry, y_exit) = if vy == 0.0
        {
            (-f64::INFINITY, f64::INFINITY)
        } else {
            (y_inv_entry / vy, y_inv_exit / vy)
        };
        let entry_time = x_entry.max(y_entry);
        let exit_time = x_exit.min(y_exit);
        let collision_time = if entry_time > exit_time || x_entry < 0.0 && y_entry < 0.0 || x_entry > 1.0 || y_entry > 1.0
        {
            1.0
        } else {
            entry_time
        };
        (AABB { x: self.x + (vx * entry_time), y: self.y + (vy * entry_time), w: self.w, h: self.h }, collision_time)
    }
}
pub struct AABBTileIterator {
    x_start: i32,
    x: i32,
    y: i32,
    x_end: i32,
    y_end: i32,
}
impl Iterator for AABBTileIterator {
    type Item = TilePosition;
    fn next(&mut self) -> Option<Self::Item> {
        if self.y >= self.y_end {
            return None;
        }
        let return_value = Some(TilePosition { x: self.x, y: self.y });
        self.x += 1;
        if self.x >= self.x_end {
            self.y += 1;
            self.x = self.x_start;
        }
        return_value
    }
}