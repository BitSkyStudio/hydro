#![feature(int_roundings, async_closure, cell_update, hash_extract_if, fn_traits)]

use std::cell::{Cell, RefCell, RefMut};
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender};
use std::time::{Duration, Instant};

use bincode::config;
use bincode::error::DecodeError;
use futures::{FutureExt, StreamExt};
use immutable_string::ImmutableString;
use mlua::{IntoLuaMulti, Lua, OwnedAnyUserData, Table};
use mlua::prelude::{LuaOwnedFunction, LuaOwnedTable};
use tokio::runtime::Runtime;
use tokio_stream::wrappers::UnboundedReceiverStream;
use uuid::Uuid;
use warp::{Filter, Sink};
use warp::http::Response;
use warp::ws::Message;

use hydro_common::{AnimationData, EntityContentMessage, LoadContentMessage, MessageC2S, MessageS2C, TileSetContentMessage};
use hydro_common::pos::{CHUNK_SIZE, ChunkOffset, ChunkPosition};

use crate::lua::{Client, Collider, Position};
use crate::util::AABB;

mod util;
mod lua;

fn main() {
    let lua = Lua::new();
    lua::init_lua_functions(&lua);
    InitEnvironment::load_into_lua(&lua);
    lua.load(std::fs::read_to_string("simple_mod.lua").unwrap()).exec().unwrap();
    let init_env = lua.remove_app_data::<InitEnvironment>().unwrap();
    let (new_clients_tx, new_clients_rx) = std::sync::mpsc::channel();
    let server = Arc::new(Server {
        lua,
        worlds: RefCell::new(HashMap::new()),
        tile_sets: init_env.tile_sets.into_inner(),
        event_handlers: init_env.event_handlers.into_inner(),
        entity_registry: init_env.entity_registry.into_inner(),
        entities: RefCell::new(HashMap::new()),
        new_clients: new_clients_rx,
        clients: RefCell::new(HashMap::new()),
        ticks_passed: Cell::new(0),
        task_queue: RefCell::new(BinaryHeap::new())
    });
    server.lua.set_app_data(server.clone());

    server.call_event("start".into(), server.lua.create_table().unwrap().into_owned()).unwrap();

    std::thread::spawn(|| {
        Runtime::new().unwrap().block_on(web_server(8080, new_clients_tx));
    });

    let server_start = Instant::now();
    loop {
        {
            let globals = server.lua.globals();
            globals.set("ticks_passed", server.ticks_passed.get()).unwrap();
            globals.set("seconds_passed", server.ticks_passed.get() as f64 / Server::TPS as f64).unwrap();
        }
        while let Ok(client) = server.new_clients.try_recv() {
            client.sender.send(MessageS2C::LoadContent(LoadContentMessage {
                name: "hydro".to_string(),
                tilesets: server.tile_sets.iter().map(|(key, value)| (key.to_string(), TileSetContentMessage {
                    asset: value.asset.0.clone(),
                    size: value.asset.1,
                    tiles: value.tile_ids.iter().map(|id| value.tiles.get(id).unwrap().asset_position).collect(),
                })).collect(),
                entities: server.entity_registry.entities.iter().map(|(key, value)| {
                    (key.to_string(), EntityContentMessage {
                        size: value.size,
                        animations: value.animations.iter().map(|(key, value)| (key.to_string(), value.clone())).collect(),
                    })
                }).collect(),
            })).unwrap();
            let client = Client::new(&server.lua, client).unwrap();
            let id = { client.borrow::<Client>().unwrap().id.clone() };
            server.clients.borrow_mut().insert(id, client.clone());
            server.call_event("join".into(), client).unwrap();
        }
        server.tick();

        let sleep_time = (server.ticks_passed.get() as f64 * (1000. / Server::TPS as f64))
            - server_start.elapsed().as_millis() as f64;
        if sleep_time > 0. {
            std::thread::sleep(Duration::from_millis(sleep_time as u64));
        }
        server.ticks_passed.update(|val| val + 1);
    }
}
async fn web_server(port: u16, new_client_tx: Sender<ClientConnection>) {
    let websocket = warp::path("ws")
        .and(warp::ws())
        .map(move |ws: warp::ws::Ws| {
            let new_client_tx = new_client_tx.clone();
            ws.on_upgrade(move |websocket| user_connected(websocket, new_client_tx.clone()))
        });
    let html = warp::path::end().map(|| {
        Response::builder().body(include_str!("../host/index.html"))
    });
    let js_lib = warp::path("mq_js_bundle.js").and(warp::path::end()).map(|| {
        Response::builder().body(include_str!("../host/mq_js_bundle.js"))
    });
    let wasm = warp::path("hydro_client.wasm").and(warp::path::end()).map(|| {
        Response::builder().header("content-type", "application/wasm").body(include_bytes!("../host/hydro_client.wasm").to_vec())
    });
    warp::serve(websocket.or(html).or(js_lib).or(wasm)).run(([0, 0, 0, 0], port)).await;
}
async fn user_connected(ws: warp::ws::WebSocket, new_client_tx: Sender<ClientConnection>) {
    println!("client connect");
    let (client_ws_sender, mut client_ws_rcv) = ws.split();
    let (client_sender_c2s, client_receiver_c2s) = std::sync::mpsc::channel();
    let (client_sender_s2c, client_receiver_s2c) = tokio::sync::mpsc::unbounded_channel();
    new_client_tx.send(ClientConnection { receiver: client_receiver_c2s, sender: client_sender_s2c }).unwrap();

    let client_receiver_s2c = UnboundedReceiverStream::new(client_receiver_s2c);
    tokio::task::spawn(
        client_receiver_s2c.map(|message| {
            let message = bincode::serde::encode_to_vec::<MessageS2C, _>(message, config::standard()).unwrap();
            Ok(Message::text(base64::encode(message)))
        }).forward(client_ws_sender).map(|result| {
            if let Err(e) = result {
                eprintln!("error sending websocket msg: {}", e);
            }
        })
    );

    while let Some(result) = client_ws_rcv.next().await {
        let msg = match result {
            Ok(msg) => msg,
            Err(e) => {
                println!("error");
                break;
            }
        };
        client_sender_c2s.send(match bincode::serde::decode_from_slice::<MessageC2S, _>(msg.as_bytes(), config::standard()){
            Ok(message) => message.0,
            Err(error) => {
                println!("decode error: {}", error);
                break;
            }
        }).unwrap();
    }
    println!("disconnect")
}
pub struct InitEnvironment {
    tile_sets: RefCell<HashMap<ImmutableString, TileSet>>,
    entity_registry: RefCell<EntityRegistry>,
    event_handlers: RefCell<HashMap<ImmutableString, Vec<LuaOwnedFunction>>>,
}
impl InitEnvironment {
    pub fn load_into_lua(lua: &Lua) {
        lua.set_app_data(InitEnvironment {
            tile_sets: RefCell::new(HashMap::new()),
            entity_registry: RefCell::new(EntityRegistry { entities: HashMap::new() }),
            event_handlers: RefCell::new(HashMap::new()),
        });

        let globals = lua.globals();
        globals.set("register_event", lua.create_function(|lua, (name, function): (String, LuaOwnedFunction)| {
            let init_env = lua.app_data_ref::<InitEnvironment>().ok_or(mlua::Error::runtime("this method can only be used during initialization"))?;
            init_env.event_handlers.borrow_mut().entry(name.into()).or_insert_with(Vec::new).push(function);
            Ok(())
        }).unwrap()).unwrap();
        globals.set("register_tileset", lua.create_function(|lua, (name, table): (String, Table)| {
            let init_env = lua.app_data_ref::<InitEnvironment>().ok_or(mlua::Error::runtime("this method can only be used during initialization"))?;
            let mut tile_sets = init_env.tile_sets.borrow_mut();
            let mut tile_set = TileSet::new({
                let assets_table: Table = table.get("asset").unwrap();
                let file: String = assets_table.get("file").unwrap();
                let size: u8 = assets_table.get("size").unwrap();
                let image_data = std::fs::read(format!("assets/{}.png", file)).unwrap();
                (image_data, size)
            });
            tile_set.register(match table.get::<_, Option<Table>>("default").unwrap() {
                Some(default) => default.into_owned(),
                None => {
                    let table = lua.create_table().unwrap();
                    table.set("id", "default").unwrap();
                    table.into_owned()
                }
            }).unwrap();
            let tiles_table: Table = table.get("tiles").unwrap();
            for tile in tiles_table.sequence_values() {
                let tile: Table = tile.unwrap();
                tile_set.register(tile.into_owned()).unwrap();
            }
            tile_sets.insert(name.into(), tile_set);
            Ok(())
        }).unwrap()).unwrap();
        globals.set("register_entity", lua.create_function(|lua, (name, table): (String, Table)| {
            let init_env = lua.app_data_ref::<InitEnvironment>().ok_or(mlua::Error::runtime("this method can only be used during initialization"))?;
            let mut entity_registry = init_env.entity_registry.borrow_mut();
            entity_registry.register(lua, name.into(), table.into_owned());
            Ok(())
        }).unwrap()).unwrap();
    }
}
pub struct Task{
    run_on: u32,
    task: Box<dyn Fn(&Server) -> Option<f64>>,
}
impl PartialEq<Self> for Task {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other).is_eq()
    }
}
impl PartialOrd<Self> for Task {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Eq for Task {}
impl Ord for Task{
    fn cmp(&self, other: &Self) -> Ordering {
        other.run_on.cmp(&self.run_on)
    }
}
pub struct Server {
    worlds: RefCell<HashMap<ImmutableString, World>>,
    tile_sets: HashMap<ImmutableString, TileSet>,
    entity_registry: EntityRegistry,
    event_handlers: HashMap<ImmutableString, Vec<LuaOwnedFunction>>,
    entities: RefCell<HashMap<Uuid, OwnedAnyUserData>>,
    new_clients: Receiver<ClientConnection>,
    clients: RefCell<HashMap<Uuid, OwnedAnyUserData>>,
    lua: Lua,
    ticks_passed: Cell<u32>,
    task_queue: RefCell<BinaryHeap<Task>>
}
impl Server {
    pub const TPS: u8 = 30;
    pub fn call_event<T: for<'a> IntoLuaMulti<'a> + Clone>(&self, id: ImmutableString, data: T) -> mlua::Result<()> {
        for event in self.event_handlers.get(&id).unwrap_or(&Vec::new()) {
            event.call(data.clone())?;
        }
        Ok(())
    }
    pub fn tick(&self) {
        self.call_event("tick".into(), self.lua.create_table().unwrap().into_owned()).unwrap();
        for client in self.clients.borrow().values() {
            client.borrow_mut::<Client>().unwrap().tick(self, client.clone());
        }
        for client in self.clients.borrow_mut().extract_if(|id, client|client.borrow::<Client>().unwrap().closed){
            self.call_event("leave".into(), client.1).unwrap();
        }
        while let Some(mut task) = self.get_next_scheduled_task(){
            if let Some(reschedule) = task.task.call((self,)){
                self.schedule_task(task.task, reschedule);
            }
        }
    }
    fn get_next_scheduled_task(&self) -> Option<Task>{
        let mut task_queue = self.task_queue.borrow_mut();
        if task_queue.peek()?.run_on <= self.ticks_passed.get() {
            task_queue.pop()
        } else {
            None
        }
    }
    pub fn schedule_task<T: Fn(&Server) -> Option<f64> + 'static>(&self, task: T, after: f64){
        self.task_queue.borrow_mut().push(Task{
            run_on: self.ticks_passed.get() + (after*Server::TPS as f64).ceil() as u32,
            task: Box::new(task)
        });
    }
    pub fn get_chunk(&self, position: ChunkPosition, world: ImmutableString) -> RefMut<Chunk> {
        RefMut::map(self.worlds.borrow_mut(), |worlds| {
            worlds.entry(world.clone()).or_insert_with(World::new).chunks.entry(position.clone()).or_insert_with(|| {
                self.schedule_task(move |server|{
                    server.call_event("load_chunk".into(), Position{
                        x: (position.x as i32 * CHUNK_SIZE) as f64,
                        y: (position.y as i32 * CHUNK_SIZE) as f64,
                        world: world.clone()
                    }).unwrap();
                    None
                }, 0.);
                Chunk::new()
            })
        })
    }
}
type ServerPtr = Arc<Server>;
pub struct ClientConnection {
    receiver: Receiver<MessageC2S>,
    sender: tokio::sync::mpsc::UnboundedSender<MessageS2C>,
}
pub struct World {
    chunks: HashMap<ChunkPosition, Chunk>,
}
impl World {
    pub fn new() -> Self {
        World {
            chunks: HashMap::new()
        }
    }
}
pub struct Chunk {
    tile_layers: HashMap<ImmutableString, ChunkTileLayer>,
    entities: HashMap<Uuid, OwnedAnyUserData>,
    viewers: RefCell<HashMap<Uuid, OwnedAnyUserData>>,
}
impl Chunk {
    pub fn new() -> Self {
        Chunk {
            tile_layers: HashMap::new(),
            entities: HashMap::new(),
            viewers: RefCell::new(HashMap::new()),
        }
    }
}
pub struct TileType {
    data: LuaOwnedTable,
    id: u32,
    collision_mask: u32,
    asset_position: Option<(u8, u8)>,
}
pub struct TileSet {
    tiles: HashMap<ImmutableString, TileType>,
    tile_ids: Vec<ImmutableString>,
    asset: (Vec<u8>, u8),
}
impl TileSet {
    pub fn new(asset: (Vec<u8>, u8)) -> Self {
        TileSet {
            tiles: HashMap::new(),
            tile_ids: Vec::new(),
            asset,
        }
    }
    pub fn register(&mut self, data: LuaOwnedTable) -> mlua::Result<()> {
        let id: ImmutableString = data.to_ref().get::<&str, String>("id").map_err(|_| mlua::Error::runtime("tile id not specified"))?.into();
        if self.tiles.contains_key(&id) {
            return Err(mlua::Error::runtime("registered two tiles with same id"));
        }
        let num_id = self.tile_ids.len() as u32;
        let collision_mask: Option<u32> = data.to_ref().get("collision_mask").unwrap();
        data.to_ref().set("collision_mask", None::<bool>).unwrap();
        let asset_pos: Option<Table> = data.to_ref().get("asset_pos").unwrap();
        data.to_ref().set("asset_pos", None::<bool>).unwrap();
        self.tile_ids.push(id.clone());
        self.tiles.insert(id, TileType {
            id: num_id,
            asset_position: asset_pos.map(|table| (table.get("x").unwrap(), table.get("y").unwrap())),
            collision_mask: collision_mask.unwrap_or(0),
            data,
        });
        Ok(())
    }
    pub fn by_id(&self, id: u32) -> Option<&TileType> {
        self.tiles.get(&self.tile_ids[id as usize])
    }
}
pub struct EntityType {
    colliders: HashMap<ImmutableString, Collider>,
    data: LuaOwnedTable,
    data_metatable: LuaOwnedTable,
    animations: HashMap<ImmutableString, AnimationData>,
    size: (f64, f64),
}

