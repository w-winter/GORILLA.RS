use super::assets::{AudioFx, VisualAssets};
use super::presentation::{
    draw_round, draw_round_with_effect, draw_round_with_turn_input_overlay,
    draw_skyline_reveal_frame, game_over_screen, run_timed, settle_enter_key,
    settle_enter_key_with_overlay,
};
use super::round::{generate_round, ArmPose, BananaFrame, Crater, RoundState};
use super::{GameConfig, Player};
use gorillas::render_ref::{BuildingExplosionPhase, GorillaExplosionPhase, RuntimeEffect};
use gorillas::{
    evaluate_shot_in_world, simulate_shot_in_world, TraceOutcome, TraceShotInput, SCR_H, SCR_W,
};
use macroquad::audio::play_sound_once;
use macroquad::prelude::{get_char_pressed, get_frame_time, is_key_pressed, next_frame, KeyCode};

const DEFAULT_ANGLE: f32 = 0.0;
const DEFAULT_VELOCITY: f32 = 0.0;
const SKYLINE_REVEAL_STEP_SECONDS: f32 = 1.0 / 40.0;
const SHOT_VISUAL_STEP_SECONDS: f32 = 0.03;
const MISS_HOLD_SECONDS: f32 = 0.25;
const POST_GORILLA_BLAST_HOLD_SECONDS: f32 = 0.63;
const VICTORY_DANCE_REST_SECONDS: f32 = 0.20;

// QBASIC `DoExplosion` expands immediately, then collapses with both a short
// busy loop (`FOR i = 1 TO 100`) and `Rest .005` on each collapse step
const BUILDING_EXPLOSION_EXPAND_STEP_SECONDS: f32 = 1.0 / 54.0;
const BUILDING_EXPLOSION_COLLAPSE_STEP_SECONDS: f32 = BUILDING_EXPLOSION_EXPAND_STEP_SECONDS;

// QBASIC `PlayerExplosion` uses three distinct phases: lower burst, upper
// burst, then an erase sweep whose source loop includes `FOR Count = 1 TO 200`
const GORILLA_EXPLOSION_LOWER_BURST_STEP_SECONDS: f32 = 1.0 / 333.0;
const GORILLA_EXPLOSION_UPPER_BURST_STEP_SECONDS: f32 = GORILLA_EXPLOSION_LOWER_BURST_STEP_SECONDS;
const GORILLA_EXPLOSION_ERASE_STEP_SECONDS: f32 = GORILLA_EXPLOSION_LOWER_BURST_STEP_SECONDS;

#[derive(Clone, Copy, Debug)]
struct Point {
    x: f32,
    y: f32,
}

#[derive(Clone, Copy, Debug)]
enum ShotOutcome {
    Miss,
    BuildingHit,
    GorillaHit(usize),
}

pub(crate) async fn play_match(config: GameConfig, audio: &AudioFx, visuals: &VisualAssets) {
    let mut players = [
        Player {
            name: config.player_names[0].clone(),
            score: 0,
        },
        Player {
            name: config.player_names[1].clone(),
            score: 0,
        },
    ];

    let mut toss_turn = 0usize;

    while players[0].score < config.target_score && players[1].score < config.target_score {
        let mut round = generate_round();

        animate_skyline_build(&round, visuals).await;

        loop {
            let (angle, velocity) = if config.number_of_players == 1 && toss_turn == 1 {
                let (cpu_angle, cpu_velocity) =
                    choose_computer_shot(&round, toss_turn, config.gravity);
                let angle_display = format!("{cpu_angle:.0}");
                let velocity_display = format!("{cpu_velocity:.0}");

                run_timed(0.35, || {
                    draw_round_with_turn_input_overlay(
                        &round,
                        &players,
                        true,
                        visuals,
                        toss_turn,
                        &angle_display,
                        None,
                        false,
                    );
                })
                .await;

                run_timed(0.35, || {
                    draw_round_with_turn_input_overlay(
                        &round,
                        &players,
                        true,
                        visuals,
                        toss_turn,
                        &angle_display,
                        Some(&velocity_display),
                        false,
                    );
                })
                .await;

                (cpu_angle, cpu_velocity)
            } else {
                prompt_turn_shot(&round, &players, toss_turn, visuals).await
            };

            let outcome = animate_shot(
                &mut round,
                &players,
                toss_turn,
                angle,
                velocity,
                config.gravity,
                audio,
                visuals,
            )
            .await;

            round.sun_shocked = false;

            match outcome {
                ShotOutcome::GorillaHit(hit_index) => {
                    let scorer = if hit_index == toss_turn {
                        1 - toss_turn
                    } else {
                        toss_turn
                    };
                    players[scorer].score += 1;

                    victory_dance(&mut round, &players, scorer, audio, visuals).await;
                    toss_turn = 1 - toss_turn;
                    break;
                }
                ShotOutcome::BuildingHit | ShotOutcome::Miss => {
                    toss_turn = 1 - toss_turn;
                }
            }
        }
    }

    game_over_screen(&players, visuals).await;
}

