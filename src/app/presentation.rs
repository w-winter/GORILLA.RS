use super::assets::{AudioFx, VisualAssets};
use super::round::{ArmPose, BananaFrame, Building, RoundState};
use super::{GameConfig, Player};
use gorillas::render_ref::{checkpoint_rgba, palette_rgb, render_runtime_scene, RuntimeEffect};
use gorillas::{BOTTOM_LINE, SCR_H, SCR_W};
use macroquad::audio::{play_sound_once, Sound};
use macroquad::prelude::*;

const DOS_TEXT_SIZE: u16 = 16;
const DOS_TEXT_COLUMN_STEP_X: f32 = 8.0;
const SPARKLE_BORDER_STEP_SECONDS: f32 = 1.0 / 30.0;
const BORDER_SPARKLE_PERIOD: usize = 5;

pub(crate) async fn intro_screen(audio: &AudioFx, visuals: &VisualAssets) {
    let mut phase = 0usize;
    let mut phase_elapsed_seconds = 0.0f32;

    #[cfg(not(target_arch = "wasm32"))]
    play_sound_once(&audio.intro_theme);

    #[cfg(target_arch = "wasm32")]
    let mut intro_theme_started = false;

    loop {
        draw_intro_screen_frame(phase, visuals);

        #[cfg(target_arch = "wasm32")]
        {
            if !intro_theme_started && intro_audio_unlock_requested() {
                play_sound_once(&audio.intro_theme);
                intro_theme_started = true;
            } else if intro_theme_started && any_key_pressed() {
                break;
            }
        }

        #[cfg(not(target_arch = "wasm32"))]
        if any_key_pressed() {
            break;
        }

        next_frame().await;
        phase_elapsed_seconds += get_frame_time();
        while phase_elapsed_seconds >= SPARKLE_BORDER_STEP_SECONDS {
            phase_elapsed_seconds -= SPARKLE_BORDER_STEP_SECONDS;
            phase = (phase + 1) % BORDER_SPARKLE_PERIOD;
        }
    }
}

pub(crate) async fn setup_screen(visuals: &VisualAssets) -> (GameConfig, bool) {
    let mut setup_lines = Vec::new();

    let number_of_players =
        prompt_setup_integer(&setup_lines, "Number of players", 2, 1, 2, 1, visuals).await as usize;

    let player1 = prompt_setup_name(&setup_lines, "Name of Player 1", "Player 1", visuals).await;
    setup_lines.push(format!(
        "Name of Player 1 (Default = 'Player 1'): {player1}"
    ));

    let player2 = if number_of_players == 1 {
        "Computer".to_string()
    } else {
        prompt_setup_name(&setup_lines, "Name of Player 2", "Player 2", visuals).await
    };
    setup_lines.push(format!(
        "Name of Player 2 (Default = 'Player 2'): {player2}"
    ));

    let target_score = prompt_setup_integer(
        &setup_lines,
        "Play to how many total points",
        3,
        1,
        99,
        2,
        visuals,
    )
    .await;
    setup_lines.push(format!(
        "Play to how many total points (Default = 3)? {target_score}"
    ));

    let gravity = prompt_setup_float(
        &setup_lines,
        "Gravity in Meters/Sec (Earth = 9.8)",
        9.8,
        visuals,
    )
    .await;
    setup_lines.push(format!("Gravity in Meters/Sec (Earth = 9.8)? {gravity:.1}"));

    let view_intro = prompt_setup_choice(&setup_lines, visuals).await;

    (
        GameConfig {
            number_of_players,
            player_names: [player1, player2],
            target_score,
            gravity,
        },
        view_intro,
    )
}

