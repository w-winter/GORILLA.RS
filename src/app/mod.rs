mod assets;
mod gameplay;
mod presentation;
mod round;

#[derive(Clone, Debug)]
pub(crate) struct Player {
    pub(crate) name: String,
    pub(crate) score: u32,
}

#[derive(Clone, Debug)]
pub(crate) struct GameConfig {
    pub(crate) number_of_players: usize,
    pub(crate) player_names: [String; 2],
    pub(crate) target_score: u32,
    pub(crate) gravity: f32,
}

pub(crate) async fn run() {
    let audio = assets::init_audio().await;
    let visuals = assets::init_visuals();

    presentation::intro_screen(&audio, &visuals).await;

    loop {
        let (config, view_intro) = presentation::setup_screen(&visuals).await;
        if view_intro {
            presentation::gorilla_intro_view(&config.player_names, &audio, &visuals).await;
        }

        gameplay::play_match(config, &audio, &visuals).await;
    }
}
