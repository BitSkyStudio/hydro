use std::collections::HashMap;
use std::fmt::Display;
use std::net::ToSocketAddrs;

use bincode::config;
use macroquad::prelude::*;
use quad_net::web_socket::WebSocket;
use uuid::Uuid;

use hydro_common::{EntityAddMessage, MessageC2S, MessageS2C};
use hydro_common::pos::{CHUNK_SIZE, ChunkPosition};

#[macroquad::main("hydro")]
async fn main() {
    //let location = web_sys::window().unwrap().document().unwrap().location().unwrap();
    //let websocket = WebSocket::new(format!("{}://{}/ws", if location.protocol().unwrap() == "https:" { "wss" } else { "ws" }, location.host().unwrap()).as_str()).unwrap();
    let mut connection = Connection::connect("ws://localhost:8080/ws");
    let mut world = World {
        chunks: HashMap::new(),
        entities: HashMap::new(),
    };
    loop {
        for message in connection.read_messages() {
            match message {
                MessageS2C::LoadChunk(position, tiles, entities) => {
                    world.chunks.insert(position, tiles);
                    for entity in entities {
                        world.add_entity(entity);
                    }
                }
                MessageS2C::UnloadChunk(position, entities) => {
                    world.chunks.remove(&position);
                    for entity in entities {
                        world.entities.remove(&entity);
                    }
                }
                MessageS2C::SetTile(position, tileset, tile) => {
                    let (chunk_position, chunk_offset) = position.to_chunk_position();
                    if let Some(chunk) = world.chunks.get_mut(&chunk_position) {
                        chunk.entry(tileset).or_insert_with(|| vec![0; (CHUNK_SIZE * CHUNK_SIZE) as usize])[chunk_offset.index()] = tile;
                    }
                }
                MessageS2C::AddEntity(entity) => {
                    world.add_entity(entity);
                }
                MessageS2C::RemoveEntity(id) => {
                    world.entities.remove(&id);
                }
                MessageS2C::MoveEntity(id, position) => {
                    if let Some(entity) = world.entities.get_mut(&id) {
                        *entity.0 = position;
                    }
                }
            }
        }

        clear_background(RED);

        draw_line(40.0, 40.0, 100.0, 200.0, 15.0, BLUE);
        draw_rectangle(screen_width() / 2.0 - 60.0, 100.0, 120.0, 60.0, GREEN);
        draw_circle(screen_width() - 30.0, screen_height() - 30.0, 15.0, YELLOW);

        draw_text("IT WORKS!", 20.0, 20.0, 30.0, DARKGRAY);

        next_frame().await
    }
}
pub struct World {
    chunks: HashMap<ChunkPosition, HashMap<String, Vec<u32>>>,
    entities: HashMap<Uuid, (Vec2, String)>,
}
impl World {
    pub fn add_entity(&mut self, entity: EntityAddMessage) {
        self.entities.insert(entity.uuid, (Vec2::new(entity.position.x, entity.position.y), entity.entity_type));
    }
}

pub struct Connection {
    socket: WebSocket,
}
impl Connection {
    pub fn connect<A: ToSocketAddrs + Display>(addr: A) -> Self {
        Connection {
            socket: WebSocket::connect(addr).unwrap()
        }
    }
    pub fn send(&self, message: MessageC2S) {
        self.socket.send_bytes(bincode::serde::encode_to_vec(message, config::standard()).unwrap().as_slice());
    }
    pub fn read_messages(&mut self) -> Vec<MessageS2C> {
        let mut messages = Vec::new();
        while let Some(message) = self.socket.try_recv() {
            messages.push(bincode::serde::decode_from_slice(message.as_slice(), config::standard()).unwrap().0);
        }
        messages
    }
}