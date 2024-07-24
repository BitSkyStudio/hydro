use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, Ordering};

use immutable_string::ImmutableString;
use mlua::{AnyUserData, Error, FromLua, Lua, OwnedAnyUserData, UserData, UserDataFields, UserDataMethods, Value};
use uuid::Uuid;

use crate::{Chunk, ChunkTileLayer, Server, ServerPtr, World};
use crate::util::{AABB, CHUNK_SIZE, TilePosition};

pub fn init_lua_functions(lua: &Lua) {
    let globals = lua.globals();

    globals.set("tps", Server::TPS).unwrap();
    globals.set("deltatime", 1. / Server::TPS as f64).unwrap();

    globals.set("pos", lua.create_function(|_, (x, y, world): (f64, f64, String)| {
        Ok(Position {
            x,
            y,
            world: world.into(),
        })
    }).unwrap()).unwrap();

    globals.set("tileset", lua.create_function(|_, (tileset): (String)| {
        Ok(LuaTileSet {
            tileset: tileset.into(),
        })
    }).unwrap()).unwrap();

    globals.set("spawn", lua.create_function(|lua, (type_id, position): (String, Position)| {
        Ok(Entity::new(lua, type_id.into(), position))
    }).unwrap()).unwrap();

    globals.set("get_entity", lua.create_function(|lua, (id, ): (String,)| {
        let uuid = Uuid::parse_str(id.as_str()).map_err(|_| Error::runtime("malformed uuid"))?;
        let server = lua.app_data_ref::<ServerPtr>().ok_or(Error::runtime("this method can only be used on server is running"))?;
        let id = server.entities.borrow().get(&uuid).cloned();
        Ok(id)
    }).unwrap()).unwrap();
}

pub struct LuaTileSet {
    tileset: ImmutableString,
}
impl UserData for LuaTileSet {
    fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_method("get_at", |lua, tile_map, (pos, ): (Position,)| {
            let server = lua.app_data_ref::<ServerPtr>().ok_or(Error::runtime("this method can only be used on server is running"))?;
            let worlds = server.worlds.borrow();
            let world = worlds.get(&pos.world).ok_or(Error::runtime("world not loaded"))?;
            let (chunk_position, chunk_offset) = pos.align_to_tile().to_chunk_position();
            let chunk = world.chunks.get(&chunk_position).ok_or(Error::runtime("chunk not loaded"))?;
            let tile_id = match chunk.tile_layers.get(&tile_map.tileset) {
                Some(tileset) => {
                    tileset.0[chunk_offset.x as usize + (chunk_offset.y as usize * CHUNK_SIZE as usize)]
                }
                None => {
                    0
                }
            };
            let tileset = server.tile_sets.get(&tile_map.tileset).ok_or(Error::runtime("tileset doesn't exist"))?;
            Ok(tileset.tiles.get(&tileset.tile_ids[tile_id as usize]).unwrap().data.clone())
        });
        methods.add_method("set_at", |lua, tile_map, (pos, id): (Position, String)| {
            let server = lua.app_data_ref::<ServerPtr>().ok_or(Error::runtime("this method can only be used on server is running"))?;
            let mut worlds = server.worlds.borrow_mut();
            let world = worlds.get_mut(&pos.world).ok_or(Error::runtime("world not loaded"))?;
            let (chunk_position, chunk_offset) = pos.align_to_tile().to_chunk_position();
            let chunk = world.chunks.get_mut(&chunk_position).ok_or(Error::runtime("chunk not loaded"))?;
            let mut tile_layer = chunk.tile_layers.entry(tile_map.tileset.clone()).or_insert_with(|| ChunkTileLayer::new());
            let tileset = server.tile_sets.get(&tile_map.tileset).ok_or(Error::runtime("tileset doesn't exist"))?;
            tile_layer.0[chunk_offset.x as usize + (chunk_offset.y as usize * CHUNK_SIZE as usize)] = tileset.tiles.get::<ImmutableString>(&id.into()).ok_or(Error::runtime("tile not found in tileset"))?.id;
            if let Some(tile_data) = tile_layer.1.remove(&chunk_offset) {
                tile_data.to_ref().set("invalid", true)?;
            }
            Ok(())
        });
        methods.add_method("get_data_at", |lua, tile_map, (pos, ): (Position,)| {
            let server = lua.app_data_ref::<ServerPtr>().ok_or(Error::runtime("this method can only be used on server is running"))?;
            let mut worlds = server.worlds.borrow_mut();
            let world = worlds.get_mut(&pos.world).ok_or(Error::runtime("world not loaded"))?;
            let (chunk_position, chunk_offset) = pos.align_to_tile().to_chunk_position();
            let chunk = world.chunks.get_mut(&chunk_position).ok_or(Error::runtime("chunk not loaded"))?;
            let mut tile_layer = chunk.tile_layers.entry(tile_map.tileset.clone()).or_insert_with(|| ChunkTileLayer::new());
            let tileset = server.tile_sets.get(&tile_map.tileset).ok_or(Error::runtime("tileset not found"))?;
            let tile_table = tileset.tiles.get(tileset.tile_ids.get(tile_layer.0[chunk_offset.x as usize + (chunk_offset.y as usize * CHUNK_SIZE as usize)] as usize).unwrap()).unwrap().data.clone();
            Ok(tile_layer.1.entry(chunk_offset).or_insert_with(move || {
                let table = lua.create_table().unwrap().into_owned();
                table.to_ref().set_metatable(Some({
                    let meta = lua.create_table().unwrap();
                    meta.set("__index", tile_table).unwrap();
                    meta
                }));
                table
            }).clone())
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
        fields.add_field_method_get("x", |_, pos| { Ok(pos.x) });
        fields.add_field_method_get("y", |_, pos| { Ok(pos.y) });
        fields.add_field_method_get("world", |_, pos| { Ok(pos.world.to_string()) });
    }
    fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_method("is_loaded", |lua, pos, ()| {
            let server = lua.app_data_ref::<ServerPtr>().ok_or(Error::runtime("this method can only be used on server is running"))?;
            let worlds = server.worlds.borrow();
            match worlds.get(&pos.world) {
                Some(world) => Ok(world.chunks.contains_key(&pos.align_to_tile().to_chunk_position().0)),
                None => Ok(false)
            }
        });
        methods.add_method("load", |lua, pos, ()| {
            let server = lua.app_data_ref::<ServerPtr>().ok_or(Error::runtime("this method can only be used on server is running"))?;
            let mut worlds = server.worlds.borrow_mut();
            let world = worlds.entry(pos.world.clone()).or_insert_with(|| World::new());
            world.chunks.entry(pos.align_to_tile().to_chunk_position().0).or_insert_with(|| Chunk::new());
            Ok(())
        });
    }
}
impl Position {
    pub fn align_to_tile(&self) -> TilePosition {
        TilePosition {
            x: self.x as i32,
            y: self.y as i32,
        }
    }
}