pub struct EntityRegistry {
    entities: HashMap<ImmutableString, EntityType>,
}
impl EntityRegistry {
    pub fn register(&mut self, lua: &Lua, id: ImmutableString, data: LuaOwnedTable) {
        let colliders: Table = data.to_ref().get("colliders").unwrap();
        data.to_ref().set("colliders", None::<bool>).unwrap();
        let width: f64 = data.to_ref().get("width").unwrap();
        data.to_ref().set("width", None::<bool>).unwrap();
        let height: f64 = data.to_ref().get("height").unwrap();
        data.to_ref().set("height", None::<bool>).unwrap();
        let animations: Table = data.to_ref().get("animations").unwrap();
        data.to_ref().set("animations", None::<bool>).unwrap();
        let data_metatable = lua.create_table().unwrap().into_owned();
        data_metatable.to_ref().set("__index", data.clone()).unwrap();
        self.entities.insert(id, EntityType {
            colliders: colliders.pairs::<String, Table>().filter_map(|collider| match collider {
                Ok((name, collider)) => Some((name.into(), Collider {
                    aabb: AABB { x: collider.get("x").unwrap(), y: collider.get("y").unwrap(), w: collider.get("w").unwrap(), h: collider.get("h").unwrap() },
                    mask: collider.get("mask").unwrap(),
                })),
                Err(_) => None,
            }).collect(),
            animations: animations.pairs::<String, Table>().filter_map(|animation| match animation {
                Ok((name, animation)) => Some((name.into(), AnimationData {
                    flip: animation.get::<_, Option<bool>>("flip").unwrap().unwrap_or(false),
                    count: animation.get("count").unwrap(),
                    looped: animation.get("loop").unwrap(),
                    period: animation.get::<_, Option<f64>>("period").unwrap().unwrap_or(0.),
                    image: std::fs::read(format!("assets/{}.png", animation.get::<_, String>("file").unwrap())).unwrap(),
                })),
                Err(_) => None
            }).collect(),
            size: (width, height),
            data_metatable,
            data,
        });
    }
}
pub struct ChunkTileLayer(Vec<u32>, HashMap<ChunkOffset, LuaOwnedTable>);
impl ChunkTileLayer {
    pub fn new() -> Self {
        ChunkTileLayer(vec![0; (CHUNK_SIZE * CHUNK_SIZE) as usize], HashMap::new())
    }
}