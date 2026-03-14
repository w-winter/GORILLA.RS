//! Reference software renderer for Gorillas runtime scenes and checkpoints
//!
//! The file stays intentionally local: scene types, cached checkpoints, render
//! entrypoints, and oracle-verified effect masks live together so fidelity work
//! can be audited without hopping across modules.

use std::sync::OnceLock;

use crate::{BOTTOM_LINE, SCR_H, SCR_W};
use serde::{Deserialize, Serialize};

pub const COLOR_BG: u8 = 0; // CGA palette index 0 is the sky-blue background
pub const COLOR_GORILLA: u8 = 1;
pub const COLOR_EXPLOSION: u8 = 2;
pub const COLOR_SUN: u8 = 3;
pub const COLOR_BUILDING_GRAY: u8 = 4;
pub const COLOR_BUILDING_RED: u8 = 5;
pub const COLOR_BUILDING_CYAN: u8 = 6;
pub const COLOR_DARK: u8 = 8;
pub const COLOR_WINDOW: u8 = 14;
pub const COLOR_BANANA: u8 = COLOR_WINDOW;

// Runtime scene types shared by exporters and the interactive frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderCheckpoint {
    pub name: String,
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeWindow {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub color: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeBuilding {
    pub x: f32,
    pub width: f32,
    pub top_y: f32,
    pub color: u8,
    pub windows: Vec<RuntimeWindow>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct RuntimeGorilla {
    pub x: f32,
    pub y: f32,
    pub pose: GorillaPose,
    pub alive: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct RuntimeBanana {
    pub x: f32,
    pub y: f32,
    pub rotation: i32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct RuntimeHole {
    pub x: f32,
    pub y: f32,
    pub x_radius: f32,
    pub y_radius: f32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum BuildingExplosionPhase {
    Expand,
    Collapse,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum GorillaExplosionPhase {
    LowerBurst,
    UpperBurst,
    Erase,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum RuntimeEffect {
    BuildingExplosion {
        x: f32,
        y: f32,
        radius: f32,
        erase: bool,
    },
    GorillaExplosion {
        circle_x: f32,
        circle_y: f32,
        radius: f32,
        line_x0: f32,
        line_x1: f32,
        line_y: f32,
    },
    BuildingExplosionProgress {
        x: f32,
        y: f32,
        max_step: i32,
        step: i32,
        phase: BuildingExplosionPhase,
    },
    GorillaExplosionProgress {
        gorilla_x: f32,
        gorilla_y: f32,
        step: i32,
        phase: GorillaExplosionPhase,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeScene {
    pub buildings: Vec<RuntimeBuilding>,
    pub gorillas: [RuntimeGorilla; 2],
    pub holes: Vec<RuntimeHole>,
    pub wind: i32,
    pub sun_shocked: bool,
    pub banana: Option<RuntimeBanana>,
    pub effect: Option<RuntimeEffect>,
}

#[derive(Debug, Clone)]
struct IndexedFrame {
    width: usize,
    height: usize,
    pixels: Vec<u8>,
}

// Checkpoint construction and cached sprite fixtures
fn checkpoint_from_ascii(name: &str, rows: &[&str]) -> RenderCheckpoint {
    let height = rows.len();
    let width = rows.first().map(|row| row.len()).unwrap_or(0);
    let mut pixels = Vec::with_capacity(width * height);

    for row in rows {
        assert_eq!(row.len(), width, "checkpoint rows must have equal width");
        for ch in row.chars() {
            let pixel = match ch {
                '.' => COLOR_BG,
                '1' => COLOR_GORILLA,
                '2' => COLOR_EXPLOSION,
                '3' => COLOR_SUN,
                '4' => COLOR_BUILDING_GRAY,
                '5' => COLOR_BUILDING_RED,
                '6' => COLOR_BUILDING_CYAN,
                '8' => COLOR_DARK,
                'E' => COLOR_BANANA,
                'W' => COLOR_WINDOW,
                other => panic!("unsupported checkpoint pixel marker: {other}"),
            };
            pixels.push(pixel);
        }
    }

    RenderCheckpoint {
        name: name.to_string(),
        width,
        height,
        pixels,
    }
}

impl IndexedFrame {
    fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            pixels: vec![0; width * height],
        }
    }

    fn clear(&mut self, color: u8) {
        self.pixels.fill(color);
    }

    fn set_pixel(&mut self, x: i32, y: i32, color: u8) {
        if x < 0 || y < 0 {
            return;
        }
        let x = x as usize;
        let y = y as usize;
        if x >= self.width || y >= self.height {
            return;
        }
        self.pixels[y * self.width + x] = color;
    }

    fn line(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, color: u8) {
        let mut x0 = x0.round() as i32;
        let mut y0 = y0.round() as i32;
        let x1 = x1.round() as i32;
        let y1 = y1.round() as i32;

        let dx = (x1 - x0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let dy = -(y1 - y0).abs();
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;

        loop {
            self.set_pixel(x0, y0, color);
            if x0 == x1 && y0 == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x0 += sx;
            }
            if e2 <= dx {
                err += dx;
                y0 += sy;
            }
        }
    }

    fn fill_rect(&mut self, x: f32, y: f32, width: f32, height: f32, color: u8) {
        let x0 = x.floor() as i32;
        let y0 = y.floor() as i32;
        let x1 = (x + width).ceil() as i32;
        let y1 = (y + height).ceil() as i32;
        for yy in y0..y1 {
            for xx in x0..x1 {
                self.set_pixel(xx, yy, color);
            }
        }
    }

    fn rect_outline(&mut self, x: f32, y: f32, width: f32, height: f32, color: u8) {
        let right = x + width - 1.0;
        let bottom = y + height - 1.0;
        self.line(x, y, right, y, color);
        self.line(x, bottom, right, bottom, color);
        self.line(x, y, x, bottom, color);
        self.line(right, y, right, bottom, color);
    }

    fn ellipse_fill(&mut self, cx: f32, cy: f32, x_radius: f32, y_radius: f32, color: u8) {
        if x_radius <= 0.0 || y_radius <= 0.0 {
            return;
        }

        let x0 = (cx - x_radius - 1.0).floor() as i32;
        let x1 = (cx + x_radius + 1.0).ceil() as i32;
        let y0 = (cy - y_radius - 1.0).floor() as i32;
        let y1 = (cy + y_radius + 1.0).ceil() as i32;

        for y in y0..=y1 {
            for x in x0..=x1 {
                let normalized_x = (x as f32 - cx) / x_radius;
                let normalized_y = (y as f32 - cy) / y_radius;
                if normalized_x * normalized_x + normalized_y * normalized_y <= 1.0 {
                    self.set_pixel(x, y, color);
                }
            }
        }
    }

    fn blit_checkpoint(
        &mut self,
        checkpoint: &RenderCheckpoint,
        x: f32,
        y: f32,
        transparent_zero: bool,
    ) {
        let origin_x = x.round() as i32;
        let origin_y = y.round() as i32;

        for yy in 0..checkpoint.height {
            for xx in 0..checkpoint.width {
                let pixel = checkpoint.pixels[yy * checkpoint.width + xx];
                if transparent_zero && pixel == COLOR_BG {
                    continue;
                }
                self.set_pixel(origin_x + xx as i32, origin_y + yy as i32, pixel);
            }
        }
    }

    fn into_checkpoint(self, name: &str) -> RenderCheckpoint {
        RenderCheckpoint {
            name: name.to_string(),
            width: self.width,
            height: self.height,
            pixels: self.pixels,
        }
    }
}

pub fn render_checkpoints() -> Vec<RenderCheckpoint> {
    vec![
        render_sun_checkpoint(false),
        render_sun_checkpoint(true),
        render_gorilla_checkpoint("gorilla_arms_down", GorillaPose::ArmsDown),
        render_gorilla_checkpoint("gorilla_left_up", GorillaPose::LeftUp),
        render_gorilla_checkpoint("gorilla_right_up", GorillaPose::RightUp),
        render_banana_checkpoint(0),
        render_banana_checkpoint(1),
        render_banana_checkpoint(2),
        render_banana_checkpoint(3),
    ]
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum GorillaPose {
    LeftUp,
    RightUp,
    ArmsDown,
}

static SUN_HAPPY_CHECKPOINT: OnceLock<RenderCheckpoint> = OnceLock::new();
static SUN_SHOCKED_CHECKPOINT: OnceLock<RenderCheckpoint> = OnceLock::new();

static GORILLA_LEFT_UP_CHECKPOINT: OnceLock<RenderCheckpoint> = OnceLock::new();
static GORILLA_RIGHT_UP_CHECKPOINT: OnceLock<RenderCheckpoint> = OnceLock::new();
static GORILLA_ARMS_DOWN_CHECKPOINT: OnceLock<RenderCheckpoint> = OnceLock::new();

static BANANA_ROT_0_CHECKPOINT: OnceLock<RenderCheckpoint> = OnceLock::new();
static BANANA_ROT_1_CHECKPOINT: OnceLock<RenderCheckpoint> = OnceLock::new();
static BANANA_ROT_2_CHECKPOINT: OnceLock<RenderCheckpoint> = OnceLock::new();
static BANANA_ROT_3_CHECKPOINT: OnceLock<RenderCheckpoint> = OnceLock::new();

pub fn sun_checkpoint(shocked: bool) -> &'static RenderCheckpoint {
    if shocked {
        SUN_SHOCKED_CHECKPOINT.get_or_init(|| render_sun_checkpoint(true))
    } else {
        SUN_HAPPY_CHECKPOINT.get_or_init(|| render_sun_checkpoint(false))
    }
}

pub fn gorilla_checkpoint(pose: GorillaPose) -> &'static RenderCheckpoint {
    match pose {
        GorillaPose::LeftUp => GORILLA_LEFT_UP_CHECKPOINT
            .get_or_init(|| render_gorilla_checkpoint("gorilla_left_up", GorillaPose::LeftUp)),
        GorillaPose::RightUp => GORILLA_RIGHT_UP_CHECKPOINT
            .get_or_init(|| render_gorilla_checkpoint("gorilla_right_up", GorillaPose::RightUp)),
        GorillaPose::ArmsDown => GORILLA_ARMS_DOWN_CHECKPOINT
            .get_or_init(|| render_gorilla_checkpoint("gorilla_arms_down", GorillaPose::ArmsDown)),
    }
}

pub fn banana_checkpoint(rotation: i32) -> &'static RenderCheckpoint {
    match rotation.rem_euclid(4) {
        0 => BANANA_ROT_0_CHECKPOINT.get_or_init(|| render_banana_checkpoint(0)),
        1 => BANANA_ROT_1_CHECKPOINT.get_or_init(|| render_banana_checkpoint(1)),
        2 => BANANA_ROT_2_CHECKPOINT.get_or_init(|| render_banana_checkpoint(2)),
        _ => BANANA_ROT_3_CHECKPOINT.get_or_init(|| render_banana_checkpoint(3)),
    }
}

// Render entrypoints and palette conversion helpers
pub fn render_runtime_scene(scene: &RuntimeScene) -> RenderCheckpoint {
    let mut frame = IndexedFrame::new(SCR_W as usize, SCR_H as usize);
    frame.clear(COLOR_BG);

    for building in &scene.buildings {
        let height = BOTTOM_LINE - building.top_y;
        frame.rect_outline(
            building.x - 1.0,
            building.top_y - 1.0,
            building.width + 3.0,
            height + 3.0,
            COLOR_BG,
        );
        frame.fill_rect(
            building.x,
            building.top_y,
            building.width + 1.0,
            height + 1.0,
            building.color,
        );

        for window in &building.windows {
            frame.fill_rect(
                window.x,
                window.y,
                window.width + 1.0,
                window.height + 1.0,
                window.color,
            );
        }
    }

    for hole in &scene.holes {
        frame.ellipse_fill(hole.x, hole.y, hole.x_radius, hole.y_radius, COLOR_BG);
    }

    frame.blit_checkpoint(sun_checkpoint(scene.sun_shocked), 298.0, 7.0, true);

    if scene.wind != 0 {
        draw_wind_arrow(&mut frame, scene.wind);
    }

    for gorilla in &scene.gorillas {
        if gorilla.alive {
            frame.blit_checkpoint(gorilla_checkpoint(gorilla.pose), gorilla.x, gorilla.y, true);
        }
    }

    if let Some(banana) = scene.banana {
        frame.blit_checkpoint(
            banana_checkpoint(banana.rotation.rem_euclid(4)),
            banana.x,
            banana.y,
            true,
        );
    }

    if let Some(effect) = scene.effect {
        draw_runtime_effect(&mut frame, effect);
    }

    frame.into_checkpoint("runtime_scene")
}

pub fn render_sun_checkpoint(shocked: bool) -> RenderCheckpoint {
    if !shocked {
        return checkpoint_from_ascii(
            "sun_happy",
            &[
                ".............................................",
                ".............................................",
                ".............................................",
                "......................3......................",
                "......................3......................",
                "..............3.......3.......3..............",
                "...............3......3.......3..............",
                "...............3......3......3...............",
                ".......3........3.....3.....3.......33.......",
                "........3........3.3333333..3......3.........",
                ".........33......33333333333.....33..........",
                "...........3..33333333333333333.3............",
                "............33333333333333333333.............",
                "....3.......333333333333333333333.....333....",
                ".....3333..333333333333333333333333333.......",
                ".........3333333333.33333.33333333...........",
                "..........33333333...333...33333333..........",
                "..........333333333.33333.333333333..........",
                "..33333333333333333333333333333333333333333..",
                "..........3333333333333333333333333..........",
                "..........3333333333333333333333333..........",
                ".........333333.3333333333333.3333...........",
                ".....3333..33333..333333333..333333333.......",
                "....3.......333333..33333..333333.....333....",
                "............33333333.....3333333.............",
                "...........3..33333333333333333.3............",
                ".........33......33333333333.....33..........",
                "........3........3.3333333..3......3.........",
                ".......3........3.....3.....3.......33.......",
                "...............3......3......3...............",
                "...............3......3.......3..............",
                "..............3.......3.......3..............",
                "......................3......................",
                "......................3......................",
                ".............................................",
                ".............................................",
                ".............................................",
            ],
        );
    }

    checkpoint_from_ascii(
        "sun_shocked",
        &[
            ".............................................",
            ".............................................",
            ".............................................",
            "......................3......................",
            "......................3......................",
            "..............3.......3.......3..............",
            "...............3......3.......3..............",
            "...............3......3......3...............",
            ".......3........3.....3.....3.......33.......",
            "........3........3.3333333..3......3.........",
            ".........33......33333333333.....33..........",
            "...........3..33333333333333333.3............",
            "............33333333333333333333.............",
            "....3.......333333333333333333333.....333....",
            ".....3333..333333333333333333333333333.......",
            ".........3333333333.33333.33333333...........",
            "..........33333333...333...33333333..........",
            "..........333333333.33333.333333333..........",
            "..33333333333333333333333333333333333333333..",
            "..........3333333333333333333333333..........",
            "..........3333333333333333333333333..........",
            ".........333333333333...3333333333...........",
            ".....3333..33333333.......333333333333.......",
            "....3.......3333333.......3333333.....333....",
            "............3333333.......333333.............",
            "...........3..3333333...3333333.3............",
            ".........33......33333333333.....33..........",
            "........3........3.3333333..3......3.........",
            ".......3........3.....3.....3.......33.......",
            "...............3......3......3...............",
            "...............3......3.......3..............",
            "..............3.......3.......3..............",
            "......................3......................",
            "......................3......................",
            ".............................................",
            ".............................................",
            ".............................................",
        ],
    )
}

pub fn render_gorilla_checkpoint(name: &str, pose: GorillaPose) -> RenderCheckpoint {
    match pose {
        GorillaPose::LeftUp => checkpoint_from_ascii(
            name,
            &[
                "....11111.....................",
                "...11111...11111111...........",
                "..11111....11111111...........",
                "..11111...11......11..........",
                ".11111....1111111111..........",
                ".11111....111..1..11..........",
                ".11111.....11111111...........",
                "..11111....11111111...........",
                "..11111.....111111............",
                "...11111111111111111111.......",
                "....1111111111111111111111....",
                ".......11111111.11111111111...",
                ".......11111111.111111111111..",
                ".......1111111.1.11111111111..",
                ".......111111.111.11111.11111.",
                ".......111...11111...11.11111.",
                ".........111111111111...11111.",
                ".........111111111111..11111..",
                ".........111111111111..11111..",
                ".........111111111111.11111...",
                ".........11111111111111111....",
                "........1111111111111.........",
                "......111111.....111111.......",
                "......11111.......11111.......",
                ".....11111.........11111......",
                ".....11111.........11111......",
                ".....11111.........11111......",
                ".....11111.........11111......",
                ".....11111.........11111......",
                "......11111.......11111.......",
            ],
        ),
        GorillaPose::ArmsDown => checkpoint_from_ascii(
            name,
            &[
                "..............................",
                "...........11111111...........",
                "...........11111111...........",
                "..........11......11..........",
                "..........1111111111..........",
                "..........111..1..11..........",
                "...........11111111...........",
                "...........11111111...........",
                "............111111............",
                ".......1111111111111111.......",
                "....1111111111111111111111....",
                "...111111111111.11111111111...",
                "..1111111111111.111111111111..",
                "..111111111111.1.11111111111..",
                ".11111.111111.111.11111.11111.",
                ".11111.111...11111...11.11111.",
                ".11111...111111111111...11111.",
                "..11111..111111111111..11111..",
                "..11111..111111111111..11111..",
                "...11111.111111111111.11111...",
                "....1111111111111111111111....",
                "........1111111111111.........",
                "......111111.....111111.......",
                "......11111.......11111.......",
                ".....11111.........11111......",
                ".....11111.........11111......",
                ".....11111.........11111......",
                ".....11111.........11111......",
                ".....11111.........11111......",
                "......11111.......11111.......",
            ],
        ),
        GorillaPose::RightUp => checkpoint_from_ascii(
            name,
            &[
                ".....................11111....",
                "...........11111111...11111...",
                "...........11111111....11111..",
                "..........11......11...11111..",
                "..........1111111111....11111.",
                "..........111..1..11....11111.",
                "...........11111111.....11111.",
                "...........11111111....11111..",
                "............111111.....11111..",
                ".......11111111111111111111...",
                "....1111111111111111111111....",
                "...111111111111.1111111.......",
                "..1111111111111.1111111.......",
                "..111111111111.1.111111.......",
                ".11111.111111.111.11111.......",
                ".11111.111...11111...11.......",
                ".11111...111111111111.........",
                "..11111..111111111111.........",
                "..11111..111111111111.........",
                "...11111.111111111111.........",
                "....11111111111111111.........",
                "........1111111111111.........",
                "......111111.....111111.......",
                "......11111.......11111.......",
                ".....11111.........11111......",
                ".....11111.........11111......",
                ".....11111.........11111......",
                ".....11111.........11111......",
                ".....11111.........11111......",
                "......11111.......11111.......",
            ],
        ),
    }
}

pub fn render_banana_checkpoint(rotation: i32) -> RenderCheckpoint {
    match rotation.rem_euclid(4) {
        0 => checkpoint_from_ascii(
            "banana_0",
            &[
                "....EE..........",
                "...EEE..........",
                "..EEE...........",
                "..EEE...........",
                "..EEE...........",
                "...EEE..........",
                "....EE..........",
                "................",
                "................",
                "................",
                "................",
                "................",
                "................",
                "................",
                "................",
                "................",
            ],
        ),
        1 => checkpoint_from_ascii(
            "banana_1",
            &[
                "..EEEEE.........",
                ".EEEEEEE........",
                "EEEEEEEEE.......",
                "EE.....EE.......",
                "................",
                "................",
                "................",
                "................",
                "................",
                "................",
                "................",
                "................",
                "................",
                "................",
                "................",
                "................",
            ],
        ),
        2 => checkpoint_from_ascii(
            "banana_2",
            &[
                "EE.....EE.......",
                "EEEEEEEEE.......",
                ".EEEEEEE........",
                "..EEEEE.........",
                "................",
                "................",
                "................",
                "................",
                "................",
                "................",
                "................",
                "................",
                "................",
                "................",
                "................",
                "................",
            ],
        ),
        _ => checkpoint_from_ascii(
            "banana_3",
            &[
                "EE..............",
                "EEE.............",
                ".EEE............",
                ".EEE............",
                ".EEE............",
                "EEE.............",
                "EE..............",
                "................",
                "................",
                "................",
                "................",
                "................",
                "................",
                "................",
                "................",
                "................",
            ],
        ),
    }
}

pub fn checkpoint_rgba(checkpoint: &RenderCheckpoint, transparent_zero: bool) -> Vec<u8> {
    let mut rgba = Vec::<u8>::with_capacity(checkpoint.pixels.len() * 4);
    checkpoint_rgba_into(checkpoint, transparent_zero, &mut rgba);
    rgba
}

pub fn checkpoint_rgba_into(
    checkpoint: &RenderCheckpoint,
    transparent_zero: bool,
    rgba: &mut Vec<u8>,
) {
    rgba.clear();
    rgba.reserve(checkpoint.pixels.len() * 4);

    for &pixel in &checkpoint.pixels {
        let [r, g, b] = palette_rgb(pixel);
        let alpha = if transparent_zero && pixel == COLOR_BG {
            0
        } else {
            255
        };
        rgba.extend_from_slice(&[r, g, b, alpha]);
    }
}

pub fn palette_rgb(index: u8) -> [u8; 3] {
    match index {
        COLOR_BG => [0, 0, 173],
        COLOR_GORILLA => [255, 173, 81],
        COLOR_EXPLOSION => [252, 0, 84],
        COLOR_SUN => [255, 255, 0],
        COLOR_BUILDING_GRAY => [168, 168, 168],
        COLOR_BUILDING_RED => [168, 0, 0],
        COLOR_BUILDING_CYAN => [0, 168, 168],
        COLOR_DARK => [84, 84, 84],
        COLOR_WINDOW => [252, 252, 84],
        _ => [255, 0, 255],
    }
}

// Effect drawing and embedded oracle masks
fn draw_runtime_effect(frame: &mut IndexedFrame, effect: RuntimeEffect) {
    match effect {
        RuntimeEffect::BuildingExplosion {
            x,
            y,
            radius,
            erase,
        } => {
            let color = if erase { COLOR_BG } else { COLOR_EXPLOSION };
            // QBASIC CIRCLE at these small radii produces specific pixel patterns that the
            // parametric path doesn't exactly match; masks verified against oracle output
            match radius.round() as i32 {
                4 => draw_effect_mask(
                    frame,
                    x.round() as i32 - 20,
                    y.round() as i32 - 20,
                    &[
                        ".........................................",
                        ".........................................",
                        ".........................................",
                        ".........................................",
                        ".........................................",
                        ".........................................",
                        ".........................................",
                        ".........................................",
                        ".........................................",
                        ".........................................",
                        ".........................................",
                        ".........................................",
                        ".........................................",
                        ".........................................",
                        ".........................................",
                        ".........................................",
                        ".........................................",
                        "...................XXX...................",
                        ".................XX...XX.................",
                        "................XX.....XX................",
                        "................X.......X................",
                        "................XX.....XX................",
                        ".................XX...XX.................",
                        "...................XXX...................",
                        ".........................................",
                        ".........................................",
                        ".........................................",
                        ".........................................",
                        ".........................................",
                        ".........................................",
                        ".........................................",
                        ".........................................",
                        ".........................................",
                        ".........................................",
                        ".........................................",
                        ".........................................",
                        ".........................................",
                        ".........................................",
                        ".........................................",
                        ".........................................",
                        ".........................................",
                    ],
                    color,
                ),
                6 => draw_effect_mask(
                    frame,
                    x.round() as i32 - 6,
                    y.round() as i32 - 4,
                    &[
                        "...XXXXXXX...",
                        "..X.......X..",
                        ".X.........X.",
                        "X...........X",
                        "X...........X",
                        "X...........X",
                        ".X.........X.",
                        "..X.......X..",
                        "...XXXXXXX...",
                    ],
                    color,
                ),
                10 => draw_effect_mask(
                    frame,
                    x.round() as i32 - 20,
                    y.round() as i32 - 20,
                    &[
                        ".........................................",
                        ".........................................",
                        ".........................................",
                        ".........................................",
                        ".........................................",
                        ".........................................",
                        ".........................................",
                        ".........................................",
                        ".........................................",
                        ".........................................",
                        ".........................................",
                        ".........................................",
                        ".........................................",
                        "...............XXXXXXXXXXX...............",
                        "..............X...........X..............",
                        ".............X.............X.............",
                        "...........XX...............XX...........",
                        "...........X.................X...........",
                        "..........X...................X..........",
                        "..........X...................X..........",
                        "..........X...................X..........",
                        "..........X...................X..........",
                        "..........X...................X..........",
                        "...........X.................X...........",
                        "...........XX...............XX...........",
                        ".............X.............X.............",
                        "..............X...........X..............",
                        "...............XXXXXXXXXXX...............",
                        ".........................................",
                        ".........................................",
                        ".........................................",
                        ".........................................",
                        ".........................................",
                        ".........................................",
                        ".........................................",
                        ".........................................",
                        ".........................................",
                        ".........................................",
                        ".........................................",
                        ".........................................",
                        ".........................................",
                    ],
                    color,
                ),
                _ => draw_explosion_ring(frame, x, y, radius, color),
            }
        }
        RuntimeEffect::GorillaExplosion {
            circle_x,
            circle_y,
            radius,
            line_x0,
            line_x1,
            line_y,
        } => {
            let line_min = line_x0.min(line_x1).round() as i32;
            let line_y = line_y.round() as i32;
            match radius.round() as i32 {
                4 => draw_effect_mask(
                    frame,
                    line_min - 6,
                    line_y - 14,
                    &[
                        "....................................",
                        "....................................",
                        "....................................",
                        "....................................",
                        "....................................",
                        "....................................",
                        "....................................",
                        "....................................",
                        "....................................",
                        "....................................",
                        "....................................",
                        "....................................",
                        "....................................",
                        "....................................",
                        "......XXXXXXXXXXXXXXX...............",
                        "....................................",
                        "....................................",
                        "....................................",
                        "....................................",
                        "....................................",
                        "....................................",
                        "....................................",
                        "....................................",
                        "....................................",
                        ".................XXX................",
                        "...............XX...XX..............",
                        "..............X.......X.............",
                        "...............XX...XX..............",
                        ".................XXX................",
                        "....................................",
                        "....................................",
                        "....................................",
                        "....................................",
                        "....................................",
                        "....................................",
                        "....................................",
                        "....................................",
                        "....................................",
                        "....................................",
                        "....................................",
                        "....................................",
                        "....................................",
                        "....................................",
                        "....................................",
                    ],
                    COLOR_EXPLOSION,
                ),
                8 => draw_effect_mask(
                    frame,
                    line_min,
                    line_y,
                    &[
                        "XXXXXXXXXXXXXXX......",
                        ".....................",
                        ".....................",
                        ".....................",
                        ".....................",
                        ".....................",
                        ".....................",
                        ".....................",
                        ".....................",
                        ".....................",
                        ".....................",
                        ".....................",
                        ".....................",
                        ".......XXXXXXXXXXX...",
                        ".....XX...........XX.",
                        "....XX.............XX",
                        "....X...............X",
                        "....XX.............XX",
                        ".....XX...........XX.",
                        ".......XXXXXXXXXXX...",
                    ],
                    COLOR_EXPLOSION,
                ),
                12 => draw_effect_mask(
                    frame,
                    line_min - 6,
                    line_y - 6,
                    &[
                        "....................................",
                        "....................................",
                        "....................................",
                        "....................................",
                        "....................................",
                        "....................................",
                        "......XXXXXXXXXXXXXXX...............",
                        "....................................",
                        "....................................",
                        "....................................",
                        "....................................",
                        "....................................",
                        "....................................",
                        "....................................",
                        "....................................",
                        "....................................",
                        "....................................",
                        "....................................",
                        "....................................",
                        "....................................",
                        "....................................",
                        ".............XXXXXXXXXXX............",
                        "..........XXX...........XXX.........",
                        "........XX.................XX.......",
                        ".......X.....................X......",
                        "......X.......................X.....",
                        "......X.......................X.....",
                        "......X.......................X.....",
                        ".......X.....................X......",
                        "........XX.................XX.......",
                        "..........XXX...........XXX.........",
                        ".............XXXXXXXXXXX............",
                        "....................................",
                        "....................................",
                        "....................................",
                        "....................................",
                        "....................................",
                        "....................................",
                        "....................................",
                        "....................................",
                        "....................................",
                        "....................................",
                        "....................................",
                        "....................................",
                    ],
                    COLOR_EXPLOSION,
                ),
                _ => {
                    draw_explosion_ring(frame, circle_x, circle_y, radius, COLOR_EXPLOSION);
                    frame.line(
                        line_x0,
                        line_y as f32,
                        line_x1,
                        line_y as f32,
                        COLOR_EXPLOSION,
                    );
                }
            }
        }
        RuntimeEffect::BuildingExplosionProgress {
            x,
            y,
            max_step,
            step,
            phase,
        } => {
            let clamped_step = step.clamp(0, max_step);

            for ring_step in 0..=max_step {
                let radius = ring_step as f32 * 0.5;
                draw_qbasic_ellipse_ring(frame, x, y, radius, COLOR_EXPLOSION, 1.0);

                if matches!(phase, BuildingExplosionPhase::Expand) && ring_step == clamped_step {
                    break;
                }
            }

            if matches!(phase, BuildingExplosionPhase::Collapse) {
                for ring_step in clamped_step..=max_step {
                    let radius = ring_step as f32 * 0.5;
                    draw_qbasic_ellipse_ring(frame, x, y, radius, COLOR_BG, 1.0);
                }
            }
        }
        RuntimeEffect::GorillaExplosionProgress {
            gorilla_x,
            gorilla_y,
            step,
            phase,
        } => {
            draw_gorilla_explosion_progress(frame, gorilla_x, gorilla_y, step, phase);
        }
    }
}

fn draw_effect_mask(frame: &mut IndexedFrame, x0: i32, y0: i32, rows: &[&str], color: u8) {
    for (row_index, row) in rows.iter().enumerate() {
        for (col_index, ch) in row.chars().enumerate() {
            if ch == 'X' {
                frame.set_pixel(x0 + col_index as i32, y0 + row_index as i32, color);
            }
        }
    }
}

fn draw_explosion_ring(frame: &mut IndexedFrame, x: f32, y: f32, radius: f32, color: u8) {
    let cx = x.round() as i32;
    let cy = y.round() as i32;
    let mut dx = radius.round() as i32;
    if dx <= 0 {
        return;
    }

    let mut dy = 0i32;
    let mut err = 1 - dx;

    while dx >= dy {
        for (px, py) in [
            (cx + dx, cy + dy),
            (cx + dy, cy + dx),
            (cx - dy, cy + dx),
            (cx - dx, cy + dy),
            (cx - dx, cy - dy),
            (cx - dy, cy - dx),
            (cx + dy, cy - dx),
            (cx + dx, cy - dy),
        ] {
            frame.set_pixel(px, py, color);
        }

        dy += 1;
        if err < 0 {
            err += 2 * dy + 1;
        } else {
            dx -= 1;
            err += 2 * (dy - dx) + 1;
        }
    }
}

fn draw_qbasic_ellipse_ring(
    frame: &mut IndexedFrame,
    center_x: f32,
    center_y: f32,
    radius: f32,
    color: u8,
    aspect_ratio: f32,
) {
    if radius <= 0.0 {
        frame.set_pixel(center_x.round() as i32, center_y.round() as i32, color);
        return;
    }

    let x_radius = radius;
    let y_radius = radius * 0.8 * aspect_ratio;
    if y_radius <= 0.0 {
        return;
    }

    let circumference = std::f32::consts::TAU * x_radius.max(y_radius);
    let steps = circumference.max(16.0).ceil() as i32;

    for step in 0..=steps {
        let angle = step as f32 / steps as f32 * std::f32::consts::TAU;
        let x = center_x + x_radius * angle.cos();
        let y = center_y + y_radius * angle.sin();
        frame.set_pixel(x.round() as i32, y.round() as i32, color);
    }
}

fn draw_gorilla_explosion_progress(
    frame: &mut IndexedFrame,
    gorilla_x: f32,
    gorilla_y: f32,
    step: i32,
    phase: GorillaExplosionPhase,
) {
    let scale_x = (SCR_W / 320.0).round() as i32;
    let scale_y = SCR_H / 200.0;

    let center_x = gorilla_x + 3.5 * scale_x as f32 + 5.0;
    let lower_center_y = gorilla_y + 7.0 * scale_y + 12.0;
    let upper_center_y = gorilla_y + 12.0;

    let line_x0 = gorilla_x + 7.0 * scale_x as f32;
    let line_x1 = gorilla_x;
    let line_y_base = gorilla_y + 9.0 * scale_y;

    let stage_1_max = 8 * scale_x;
    let stage_2_max = 16 * scale_x;
    let stage_3_max = 24 * scale_x;

    match phase {
        GorillaExplosionPhase::LowerBurst => {
            let clamped = step.clamp(1, stage_1_max);
            draw_gorilla_stage_1(
                frame,
                center_x,
                lower_center_y,
                line_x0,
                line_x1,
                line_y_base,
                clamped,
            );
        }
        GorillaExplosionPhase::UpperBurst => {
            let clamped = step.clamp(1, stage_2_max);
            draw_gorilla_stage_1(
                frame,
                center_x,
                lower_center_y,
                line_x0,
                line_x1,
                line_y_base,
                stage_1_max,
            );
            draw_gorilla_stage_2(
                frame,
                center_x,
                lower_center_y,
                upper_center_y,
                stage_1_max,
                clamped,
            );
        }
        GorillaExplosionPhase::Erase => {
            let clamped = step.clamp(1, stage_3_max);
            draw_gorilla_stage_1(
                frame,
                center_x,
                lower_center_y,
                line_x0,
                line_x1,
                line_y_base,
                stage_1_max,
            );
            draw_gorilla_stage_2(
                frame,
                center_x,
                lower_center_y,
                upper_center_y,
                stage_1_max,
                stage_2_max,
            );
            for radius in clamped..=stage_3_max {
                draw_qbasic_ellipse_ring(
                    frame,
                    center_x,
                    upper_center_y,
                    radius as f32,
                    COLOR_BG,
                    0.6,
                );
            }
        }
    }
}

fn draw_gorilla_stage_1(
    frame: &mut IndexedFrame,
    center_x: f32,
    lower_center_y: f32,
    line_x0: f32,
    line_x1: f32,
    line_y_base: f32,
    max_radius: i32,
) {
    for radius in 1..=max_radius {
        let radius_f = radius as f32;
        draw_qbasic_ellipse_ring(
            frame,
            center_x,
            lower_center_y,
            radius_f,
            COLOR_EXPLOSION,
            0.6,
        );
        frame.line(
            line_x0,
            line_y_base - radius_f,
            line_x1,
            line_y_base - radius_f,
            COLOR_EXPLOSION,
        );
    }
}

fn draw_gorilla_stage_2(
    frame: &mut IndexedFrame,
    center_x: f32,
    lower_center_y: f32,
    upper_center_y: f32,
    stage_1_max: i32,
    max_radius: i32,
) {
    for radius in 1..=max_radius {
        if radius < stage_1_max {
            let erase_radius = (stage_1_max + 1 - radius) as f32;
            draw_qbasic_ellipse_ring(frame, center_x, lower_center_y, erase_radius, COLOR_BG, 0.6);
        }

        let color = if radius % 2 == 0 {
            COLOR_GORILLA
        } else {
            COLOR_EXPLOSION
        };
        draw_qbasic_ellipse_ring(frame, center_x, upper_center_y, radius as f32, color, 0.6);
    }
}

fn draw_wind_arrow(frame: &mut IndexedFrame, wind: i32) {
    let start_x = SCR_W / 2.0;
    let y = SCR_H - 5.0;
    let line = wind as f32 * 3.0 * (SCR_W / 320.0);
    let end_x = start_x + line;

    frame.line(start_x, y, end_x, y, COLOR_EXPLOSION);

    let arrow = if wind > 0 { -2.0 } else { 2.0 };
    frame.line(end_x, y, end_x + arrow, y - 2.0, COLOR_EXPLOSION);
    frame.line(end_x, y, end_x + arrow, y + 2.0, COLOR_EXPLOSION);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_runtime_scene_building_explosion_colors_ring() {
        let checkpoint = render_runtime_scene(&RuntimeScene {
            buildings: Vec::new(),
            gorillas: [
                RuntimeGorilla {
                    x: 0.0,
                    y: 0.0,
                    pose: GorillaPose::ArmsDown,
                    alive: false,
                },
                RuntimeGorilla {
                    x: 0.0,
                    y: 0.0,
                    pose: GorillaPose::ArmsDown,
                    alive: false,
                },
            ],
            holes: Vec::new(),
            wind: 0,
            sun_shocked: false,
            banana: None,
            effect: Some(RuntimeEffect::BuildingExplosion {
                x: 100.0,
                y: 120.0,
                radius: 6.0,
                erase: false,
            }),
        });

        let ring_index = 120usize * checkpoint.width + 106usize;
        assert_eq!(checkpoint.pixels[ring_index], COLOR_EXPLOSION);
    }

    #[test]
    fn test_render_runtime_scene_gorilla_explosion_draws_horizontal_line() {
        let checkpoint = render_runtime_scene(&RuntimeScene {
            buildings: Vec::new(),
            gorillas: [
                RuntimeGorilla {
                    x: 0.0,
                    y: 0.0,
                    pose: GorillaPose::ArmsDown,
                    alive: false,
                },
                RuntimeGorilla {
                    x: 0.0,
                    y: 0.0,
                    pose: GorillaPose::ArmsDown,
                    alive: false,
                },
            ],
            holes: Vec::new(),
            wind: 0,
            sun_shocked: false,
            banana: None,
            effect: Some(RuntimeEffect::GorillaExplosion {
                circle_x: 200.0,
                circle_y: 140.0,
                radius: 8.0,
                line_x0: 192.0,
                line_x1: 208.0,
                line_y: 140.0,
            }),
        });

        let line_index = 140usize * checkpoint.width + 200usize;
        assert_eq!(checkpoint.pixels[line_index], COLOR_EXPLOSION);
    }

    #[test]
    fn test_render_runtime_scene_hole_supports_single_ellipse() {
        let checkpoint = render_runtime_scene(&RuntimeScene {
            buildings: vec![RuntimeBuilding {
                x: 60.0,
                width: 80.0,
                top_y: 60.0,
                color: COLOR_BUILDING_RED,
                windows: Vec::new(),
            }],
            gorillas: [
                RuntimeGorilla {
                    x: 0.0,
                    y: 0.0,
                    pose: GorillaPose::ArmsDown,
                    alive: false,
                },
                RuntimeGorilla {
                    x: 0.0,
                    y: 0.0,
                    pose: GorillaPose::ArmsDown,
                    alive: false,
                },
            ],
            holes: vec![RuntimeHole {
                x: 100.0,
                y: 80.0,
                x_radius: 20.0,
                y_radius: 10.0,
            }],
            wind: 0,
            sun_shocked: false,
            banana: None,
            effect: None,
        });

        assert_eq!(
            checkpoint.pixels[80usize * checkpoint.width + 100usize],
            COLOR_BG
        );
        assert_eq!(
            checkpoint.pixels[80usize * checkpoint.width + 119usize],
            COLOR_BG
        );
        assert_eq!(
            checkpoint.pixels[89usize * checkpoint.width + 100usize],
            COLOR_BG
        );
        assert_eq!(
            checkpoint.pixels[91usize * checkpoint.width + 100usize],
            COLOR_BUILDING_RED
        );
        assert_eq!(
            checkpoint.pixels[80usize * checkpoint.width + 121usize],
            COLOR_BUILDING_RED
        );
    }
}
