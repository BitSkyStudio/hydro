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
    SetTile(TilePosition, u32),
    AddEntity(EntityAddMessage),
    RemoveEntity(Uuid),
    MoveEntity(Uuid, Vec2),
}
#[derive(Serialize, Deserialize)]
pub struct EntityAddMessage {
    uuid: Uuid,
    entity_type: String,
    position: Vec2,
}