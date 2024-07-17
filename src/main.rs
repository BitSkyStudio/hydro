#![feature(int_roundings)]

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;
use immutable_string::ImmutableString;
use mlua::Lua;
use mlua::prelude::LuaOwnedTable;
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
    });
    server.lua.set_app_data(server.clone());
}
pub struct InitEnvironment{
    tile_sets: RefCell<HashMap<ImmutableString, TileSet>>,
}
impl InitEnvironment{
    pub fn load_into_lua(lua: &Lua){
        lua.set_app_data(InitEnvironment{
            tile_sets: RefCell::new(HashMap::new()),
        });
    }
}
pub struct Server{
    worlds: RefCell<HashMap<ImmutableString, World>>,
    tile_sets: HashMap<ImmutableString, TileSet>,
    lua: Lua,
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