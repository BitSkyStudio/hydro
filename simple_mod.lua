register_tileset("main", {
    tiles = {
        {
            id = "stone",
            aaa = 5
        }
    }
})

register_entity("player", {
    test = "abc"
})

register_event("start", function()
    pos1 = pos(0, 0, "lobby")
    print(pos1:is_loaded())
    pos1:load()
    print(pos1:is_loaded())
    --tileset("main"):set_at(pos1, "stone")
    print(tileset("main"):get_data_at(pos1).aaa)
    print(spawn("player", pos1).data.test)
    print("here")
end)

register_event("tick", function()
    print("Here2")
    --print(ticks_passed..":"..seconds_passed)
end)