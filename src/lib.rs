//! Core game logic and fidelity tooling support

pub mod play_ref;
pub mod render_ref;

use std::error::Error;
use std::fmt;

use render_ref::{gorilla_checkpoint, sun_checkpoint, GorillaPose};
use serde::{Deserialize, Serialize};

pub const SCR_W: f32 = 640.0;
pub const SCR_H: f32 = 350.0;
pub const BOTTOM_LINE: f32 = 335.0;
pub const SUN_Y: f32 = 25.0;
pub const SUN_HIT_HEIGHT: f32 = 39.0;
pub const GORILLA_WIDTH: f32 = 30.0;
pub const GORILLA_HEIGHT: f32 = 30.0;

pub const SHOT_TIME_UNITS_PER_SECOND: f32 = 5.0;
pub const SHOT_SIMULATION_STEP: f32 = 0.1;
pub const SHOT_MAX_STEPS: usize = 2200;

const SUN_CHECKPOINT_X: i32 = 298;
const SUN_CHECKPOINT_Y: i32 = 7;
const SUN_EXIT_RADIUS: f32 = 20.0;
const SHOT_MIN_VELOCITY: f32 = 2.0;
const SELF_HIT_GRACE_TIME: f32 = 0.35;
const RIGHT_THROW_START_X_OFFSET: f32 = 25.0;
const THROW_START_Y_OFFSET: f32 = 7.0;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceScenario {
    pub gravity: f32,
    pub round: TraceRound,
    pub shots: Vec<TraceShotInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceRound {
    pub wind: i32,
    pub buildings: Vec<TraceBuilding>,
    pub gorillas: [TraceGorilla; 2],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceBuilding {
    pub x: f32,
    pub width: f32,
    pub top_y: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceGorilla {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceShotInput {
    pub player: usize,
    pub angle: f32,
    pub velocity: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioTrace {
    pub gravity: f32,
    pub round: TraceRound,
    pub shots: Vec<TraceShotResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceShotResult {
    pub player: usize,
    pub input_angle: f32,
    pub effective_angle: f32,
    pub velocity: f32,
    pub start_x: f32,
    pub start_y: f32,
    pub points: Vec<TracePoint>,
    pub outcome: TraceOutcome,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TracePoint {
    pub step: usize,
    pub t: f32,
    pub x: f32,
    pub y: f32,
    pub rotation: i32,
    pub in_sun: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind")]
pub enum TraceOutcome {
    Miss,
    BuildingHit {
        x: f32,
        y: f32,
        building_index: usize,
    },
    GorillaHit {
        x: f32,
        y: f32,
        gorilla_index: usize,
    },
}

#[derive(Debug, Clone, Copy)]
pub struct ShotBuilding {
    pub x: f32,
    pub width: f32,
    pub top_y: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct ShotGorilla {
    pub x: f32,
    pub y: f32,
    pub pose: GorillaPose,
    pub alive: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct ShotCrater {
    pub x: f32,
    pub y: f32,
    pub x_radius: f32,
    pub y_radius: f32,
}

#[derive(Debug, Clone)]
pub struct ShotWorld {
    pub wind: i32,
    pub buildings: Vec<ShotBuilding>,
    pub gorillas: [ShotGorilla; 2],
    pub craters: Vec<ShotCrater>,
    pub sun_shocked: bool,
}

#[derive(Debug, Clone)]
pub struct ShotEvaluation {
    pub outcome: TraceOutcome,
    pub min_distance: f32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShotSimulationError {
    InvalidPlayerId(usize),
}

impl fmt::Display for ShotSimulationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPlayerId(player_id) => {
                write!(f, "invalid player id {player_id}; expected 1 or 2")
            }
        }
    }
}

impl Error for ShotSimulationError {}

#[derive(Debug, Clone, Copy)]
enum CollisionPointValue {
    Empty,
    Sun,
    Building(usize),
    Gorilla(usize),
}

#[derive(Debug, Clone)]
struct ShotSummary {
    player: usize,
    input_angle: f32,
    effective_angle: f32,
    velocity: f32,
    start_x: f32,
    start_y: f32,
    outcome: TraceOutcome,
}

pub fn shot_world_from_trace(round: &TraceRound) -> ShotWorld {
    ShotWorld {
        wind: round.wind,
        buildings: round
            .buildings
            .iter()
            .map(|building| ShotBuilding {
                x: building.x,
                width: building.width,
                top_y: building.top_y,
            })
            .collect(),
        gorillas: [
            ShotGorilla {
                x: round.gorillas[0].x,
                y: round.gorillas[0].y,
                pose: GorillaPose::ArmsDown,
                alive: true,
            },
            ShotGorilla {
                x: round.gorillas[1].x,
                y: round.gorillas[1].y,
                pose: GorillaPose::ArmsDown,
                alive: true,
            },
        ],
        craters: Vec::new(),
        sun_shocked: false,
    }
}

pub fn simulate_trace_scenario(
    scenario: &TraceScenario,
) -> Result<ScenarioTrace, ShotSimulationError> {
    let shots = scenario
        .shots
        .iter()
        .map(|shot| simulate_shot(&scenario.round, scenario.gravity, shot))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(ScenarioTrace {
        gravity: scenario.gravity,
        round: scenario.round.clone(),
        shots,
    })
}

pub fn simulate_shot(
    round: &TraceRound,
    gravity: f32,
    shot: &TraceShotInput,
) -> Result<TraceShotResult, ShotSimulationError> {
    let world = shot_world_from_trace(round);
    simulate_shot_in_world(&world, gravity, shot)
}

pub fn simulate_shot_in_world(
    world: &ShotWorld,
    gravity: f32,
    shot: &TraceShotInput,
) -> Result<TraceShotResult, ShotSimulationError> {
    let mut points = Vec::new();
    let summary = simulate_shot_core(world, gravity, shot, |point| {
        points.push(point);
    })?;

    Ok(TraceShotResult {
        player: summary.player,
        input_angle: summary.input_angle,
        effective_angle: summary.effective_angle,
        velocity: summary.velocity,
        start_x: summary.start_x,
        start_y: summary.start_y,
        points,
        outcome: summary.outcome,
    })
}

pub fn evaluate_shot_in_world(
    world: &ShotWorld,
    gravity: f32,
    shot: &TraceShotInput,
    target_x: f32,
    target_y: f32,
) -> Result<ShotEvaluation, ShotSimulationError> {
    let mut min_distance = f32::INFINITY;

    let summary = simulate_shot_core(world, gravity, shot, |point| {
        let dx = point.x - target_x;
        let dy = point.y - target_y;
        min_distance = min_distance.min((dx * dx + dy * dy).sqrt());
    })?;

    Ok(ShotEvaluation {
        outcome: summary.outcome,
        min_distance,
    })
}

fn simulate_shot_core<F>(
    world: &ShotWorld,
    gravity: f32,
    shot: &TraceShotInput,
    mut on_point: F,
) -> Result<ShotSummary, ShotSimulationError>
where
    F: FnMut(TracePoint),
{
    let thrower = thrower_from_player_id(shot.player)?;
    let effective_angle = if thrower == 1 {
        180.0 - shot.angle
    } else {
        shot.angle
    };

    let thrower_gorilla = world.gorillas[thrower];
    let mut start_x = thrower_gorilla.x;
    let start_y = thrower_gorilla.y - THROW_START_Y_OFFSET;
    if thrower == 1 {
        start_x += RIGHT_THROW_START_X_OFFSET;
    }

    if shot.velocity < SHOT_MIN_VELOCITY {
        let point = TracePoint {
            step: 0,
            t: 0.0,
            x: thrower_gorilla.x,
            y: thrower_gorilla.y,
            rotation: 0,
            in_sun: false,
        };
        on_point(point);

        return Ok(ShotSummary {
            player: shot.player,
            input_angle: shot.angle,
            effective_angle,
            velocity: shot.velocity,
            start_x,
            start_y,
            outcome: TraceOutcome::GorillaHit {
                x: thrower_gorilla.x,
                y: thrower_gorilla.y,
                gorilla_index: thrower,
            },
        });
    }

    let angle_radians = effective_angle.to_radians();
    let init_x_vel = angle_radians.cos() * shot.velocity;
    let init_y_vel = angle_radians.sin() * shot.velocity;

    let mut shot_in_sun = false;
    let mut sun_shocked = world.sun_shocked;

    for step in 0..SHOT_MAX_STEPS {
        let t = step as f32 * SHOT_SIMULATION_STEP;
        let x = start_x + (init_x_vel * t) + (0.5 * (world.wind as f32 / 5.0) * t * t);
        let y = start_y + (-init_y_vel * t) + (0.5 * gravity * t * t);

        let out_of_bounds = x >= SCR_W - 10.0 || x <= 3.0 || y >= SCR_H - 3.0;

        let mut impact: Option<TraceOutcome> = None;
        let mut saw_sun_pixel = false;

        if !out_of_bounds && y > 0.0 {
            for (sample_x, sample_y) in projectile_sample_points(thrower, x, y) {
                match collision_point_value(world, sun_shocked, sample_x, sample_y) {
                    CollisionPointValue::Empty => {}
                    CollisionPointValue::Sun => {
                        if sample_y < SUN_HIT_HEIGHT {
                            shot_in_sun = true;
                            sun_shocked = true;
                            saw_sun_pixel = true;
                        }
                    }
                    CollisionPointValue::Building(building_index) => {
                        impact = Some(TraceOutcome::BuildingHit {
                            x,
                            y,
                            building_index,
                        });
                        break;
                    }
                    CollisionPointValue::Gorilla(gorilla_index) => {
                        if gorilla_index == thrower && t < SELF_HIT_GRACE_TIME {
                            continue;
                        }

                        impact = Some(TraceOutcome::GorillaHit {
                            x,
                            y,
                            gorilla_index,
                        });
                        break;
                    }
                }
            }

            if shot_in_sun
                && !saw_sun_pixel
                && ((SCR_W / 2.0 - x).abs() > SUN_EXIT_RADIUS || y > SUN_HIT_HEIGHT)
            {
                shot_in_sun = false;
            }
        }

        on_point(TracePoint {
            step,
            t,
            x,
            y,
            rotation: (step as i32).rem_euclid(4),
            in_sun: shot_in_sun,
        });

        if out_of_bounds {
            return Ok(ShotSummary {
                player: shot.player,
                input_angle: shot.angle,
                effective_angle,
                velocity: shot.velocity,
                start_x,
                start_y,
                outcome: TraceOutcome::Miss,
            });
        }

        if let Some(outcome) = impact {
            return Ok(ShotSummary {
                player: shot.player,
                input_angle: shot.angle,
                effective_angle,
                velocity: shot.velocity,
                start_x,
                start_y,
                outcome,
            });
        }
    }

    Ok(ShotSummary {
        player: shot.player,
        input_angle: shot.angle,
        effective_angle,
        velocity: shot.velocity,
        start_x,
        start_y,
        outcome: TraceOutcome::Miss,
    })
}

fn thrower_from_player_id(player_id: usize) -> Result<usize, ShotSimulationError> {
    match player_id {
        1 | 2 => Ok(player_id - 1),
        other => Err(ShotSimulationError::InvalidPlayerId(other)),
    }
}

fn projectile_sample_points(thrower: usize, x: f32, y: f32) -> [(f32, f32); 2] {
    if thrower == 0 {
        [(x + 8.0, y), (x + 4.0, y + 6.0)]
    } else {
        [(x, y), (x + 4.0, y + 6.0)]
    }
}

fn collision_point_value(
    world: &ShotWorld,
    sun_shocked: bool,
    x: f32,
    y: f32,
) -> CollisionPointValue {
    if sun_pixel_hit(sun_shocked, x, y) {
        return CollisionPointValue::Sun;
    }

    if let Some(gorilla_index) = gorilla_pixel_collision(world, x, y) {
        return CollisionPointValue::Gorilla(gorilla_index);
    }

    if let Some(building_index) = building_collision(world, x, y) {
        return CollisionPointValue::Building(building_index);
    }

    CollisionPointValue::Empty
}

fn sun_pixel_hit(sun_shocked: bool, x: f32, y: f32) -> bool {
    let checkpoint = sun_checkpoint(sun_shocked);
    let local_x = x.round() as i32 - SUN_CHECKPOINT_X;
    let local_y = y.round() as i32 - SUN_CHECKPOINT_Y;

    if local_x < 0
        || local_y < 0
        || local_x >= checkpoint.width as i32
        || local_y >= checkpoint.height as i32
    {
        return false;
    }

    checkpoint.pixels[local_y as usize * checkpoint.width + local_x as usize] != 0
}

fn gorilla_pixel_collision(world: &ShotWorld, x: f32, y: f32) -> Option<usize> {
    for (index, gorilla) in world.gorillas.iter().enumerate() {
        if !gorilla.alive {
            continue;
        }

        let checkpoint = gorilla_checkpoint(gorilla.pose);
        let local_x = x.round() as i32 - gorilla.x.round() as i32;
        let local_y = y.round() as i32 - gorilla.y.round() as i32;

        if local_x < 0
            || local_y < 0
            || local_x >= checkpoint.width as i32
            || local_y >= checkpoint.height as i32
        {
            continue;
        }

        let pixel = checkpoint.pixels[local_y as usize * checkpoint.width + local_x as usize];
        if pixel != 0 {
            return Some(index);
        }
    }

    None
}

fn building_collision(world: &ShotWorld, x: f32, y: f32) -> Option<usize> {
    world.buildings.iter().position(|building| {
        let inside_x = x >= building.x && x <= building.x + building.width;
        let inside_y = y >= building.top_y && y <= BOTTOM_LINE;
        inside_x && inside_y && !inside_any_crater(world, x, y)
    })
}

fn inside_any_crater(world: &ShotWorld, x: f32, y: f32) -> bool {
    world.craters.iter().any(|crater| {
        if crater.x_radius <= 0.0 || crater.y_radius <= 0.0 {
            return false;
        }

        let normalized_x = (x - crater.x) / crater.x_radius;
        let normalized_y = (y - crater.y) / crater.y_radius;
        normalized_x * normalized_x + normalized_y * normalized_y <= 1.0
    })
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use super::*;

    fn sample_trace_round() -> TraceRound {
        TraceRound {
            wind: 0,
            buildings: vec![TraceBuilding {
                x: 100.0,
                width: 40.0,
                top_y: 200.0,
            }],
            gorillas: [
                TraceGorilla { x: 50.0, y: 140.0 },
                TraceGorilla { x: 220.0, y: 140.0 },
            ],
        }
    }

    fn approx_eq(left: f32, right: f32) {
        assert!(
            (left - right).abs() <= 0.001,
            "expected {left} to be within 0.001 of {right}"
        );
    }

    #[test]
    fn test_trace_rejects_invalid_player_id() {
        let round = sample_trace_round();
        let err = simulate_shot(
            &round,
            9.8,
            &TraceShotInput {
                player: 3,
                angle: 45.0,
                velocity: 55.0,
            },
        )
        .expect_err("invalid player id must fail");

        assert_eq!(err, ShotSimulationError::InvalidPlayerId(3));
    }

    #[test]
    fn test_sample_trace_scenario_matches_golden_values() {
        let scenario_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tools")
            .join("sample_trace_scenario.json");
        let scenario_text = fs::read_to_string(scenario_path).expect("sample scenario must exist");
        let scenario: TraceScenario =
            serde_json::from_str(&scenario_text).expect("sample scenario must parse");
        let trace = simulate_trace_scenario(&scenario).expect("sample scenario must simulate");

        assert_eq!(trace.shots.len(), 2);

        let shot_1 = &trace.shots[0];
        assert_eq!(shot_1.points.len(), 97);
        match shot_1.outcome {
            TraceOutcome::BuildingHit {
                x,
                y,
                building_index,
            } => {
                approx_eq(x, 505.94153);
                approx_eq(y, 152.2905);
                assert_eq!(building_index, 9);
            }
            ref other => panic!("unexpected first shot outcome: {other:?}"),
        }
        approx_eq(shot_1.points[0].x, 71.0);
        approx_eq(shot_1.points[0].y, 108.0);
        assert_eq!(shot_1.points[0].rotation, 0);
        approx_eq(shot_1.points[5].x, 92.2882);
        approx_eq(shot_1.points[5].y, 88.011795);
        assert_eq!(shot_1.points[5].rotation, 1);
        approx_eq(shot_1.points[96].x, 505.94153);
        approx_eq(shot_1.points[96].y, 152.2905);
        assert_eq!(shot_1.points[96].rotation, 0);

        let shot_2 = &trace.shots[1];
        assert_eq!(shot_2.points.len(), 112);
        match shot_2.outcome {
            TraceOutcome::BuildingHit {
                x,
                y,
                building_index,
            } => {
                approx_eq(x, 190.22765);
                approx_eq(y, 152.98865);
                assert_eq!(building_index, 3);
            }
            ref other => panic!("unexpected second shot outcome: {other:?}"),
        }
        approx_eq(shot_2.points[0].x, 548.0);
        approx_eq(shot_2.points[0].y, 113.0);
        assert_eq!(shot_2.points[0].rotation, 0);
        approx_eq(shot_2.points[5].x, 530.2941);
        approx_eq(shot_2.points[5].y, 88.83129);
        assert_eq!(shot_2.points[5].rotation, 1);
        approx_eq(shot_2.points[111].x, 190.22765);
        approx_eq(shot_2.points[111].y, 152.98865);
        assert_eq!(shot_2.points[111].rotation, 3);
    }

    #[test]
    fn test_sun_hit_switches_simulation_to_shocked_sun_mask() {
        let world = ShotWorld {
            wind: 0,
            buildings: Vec::new(),
            gorillas: [
                ShotGorilla {
                    x: 200.0,
                    y: 250.0,
                    pose: GorillaPose::ArmsDown,
                    alive: true,
                },
                ShotGorilla {
                    x: 560.0,
                    y: 250.0,
                    pose: GorillaPose::ArmsDown,
                    alive: true,
                },
            ],
            craters: Vec::new(),
            sun_shocked: false,
        };
        let shot = TraceShotInput {
            player: 1,
            angle: 64.0,
            velocity: 112.0,
        };

        let trace =
            simulate_shot_in_world(&world, 9.8, &shot).expect("sun test shot must simulate");

        assert!(trace.points[24].in_sun);
        assert!(trace.points[25].in_sun);

        let point = trace.points[25];
        let (sample_x, sample_y) = projectile_sample_points(0, point.x, point.y)[1];
        assert!(!sun_pixel_hit(false, sample_x, sample_y));
        assert!(sun_pixel_hit(true, sample_x, sample_y));
    }

    #[test]
    fn test_building_collision_ignores_points_inside_crater_hole() {
        let mut world = shot_world_from_trace(&sample_trace_round());
        world.craters.push(ShotCrater {
            x: 110.0,
            y: 220.0,
            x_radius: 12.0,
            y_radius: 6.0,
        });

        assert!(building_collision(&world, 121.0, 220.0).is_none());
        assert!(building_collision(&world, 110.0, 225.0).is_none());
        assert!(building_collision(&world, 121.0, 227.0).is_some());
    }

    #[test]
    fn test_ai_evaluator_matches_simulation_outcome() {
        let round = sample_trace_round();
        let world = shot_world_from_trace(&round);
        let shot = TraceShotInput {
            player: 1,
            angle: 45.0,
            velocity: 55.0,
        };

        let full = simulate_shot_in_world(&world, 9.8, &shot).expect("simulation must succeed");
        let eval = evaluate_shot_in_world(&world, 9.8, &shot, 235.0, 155.0)
            .expect("evaluation must succeed");

        assert_eq!(full.outcome, eval.outcome);
        assert!(eval.min_distance.is_finite());
    }
}