pub(crate) async fn gorilla_intro_view(
    player_names: &[String; 2],
    audio: &AudioFx,
    visuals: &VisualAssets,
) {
    show_intro_pose(
        player_names,
        ArmPose::ArmsDown,
        ArmPose::ArmsDown,
        1.0,
        visuals,
    )
    .await;

    play_intro_pose_with_sound(
        player_names,
        ArmPose::LeftUp,
        ArmPose::RightUp,
        &audio.intro_riff_1,
        audio.intro_riff_1_duration + 0.3,
        visuals,
    )
    .await;

    play_intro_pose_with_sound(
        player_names,
        ArmPose::RightUp,
        ArmPose::LeftUp,
        &audio.intro_riff_2,
        audio.intro_riff_2_duration + 0.3,
        visuals,
    )
    .await;

    play_intro_pose_with_sound(
        player_names,
        ArmPose::LeftUp,
        ArmPose::RightUp,
        &audio.intro_riff_3,
        audio.intro_riff_3_duration + 0.3,
        visuals,
    )
    .await;

    play_intro_pose_with_sound(
        player_names,
        ArmPose::RightUp,
        ArmPose::LeftUp,
        &audio.intro_riff_4,
        audio.intro_riff_4_duration + 0.3,
        visuals,
    )
    .await;

    for _ in 0..4 {
        play_intro_pose_with_sound(
            player_names,
            ArmPose::LeftUp,
            ArmPose::RightUp,
            &audio.intro_fast,
            audio.intro_fast_duration + 0.1,
            visuals,
        )
        .await;

        play_intro_pose_with_sound(
            player_names,
            ArmPose::RightUp,
            ArmPose::LeftUp,
            &audio.intro_fast,
            audio.intro_fast_duration + 0.1,
            visuals,
        )
        .await;
    }
}

pub(crate) async fn game_over_screen(players: &[Player; 2], visuals: &VisualAssets) {
    let mut phase = 0usize;
    let mut phase_elapsed_seconds = 0.0f32;

    loop {
        draw_game_over_frame(players, phase, visuals);

        if any_key_pressed() {
            return;
        }

        next_frame().await;
        phase_elapsed_seconds += get_frame_time();
        while phase_elapsed_seconds >= SPARKLE_BORDER_STEP_SECONDS {
            phase_elapsed_seconds -= SPARKLE_BORDER_STEP_SECONDS;
            phase = (phase + 1) % BORDER_SPARKLE_PERIOD;
        }
    }
}

pub(crate) fn draw_skyline_reveal_frame(
    round: &RoundState,
    building_count: usize,
    visuals: &VisualAssets,
) {
    visuals.begin_frame();
    clear_background(palette_color(0));

    for building in round.buildings.iter().take(building_count) {
        draw_building_for_intro_transition(building);
    }

    if building_count == round.buildings.len() {
        draw_wind_for_intro_transition(round.wind);
    }

    visuals.present_frame();
}

pub(crate) fn draw_round(
    round: &RoundState,
    players: &[Player; 2],
    banana: Option<BananaFrame>,
    show_names: bool,
    visuals: &VisualAssets,
) {
    draw_round_with_effect(round, players, banana, None, show_names, visuals);
}

pub(crate) fn draw_round_with_turn_input_overlay(
    round: &RoundState,
    players: &[Player; 2],
    show_names: bool,
    visuals: &VisualAssets,
    player_index: usize,
    angle_display: &str,
    velocity_display: Option<&str>,
    show_cursor: bool,
) {
    visuals.begin_frame();
    draw_round_contents(round, players, None, None, show_names, visuals);
    draw_turn_input_overlay(
        player_index,
        angle_display,
        velocity_display,
        show_cursor,
        visuals,
    );
    visuals.present_frame();
}

pub(crate) fn draw_round_with_effect(
    round: &RoundState,
    players: &[Player; 2],
    banana: Option<BananaFrame>,
    effect: Option<RuntimeEffect>,
    show_names: bool,
    visuals: &VisualAssets,
) {
    visuals.begin_frame();
    draw_round_contents(round, players, banana, effect, show_names, visuals);
    visuals.present_frame();
}

pub(crate) async fn run_timed<F>(seconds: f32, mut draw_fn: F)
where
    F: FnMut(),
{
    let mut elapsed = 0.0f32;
    while elapsed < seconds {
        draw_fn();
        elapsed += get_frame_time();
        next_frame().await;
    }
}

pub(crate) async fn settle_enter_key() {
    while is_key_down(KeyCode::Enter) {
        next_frame().await;
    }

    while get_char_pressed().is_some() {}
}

pub(crate) async fn settle_enter_key_with_overlay<F>(mut draw_frame: F)
where
    F: FnMut(),
{
    while is_key_down(KeyCode::Enter) {
        draw_frame();
        next_frame().await;
    }

    while get_char_pressed().is_some() {}
}