async fn prompt_turn_shot(
    round: &RoundState,
    players: &[Player; 2],
    player_index: usize,
    visuals: &VisualAssets,
) -> (f32, f32) {
    settle_enter_key().await;

    let mut angle_input = String::new();
    let angle = loop {
        draw_round_with_turn_input_overlay(
            round,
            players,
            true,
            visuals,
            player_index,
            &angle_input,
            None,
            true,
        );

        while let Some(ch) = get_char_pressed() {
            let allowed = ch.is_ascii_digit() || (ch == '.' && !angle_input.contains('.'));
            if allowed && angle_input.len() < 6 {
                angle_input.push(ch);
            }
        }

        if is_key_pressed(KeyCode::Backspace) {
            angle_input.pop();
        }

        if is_key_pressed(KeyCode::Enter) {
            if angle_input.trim().is_empty() {
                break DEFAULT_ANGLE;
            }

            if let Ok(parsed) = angle_input.parse::<f32>() {
                if (0.0..=360.0).contains(&parsed) {
                    break parsed;
                }
            }

            angle_input.clear();
        }

        next_frame().await;
    };

    let mut velocity_input = String::new();
    let angle_display = format!("{angle:.0}");

    settle_enter_key_with_overlay(|| {
        draw_round_with_turn_input_overlay(
            round,
            players,
            true,
            visuals,
            player_index,
            &angle_display,
            Some(""),
            true,
        );
    })
    .await;
    let velocity = loop {
        draw_round_with_turn_input_overlay(
            round,
            players,
            true,
            visuals,
            player_index,
            &angle_display,
            Some(&velocity_input),
            true,
        );

        while let Some(ch) = get_char_pressed() {
            let allowed = ch.is_ascii_digit() || (ch == '.' && !velocity_input.contains('.'));
            if allowed && velocity_input.len() < 6 {
                velocity_input.push(ch);
            }
        }

        if is_key_pressed(KeyCode::Backspace) {
            velocity_input.pop();
        }

        if is_key_pressed(KeyCode::Enter) {
            if velocity_input.trim().is_empty() {
                break DEFAULT_VELOCITY;
            }

            if let Ok(parsed) = velocity_input.parse::<f32>() {
                if parsed >= 0.0 {
                    break parsed;
                }
            }

            velocity_input.clear();
        }

        next_frame().await;
    };

    (angle, velocity)
}

fn choose_computer_shot(round: &RoundState, thrower: usize, gravity: f32) -> (f32, f32) {
    let world = round.shot_world();
    let target = round.gorillas[1 - thrower];
    let target_x = target.x + 15.0;
    let target_y = target.y + 15.0;

    let player_number = thrower + 1;
    let mut best_angle = 45.0;
    let mut best_velocity = 60.0;
    let mut best_distance = f32::INFINITY;

    for angle in (5..=85).step_by(2) {
        for velocity in (15..=120).step_by(2) {
            let evaluation = evaluate_shot_in_world(
                &world,
                gravity,
                &TraceShotInput {
                    player: player_number,
                    angle: angle as f32,
                    velocity: velocity as f32,
                },
                target_x,
                target_y,
            )
            .expect("computer shot evaluation must use a valid player id");

            if let TraceOutcome::GorillaHit { gorilla_index, .. } = evaluation.outcome {
                if gorilla_index == 1 - thrower {
                    return (angle as f32, velocity as f32);
                }
            }

            if evaluation.min_distance < best_distance {
                best_distance = evaluation.min_distance;
                best_angle = angle as f32;
                best_velocity = velocity as f32;
            }
        }
    }

    (best_angle, best_velocity)
}

