use gorillas::render_ref::{
    RuntimeEffect, RuntimeScene, COLOR_BUILDING_CYAN, COLOR_BUILDING_GRAY, COLOR_BUILDING_RED,
    COLOR_DARK, COLOR_WINDOW,
};
use gorillas::{ShotBuilding, ShotCrater, ShotGorilla, ShotWorld, BOTTOM_LINE, SCR_W};
use macroquad::rand::gen_range;

pub(crate) use gorillas::render_ref::{
    GorillaPose as ArmPose, RuntimeBanana as BananaFrame, RuntimeBuilding as Building,
    RuntimeGorilla as Gorilla, RuntimeHole as Crater, RuntimeWindow as WindowCell,
};

#[derive(Clone, Debug)]
pub(crate) struct RoundState {
    pub(crate) buildings: Vec<Building>,
    pub(crate) gorillas: [Gorilla; 2],
    pub(crate) craters: Vec<Crater>,
    pub(crate) wind: i32,
    pub(crate) sun_shocked: bool,
}

impl RoundState {
    pub(crate) fn shot_world(&self) -> ShotWorld {
        ShotWorld {
            wind: self.wind,
            buildings: self
                .buildings
                .iter()
                .map(|building| ShotBuilding {
                    x: building.x,
                    width: building.width,
                    top_y: building.top_y,
                })
                .collect(),
            gorillas: self.gorillas.map(|gorilla| ShotGorilla {
                x: gorilla.x,
                y: gorilla.y,
                pose: gorilla.pose,
                alive: gorilla.alive,
            }),
            craters: self
                .craters
                .iter()
                .map(|crater| ShotCrater {
                    x: crater.x,
                    y: crater.y,
                    x_radius: crater.x_radius,
                    y_radius: crater.y_radius,
                })
                .collect(),
            sun_shocked: self.sun_shocked,
        }
    }

    pub(crate) fn render_scene(
        &self,
        banana: Option<BananaFrame>,
        effect: Option<RuntimeEffect>,
    ) -> RuntimeScene {
        RuntimeScene {
            buildings: self.buildings.clone(),
            gorillas: self.gorillas,
            holes: self.craters.clone(),
            wind: self.wind,
            sun_shocked: self.sun_shocked,
            banana,
            effect,
        }
    }
}

pub(crate) fn generate_round() -> RoundState {
    let mut buildings = Vec::new();
    let mut bcoords = Vec::<(f32, f32)>::new();

    let slope = rand_inclusive(1, 6);
    let mut new_height = match slope {
        1 => 15,
        2 => 130,
        3..=5 => 15,
        _ => 130,
    };

    let mut x = 2.0f32;

    while x <= SCR_W - 10.0 {
        match slope {
            1 => new_height += 10,
            2 => new_height -= 10,
            3..=5 => {
                if x <= SCR_W / 2.0 {
                    new_height += 20;
                } else {
                    new_height -= 20;
                }
            }
            _ => {
                if x <= SCR_W / 2.0 {
                    new_height -= 20;
                } else {
                    new_height += 20;
                }
            }
        }

        let mut width = rand_inclusive(1, 37) as f32 + 37.0;
        if x + width > SCR_W {
            width = (SCR_W - x - 2.0).max(10.0);
        }

        let mut bheight = rand_inclusive(1, 120) + new_height;
        bheight = bheight.clamp(25, 265);

        let top_y = (BOTTOM_LINE - bheight as f32).max(70.0);

        let color = match rand_inclusive(1, 3) {
            1 => COLOR_BUILDING_GRAY,
            2 => COLOR_BUILDING_RED,
            _ => COLOR_BUILDING_CYAN,
        };

        let mut windows = Vec::new();
        let mut c = x + 3.0;
        while c < x + width - 3.0 {
            let mut i = (BOTTOM_LINE - top_y) - 3.0;
            while i >= 7.0 {
                let win_color = if rand_inclusive(1, 4) == 1 {
                    COLOR_DARK
                } else {
                    COLOR_WINDOW
                };

                windows.push(WindowCell {
                    x: c,
                    y: BOTTOM_LINE - i,
                    width: 3.0,
                    height: 6.0,
                    color: win_color,
                });

                i -= 15.0;
            }
            c += 10.0;
        }

        buildings.push(Building {
            x,
            width,
            top_y,
            color,
            windows,
        });

        bcoords.push((x, top_y));
        x += width + 2.0;
    }

    if buildings.len() < 6 {
        return generate_round();
    }

    let left_b = rand_inclusive(2, 3) as usize;
    let right_b = buildings
        .len()
        .saturating_sub(rand_inclusive(2, 3) as usize + 1);

    let left_width = bcoords[left_b + 1].0 - bcoords[left_b].0;
    let right_width = bcoords[right_b + 1].0 - bcoords[right_b].0;

    let left_x = bcoords[left_b].0 + left_width / 2.0 - 14.0;
    let right_x = bcoords[right_b].0 + right_width / 2.0 - 14.0;
    let left_y = bcoords[left_b].1 - 30.0;
    let right_y = bcoords[right_b].1 - 30.0;

    let mut wind = rand_inclusive(-5, 5);
    if rand_inclusive(1, 3) == 1 {
        let gust = rand_inclusive(1, 10);
        wind = if wind > 0 { wind + gust } else { wind - gust };
    }

    RoundState {
        buildings,
        gorillas: [
            Gorilla {
                x: left_x,
                y: left_y,
                pose: ArmPose::ArmsDown,
                alive: true,
            },
            Gorilla {
                x: right_x,
                y: right_y,
                pose: ArmPose::ArmsDown,
                alive: true,
            },
        ],
        craters: Vec::new(),
        wind,
        sun_shocked: false,
    }
}

