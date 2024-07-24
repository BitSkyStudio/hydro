register_tileset("main", {
    tiles = {
        {
            id = "air",
            aaa = 5,
            collision_mask = 0
        },
        {
            id = "stone",
            collision_mask = 1
        }
    }
})

register_entity("player", {
    test = "abc",
    colliders={
        main={
            x=0,
            y=0,
            w=0.3,
            h=0.8,
            mask=1
        }
    }
})

register_event("start", function()
    pos1 = pos(0, 0, "lobby")
    print(pos1:is_loaded())
    pos1:load()
    print(pos1:is_loaded())
    --tileset("main"):set_at(pos1, "stone")
    print(tileset("main"):get_data_at(pos1).aaa)
    print(spawn("player", pos1):get_collider("main"):tiles_overlapping())
    print("here")
    print(type(pos1))
end)

register_event("tick", function()
    --print(ticks_passed..":"..seconds_passed)
end)