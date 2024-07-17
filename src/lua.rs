use immutable_string::ImmutableString;
use mlua::{Error, FromLua, Lua, UserData, UserDataFields, UserDataMethods};
use crate::{Chunk, ChunkTileLayer, Server, ServerPtr, World};
use crate::util::{CHUNK_SIZE, TilePosition};

pub fn init_lua_functions(lua: &Lua){
    let globals = lua.globals();

    globals.set("tps", Server::TPS).unwrap();
    globals.set("deltatime", 1./Server::TPS as f64).unwrap();

    globals.set("pos", lua.create_function(|_, (x,y,world):(f64,f64,String)|{
        Ok(Position{
            x,
            y,
            world: world.into(),
        })
    }).unwrap()).unwrap();

    globals.set("tileset", lua.create_function(|_, (tileset):(String)|{
        Ok(LuaTileSet{
            tileset: tileset.into(),
        })
    }).unwrap()).unwrap();
}

pub struct LuaTileSet{
    tileset: ImmutableString,
}
impl UserData for LuaTileSet{
    fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_method("get_at", |lua, tile_map, (pos,): (Position,)|{
            let server = lua.app_data_ref::<ServerPtr>().ok_or(Error::runtime("this method can only be used on server is running"))?;
            let worlds = server.worlds.borrow();
            let world = worlds.get(&pos.world).ok_or(Error::runtime("world not loaded"))?;
            let (chunk_position, chunk_offset) = pos.align_to_tile().to_chunk_position();
            let chunk = world.chunks.get(&chunk_position).ok_or(Error::runtime("chunk not loaded"))?;
            let tile_id = match chunk.tile_layers.get(&tile_map.tileset){
                Some(tileset) => {
                    tileset.0[chunk_offset.x as usize+(chunk_offset.y as usize * CHUNK_SIZE as usize)]
                }
                None => {
                    0
                }
            };
            let tileset = server.tile_sets.get(&tile_map.tileset).ok_or(Error::runtime("tileset doesn't exist"))?;
            Ok(tileset.tiles.get(&tileset.tile_ids[tile_id as usize]).unwrap().clone())
        });
        methods.add_method("set_at", |lua, tile_map, (pos,id): (Position,String)|{
            let server = lua.app_data_ref::<ServerPtr>().ok_or(Error::runtime("this method can only be used on server is running"))?;
            let mut worlds = server.worlds.borrow_mut();
            let world = worlds.get_mut(&pos.world).ok_or(Error::runtime("world not loaded"))?;
            let (chunk_position, chunk_offset) = pos.align_to_tile().to_chunk_position();
            let chunk = world.chunks.get_mut(&chunk_position).ok_or(Error::runtime("chunk not loaded"))?;
            let mut tile_layer = chunk.tile_layers.entry(tile_map.tileset.clone()).or_insert_with(||ChunkTileLayer::new());
            let tileset = server.tile_sets.get(&tile_map.tileset).ok_or(Error::runtime("tileset doesn't exist"))?;
            tile_layer.0[chunk_offset.x as usize+(chunk_offset.y as usize * CHUNK_SIZE as usize)] = tileset.tiles.get::<ImmutableString>(&id.into()).ok_or(Error::runtime("tile not found in tileset"))?.1;
            if let Some(tile_data) = tile_layer.1.remove(&chunk_offset){
                tile_data.to_ref().set("invalid", true)?;
            }
            Ok(())
        });
        methods.add_method("get_data_at", |lua, tile_map, (pos,): (Position,)|{
            Err::<(), _>(Error::runtime("aaa"))
        });
    }
}
#[derive(Clone, FromLua)]
pub struct Position {
    pub x: f64,
    pub y: f64,
    pub world: ImmutableString,
}
impl UserData for Position {
    fn add_fields<'lua, F: UserDataFields<'lua, Self>>(fields: &mut F) {
        fields.add_field_method_get("x", |_, pos|{Ok(pos.x)});
        fields.add_field_method_get("y", |_, pos|{Ok(pos.y)});
        fields.add_field_method_get("world", |_, pos|{Ok(pos.world.to_string())});
    }
    fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_method("is_loaded", |lua, pos, ()|{
            let server = lua.app_data_ref::<ServerPtr>().ok_or(Error::runtime("this method can only be used on server is running"))?;
            let worlds = server.worlds.borrow();
            match worlds.get(&pos.world){
                Some(world) => Ok(world.chunks.contains_key(&pos.align_to_tile().to_chunk_position().0)),
                None => Ok(false)
            }
        });
        methods.add_method("load", |lua,pos, ()|{
            let server = lua.app_data_ref::<ServerPtr>().ok_or(Error::runtime("this method can only be used on server is running"))?;
            let mut worlds = server.worlds.borrow_mut();
            let world = worlds.entry(pos.world.clone()).or_insert_with(||World::new());
            world.chunks.entry(pos.align_to_tile().to_chunk_position().0).or_insert_with(||Chunk::new());
            Ok(())
        });
    }
}
impl Position {
    pub fn align_to_tile(&self) -> TilePosition{
        TilePosition{
            x: self.x as i32,
            y: self.y as i32,
        }
    }
}