fn draw_intro_screen_frame(phase: usize, visuals: &VisualAssets) {
    visuals.begin_frame();
    clear_background(BLACK);
    draw_sparkle_border(phase, visuals);

    draw_centered_text(
        visuals,
        text_row_y(4),
        "R u s t   G O R I L L A S",
        DOS_TEXT_SIZE,
        WHITE,
    );
    draw_centered_text(
        visuals,
        text_row_y(6),
        "Copyright (C) IBM Corporation 1991",
        DOS_TEXT_SIZE,
        LIGHTGRAY,
    );

    let lines = [
        "Your mission is to hit your opponent with the exploding",
        "banana by varying the angle and power of your throw, taking",
        "into account wind speed, gravity, and the city skyline.",
        "The wind speed is shown by a directional arrow at the bottom",
        "of the playing field, its length relative to its strength.",
    ];

    for (idx, line) in lines.iter().enumerate() {
        draw_centered_text(
            visuals,
            text_row_y(8 + idx as i32),
            line,
            DOS_TEXT_SIZE,
            LIGHTGRAY,
        );
    }

    draw_centered_text(
        visuals,
        text_row_y(24),
        "Press any key to continue",
        DOS_TEXT_SIZE,
        LIGHTGRAY,
    );

    visuals.present_frame();
}

#[cfg(target_arch = "wasm32")]
fn intro_audio_unlock_requested() -> bool {
    any_key_pressed()
        || is_mouse_button_pressed(MouseButton::Left)
        || is_mouse_button_pressed(MouseButton::Right)
        || is_mouse_button_pressed(MouseButton::Middle)
}

async fn prompt_setup_name(
    existing_lines: &[String],
    label: &str,
    default: &str,
    visuals: &VisualAssets,
) -> String {
    settle_enter_key().await;
    let mut value = String::new();

    loop {
        let active = format!("{label} (Default = '{default}'): {value}_");
        draw_setup_screen(existing_lines, Some(&active), visuals);

        while let Some(ch) = get_char_pressed() {
            if (ch.is_ascii_alphanumeric() || ch == ' ' || ch == '-' || ch == '_')
                && value.chars().count() < 10
            {
                value.push(ch);
            }
        }

        if is_key_pressed(KeyCode::Backspace) {
            value.pop();
        }

        if is_key_pressed(KeyCode::Enter) {
            let trimmed = value.trim();
            return if trimmed.is_empty() {
                default.to_string()
            } else {
                trimmed.chars().take(10).collect()
            };
        }

        next_frame().await;
    }
}

async fn prompt_setup_integer(
    existing_lines: &[String],
    label: &str,
    default: u32,
    min_value: u32,
    max_value: u32,
    max_digits: usize,
    visuals: &VisualAssets,
) -> u32 {
    settle_enter_key().await;
    let mut value = String::new();

    loop {
        let active = format!("{label} (Default = {default})? {value}_");
        draw_setup_screen(existing_lines, Some(&active), visuals);

        while let Some(ch) = get_char_pressed() {
            if ch.is_ascii_digit() && value.len() < max_digits {
                value.push(ch);
            }
        }

        if is_key_pressed(KeyCode::Backspace) {
            value.pop();
        }

        if is_key_pressed(KeyCode::Enter) {
            if value.is_empty() {
                return default;
            }

            if let Ok(parsed) = value.parse::<u32>() {
                if parsed >= min_value && parsed <= max_value {
                    return parsed;
                }
            }

            value.clear();
        }

        next_frame().await;
    }
}

async fn prompt_setup_float(
    existing_lines: &[String],
    label: &str,
    default: f32,
    visuals: &VisualAssets,
) -> f32 {
    settle_enter_key().await;
    let mut value = String::new();

    loop {
        let active = format!("{label}? {value}_");
        draw_setup_screen(existing_lines, Some(&active), visuals);

        while let Some(ch) = get_char_pressed() {
            let can_append = ch.is_ascii_digit() || (ch == '.' && !value.contains('.'));
            if can_append && value.len() < 6 {
                value.push(ch);
            }
        }

        if is_key_pressed(KeyCode::Backspace) {
            value.pop();
        }

        if is_key_pressed(KeyCode::Enter) {
            if value.is_empty() {
                return default;
            }

            if let Ok(parsed) = value.parse::<f32>() {
                if parsed > 0.0 {
                    return parsed;
                }
            }

            value.clear();
        }

        next_frame().await;
    }
}

