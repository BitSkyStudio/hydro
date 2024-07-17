#![feature(int_roundings)]

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::time::{Duration, Instant};
use immutable_string::ImmutableString;
use mlua::{Lua};
use mlua::prelude::{LuaOwnedFunction, LuaOwnedTable};
use crate::util::{CHUNK_SIZE, ChunkOffset, ChunkPosition};

mod util;
mod lua;

fn main() {
    let lua = Lua::new();
    lua::init_lua_functions(&lua);
    InitEnvironment::load_into_lua(&lua);
    lua.load(std::fs::read_to_string("simple_mod.lua").unwrap()).exec().unwrap();
    let init_env = lua.remove_app_data::<InitEnvironment>().unwrap();
    let server = Arc::new(Server{
        lua,
        worlds: RefCell::new(HashMap::new()),
        tile_sets: init_env.tile_sets.into_inner(),
        event_handlers: init_env.event_handlers.into_inner(),
    });
    server.lua.set_app_data(server.clone());

    server.call_event("start".into(), server.lua.create_table().unwrap().into_owned()).unwrap();

    let server_start = Instant::now();
    let mut ticks_passed = 0u32;
    loop{
        {
            let globals = server.lua.globals();
            globals.set("ticks_passed", ticks_passed).unwrap();
            globals.set("seconds_passed", server_start.elapsed().as_secs()).unwrap();
        }
        server.tick();
        let sleep_time = (ticks_passed as f64 * (1000. / Server::TPS as f64))
            - server_start.elapsed().as_millis() as f64;
        if sleep_time > 0. {
            std::thread::sleep(Duration::from_millis(sleep_time as u64));
        }
        ticks_passed += 1;
    }
}
pub struct InitEnvironment{
    tile_sets: RefCell<HashMap<ImmutableString, TileSet>>,
    event_handlers: RefCell<HashMap<ImmutableString, Vec<LuaOwnedFunction>>>,
}
impl InitEnvironment{
    pub fn load_into_lua(lua: &Lua){
        lua.set_app_data(InitEnvironment{
            tile_sets: RefCell::new(HashMap::new()),
            event_handlers: RefCell::new(HashMap::new()),
        });

        let globals = lua.globals();
        globals.set("register_event", lua.create_function(|lua, (name, function): (String, LuaOwnedFunction)|{
            let init_env = lua.app_data_ref::<InitEnvironment>().ok_or(mlua::Error::runtime("this method can only be used during initialization"))?;
            init_env.event_handlers.borrow_mut().entry(name.into()).or_insert_with(||Vec::new()).push(function);
            Ok(())
        }).unwrap()).unwrap()
    }
}
pub struct Server{
    worlds: RefCell<HashMap<ImmutableString, World>>,
    tile_sets: HashMap<ImmutableString, TileSet>,
    event_handlers: HashMap<ImmutableString, Vec<LuaOwnedFunction>>,
    lua: Lua,
}
impl Server{
    pub const TPS: u8 = 30;
    pub fn call_event(&self, id: ImmutableString, data: LuaOwnedTable) -> mlua::Result<()>{
        for event in self.event_handlers.get(&id).unwrap_or(&Vec::new()){
            event.call(data.clone())?;
        }
        Ok(())
    }
    pub fn tick(&self){
        self.call_event("tick".into(), self.lua.create_table().unwrap().into_owned()).unwrap();
    }
}
type ServerPtr = Arc<Server>;
pub struct World{
    chunks: HashMap<ChunkPosition, Chunk>,
}
impl World{
    pub fn new() -> Self{
        World{
            chunks: HashMap::new()
        }
    }
}
pub struct Chunk{
    tile_layers: HashMap<ImmutableString, ChunkTileLayer>,
}
impl Chunk{
    pub fn new() -> Self{
        Chunk{
            tile_layers: HashMap::new()
        }
    }
}
pub struct TileSet{
    tiles: HashMap<ImmutableString, (LuaOwnedTable, u32)>,
    tile_ids: Vec<ImmutableString>,
}
impl TileSet{
    pub fn new() -> Self{
        TileSet{
            tiles: HashMap::new(),
            tile_ids: Vec::new(),
        }
    }
    pub fn register(&mut self, id: ImmutableString, data: LuaOwnedTable) -> mlua::Result<()>{
        if self.tiles.contains_key(&id){
            return Err(mlua::Error::runtime("registered two tiles with same id"));
        }
        data.to_ref().set("id", id.to_string()).unwrap();
        let num_id = self.tile_ids.len() as u32;
        self.tile_ids.push(id.clone());
        self.tiles.insert(id, (data, num_id));
        Ok(())
    }
}
pub struct ChunkTileLayer(Vec<u32>, HashMap<ChunkOffset,LuaOwnedTable>);
impl ChunkTileLayer{
    pub fn new() -> Self{
        ChunkTileLayer(vec![0; (CHUNK_SIZE * CHUNK_SIZE) as usize], HashMap::new())
    }
}