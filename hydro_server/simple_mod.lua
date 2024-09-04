register_tileset("main", {
    tiles = {
        {
            id = "stone",
            collision_mask = 1,
            asset_pos = {x=8,y=5}
        }
    },
    asset = {
        file = "root_tileset0",
        size = 8,
    }
})

register_entity("player", {
    test = "abc",
    colliders={
        main={
            x=0,
            y=0,
            w=0.4,
            h=0.8,
            mask=1
        }
    },
    animations={
        default={
            file="player",
            count=1,
            period=1,
            loop=true,
            flip=false,
        }
    },
    width=0.4,
    height=0.8
})

register_event("start", function()
    local pos1 = pos(0, 0, "lobby")
    tileset("main"):set_at(pos1, "stone")
    tileset("main"):set_at(pos(1, 1, "lobby"), "stone")
    print(tileset("main"):get_data_at(pos1).aaa)

    schedule(function()
        print("here")
        return 1
    end, 3)
end)
register_event("join", function(client)
    local player_entity = spawn("player", pos(-2, 0, "lobby"))
    client:set_camera_entity(player_entity)
    client.controlling_entity = player_entity
end)
register_event("leave", function(client)
    client.controlling_entity:remove()
end)
register_event("tick", function()
    for id,client in pairs(get_clients()) do
        --print("mouse"..client.mouse_position.x..":"..client.mouse_position.y.."-"..client.mouse_position.world)
        new_position = client.controlling_entity.position
        speed = 1/20
        if client:is_key_down(keys.w) then
            new_position = new_position:move(0, -1*speed)
        end
        if client:is_key_down(keys.s) then
            new_position = new_position:move(0, 1*speed)
        end

        _,client.controlling_entity.position = client.controlling_entity:get_collider("main"):test_sweep(1, new_position)
        new_position = client.controlling_entity.position

        if client:is_key_down(keys.a) then
            new_position = new_position:move(-1*speed, 0)
        end
        if client:is_key_down(keys.d) then
            new_position = new_position:move(1*speed, 0)
        end
        _,client.controlling_entity.position = client.controlling_entity:get_collider("main"):test_sweep(1, new_position)
    end
end)
register_event("load_chunk", function(position)
    print(position.chunk_x..":"..position.chunk_y.."-"..position.world)
end)