async fn prompt_setup_choice(existing_lines: &[String], visuals: &VisualAssets) -> bool {
    settle_enter_key().await;

    loop {
        visuals.begin_frame();
        draw_setup_screen_contents(existing_lines, None, visuals);
        draw_centered_text(
            visuals,
            text_row_y(16),
            "--------------",
            DOS_TEXT_SIZE,
            WHITE,
        );
        draw_centered_text(
            visuals,
            text_row_y(18),
            "V = View Intro",
            DOS_TEXT_SIZE,
            WHITE,
        );
        draw_centered_text(
            visuals,
            text_row_y(19),
            "P = Play Game",
            DOS_TEXT_SIZE,
            WHITE,
        );
        draw_centered_text(
            visuals,
            text_row_y(21),
            "Your Choice?",
            DOS_TEXT_SIZE,
            WHITE,
        );
        visuals.present_frame();

        if let Some(ch) = get_char_pressed() {
            match ch.to_ascii_uppercase() {
                'V' => return true,
                'P' => return false,
                _ => {}
            }
        }

        next_frame().await;
    }
}

fn draw_setup_screen(existing_lines: &[String], active_line: Option<&str>, visuals: &VisualAssets) {
    visuals.begin_frame();
    draw_setup_screen_contents(existing_lines, active_line, visuals);
    visuals.present_frame();
}

fn draw_setup_screen_contents(
    existing_lines: &[String],
    active_line: Option<&str>,
    visuals: &VisualAssets,
) {
    clear_background(BLACK);

    let text_x = 88.0;
    let start_row = 8;

    for (index, line) in existing_lines.iter().enumerate() {
        let row = start_row + index as i32;
        draw_ui_text(visuals, line, text_x, text_row_y(row), DOS_TEXT_SIZE, WHITE);
    }

    if let Some(line) = active_line {
        let row = start_row + existing_lines.len() as i32;
        draw_ui_text(visuals, line, text_x, text_row_y(row), DOS_TEXT_SIZE, WHITE);
    }
}

async fn play_intro_pose_with_sound(
    names: &[String; 2],
    left_pose: ArmPose,
    right_pose: ArmPose,
    sound: &Sound,
    duration: f32,
    visuals: &VisualAssets,
) {
    play_sound_once(sound);
    show_intro_pose(names, left_pose, right_pose, duration, visuals).await;
}

async fn show_intro_pose(
    names: &[String; 2],
    left_pose: ArmPose,
    right_pose: ArmPose,
    duration: f32,
    visuals: &VisualAssets,
) {
    run_timed(duration, || {
        draw_intro_frame(names, left_pose, right_pose, visuals);
    })
    .await;
}

fn draw_intro_frame(
    names: &[String; 2],
    left_pose: ArmPose,
    right_pose: ArmPose,
    visuals: &VisualAssets,
) {
    visuals.begin_frame();
    clear_background(Color::from_rgba(0, 0, 173, 255));

    draw_centered_text(visuals, 28.0, "R U S T   G O R I L L A S", 28, WHITE);
    draw_centered_text(visuals, 82.0, "STARRING:", 24, WHITE);
    draw_centered_text(
        visuals,
        110.0,
        &format!("{} AND {}", names[0], names[1]),
        22,
        WHITE,
    );

    draw_intro_gorilla(265.0, 175.0, left_pose, visuals);
    draw_intro_gorilla(325.0, 175.0, right_pose, visuals);
    visuals.present_frame();
}

fn draw_intro_gorilla(x: f32, y: f32, pose: ArmPose, visuals: &VisualAssets) {
    let texture = match pose {
        ArmPose::LeftUp => &visuals.gorilla_left,
        ArmPose::RightUp => &visuals.gorilla_right,
        ArmPose::ArmsDown => &visuals.gorilla_down,
    };

    draw_texture_ex(
        texture,
        x,
        y,
        WHITE,
        DrawTextureParams {
            dest_size: Some(vec2(28.0, 30.0)),
            ..Default::default()
        },
    );
}

