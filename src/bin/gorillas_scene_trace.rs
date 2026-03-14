use std::env;
use std::fs;
use std::path::PathBuf;

use gorillas::render_ref::{
    palette_rgb, render_runtime_scene, GorillaPose, RenderCheckpoint, RuntimeBanana,
    RuntimeBuilding, RuntimeEffect, RuntimeGorilla, RuntimeScene, RuntimeWindow,
    COLOR_BUILDING_CYAN, COLOR_BUILDING_GRAY, COLOR_BUILDING_RED, COLOR_DARK, COLOR_WINDOW,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = PathBuf::from(
        env::args()
            .nth(1)
            .ok_or("usage: cargo run --bin gorillas_scene_trace -- <output-dir>")?,
    );
    fs::create_dir_all(&output_dir)?;

    let checkpoints = sample_scene_checkpoints();

    for checkpoint in &checkpoints {
        let ppm_path = output_dir.join(format!("{}.ppm", checkpoint.name));
        fs::write(ppm_path, ppm_bytes(checkpoint))?;
    }

    let manifest = serde_json::to_string_pretty(&checkpoints)? + "\n";
    fs::write(output_dir.join("scene_checkpoints.json"), manifest)?;

    Ok(())
}

fn sample_scene_checkpoints() -> Vec<RenderCheckpoint> {
    let base = sample_scene();

    let full_scene = render_runtime_scene(&base);
    let projectile_frame = render_runtime_scene(&RuntimeScene {
        banana: Some(RuntimeBanana {
            x: 307.0,
            y: 124.0,
            rotation: 1,
        }),
        ..base.clone()
    });
    let building_hit_frame = render_runtime_scene(&sample_building_hit_scene(&base, 6.0));
    let gorilla_hit_frame = render_runtime_scene(&sample_gorilla_hit_scene(&base, 8.0));

    let sun_shocked_scene = render_runtime_scene(&RuntimeScene {
        sun_shocked: true,
        ..base.clone()
    });
    let right_up_pose_scene = render_runtime_scene(&RuntimeScene {
        gorillas: [
            base.gorillas[0],
            RuntimeGorilla {
                pose: GorillaPose::RightUp,
                ..base.gorillas[1]
            },
        ],
        ..base.clone()
    });

    let banana_rot_0_scene = render_runtime_scene(&RuntimeScene {
        banana: Some(RuntimeBanana {
            x: 307.0,
            y: 124.0,
            rotation: 0,
        }),
        ..base.clone()
    });
    let banana_rot_2_scene = render_runtime_scene(&RuntimeScene {
        banana: Some(RuntimeBanana {
            x: 307.0,
            y: 124.0,
            rotation: 2,
        }),
        ..base.clone()
    });
    let banana_rot_3_scene = render_runtime_scene(&RuntimeScene {
        banana: Some(RuntimeBanana {
            x: 307.0,
            y: 124.0,
            rotation: 3,
        }),
        ..base.clone()
    });

    let building_hit_r4_scene = render_runtime_scene(&sample_building_hit_scene(&base, 4.0));
    let building_hit_r10_scene = render_runtime_scene(&sample_building_hit_scene(&base, 10.0));

    let gorilla_hit_r4_scene = render_runtime_scene(&sample_gorilla_hit_scene(&base, 4.0));
    let gorilla_hit_r12_scene = render_runtime_scene(&sample_gorilla_hit_scene(&base, 12.0));

    vec![
        named_checkpoint("full_scene", full_scene),
        named_checkpoint("projectile_frame", projectile_frame),
        named_checkpoint("building_hit_frame", building_hit_frame),
        named_checkpoint("gorilla_hit_frame", gorilla_hit_frame),
        crop_checkpoint("sun_shocked_crop", &sun_shocked_scene, 298, 7, 45, 37),
        crop_checkpoint("right_up_pose_crop", &right_up_pose_scene, 460, 128, 30, 30),
        crop_checkpoint("banana_rot_0_crop", &banana_rot_0_scene, 307, 124, 16, 16),
        crop_checkpoint("banana_rot_2_crop", &banana_rot_2_scene, 307, 124, 16, 16),
        crop_checkpoint("banana_rot_3_crop", &banana_rot_3_scene, 307, 124, 16, 16),
        crop_checkpoint(
            "building_hit_r4_crop",
            &building_hit_r4_scene,
            198,
            172,
            41,
            41,
        ),
        crop_checkpoint(
            "building_hit_r10_crop",
            &building_hit_r10_scene,
            198,
            172,
            41,
            41,
        ),
        crop_checkpoint(
            "gorilla_hit_r4_crop",
            &gorilla_hit_r4_scene,
            454,
            126,
            36,
            44,
        ),
        crop_checkpoint(
            "gorilla_hit_r12_crop",
            &gorilla_hit_r12_scene,
            454,
            126,
            36,
            44,
        ),
    ]
}

