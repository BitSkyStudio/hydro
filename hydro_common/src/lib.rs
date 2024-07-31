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
    UpdateEntityPosition(Uuid, Vec2),
    UpdateEntityAnimation(Uuid, RunningAnimation),
    LoadContent(LoadContentMessage),
    CameraInfo(Vec2),
}
#[derive(Serialize, Deserialize)]
pub struct EntityAddMessage {
    pub uuid: Uuid,
    pub entity_type: String,
    pub position: Vec2,
    pub animation: RunningAnimation,
}
#[derive(Serialize, Deserialize)]
pub struct LoadContentMessage {
    pub tilesets: HashMap<String, TileSetContentMessage>,
    pub entities: HashMap<String, EntityContentMessage>,
}
#[derive(Serialize, Deserialize)]
pub struct TileSetContentMessage {
    pub asset: Vec<u8>,
    pub size: u8,
    pub tiles: Vec<Option<(u8, u8)>>,
}
#[derive(Serialize, Deserialize)]
pub struct EntityContentMessage {
    pub animations: HashMap<String, AnimationData>,
    pub size: (f64, f64),
}
#[derive(Serialize, Deserialize, Clone)]
pub struct AnimationData {
    pub image: Vec<u8>,
    pub count: u16,
    pub period: f64,
    pub looped: bool,
    pub flip: bool,
}
#[derive(Serialize, Deserialize)]
pub struct RunningAnimation {
    pub id: String,
    pub time: f32,
}