fn rand_inclusive(min: i32, max: i32) -> i32 {
    gen_range(min, max + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_round_state() -> RoundState {
        RoundState {
            buildings: vec![Building {
                x: 100.0,
                width: 40.0,
                top_y: 180.0,
                color: COLOR_BUILDING_GRAY,
                windows: Vec::new(),
            }],
            gorillas: [
                Gorilla {
                    x: 120.0,
                    y: 150.0,
                    pose: ArmPose::LeftUp,
                    alive: true,
                },
                Gorilla {
                    x: 420.0,
                    y: 140.0,
                    pose: ArmPose::RightUp,
                    alive: false,
                },
            ],
            craters: vec![Crater {
                x: 118.0,
                y: 210.0,
                x_radius: 10.0,
                y_radius: 10.0,
            }],
            wind: -3,
            sun_shocked: true,
        }
    }

    #[test]
    fn test_runtime_world_adapter_preserves_craters_and_gorilla_state() {
        let round = sample_round_state();
        let world = round.shot_world();

        assert_eq!(world.gorillas[0].x, round.gorillas[0].x);
        assert_eq!(world.gorillas[0].y, round.gorillas[0].y);
        assert_eq!(world.gorillas[0].pose, ArmPose::LeftUp);
        assert!(world.gorillas[0].alive);

        assert_eq!(world.gorillas[1].x, round.gorillas[1].x);
        assert_eq!(world.gorillas[1].y, round.gorillas[1].y);
        assert_eq!(world.gorillas[1].pose, ArmPose::RightUp);
        assert!(!world.gorillas[1].alive);

        assert_eq!(world.craters.len(), 1);
        assert_eq!(world.craters[0].x, round.craters[0].x);
        assert_eq!(world.craters[0].y, round.craters[0].y);
        assert_eq!(world.craters[0].x_radius, round.craters[0].x_radius);
        assert_eq!(world.craters[0].y_radius, round.craters[0].y_radius);
    }

    #[test]
    fn test_runtime_world_adapter_preserves_sun_shocked_and_wind() {
        let round = sample_round_state();
        let world = round.shot_world();

        assert_eq!(world.wind, round.wind);
        assert_eq!(world.sun_shocked, round.sun_shocked);
        assert_eq!(world.buildings.len(), round.buildings.len());
        assert_eq!(world.buildings[0].x, round.buildings[0].x);
        assert_eq!(world.buildings[0].width, round.buildings[0].width);
        assert_eq!(world.buildings[0].top_y, round.buildings[0].top_y);
    }

    #[test]
    fn test_render_scene_preserves_overlay_and_runtime_state() {
        let round = sample_round_state();
        let scene = round.render_scene(
            Some(BananaFrame {
                x: 140.0,
                y: 80.0,
                rotation: 2,
            }),
            Some(RuntimeEffect::BuildingExplosion {
                x: 90.0,
                y: 75.0,
                radius: 12.0,
                erase: false,
            }),
        );

        assert_eq!(scene.buildings[0].color, round.buildings[0].color);
        assert_eq!(scene.gorillas[0].pose, round.gorillas[0].pose);
        assert_eq!(scene.holes[0].x_radius, round.craters[0].x_radius);
        assert_eq!(scene.wind, round.wind);
        assert!(scene.sun_shocked);

        let banana = scene.banana.expect("expected banana overlay");
        assert_eq!(banana.x, 140.0);
        assert_eq!(banana.y, 80.0);
        assert_eq!(banana.rotation, 2);
        assert!(matches!(
            scene.effect,
            Some(RuntimeEffect::BuildingExplosion { erase: false, .. })
        ));
    }
}