fn draw_building_for_intro_transition(building: &Building) {
    let height = BOTTOM_LINE - building.top_y;

    draw_rectangle_lines(
        building.x - 1.0,
        building.top_y - 1.0,
        building.width + 3.0,
        height + 3.0,
        1.0,
        palette_color(0),
    );

    draw_rectangle(
        building.x,
        building.top_y,
        building.width + 1.0,
        height + 1.0,
        palette_color(building.color),
    );

    for window in &building.windows {
        draw_rectangle(
            window.x,
            window.y,
            window.width + 1.0,
            window.height + 1.0,
            palette_color(window.color),
        );
    }
}

fn draw_wind_for_intro_transition(wind: i32) {
    if wind == 0 {
        return;
    }

    let start_x = SCR_W / 2.0;
    let y = SCR_H - 5.0;
    let line = wind as f32 * 3.0 * (SCR_W / 320.0);
    let end_x = start_x + line;

    draw_line(start_x, y, end_x, y, 1.0, palette_color(2));

    let arrow = if wind > 0 { -2.0 } else { 2.0 };
    draw_line(end_x, y, end_x + arrow, y - 2.0, 1.0, palette_color(2));
    draw_line(end_x, y, end_x + arrow, y + 2.0, 1.0, palette_color(2));
}

fn palette_color(index: u8) -> Color {
    let [red, green, blue] = palette_rgb(index);
    Color::from_rgba(red, green, blue, 255)
}

fn draw_game_over_frame(players: &[Player; 2], phase: usize, visuals: &VisualAssets) {
    visuals.begin_frame();
    clear_background(BLACK);
    draw_sparkle_border(phase, visuals);

    draw_centered_text(
        visuals,
        text_row_y(8),
        "GAME OVER!",
        DOS_TEXT_SIZE,
        LIGHTGRAY,
    );
    draw_centered_text(visuals, text_row_y(10), "Score:", DOS_TEXT_SIZE, LIGHTGRAY);

    draw_ui_text(
        visuals,
        &players[0].name,
        230.0,
        text_row_y(11),
        DOS_TEXT_SIZE,
        LIGHTGRAY,
    );
    draw_ui_text(
        visuals,
        &players[1].name,
        230.0,
        text_row_y(12),
        DOS_TEXT_SIZE,
        LIGHTGRAY,
    );
    draw_ui_text(
        visuals,
        &players[0].score.to_string(),
        390.0,
        text_row_y(11),
        DOS_TEXT_SIZE,
        LIGHTGRAY,
    );
    draw_ui_text(
        visuals,
        &players[1].score.to_string(),
        390.0,
        text_row_y(12),
        DOS_TEXT_SIZE,
        LIGHTGRAY,
    );

    draw_centered_text(
        visuals,
        text_row_y(24),
        "Press any key to continue",
        DOS_TEXT_SIZE,
        LIGHTGRAY,
    );

    visuals.present_frame();
}

fn draw_turn_input_overlay(
    player_index: usize,
    angle_display: &str,
    velocity_display: Option<&str>,
    show_cursor: bool,
    visuals: &VisualAssets,
) {
    if let Some(velocity_value) = velocity_display {
        let velocity_suffix = if show_cursor { "_" } else { "" };
        draw_player_prompt_line(
            visuals,
            player_index,
            44.0,
            &format!("Angle: {angle_display}"),
        );
        draw_player_prompt_line(
            visuals,
            player_index,
            68.0,
            &format!("Velocity: {velocity_value}{velocity_suffix}"),
        );
    } else {
        let angle_suffix = if show_cursor { "_" } else { "" };
        draw_player_prompt_line(
            visuals,
            player_index,
            44.0,
            &format!("Angle: {angle_display}{angle_suffix}"),
        );
    }
}

fn draw_player_prompt_line(visuals: &VisualAssets, player_index: usize, y: f32, text: &str) {
    let x = if player_index == 0 {
        8.0
    } else {
        SCR_W - measure_ui_text(visuals, text, DOS_TEXT_SIZE).width - 8.0
    };

    draw_ui_text(visuals, text, x, y, DOS_TEXT_SIZE, WHITE);
}

