#![feature(int_roundings)]

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::pos::{ChunkPosition, TilePosition, Vec2};

pub mod pos;

#[derive(Serialize, Deserialize)]
pub enum MessageC2S {}
#[derive(Serialize, Deserialize)]
pub enum MessageS2C {
    LoadChunk(ChunkPosition, HashMap<String, Vec<u32>>, Vec<EntityAddMessage>),
    UnloadChunk(ChunkPosition, Vec<Uuid>),
    SetTile(TilePosition, String, u32),
    AddEntity(EntityAddMessage),
    RemoveEntity(Uuid),
    MoveEntity(Uuid, Vec2),
    LoadContent(LoadContentMessage),
}
#[derive(Serialize, Deserialize)]
pub struct EntityAddMessage {
    pub uuid: Uuid,
    pub entity_type: String,
    pub position: Vec2,
}
#[derive(Serialize, Deserialize)]
pub struct LoadContentMessage {
    pub tilesets: HashMap<String, TileSetContent>,
}
#[derive(Serialize, Deserialize)]
pub struct TileSetContent {
    pub asset: Vec<u8>,
    pub size: u8,
    pub tiles: Vec<Option<(u8, u8)>>,
}