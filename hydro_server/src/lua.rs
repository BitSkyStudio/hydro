use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::TryRecvError;

use immutable_string::ImmutableString;
use mlua::{AnyUserData, Error, FromLua, Function, Lua, OwnedAnyUserData, OwnedFunction, Table, UserData, UserDataFields, UserDataMethods, Value};
use tiled::{ChunkData, LayerType, Properties, PropertyValue, TileLayer};
use uuid::Uuid;

use hydro_common::{EntityAddMessage, MessageC2S, MessageS2C, MouseButton, PlayerInputMessage, RunningAnimation};
use hydro_common::pos::{CHUNK_SIZE, ChunkPosition, TilePosition, Vec2};

use crate::{ChunkTileLayer, ClientConnection, Server, ServerPtr};
use crate::util::AABB;

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
        let server = lua.app_data_ref::<ServerPtr>().ok_or(Error::runtime("this method can only be used on running server"))?;
        let id = server.entities.borrow().get(&uuid).cloned();
        Ok(id)
    }).unwrap()).unwrap();
    globals.set("get_client", lua.create_function(|lua, (id, ): (String,)| {
        let uuid = Uuid::parse_str(id.as_str()).map_err(|_| Error::runtime("malformed uuid"))?;
        let server = lua.app_data_ref::<ServerPtr>().ok_or(Error::runtime("this method can only be used on running server"))?;
        let id = server.clients.borrow().get(&uuid).cloned();
        Ok(id)
    }).unwrap()).unwrap();
    globals.set("get_clients", lua.create_function(|lua, ()| {
        let server = lua.app_data_ref::<ServerPtr>().ok_or(Error::runtime("this method can only be used on running server"))?;
        let clients = server.clients.borrow();
        Ok(clients.iter().map(|(key,value)|(key.to_string(), value.clone())).collect::<HashMap<String, OwnedAnyUserData>>())
    }).unwrap()).unwrap();
    globals.set("schedule", lua.create_function(|lua, (task, after): (OwnedFunction,f64)| {
        let server = lua.app_data_ref::<ServerPtr>().ok_or(Error::runtime("this method can only be used on running server"))?;
        server.schedule_task(move |server|{
            let reschedule = task.call::<(),Value>(()).unwrap();
            match reschedule{
                Value::Nil => None,
                Value::Integer(value) => Some(value as f64),
                Value::Number(value) => Some(value as f64),
                _ => panic!(),
            }
        }, after);
        Ok(())
    }).unwrap()).unwrap();

    globals.set("load_map_into_world", lua.create_function(|lua, (map, world, tilesets): (String, String, Table)|{
        let server = lua.app_data_ref::<ServerPtr>().ok_or(Error::runtime("this method can only be used on running server"))?;
        let world: ImmutableString = world.into();
        let mut map = tiled::Loader::new().load_tmx_map(map).unwrap();
        for layer in map.layers() {
            match layer.layer_type(){
                LayerType::Tiles(tiles) => {
                    if let Some(tileset) = tilesets.get::<_, String>(layer.name.as_str()).ok().map(|tileset|Into::<ImmutableString>::into(tileset)) {
                        match tiles{
                            TileLayer::Finite(finite) => {}
                            TileLayer::Infinite(infinite) => {
                                for (pos, chunk) in infinite.chunks() {
                                    for x in 0..ChunkData::WIDTH {
                                        for y in 0..ChunkData::HEIGHT {
                                            let position = TilePosition {
                                                x: pos.0 * ChunkData::WIDTH as i32 + x as i32,
                                                y: pos.1 * ChunkData::HEIGHT as i32 + y as i32,
                                            };
                                            if let Some(tile) = chunk.get_tile(x as i32, y as i32) {
                                                server.set_tile(position, world.clone(), tileset.clone(), format!("{}", tile.id()).into())?;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                LayerType::Objects(objects) => {
                    for object in objects.objects() {
                        let id = match object.properties.get("Class").unwrap(){
                            PropertyValue::StringValue(string) => string,
                            _ => panic!(),
                        };
                        let mut entity = Entity::new(lua, id.as_str().into(), Position{
                            x: object.x as f64,
                            y: object.y as f64,
                            world: world.clone(),
                        }).unwrap();
                        load_tiled_properties_into_lua_table(lua, &entity.to_ref().nth_user_value::<Table>(2).unwrap(), &object.properties);
                    }
                }
                LayerType::Image(_) => {}
                LayerType::Group(_) => {}
            }
        }
        Ok(())
    }).unwrap()).unwrap();

    {
        let keys = lua.create_table().unwrap();
        keys.set("0", 0x30).unwrap();
        keys.set("1", 0x31).unwrap();
        keys.set("2", 0x32).unwrap();
        keys.set("3", 0x33).unwrap();
        keys.set("4", 0x34).unwrap();
        keys.set("5", 0x35).unwrap();
        keys.set("6", 0x36).unwrap();
        keys.set("7", 0x37).unwrap();
        keys.set("8", 0x38).unwrap();
        keys.set("9", 0x39).unwrap();
        keys.set("a", 0x41).unwrap();
        keys.set("b", 0x42).unwrap();
        keys.set("c", 0x43).unwrap();
        keys.set("d", 0x44).unwrap();
        keys.set("e", 0x45).unwrap();
        keys.set("f", 0x46).unwrap();
        keys.set("g", 0x47).unwrap();
        keys.set("h", 0x48).unwrap();
        keys.set("i", 0x49).unwrap();
        keys.set("j", 0x4a).unwrap();
        keys.set("k", 0x4b).unwrap();
        keys.set("l", 0x4c).unwrap();
        keys.set("m", 0x4d).unwrap();
        keys.set("n", 0x4e).unwrap();
        keys.set("o", 0x4f).unwrap();
        keys.set("p", 0x50).unwrap();
        keys.set("q", 0x51).unwrap();
        keys.set("r", 0x52).unwrap();
        keys.set("s", 0x53).unwrap();
        keys.set("t", 0x54).unwrap();
        keys.set("u", 0x55).unwrap();
        keys.set("v", 0x56).unwrap();
        keys.set("w", 0x57).unwrap();
        keys.set("x", 0x58).unwrap();
        keys.set("y", 0x59).unwrap();
        keys.set("z", 0x5a).unwrap();
        keys.set("right", 0xff53).unwrap();
        keys.set("left", 0xff51).unwrap();
        keys.set("down", 0xff54).unwrap();
        keys.set("up", 0xff52).unwrap();
        keys.set("lshift", 0xffe1).unwrap();
        keys.set("rshift", 0xffe2).unwrap();
        globals.set("keys", keys).unwrap();
    }
}
pub fn load_tiled_properties_into_lua_table(lua: &Lua, table: &Table, properties: &Properties){
    for (name, property) in properties{
        match property{
            PropertyValue::BoolValue(value) => {
                table.set(name.as_str(), *value).unwrap();
            }
            PropertyValue::FloatValue(value) => {
                table.set(name.as_str(), *value).unwrap();
            }
            PropertyValue::IntValue(value) => {
                table.set(name.as_str(), *value).unwrap();
            }
            PropertyValue::StringValue(value) => {
                table.set(name.as_str(), value.as_str()).unwrap();
            }
            PropertyValue::ClassValue { property_type, properties } => {
                let class = lua.create_table().unwrap();
                class.set("type", property_type.as_str()).unwrap();
                load_tiled_properties_into_lua_table(lua, &class, properties);
                table.set(name.as_str(), class).unwrap();
            }
            _ => unimplemented!()
        }
    }
}
pub struct LuaTileSet {
    tileset: ImmutableString,
}
impl UserData for LuaTileSet {
    fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_method("get_at", |lua, tile_map, (pos, ): (Position,)| {
            let server = lua.app_data_ref::<ServerPtr>().ok_or(Error::runtime("this method can only be used on running server"))?;
            let (chunk_position, chunk_offset) = pos.align_to_tile().to_chunk_position();
            let chunk = server.get_chunk(chunk_position, pos.world);
            let tile_id = match chunk.tile_layers.get(&tile_map.tileset) {
                Some(tileset) => {
                    tileset.0[chunk_offset.index()]
                }
                None => {
                    0
                }
            };
            let tileset = server.tile_sets.get(&tile_map.tileset).ok_or(Error::runtime("tileset doesn't exist"))?;
            Ok(tileset.tiles.get(&tileset.tile_ids[tile_id as usize]).unwrap().data.clone())
        });
        methods.add_method("set_at", |lua, tile_map, (pos, id): (Position, String)| {
            let server = lua.app_data_ref::<ServerPtr>().ok_or(Error::runtime("this method can only be used on running server"))?;
            let tile_pos = pos.align_to_tile();
            server.set_tile(tile_pos, pos.world.clone(), tile_map.tileset.clone(), id.into())
        });
        methods.add_method("get_data_at", |lua, tile_map, (pos, ): (Position,)| {
            let server = lua.app_data_ref::<ServerPtr>().ok_or(Error::runtime("this method can only be used on running server"))?;
            let (chunk_position, chunk_offset) = pos.align_to_tile().to_chunk_position();
            let mut chunk = server.get_chunk(chunk_position, pos.world);
            let tile_layer = chunk.tile_layers.entry(tile_map.tileset.clone()).or_insert_with(|| ChunkTileLayer::new());
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
        fields.add_field_method_get("chunk_x", |_, pos| { Ok(pos.align_to_tile().to_chunk_position().0.x) });
        fields.add_field_method_get("chunk_y", |_, pos| { Ok(pos.align_to_tile().to_chunk_position().0.y) });
    }
    fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_method("move", |_, pos, (x,y): (f64,f64)|{
            Ok(Position{
                x: pos.x + x,
                y: pos.y + y,
                world: pos.world.clone(),
            })
        });
        methods.add_method("distance", |_, pos, other: Position|{
            if pos.world != other.world{
                return Err(Error::runtime(format!("mismatched world {}:{}", pos.world, other.world)));
            }
            Ok(((pos.x-other.x).powi(2)+(pos.y-other.y).powi(2)).sqrt())
        });
        methods.add_method("add", |_, pos, other: Position|{
            if pos.world != other.world{
                return Err(Error::runtime(format!("mismatched world {}:{}", pos.world, other.world)));
            }
            Ok(Position{
                x: pos.x + other.x,
                y: pos.y + other.y,
                world: pos.world.clone()
            })
        });
        methods.add_method("sub", |_, pos, other: Position|{
            if pos.world != other.world{
                return Err(Error::runtime(format!("mismatched world {}:{}", pos.world, other.world)));
            }
            Ok(Position{
                x: pos.x - other.x,
                y: pos.y - other.y,
                world: pos.world.clone()
            })
        });
        methods.add_method("scl", |_, pos, scalar: f64|{
            Ok(Position{
                x: pos.x * scalar,
                y: pos.y * scalar,
                world: pos.world.clone()
            })
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
#[derive(Clone)]
pub struct EntityAnimation {
    begin_time: u32,
    animation: ImmutableString,
}
impl EntityAnimation {
    pub fn running_for(&self, server: &Server) -> f64 {
        (server.ticks_passed.get() - self.begin_time) as f64 / Server::TPS as f64
    }
}
pub struct Entity {
    pub type_id: ImmutableString,
    pub uuid: Uuid,
    pub position: RefCell<Position>,
    removed: AtomicBool,
    animation: RefCell<EntityAnimation>,
}
impl Entity {
    pub fn new(lua: &Lua, id: ImmutableString, position: Position) -> mlua::Result<OwnedAnyUserData> {
        let server = lua.app_data_ref::<ServerPtr>().ok_or(Error::runtime("this method can only be used on running server"))?;
        let mut chunk = server.get_chunk(position.align_to_tile().to_chunk_position().0, position.world.clone());
        let table = lua.create_table().unwrap().into_owned();
        table.to_ref().set_metatable(Some(server.entity_registry.entities.get(&id).unwrap().data_metatable.to_ref()));
        let uuid = Uuid::new_v4();
        let user_data = lua.create_userdata(Entity {
            type_id: id,
            uuid,
            position: RefCell::new(position.clone()),
            removed: AtomicBool::new(false),
            animation: RefCell::new(EntityAnimation {
                animation: "default".into(),
                begin_time: server.ticks_passed.get(),
            }),
        }).unwrap().into_owned();
        user_data.to_ref().set_nth_user_value(2, table).unwrap();
        server.entities.borrow_mut().insert(uuid, user_data.clone());
        chunk.entities.insert(uuid, user_data.clone());
        for viewer in chunk.viewers.borrow().values(){
            let _ = viewer.borrow::<Client>().unwrap().connection.sender.send(MessageS2C::AddEntity(user_data.borrow::<Entity>().unwrap().create_add_message(&server)));
        }
        Ok(user_data)
    }
    pub fn create_add_message(&self, server: &Server) -> EntityAddMessage {
        let position = self.position.borrow();
        let animation = self.animation.borrow();
        EntityAddMessage {
            position: Vec2 { x: position.x, y: position.y },
            entity_type: self.type_id.to_string(),
            uuid: self.uuid,
            animation: RunningAnimation {
                id: animation.animation.to_string(),
                time: animation.running_for(server) as f32,
            },
        }
    }
    fn sync_animations(&self, server: &Server) {
        let position = self.position.borrow();
        let animation = self.animation.borrow();
        for viewer in server.get_chunk(position.align_to_tile().to_chunk_position().0, position.world.clone()).viewers.borrow().values() {
            let _ = viewer.borrow::<Client>().unwrap().connection.sender.send(MessageS2C::UpdateEntityAnimation(self.uuid, RunningAnimation { id: animation.animation.to_string(), time: animation.running_for(server) as f32 }));
        }
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

            let server = lua.app_data_ref::<ServerPtr>().ok_or(Error::runtime("this method can only be used on running server"))?;

            let old_position = entity.position.borrow().clone();
            let old_chunk_position = old_position.align_to_tile().to_chunk_position().0;
            let new_chunk_position = position.align_to_tile().to_chunk_position().0;
            if old_position.world != position.world || old_chunk_position != new_chunk_position {
                let old_viewers: HashSet<Uuid> = {
                    let mut old_chunk = server.get_chunk(old_chunk_position, old_position.world);
                    old_chunk.entities.remove(&entity.uuid);
                    let v = old_chunk.viewers.borrow().keys().cloned().collect();
                    v
                };
                let new_viewers: HashSet<Uuid> = {
                    let mut new_chunk = server.get_chunk(new_chunk_position, position.world.clone());
                    new_chunk.entities.insert(entity.uuid, entity_obj);
                    let v = new_chunk.viewers.borrow().keys().cloned().collect();
                    v
                };
                for new_viewer in new_viewers.difference(&old_viewers) {
                    server.try_send_message_to(*new_viewer, MessageS2C::AddEntity(entity.create_add_message(&server)));
                }
                for viewer in new_viewers.intersection(&old_viewers) {
                    server.try_send_message_to(*viewer, MessageS2C::UpdateEntityPosition(entity.uuid, Vec2 { x: position.x, y: position.y }));
                }
                for old_viewer in old_viewers.difference(&new_viewers) {
                    server.try_send_message_to(*old_viewer, MessageS2C::RemoveEntity(entity.uuid.clone()));
                }
            } else {
                for viewer in server.get_chunk(position.align_to_tile().to_chunk_position().0, position.world.clone()).viewers.borrow().keys() {
                    server.try_send_message_to(*viewer, MessageS2C::UpdateEntityPosition(entity.uuid, Vec2 { x: position.x, y: position.y }));
                }
            }
            *entity.position.borrow_mut() = position;
            Ok(())
        });
        fields.add_field_method_set("animation", |lua, entity, animation: String| {
            let server = lua.app_data_ref::<ServerPtr>().ok_or(Error::runtime("this method can only be used on running server"))?;
            let animation_id = animation.into();
            if server.entity_registry.entities.get(&entity.type_id).unwrap().animations.contains_key(&animation_id) {
                return Err(Error::runtime("animation doesn't exist"))?;
            }
            {
                let mut animation = entity.animation.borrow_mut();
                animation.animation = animation_id;
                animation.begin_time = server.ticks_passed.get();
            }
            entity.sync_animations(&server);
            Ok(())
        });
        fields.add_field_method_get("animation", |lua, entity|{
            Ok(entity.animation.borrow().animation.to_string())
        });
        fields.add_field_method_set("animation_time", |lua, entity, time: f64|{
            let server = lua.app_data_ref::<ServerPtr>().ok_or(Error::runtime("this method can only be used on running server"))?;
            {
                let mut animation = entity.animation.borrow_mut();
                animation.begin_time = server.ticks_passed.get()-(time*Server::TPS as f64) as u32;
            }
            entity.sync_animations(&server);
            Ok(())
        });
        fields.add_field_method_get("animation_time", |lua, entity|{
            let server = lua.app_data_ref::<ServerPtr>().ok_or(Error::runtime("this method can only be used on running server"))?;
            Ok((server.ticks_passed.get()-entity.animation.borrow().begin_time) as f64/Server::TPS as f64)
        });
        fields.add_field_method_get("id", |lua, entity| {
            Ok(entity.uuid.to_string())
        });
        fields.add_field_method_get("removed", |lua, entity| {
            Ok(entity.removed.load(Ordering::SeqCst))
        });
    }
    fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_method("remove", |lua, entity, args: ()| {
            let server = lua.app_data_ref::<ServerPtr>().ok_or(Error::runtime("this method can only be used on running server"))?;
            server.entities.borrow_mut().remove(&entity.uuid);
            let position = entity.position.borrow().clone();
            let chunk = position.align_to_tile().to_chunk_position().0;
            let mut chunk = server.get_chunk(chunk, position.world);
            chunk.entities.remove(&entity.uuid);
            entity.removed.load(Ordering::SeqCst);
            for viewer in chunk.viewers.borrow().keys() {
                server.try_send_message_to(*viewer, MessageS2C::RemoveEntity(entity.uuid));
            }
            Ok(())
        });
        methods.add_method("get_collider", |lua, entity, name: String| {
            let server = lua.app_data_ref::<ServerPtr>().ok_or(Error::runtime("this method can only be used on running server"))?;
            let aabb = server.entity_registry.entities.get(&entity.type_id).unwrap().colliders.get::<ImmutableString>(&name.into()).unwrap().aabb;
            Ok(LuaAABB {
                aabb: aabb.offset(&*entity.position.borrow()),
                world: entity.position.borrow().world.clone(),
            })
        });
        methods.add_meta_function("__index", |lua, (entity, key): (AnyUserData, Value)| {
            entity.nth_user_value::<Table>(2).unwrap().get::<Value, Value>(key)
        });
        methods.add_meta_function("__newindex", |lua, (entity, key, value): (AnyUserData, Value, Value)| {
            entity.nth_user_value::<Table>(2).unwrap().set(key, value)
        });
    }
}

#[derive(Clone)]
pub struct LuaAABB {
    aabb: AABB,
    world: ImmutableString,
}
impl LuaAABB{
    pub fn collides(&self, server: &Server, mask: u32) -> bool{
        for tile in self.aabb.tiles_overlapping() {
            let (chunk_position, chunk_offset) = tile.to_chunk_position();
            let chunk = server.get_chunk(chunk_position, self.world.clone());
            for (tileset, tile_layer) in chunk.tile_layers.iter() {
                let tile_type = server.tile_sets.get(tileset).unwrap().by_id(tile_layer.0[chunk_offset.x as usize + (chunk_offset.y as usize * CHUNK_SIZE as usize)]).unwrap();
                if tile_type.collision_mask & mask != 0 {
                    return true;
                }
            }
        }
        false
    }
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
            let mut server = lua.app_data_ref::<ServerPtr>().ok_or(Error::runtime("this method can only be used on running server"))?;
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
            collided |= aabb.collides(&server, mask);
            Ok(collided)
        });
        methods.add_method("test_sweep", |lua: &Lua, aabb, (mask, target_position): (u32, Position)| {
            if target_position.world != aabb.world {
                return Err(Error::runtime("mismatched world"));
            }
            let mut server = lua.app_data_ref::<ServerPtr>().ok_or(Error::runtime("this method can only be used on running server"))?;
            let mut collision_time: f64 = 1.;
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
            if aabb.collides(&server, mask){
                collision_time = 0.;
            }
            let movement = Vec2{x: (target_position.x-aabb.aabb.x)/5., y: (target_position.y-aabb.aabb.y)/5.};
            for i in 0..5 {
                let mut aabb = aabb.clone();
                aabb.aabb.x += movement.x*(i+1) as f64;
                aabb.aabb.y += movement.y*(i+1) as f64;
                if aabb.collides(&server, mask){
                    collision_time = collision_time.min(i as f64/5.);
                    break;
                }
            }
            //todo: this is stupid
            Ok((collision_time, Position{
                x: aabb.aabb.x + (target_position.x-aabb.aabb.x) * collision_time,
                y: aabb.aabb.y + (target_position.y-aabb.aabb.y) * collision_time,
                world: aabb.world.clone(),
            }))
        });
    }
}

pub struct Client {
    pub(crate) connection: ClientConnection,
    camera: ClientCameraType,
    pub(crate) closed: bool,
    pub id: Uuid,
    player_input: PlayerInputMessage,
}
impl Client {
    pub fn new(lua: &Lua, connection: ClientConnection) -> mlua::Result<OwnedAnyUserData> {
        let user_data = lua.create_userdata(Client {
            connection,
            camera: ClientCameraType::None,
            id: Uuid::new_v4(),
            closed: false,
            player_input: PlayerInputMessage::default(),
        }).unwrap().into_owned();
        let table = lua.create_table().unwrap().into_owned();
        user_data.to_ref().set_nth_user_value(2, table).unwrap();
        Ok(user_data)
    }
    pub fn set_camera(&mut self, server: &Server, lua_ref: OwnedAnyUserData, new_camera: ClientCameraType) {
        let old = self.camera.get_loaded_chunks();
        let new = new_camera.get_loaded_chunks();
        if old.0 == new.0 {
            for old_chunk_position in old.1.difference(&new.1) {
                let old_chunk = server.get_chunk(*old_chunk_position, old.0.clone());
                old_chunk.viewers.borrow_mut().remove(&self.id);
                self.connection.sender.send(MessageS2C::UnloadChunk(*old_chunk_position, old_chunk.entities.keys().cloned().collect())).expect("TODO: panic message");
            }
            for new_chunk_position in new.1.difference(&old.1) {
                let new_chunk = server.get_chunk(*new_chunk_position, new.0.clone());
                new_chunk.viewers.borrow_mut().insert(self.id, lua_ref.clone());
                let _ = self.connection.sender.send(MessageS2C::LoadChunk(*new_chunk_position,
                                                                          new_chunk.tile_layers.iter().map(|(key, value)| (key.to_string(), value.0.clone())).collect(),
                                                                          new_chunk.entities.values().map(|entity| entity.borrow::<Entity>().unwrap().create_add_message(server)).collect(),
                ));
            }
        } else {
            for old_chunk_position in old.1 {
                let old_chunk = server.get_chunk(old_chunk_position, old.0.clone());
                old_chunk.viewers.borrow_mut().remove(&self.id);
                let _ = self.connection.sender.send(MessageS2C::UnloadChunk(old_chunk_position, old_chunk.entities.keys().cloned().collect()));
            }
            for new_chunk_position in new.1 {
                let new_chunk = server.get_chunk(new_chunk_position, new.0.clone());
                new_chunk.viewers.borrow_mut().insert(self.id, lua_ref.clone());
                let _ = self.connection.sender.send(MessageS2C::LoadChunk(new_chunk_position,
                                                                          new_chunk.tile_layers.iter().map(|(key, value)| (key.to_string(), value.0.clone())).collect(),
                                                                          new_chunk.entities.values().map(|entity| entity.borrow::<Entity>().unwrap().create_add_message(server)).collect(),
                ));
            }
        }
        let camera_position = new_camera.get_position();
        if let Some(camera_position) = camera_position {
            let _ = self.connection.sender.send(MessageS2C::CameraInfo(Vec2 { x: camera_position.x, y: camera_position.y }));
        }
        self.camera = new_camera;
    }
    pub fn tick(&mut self, server: &Server, lua_ref: OwnedAnyUserData) {
        self.player_input = PlayerInputMessage::default();
        loop {
            match self.connection.receiver.try_recv() {
                Ok(message) => {
                    match message {
                        MessageC2S::PlayerInput(mut player_input) => {
                            self.player_input.keys_down = player_input.keys_down;
                            self.player_input.keys_pressed.extend(player_input.keys_pressed.drain());
                            self.player_input.keys_released.extend(player_input.keys_released.drain());
                            self.player_input.buttons_down = player_input.buttons_down;
                            self.player_input.buttons_pressed.extend(player_input.buttons_pressed.drain());
                            self.player_input.buttons_released.extend(player_input.buttons_released.drain());
                            self.player_input.mouse_position = player_input.mouse_position;
                        }
                    }
                }
                Err(TryRecvError::Disconnected) => {
                    self.closed = true;
                    break;
                }
                Err(TryRecvError::Empty) => break,
            }
        }
        match &self.camera {
            ClientCameraType::Entity(_) => {
                self.set_camera(server, lua_ref.clone(), self.camera.clone())
            }
            _ => {}
        }
    }
}
impl UserData for Client {
    fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_function("set_camera_position", |lua, (client, pos): (OwnedAnyUserData, Position)| {
            let server = lua.app_data_ref::<ServerPtr>().ok_or(Error::runtime("this method can only be used on running server"))?;
            client.borrow_mut::<Client>().unwrap().set_camera(&server, client.clone(), ClientCameraType::Position(pos));
            Ok(())
        });
        methods.add_function("set_camera_entity", |lua, (client, entity): (OwnedAnyUserData, OwnedAnyUserData)| {
            let server = lua.app_data_ref::<ServerPtr>().ok_or(Error::runtime("this method can only be used on running server"))?;
            client.borrow_mut::<Client>().unwrap().set_camera(&server, client.clone(), ClientCameraType::Entity(entity));
            Ok(())
        });
        methods.add_function("remove_camera", |lua, client: OwnedAnyUserData| {
            let server = lua.app_data_ref::<ServerPtr>().ok_or(Error::runtime("this method can only be used on running server"))?;
            client.borrow_mut::<Client>().unwrap().set_camera(&server, client.clone(), ClientCameraType::None);
            Ok(())
        });
        methods.add_method("is_key_down", |lua, client, key: u16|{
            Ok(client.player_input.keys_down.contains(&key))
        });
        methods.add_method("is_key_pressed", |lua, client, key: u16|{
            Ok(client.player_input.keys_pressed.contains(&key))
        });
        methods.add_method("is_key_released", |lua, client, key: u16|{
            Ok(client.player_input.keys_released.contains(&key))
        });
        methods.add_method("is_button_down", |lua, client, button: u8|{
            Ok(client.player_input.buttons_down.contains(&MouseButton::from(button)))
        });
        methods.add_method("is_button_pressed", |lua, client, button: u8|{
            Ok(client.player_input.buttons_pressed.contains(&MouseButton::from(button)))
        });
        methods.add_method("is_button_released", |lua, client, button: u8|{
            Ok(client.player_input.buttons_released.contains(&MouseButton::from(button)))
        });
        methods.add_meta_function("__index", |lua, (client, key): (AnyUserData, Value)| {
            client.nth_user_value::<Table>(2).unwrap().get::<Value, Value>(key)
        });
        methods.add_meta_function("__newindex", |lua, (client, key, value): (AnyUserData, Value, Value)| {
            client.nth_user_value::<Table>(2).unwrap().set(key, value)
        });
    }
    fn add_fields<'lua, F: UserDataFields<'lua, Self>>(fields: &mut F) {
        fields.add_field_method_get("mouse_position", |lua, client|{
            let mouse_position = client.player_input.mouse_position;
            Ok(client.camera.get_position().map(|position|{
                Position{world: position.world, x: mouse_position.x as f64, y: mouse_position.y as f64 }
            }))
        });
    }
}
#[derive(Clone)]
pub enum ClientCameraType {
    None,
    Position(Position),
    Entity(OwnedAnyUserData),
}
impl ClientCameraType {
    pub fn get_position(&self) -> Option<Position> {
        match self {
            ClientCameraType::None => return None,
            ClientCameraType::Position(position) => Some(position.clone()),
            ClientCameraType::Entity(entity) => {
                let entity = entity.borrow::<Entity>().unwrap();
                let pos = entity.position.borrow().clone();
                Some(pos)
            }
        }
    }
    pub fn get_loaded_chunks(&self) -> (ImmutableString, HashSet<ChunkPosition>) {
        let position = match self.get_position() {
            Some(position) => position,
            None => return ("".into(), HashSet::new()),
        };
        let base_chunk_position = position.align_to_tile().to_chunk_position().0;
        let load_radius = 4;
        (position.world.clone(), ((-load_radius)..=load_radius).map(|x| ((-load_radius)..=load_radius).map(move |y| ChunkPosition { x: base_chunk_position.x + x, y: base_chunk_position.y + y })).flatten().collect())
    }
}