fn draw_round_contents(
    round: &RoundState,
    players: &[Player; 2],
    banana: Option<BananaFrame>,
    effect: Option<RuntimeEffect>,
    show_names: bool,
    visuals: &VisualAssets,
) {
    let scene = round.render_scene(banana, effect);
    let checkpoint = render_runtime_scene(&scene);
    let scene_rgba = checkpoint_rgba(&checkpoint, false);
    visuals.scene.update_from_bytes(
        checkpoint.width as u32,
        checkpoint.height as u32,
        scene_rgba.as_slice(),
    );

    clear_background(BLACK);
    draw_texture_ex(
        &visuals.scene,
        0.0,
        0.0,
        WHITE,
        DrawTextureParams {
            dest_size: Some(vec2(SCR_W, SCR_H)),
            ..Default::default()
        },
    );

    if show_names {
        draw_ui_text(visuals, &players[0].name, 8.0, 20.0, DOS_TEXT_SIZE, WHITE);
        let right_width = measure_ui_text(visuals, &players[1].name, DOS_TEXT_SIZE).width;
        draw_ui_text(
            visuals,
            &players[1].name,
            SCR_W - right_width - 8.0,
            20.0,
            DOS_TEXT_SIZE,
            WHITE,
        );
    }

    let score = format!("{}>Score<{}", players[0].score, players[1].score);
    let score_metrics = measure_ui_text(visuals, &score, DOS_TEXT_SIZE);
    let score_x = ((SCR_W - score_metrics.width) / 2.0).round();
    let score_y = 323.0;
    let score_box_top = (score_y - score_metrics.offset_y - 2.0).round();
    let score_box_height = (score_metrics.height + 4.0).ceil();
    draw_rectangle(
        score_x - 4.0,
        score_box_top,
        score_metrics.width + 8.0,
        score_box_height,
        Color::from_rgba(0, 0, 173, 255),
    );
    draw_ui_text(visuals, &score, score_x, score_y, DOS_TEXT_SIZE, WHITE);
}

fn text_row_y(row: i32) -> f32 {
    12.0 + (row as f32 - 1.0) * 14.0
}

fn text_column_x(column: usize) -> f32 {
    (column.saturating_sub(1) as f32) * DOS_TEXT_COLUMN_STEP_X
}

fn draw_sparkle_border(phase: usize, visuals: &VisualAssets) {
    for column in 0..80usize {
        let top_star = (column + phase) % BORDER_SPARKLE_PERIOD == 0;
        let bottom_star = (column + BORDER_SPARKLE_PERIOD - 1 + BORDER_SPARKLE_PERIOD - phase)
            % BORDER_SPARKLE_PERIOD
            == 0;

        if top_star {
            draw_ui_text(
                visuals,
                "*",
                text_column_x(column + 1),
                text_row_y(1),
                DOS_TEXT_SIZE,
                RED,
            );
        }

        if bottom_star {
            draw_ui_text(
                visuals,
                "*",
                text_column_x(column + 1),
                text_row_y(22),
                DOS_TEXT_SIZE,
                RED,
            );
        }
    }

    let phase_step = phase as i32 + 1;
    for row in 2..=21 {
        let sparkle = (phase_step + row) % BORDER_SPARKLE_PERIOD as i32 == 1;
        if sparkle {
            draw_ui_text(
                visuals,
                "*",
                text_column_x(80),
                text_row_y(row),
                DOS_TEXT_SIZE,
                RED,
            );
            draw_ui_text(
                visuals,
                "*",
                text_column_x(1),
                text_row_y(23 - row),
                DOS_TEXT_SIZE,
                RED,
            );
        }
    }
}

fn draw_centered_text(visuals: &VisualAssets, y: f32, text: &str, size: u16, color: Color) {
    let measure = measure_ui_text(visuals, text, size);
    draw_ui_text(
        visuals,
        text,
        ((SCR_W - measure.width) / 2.0).round(),
        y,
        size,
        color,
    );
}

fn measure_ui_text(visuals: &VisualAssets, text: &str, size: u16) -> TextDimensions {
    measure_text(text, Some(&visuals.ui_font), size, 1.0)
}

fn draw_ui_text(
    visuals: &VisualAssets,
    text: &str,
    x: f32,
    y: f32,
    size: u16,
    color: Color,
) -> TextDimensions {
    draw_text_ex(
        text,
        x.round(),
        y.round(),
        TextParams {
            font: Some(&visuals.ui_font),
            font_size: size,
            font_scale: 1.0,
            color,
            ..Default::default()
        },
    )
}

fn any_key_pressed() -> bool {
    get_last_key_pressed().is_some() || get_char_pressed().is_some()
}