async fn animate_skyline_build(round: &RoundState, visuals: &VisualAssets) {
    for building_count in 1..=round.buildings.len() {
        run_timed(SKYLINE_REVEAL_STEP_SECONDS, || {
            draw_skyline_reveal_frame(round, building_count, visuals);
        })
        .await;
    }
}

async fn animate_shot(
    round: &mut RoundState,
    players: &[Player; 2],
    thrower: usize,
    input_angle: f32,
    velocity: f32,
    gravity: f32,
    audio: &AudioFx,
    visuals: &VisualAssets,
) -> ShotOutcome {
    round.gorillas[thrower].pose = if thrower == 0 {
        ArmPose::LeftUp
    } else {
        ArmPose::RightUp
    };

    run_timed(0.1, || {
        draw_round(round, players, None, false, visuals);
    })
    .await;

    round.gorillas[thrower].pose = ArmPose::ArmsDown;
    play_sound_once(&audio.throw);

    let simulation = simulate_shot_in_world(
        &round.shot_world(),
        gravity,
        &TraceShotInput {
            player: thrower + 1,
            angle: input_angle,
            velocity,
        },
    )
    .expect("shot simulation must use a valid player id");

    let seconds_per_step = SHOT_VISUAL_STEP_SECONDS;
    let mut step_index = 0usize;
    let mut elapsed_seconds = 0.0f32;

    if simulation.points[step_index].in_sun {
        round.sun_shocked = true;
    }

    loop {
        let point = simulation.points[step_index];
        let banana = if point.in_sun {
            None
        } else {
            Some(BananaFrame {
                x: point.x,
                y: point.y,
                rotation: point.rotation,
            })
        };

        draw_round(round, players, banana, false, visuals);

        if step_index + 1 >= simulation.points.len() {
            break;
        }

        next_frame().await;
        elapsed_seconds += get_frame_time();

        while elapsed_seconds >= seconds_per_step && step_index + 1 < simulation.points.len() {
            elapsed_seconds -= seconds_per_step;
            step_index += 1;
            if simulation.points[step_index].in_sun {
                round.sun_shocked = true;
            }
        }
    }

    match simulation.outcome {
        TraceOutcome::Miss => {
            run_timed(MISS_HOLD_SECONDS, || {
                draw_round(round, players, None, false, visuals);
            })
            .await;
            ShotOutcome::Miss
        }
        TraceOutcome::GorillaHit { gorilla_index, .. } => {
            explode_gorilla(round, players, gorilla_index, audio, visuals).await;
            ShotOutcome::GorillaHit(gorilla_index)
        }
        TraceOutcome::BuildingHit { x, y, .. } => {
            do_explosion(round, players, Point { x, y }, audio, visuals).await;
            ShotOutcome::BuildingHit
        }
    }
}

async fn do_explosion(
    round: &mut RoundState,
    players: &[Player; 2],
    center: Point,
    audio: &AudioFx,
    visuals: &VisualAssets,
) {
    play_sound_once(&audio.building_explode);

    let max_step = ((SCR_H / 50.0) / 0.5).round() as i32;

    for step in 0..=max_step {
        run_timed(BUILDING_EXPLOSION_EXPAND_STEP_SECONDS, || {
            draw_round_with_effect(
                round,
                players,
                None,
                Some(RuntimeEffect::BuildingExplosionProgress {
                    x: center.x,
                    y: center.y,
                    max_step,
                    step,
                    phase: BuildingExplosionPhase::Expand,
                }),
                false,
                visuals,
            );
        })
        .await;
    }

    for step in (0..=max_step).rev() {
        run_timed(BUILDING_EXPLOSION_COLLAPSE_STEP_SECONDS, || {
            draw_round_with_effect(
                round,
                players,
                None,
                Some(RuntimeEffect::BuildingExplosionProgress {
                    x: center.x,
                    y: center.y,
                    max_step,
                    step,
                    phase: BuildingExplosionPhase::Collapse,
                }),
                false,
                visuals,
            );
        })
        .await;
    }

    round.craters.push(Crater {
        x: center.x,
        y: center.y,
        x_radius: SCR_H / 50.0,
        y_radius: SCR_H / 50.0,
    });
}

