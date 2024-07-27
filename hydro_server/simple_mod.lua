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
    pos1 = pos(0, 0, "lobby")
    tileset("main"):set_at(pos1, "stone")
    tileset("main"):set_at(pos(1, 1, "lobby"), "stone")
    print(tileset("main"):get_data_at(pos1).aaa)
    print(spawn("player", pos1):get_collider("main"):tiles_overlapping())
    print("here")
    print(type(pos1))
end)

register_event("tick", function()
    --print(ticks_passed..":"..seconds_passed)
end)