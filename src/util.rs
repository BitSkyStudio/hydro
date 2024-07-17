pub const CHUNK_SIZE: i32 = 32;

#[derive(Copy, Clone, Hash, Eq, PartialEq)]
pub struct ChunkPosition{
    pub x: i16,
    pub y: i16,
}
#[derive(Copy, Clone, Hash, Eq, PartialEq)]
pub struct ChunkOffset{
    pub x: u8,
    pub y: u8,
}
#[derive(Copy, Clone)]
pub struct TilePosition{
    pub x: i32,
    pub y: i32,
}
impl TilePosition{
    pub fn to_chunk_position(self) -> (ChunkPosition, ChunkOffset){
        (
            ChunkPosition{
                x: self.x.div_floor(CHUNK_SIZE) as i16,
                y: self.y.div_floor(CHUNK_SIZE) as i16,
            },
            ChunkOffset{
                x: self.x.rem_euclid(CHUNK_SIZE) as u8,
                y: self.y.rem_euclid(CHUNK_SIZE) as u8,
            }
        )
    }
}