async fn explode_gorilla(
    round: &mut RoundState,
    players: &[Player; 2],
    gorilla_index: usize,
    audio: &AudioFx,
    visuals: &VisualAssets,
) {
    let gorilla_x = round.gorillas[gorilla_index].x;
    let gorilla_y = round.gorillas[gorilla_index].y;

    let scale_x = (SCR_W / 320.0).round() as i32;

    let upper_center_x = gorilla_x + 3.5 * scale_x as f32 + 5.0;
    let upper_center_y = gorilla_y + 12.0;

    let stage_1_max_step = 8 * scale_x;
    let stage_2_max_step = 16 * scale_x;
    let stage_3_max_step = 24 * scale_x;

    play_sound_once(&audio.gorilla_explode);

    run_timed(GORILLA_EXPLOSION_LOWER_BURST_STEP_SECONDS, || {
        draw_round_with_effect(
            round,
            players,
            None,
            Some(RuntimeEffect::GorillaExplosionProgress {
                gorilla_x,
                gorilla_y,
                step: 1,
                phase: GorillaExplosionPhase::LowerBurst,
            }),
            false,
            visuals,
        );
    })
    .await;

    round.gorillas[gorilla_index].alive = false;

    for step in (2..=stage_1_max_step).step_by(2) {
        run_timed(GORILLA_EXPLOSION_LOWER_BURST_STEP_SECONDS, || {
            draw_round_with_effect(
                round,
                players,
                None,
                Some(RuntimeEffect::GorillaExplosionProgress {
                    gorilla_x,
                    gorilla_y,
                    step,
                    phase: GorillaExplosionPhase::LowerBurst,
                }),
                false,
                visuals,
            );
        })
        .await;
    }

    run_timed(GORILLA_EXPLOSION_UPPER_BURST_STEP_SECONDS, || {
        draw_round_with_effect(
            round,
            players,
            None,
            Some(RuntimeEffect::GorillaExplosionProgress {
                gorilla_x,
                gorilla_y,
                step: 1,
                phase: GorillaExplosionPhase::UpperBurst,
            }),
            false,
            visuals,
        );
    })
    .await;

    for step in (2..=stage_2_max_step).step_by(2) {
        run_timed(GORILLA_EXPLOSION_UPPER_BURST_STEP_SECONDS, || {
            draw_round_with_effect(
                round,
                players,
                None,
                Some(RuntimeEffect::GorillaExplosionProgress {
                    gorilla_x,
                    gorilla_y,
                    step,
                    phase: GorillaExplosionPhase::UpperBurst,
                }),
                false,
                visuals,
            );
        })
        .await;
    }

    for step in (1..stage_3_max_step).rev().step_by(2) {
        run_timed(GORILLA_EXPLOSION_ERASE_STEP_SECONDS, || {
            draw_round_with_effect(
                round,
                players,
                None,
                Some(RuntimeEffect::GorillaExplosionProgress {
                    gorilla_x,
                    gorilla_y,
                    step,
                    phase: GorillaExplosionPhase::Erase,
                }),
                false,
                visuals,
            );
        })
        .await;
    }

    round.craters.push(Crater {
        x: upper_center_x,
        y: upper_center_y,
        x_radius: stage_3_max_step as f32,
        y_radius: stage_3_max_step as f32 * 0.8 * 0.6,
    });

    run_timed(POST_GORILLA_BLAST_HOLD_SECONDS, || {
        draw_round(round, players, None, false, visuals);
    })
    .await;
}

async fn victory_dance(
    round: &mut RoundState,
    players: &[Player; 2],
    winner_index: usize,
    audio: &AudioFx,
    visuals: &VisualAssets,
) {
    for _ in 0..4 {
        round.gorillas[winner_index].pose = ArmPose::LeftUp;
        play_sound_once(&audio.victory_dance);
        run_timed(
            audio.victory_dance_duration + VICTORY_DANCE_REST_SECONDS,
            || {
                draw_round(round, players, None, false, visuals);
            },
        )
        .await;

        round.gorillas[winner_index].pose = ArmPose::RightUp;
        play_sound_once(&audio.victory_dance);
        run_timed(
            audio.victory_dance_duration + VICTORY_DANCE_REST_SECONDS,
            || {
                draw_round(round, players, None, false, visuals);
            },
        )
        .await;
    }

    round.gorillas[winner_index].pose = ArmPose::ArmsDown;
}
