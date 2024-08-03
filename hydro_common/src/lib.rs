#![feature(int_roundings)]

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::pos::{ChunkPosition, TilePosition, Vec2};

pub mod pos;

#[derive(Serialize, Deserialize, Hash, Eq, PartialEq, Copy, Clone)]
#[repr(u8)]
pub enum MouseButton{
    Left = 0,
    Right = 1,
    Middle = 2,
}
impl From<u8> for MouseButton{
    fn from(value: u8) -> Self {
        unsafe { std::mem::transmute(value) }
    }
}
#[derive(Serialize, Deserialize)]
pub enum MessageC2S {
    PlayerInput(PlayerInputMessage)
}
#[derive(Serialize, Deserialize, Default)]
pub struct PlayerInputMessage {
    pub keys_down: HashSet<u16>,
    pub keys_pressed: HashSet<u16>,
    pub keys_released: HashSet<u16>,
    pub buttons_down: HashSet<MouseButton>,
    pub buttons_pressed: HashSet<MouseButton>,
    pub buttons_released: HashSet<MouseButton>,
    pub mouse_position: Vec2,
}
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
    pub name: String,
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