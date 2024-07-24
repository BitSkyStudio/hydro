use macroquad::prelude::*;
use quad_net::web_socket::WebSocket;

#[macroquad::main("BasicShapes")]
async fn main() {
    //let location = web_sys::window().unwrap().document().unwrap().location().unwrap();
    //let websocket = WebSocket::new(format!("{}://{}/ws", if location.protocol().unwrap() == "https:" { "wss" } else { "ws" }, location.host().unwrap()).as_str()).unwrap();
    let websocket = WebSocket::connect("ws://localhost:8080/ws").unwrap();
    loop {
        clear_background(RED);

        draw_line(40.0, 40.0, 100.0, 200.0, 15.0, BLUE);
        draw_rectangle(screen_width() / 2.0 - 60.0, 100.0, 120.0, 60.0, GREEN);
        draw_circle(screen_width() - 30.0, screen_height() - 30.0, 15.0, YELLOW);

        draw_text("IT WORKS!", 20.0, 20.0, 30.0, DARKGRAY);

        next_frame().await
    }
}