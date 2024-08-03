use std::collections::{HashMap, HashSet};
use std::fmt::Display;
use std::net::ToSocketAddrs;

use bincode::config;
use macroquad::prelude::*;
use quad_net::web_socket::WebSocket;
use sapp_jsutils::JsObject;
use uuid::Uuid;

use hydro_common::{AnimationData, EntityAddMessage, MessageC2S, MessageS2C, PlayerInputMessage, RunningAnimation};
use hydro_common::pos::{CHUNK_SIZE, ChunkOffset, ChunkPosition};

#[macroquad::main("hydro")]
async fn main() {
    //let location = web_sys::window().unwrap().document().unwrap().location().unwrap();
    //let websocket = WebSocket::new(format!("{}://{}/ws", if location.protocol().unwrap() == "https:" { "wss" } else { "ws" }, location.host().unwrap()).as_str()).unwrap();
    info!("here2");
    let mut connection = Connection::connect("ws://localhost:8080/ws");
    let mut world = World {
        chunks: HashMap::new(),
        entities: HashMap::new(),
    };
    let mut camera_position = Vec2 { x: 0., y: 0. };
    let mut content = None;
    let mut connected = false;
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
                MessageS2C::UpdateEntityPosition(id, position) => {
                    if let Some(entity) = world.entities.get_mut(&id) {
                        entity.0 = Vec2::new(position.x, position.y);
                    }
                }
                MessageS2C::UpdateEntityAnimation(id, animation) => {
                    if let Some(entity) = world.entities.get_mut(&id) {
                        entity.2 = animation;
                    }
                }
                MessageS2C::LoadContent(content_msg) => {
                    connected = true;
                    unsafe { set_title_name(JsObject::string(content_msg.name.as_str())); }
                    content = Some(Content {
                        tilesets: content_msg.tilesets.into_iter().map(|(key, value)| {
                            let texture = Texture2D::from_file_with_format(value.asset.as_slice(), Some(ImageFormat::Png));
                            texture.set_filter(FilterMode::Nearest);
                            (key, TileSetContent {
                                asset: texture,
                                size: value.size,
                                tiles: value.tiles,
                            })
                        }).collect(),
                        entities: content_msg.entities.into_iter().map(|(key, value)| {
                            (key, EntityContent {
                                size: value.size,
                                animations: value.animations.into_iter().map(|(key, value)| {
                                    let texture = Texture2D::from_file_with_format(value.image.as_slice(), Some(ImageFormat::Png));
                                    texture.set_filter(FilterMode::Nearest);
                                    (key, AnimationDataTextured {
                                        image: texture,
                                        period: value.period,
                                        count: value.count,
                                        flip: value.flip,
                                        looped: value.looped,
                                    })
                                }).collect(),
                            })
                        }).collect(),
                    });
                }
                MessageS2C::CameraInfo(position) => {
                    camera_position = Vec2::new(position.x, position.y);
                }
            }
        }

        let zoom = 200.;
        let camera = Camera2D {
            target: camera_position,
            zoom: Vec2::new(1. / (screen_width() / zoom), 1. / (screen_height() / zoom)),
            ..Default::default()
        };

        if connected {
            let mut buttons_down = HashSet::new();
            let mut buttons_pressed = HashSet::new();
            let mut buttons_released = HashSet::new();
            for button in [MouseButton::Left, MouseButton::Right, MouseButton::Middle] {
                let net_button = match button {
                    MouseButton::Left => hydro_common::MouseButton::Left,
                    MouseButton::Right => hydro_common::MouseButton::Right,
                    MouseButton::Middle => hydro_common::MouseButton::Middle,
                    MouseButton::Unknown => unreachable!(),
                };
                if is_mouse_button_down(button) {
                    buttons_down.insert(net_button);
                }
                if is_mouse_button_pressed(button) {
                    buttons_pressed.insert(net_button);
                }
                if is_mouse_button_released(button) {
                    buttons_released.insert(net_button);
                }
            }
            let mouse = mouse_position();
            let mouse = camera.screen_to_world(Vec2::new(mouse.0, mouse.1));
            connection.send(MessageC2S::PlayerInput(PlayerInputMessage {
                keys_down: get_keys_down().iter().map(|key| *key as u16).collect(),
                keys_pressed: get_keys_pressed().iter().map(|key| *key as u16).collect(),
                keys_released: get_keys_released().iter().map(|key| *key as u16).collect(),
                buttons_down,
                buttons_pressed,
                buttons_released,
                mouse_position: hydro_common::pos::Vec2{x: mouse.x, y: mouse.y},
            }));
        }
        clear_background(RED);
        set_camera(&camera);
        if let Some(content) = &content {
            for (position, tiles) in &world.chunks {
                for (tileset, tiles) in tiles {
                    let tileset = content.tilesets.get(tileset).unwrap();
                    for x in 0..CHUNK_SIZE {
                        for y in 0..CHUNK_SIZE {
                            if let Some(tileset_position) = tileset.tiles.get(tiles[ChunkOffset { x: x as u8, y: y as u8 }.index()] as usize).unwrap() {
                                let source = Rect::new((tileset_position.0 * tileset.size) as f32, (tileset_position.1 * tileset.size) as f32, tileset.size as f32, tileset.size as f32);
                                draw_texture_ex(&tileset.asset, x as f32 + (position.x as i32 * CHUNK_SIZE) as f32, y as f32 + (position.y as i32 * CHUNK_SIZE) as f32, WHITE, DrawTextureParams {
                                    dest_size: Some(Vec2::new(1., 1.)),
                                    source: Some(source),
                                    ..Default::default()
                                });
                            }
                        }
                    }
                }
            }
            for (position, entity_type, animation) in world.entities.values() {
                let entity = content.entities.get(entity_type).unwrap();
                let animation_data = entity.animations.get(&animation.id).unwrap();
                let image_size = animation_data.image.size();
                let frame = (animation.time / animation_data.period as f32) as usize;
                let frame = if animation_data.looped { frame % animation_data.count as usize } else { frame.min(animation_data.count as usize - 1) };
                let width = image_size.x / animation_data.count as f32;
                draw_texture_ex(&animation_data.image, position.x, position.y, WHITE, DrawTextureParams {
                    dest_size: Some(Vec2::new(entity.size.0 as f32, entity.size.1 as f32)),
                    source: Some(Rect::new(width * frame as f32, 0., width, image_size.y)),
                    flip_y: animation_data.flip,
                    ..Default::default()
                });
            }
        }

        /*draw_line(40.0, 40.0, 100.0, 200.0, 15.0, BLUE);
        draw_rectangle(screen_width() / 2.0 - 60.0, 100.0, 120.0, 60.0, GREEN);
        draw_circle(screen_width() - 30.0, screen_height() - 30.0, 15.0, YELLOW);

        draw_text("IT WORKS!", 20.0, 20.0, 30.0, DARKGRAY);*/

        next_frame().await
    }
}
pub struct Content {
    pub tilesets: HashMap<String, TileSetContent>,
    pub entities: HashMap<String, EntityContent>,
}
pub struct TileSetContent {
    pub asset: Texture2D,
    pub size: u8,
    pub tiles: Vec<Option<(u8, u8)>>,
}
pub struct EntityContent {
    pub animations: HashMap<String, AnimationDataTextured>,
    pub size: (f64, f64),
}
pub struct AnimationDataTextured {
    pub image: Texture2D,
    pub count: u16,
    pub period: f64,
    pub looped: bool,
    pub flip: bool,
}
pub struct World {
    chunks: HashMap<ChunkPosition, HashMap<String, Vec<u32>>>,
    entities: HashMap<Uuid, (Vec2, String, RunningAnimation)>,
}
impl World {
    pub fn add_entity(&mut self, entity: EntityAddMessage) {
        self.entities.insert(entity.uuid, (Vec2::new(entity.position.x, entity.position.y), entity.entity_type, entity.animation));
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
            messages.push(bincode::serde::decode_from_slice(base64::decode(message).unwrap().as_slice(), config::standard()).unwrap().0);
        }
        messages
    }
}
extern "C" {
    fn set_title_name(name: JsObject);
}