fn sample_building_hit_scene(base: &RuntimeScene, radius: f32) -> RuntimeScene {
    RuntimeScene {
        effect: Some(RuntimeEffect::BuildingExplosion {
            x: 218.0,
            y: 192.0,
            radius,
            erase: false,
        }),
        ..base.clone()
    }
}

fn sample_gorilla_hit_scene(base: &RuntimeScene, radius: f32) -> RuntimeScene {
    RuntimeScene {
        effect: Some(RuntimeEffect::GorillaExplosion {
            circle_x: 472.0,
            circle_y: 152.25,
            radius,
            line_x0: 474.0,
            line_x1: 460.0,
            line_y: 143.75 - radius,
        }),
        ..base.clone()
    }
}

fn sample_scene() -> RuntimeScene {
    RuntimeScene {
        buildings: vec![
            RuntimeBuilding {
                x: 2.0,
                width: 86.0,
                top_y: 144.0,
                color: COLOR_BUILDING_GRAY,
                windows: building_windows(2.0, 86.0, 144.0),
            },
            RuntimeBuilding {
                x: 90.0,
                width: 78.0,
                top_y: 184.0,
                color: COLOR_BUILDING_RED,
                windows: building_windows(90.0, 78.0, 184.0),
            },
            RuntimeBuilding {
                x: 170.0,
                width: 104.0,
                top_y: 128.0,
                color: COLOR_BUILDING_CYAN,
                windows: building_windows(170.0, 104.0, 128.0),
            },
            RuntimeBuilding {
                x: 276.0,
                width: 92.0,
                top_y: 172.0,
                color: COLOR_BUILDING_GRAY,
                windows: building_windows(276.0, 92.0, 172.0),
            },
            RuntimeBuilding {
                x: 370.0,
                width: 110.0,
                top_y: 160.0,
                color: COLOR_BUILDING_RED,
                windows: building_windows(370.0, 110.0, 160.0),
            },
            RuntimeBuilding {
                x: 482.0,
                width: 70.0,
                top_y: 190.0,
                color: COLOR_BUILDING_CYAN,
                windows: building_windows(482.0, 70.0, 190.0),
            },
            RuntimeBuilding {
                x: 554.0,
                width: 84.0,
                top_y: 138.0,
                color: COLOR_BUILDING_GRAY,
                windows: building_windows(554.0, 84.0, 138.0),
            },
        ],
        gorillas: [
            RuntimeGorilla {
                x: 206.0,
                y: 98.0,
                pose: GorillaPose::LeftUp,
                alive: true,
            },
            RuntimeGorilla {
                x: 460.0,
                y: 128.0,
                pose: GorillaPose::ArmsDown,
                alive: true,
            },
        ],
        holes: Vec::new(),
        wind: -4,
        sun_shocked: false,
        banana: None,
        effect: None,
    }
}

fn building_windows(x: f32, width: f32, top_y: f32) -> Vec<RuntimeWindow> {
    let mut windows = Vec::new();
    let mut column = x + 3.0;
    let mut is_lit = true;
    let building_height = 335.0 - top_y;

    while column < x + width - 3.0 {
        let mut offset = building_height - 3.0;
        while offset >= 7.0 {
            windows.push(RuntimeWindow {
                x: column,
                y: 335.0 - offset,
                width: 3.0,
                height: 6.0,
                color: if is_lit { COLOR_WINDOW } else { COLOR_DARK },
            });
            is_lit = !is_lit;
            offset -= 15.0;
        }
        column += 10.0;
    }

    windows
}

fn named_checkpoint(name: &str, mut checkpoint: RenderCheckpoint) -> RenderCheckpoint {
    checkpoint.name = name.to_string();
    checkpoint
}

fn crop_checkpoint(
    name: &str,
    checkpoint: &RenderCheckpoint,
    x0: usize,
    y0: usize,
    width: usize,
    height: usize,
) -> RenderCheckpoint {
    assert!(x0 + width <= checkpoint.width);
    assert!(y0 + height <= checkpoint.height);

    let mut pixels = Vec::with_capacity(width * height);
    for row in 0..height {
        let src_start = (y0 + row) * checkpoint.width + x0;
        let src_end = src_start + width;
        pixels.extend_from_slice(&checkpoint.pixels[src_start..src_end]);
    }

    RenderCheckpoint {
        name: name.to_string(),
        width,
        height,
        pixels,
    }
}

fn ppm_bytes(checkpoint: &RenderCheckpoint) -> Vec<u8> {
    let mut payload = Vec::<u8>::new();
    payload.extend_from_slice(
        format!("P6\n{} {}\n255\n", checkpoint.width, checkpoint.height).as_bytes(),
    );

    for &pixel in &checkpoint.pixels {
        payload.extend_from_slice(&palette_rgb(pixel));
    }

    payload
}
