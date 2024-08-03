use serde::{Deserialize, Serialize};

pub const CHUNK_SIZE: i32 = 32;

#[derive(Copy, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct ChunkPosition {
    pub x: i16,
    pub y: i16,
}
#[derive(Copy, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct ChunkOffset {
    pub x: u8,
    pub y: u8,
}
impl ChunkOffset {
    pub fn index(&self) -> usize {
        self.x as usize + (self.y as usize * CHUNK_SIZE as usize)
    }
}
#[derive(Copy, Clone, Serialize, Deserialize)]
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

#[derive(Serialize, Deserialize, Default, Copy, Clone)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}