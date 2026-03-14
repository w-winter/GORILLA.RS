mod app;

use gorillas::{SCR_H, SCR_W};
use macroquad::prelude::Conf;

fn window_conf() -> Conf {
    Conf {
        window_title: "Gorillas".to_string(),
        window_width: SCR_W as i32,
        window_height: SCR_H as i32,
        high_dpi: false,
        window_resizable: true,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    app::run().await;
}