pub struct Collider {
    pub(crate) aabb: AABB,
    pub(crate) mask: u32,
}
pub struct Entity {
    type_id: ImmutableString,
    uuid: Uuid,
    position: RefCell<Position>,
    removed: AtomicBool,
}
impl Entity {
    pub fn new(lua: &Lua, id: ImmutableString, position: Position) -> mlua::Result<OwnedAnyUserData> {
        let server = lua.app_data_ref::<ServerPtr>().ok_or(Error::runtime("this method can only be used on server is running"))?;
        let mut worlds = server.worlds.borrow_mut();
        let chunk = worlds.get_mut(&position.world).ok_or(Error::runtime("world not loaded"))?.chunks.get_mut(&position.align_to_tile().to_chunk_position().0).ok_or(Error::runtime("chunk not loaded"))?;
        let table = lua.create_table().unwrap().into_owned();
        let metatable = lua.create_table().unwrap().into_owned();
        metatable.to_ref().set("__index", server.entity_registry.entities.get(&id).unwrap().data.to_ref()).unwrap();
        table.to_ref().set_metatable(Some(metatable.to_ref()));
        let uuid = Uuid::new_v4();
        let user_data = lua.create_userdata(Entity {
            type_id: id,
            uuid,
            position: RefCell::new(position.clone()),
            removed: AtomicBool::new(false),
        }).unwrap().into_owned();
        user_data.to_ref().set_nth_user_value(2, table).unwrap();
        server.entities.borrow_mut().insert(uuid, user_data.clone());
        chunk.entities.insert(uuid, user_data.clone());
        Ok(user_data)
    }
}
impl UserData for Entity {
    fn add_fields<'lua, F: UserDataFields<'lua, Self>>(fields: &mut F) {
        fields.add_field_method_get("position", |lua, entity| {
            Ok(entity.position.borrow().clone())
        });
        fields.add_field_function_set("position", |lua, entity_obj, position: Position| {
            let entity_obj = entity_obj.into_owned();
            let entity = entity_obj.clone();
            let entity = entity.borrow::<Entity>().unwrap();

            let server = lua.app_data_ref::<ServerPtr>().ok_or(Error::runtime("this method can only be used on server is running"))?;

            let old_position = entity.position.borrow().clone();
            let old_chunk_position = old_position.align_to_tile().to_chunk_position().0;
            let new_chunk_position = position.align_to_tile().to_chunk_position().0;
            if old_position.world != position.world || old_chunk_position != new_chunk_position {
                let mut worlds = server.worlds.borrow_mut();
                {
                    let old_world = worlds.get_mut(&old_position.world).unwrap();
                    old_world.chunks.get_mut(&old_chunk_position).unwrap().entities.remove(&entity.uuid);
                }
                {
                    let new_world = worlds.get_mut(&position.world).unwrap();
                    new_world.chunks.get_mut(&new_chunk_position).unwrap().entities.insert(entity.uuid, entity_obj);
                }
            }
            *entity.position.borrow_mut() = position;
            Ok(())
        });
        fields.add_field_method_get("id", |lua, entity| {
            Ok(entity.uuid.to_string())
        });
        fields.add_field_method_get("removed", |lua, entity| {
            Ok(entity.removed.load(Ordering::SeqCst))
        });
        fields.add_field_function_get("data", |lua, entity: AnyUserData| {
            entity.nth_user_value::<Value>(2)
        })
    }
    fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_method("remove", |lua, entity, args: ()| {
            let mut server = lua.app_data_ref::<ServerPtr>().ok_or(Error::runtime("this method can only be used on server is running"))?;
            server.entities.borrow_mut().remove(&entity.uuid);
            let position = entity.position.borrow().clone();
            let chunk = position.align_to_tile().to_chunk_position().0;
            let mut worlds = server.worlds.borrow_mut();
            let mut chunk = worlds.get_mut(&position.world).unwrap().chunks.get_mut(&chunk).unwrap();
            chunk.entities.remove(&entity.uuid);
            entity.removed.load(Ordering::SeqCst);
            Ok(())
        });
        methods.add_method("get_collider", |lua, entity, name: String| {
            let mut server = lua.app_data_ref::<ServerPtr>().ok_or(Error::runtime("this method can only be used on server is running"))?;
            let aabb = server.entity_registry.entities.get(&entity.type_id).unwrap().colliders.get::<ImmutableString>(&name.into()).unwrap().aabb;
            Ok(LuaAABB {
                aabb: aabb.offset(&*entity.position.borrow()),
                world: entity.position.borrow().world.clone(),
            })
        });
    }
}

pub struct LuaAABB {
    aabb: AABB,
    world: ImmutableString,
}
impl UserData for LuaAABB {
    fn add_fields<'lua, F: UserDataFields<'lua, Self>>(fields: &mut F) {
        fields.add_field_method_get("x", |_, aabb| Ok(aabb.aabb.x));
        fields.add_field_method_get("y", |_, aabb| Ok(aabb.aabb.y));
        fields.add_field_method_get("w", |_, aabb| Ok(aabb.aabb.w));
        fields.add_field_method_get("h", |_, aabb| Ok(aabb.aabb.h));
    }
    fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_method("position", |_, aabb, ()| {
            Ok(Position {
                x: aabb.aabb.x,
                y: aabb.aabb.y,
                world: aabb.world.clone(),
            })
        });
        methods.add_method("center", |_, aabb, ()| {
            Ok(Position {
                x: aabb.aabb.x + aabb.aabb.w / 2.,
                y: aabb.aabb.y + aabb.aabb.h / 2.,
                world: aabb.world.clone(),
            })
        });
        methods.add_method("tiles_overlapping", |lua, aabb, ()| {
            let table = lua.create_table().unwrap();
            for position in aabb.aabb.tiles_overlapping() {
                table.push(Position {
                    x: position.x as f64,
                    y: position.y as f64,
                    world: aabb.world.clone(),
                }).unwrap();
            }
            Ok(table)
        });
        methods.add_method("test_collisions", |lua: &Lua, aabb, mask: u32| {
            let mut server = lua.app_data_ref::<ServerPtr>().ok_or(Error::runtime("this method can only be used on server is running"))?;
            let mut collided = false;
            for entity in server.entities.borrow().values() {
                let entity: std::cell::Ref<Entity> = entity.borrow().unwrap();
                let position = entity.position.borrow().clone();
                if position.world != aabb.world {
                    continue;
                }
                let entity_type = server.entity_registry.entities.get(&entity.type_id).unwrap();
                for collider in entity_type.colliders.values() {
                    if (collider.mask & mask != 0) && collider.aabb.offset(&position).collides(aabb.aabb) {
                        collided = true;
                    }
                }
            }
            let world = server.worlds.borrow().get(&aabb.world).unwrap();
            for tile in aabb.aabb.tiles_overlapping() {
                let (chunk_position, chunk_offset) = tile.to_chunk_position();
                let chunk = world.chunks.get(&chunk_position).unwrap();
                for (tileset, tile_layer) in chunk.tile_layers.iter() {
                    let tile_type = server.tile_sets.get(tileset).unwrap().by_id(tile_layer.0[chunk_offset.x as usize + (chunk_offset.y as usize * CHUNK_SIZE as usize)]).unwrap();
                    if tile_type.collision_mask & mask != 0 {
                        collided = true;
                    }
                }
            }
            Ok(collided)
        });
        methods.add_method("test_sweep", |lua: &Lua, aabb, (mask, target_position): (u32, Position)| {
            if target_position.world != aabb.world {
                return Err(Error::runtime("mismatched world"));
            }
            let mut server = lua.app_data_ref::<ServerPtr>().ok_or(Error::runtime("this method can only be used on server is running"))?;
            let mut collision_time = 1.;
            for entity in server.entities.borrow().values() {
                let entity: std::cell::Ref<Entity> = entity.borrow().unwrap();
                let position = entity.position.borrow().clone();
                if position.world != aabb.world {
                    continue;
                }
                let entity_type = server.entity_registry.entities.get(&entity.type_id).unwrap();
                for collider in entity_type.colliders.values() {
                    if collider.mask & mask != 0 {
                        collision_time = collision_time.min(collider.aabb.offset(&position).sweep(&aabb.aabb, target_position.clone()).1);
                    }
                }
            }
            //todo: tiles
            Ok(collision_time)
        });
    }
}