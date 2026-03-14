use std::cell::Cell;

use gorillas::play_ref::{
    canonical_play_sequence, sound_from_play_sequence as synth_play_wav, trace_named_sequence,
};
use gorillas::render_ref::{
    checkpoint_rgba, render_gorilla_checkpoint, GorillaPose, RenderCheckpoint,
};
use gorillas::{SCR_H, SCR_W};
use macroquad::audio::{load_sound_from_bytes, Sound};
use macroquad::prelude::*;
use macroquad::text::load_ttf_font_from_bytes;

#[derive(Clone, Debug)]
pub(crate) struct AudioFx {
    pub(crate) intro_theme: Sound,
    pub(crate) throw: Sound,
    pub(crate) building_explode: Sound,
    pub(crate) gorilla_explode: Sound,
    pub(crate) intro_fast: Sound,
    pub(crate) victory_dance: Sound,
    pub(crate) intro_riff_1: Sound,
    pub(crate) intro_riff_2: Sound,
    pub(crate) intro_riff_3: Sound,
    pub(crate) intro_riff_4: Sound,
    pub(crate) intro_fast_duration: f32,
    pub(crate) victory_dance_duration: f32,
    pub(crate) intro_riff_1_duration: f32,
    pub(crate) intro_riff_2_duration: f32,
    pub(crate) intro_riff_3_duration: f32,
    pub(crate) intro_riff_4_duration: f32,
}

pub(crate) struct VisualAssets {
    pub(crate) scene: Texture2D,
    pub(crate) gorilla_left: Texture2D,
    pub(crate) gorilla_right: Texture2D,
    pub(crate) gorilla_down: Texture2D,
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) virtual_target: RenderTarget,
    pub(crate) virtual_camera: Camera2D,
    pub(crate) ui_font: Font,
    fullscreen_enabled: Cell<bool>,
}

impl VisualAssets {
    pub(crate) fn begin_frame(&self) {
        handle_fullscreen_shortcut(self);
        set_camera(&self.virtual_camera);
    }

    pub(crate) fn present_frame(&self) {
        #[cfg(target_arch = "wasm32")]
        {
            set_default_camera();
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            set_default_camera();
            clear_background(BLACK);

            let scale = f32::min(screen_width() / SCR_W, screen_height() / SCR_H);
            let dest_width = SCR_W * scale;
            let dest_height = SCR_H * scale;
            let dest_x = (screen_width() - dest_width) * 0.5;
            let dest_y = (screen_height() - dest_height) * 0.5;

            draw_texture_ex(
                &self.virtual_target.texture,
                dest_x,
                dest_y,
                WHITE,
                DrawTextureParams {
                    dest_size: Some(vec2(dest_width, dest_height)),
                    flip_y: true,
                    ..Default::default()
                },
            );
        }
    }
}

#[cfg(target_os = "macos")]
fn fullscreen_shortcut_pressed() -> bool {
    is_key_pressed(KeyCode::F)
        && (is_key_down(KeyCode::LeftControl) || is_key_down(KeyCode::RightControl))
        && (is_key_down(KeyCode::LeftSuper) || is_key_down(KeyCode::RightSuper))
}

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "macos")))]
fn fullscreen_shortcut_pressed() -> bool {
    is_key_pressed(KeyCode::F11)
}

#[cfg(target_arch = "wasm32")]
fn fullscreen_shortcut_pressed() -> bool {
    false
}

fn handle_fullscreen_shortcut(visuals: &VisualAssets) {
    if fullscreen_shortcut_pressed() {
        let next_fullscreen_state = !visuals.fullscreen_enabled.get();
        set_fullscreen(next_fullscreen_state);
        visuals.fullscreen_enabled.set(next_fullscreen_state);
    }
}

pub(crate) fn init_visuals() -> VisualAssets {
    let scene = Texture2D::from_rgba8(
        SCR_W as u16,
        SCR_H as u16,
        &vec![0u8; SCR_W as usize * SCR_H as usize * 4],
    );
    scene.set_filter(FilterMode::Nearest);

    #[cfg(target_arch = "wasm32")]
    let virtual_camera = Camera2D::from_display_rect(Rect::new(0.0, SCR_H, SCR_W, -SCR_H));

    #[cfg(not(target_arch = "wasm32"))]
    let (virtual_camera, virtual_target) = {
        let mut virtual_camera = Camera2D::from_display_rect(Rect::new(0.0, 0.0, SCR_W, SCR_H));
        let virtual_target = render_target(SCR_W as u32, SCR_H as u32);
        virtual_target.texture.set_filter(FilterMode::Nearest);
        virtual_camera.render_target = Some(virtual_target.clone());
        (virtual_camera, virtual_target)
    };

    let gorilla_left = texture_from_checkpoint(
        &render_gorilla_checkpoint("intro_gorilla_left", GorillaPose::LeftUp),
        true,
    );
    let gorilla_right = texture_from_checkpoint(
        &render_gorilla_checkpoint("intro_gorilla_right", GorillaPose::RightUp),
        true,
    );
    let gorilla_down = texture_from_checkpoint(
        &render_gorilla_checkpoint("intro_gorilla_down", GorillaPose::ArmsDown),
        true,
    );
    let mut ui_font =
        load_ttf_font_from_bytes(include_bytes!("../../assets/fonts/Ac437_IBM_VGA_9x16.ttf"))
            .expect("failed to load bundled DOS UI font");
    ui_font.set_filter(FilterMode::Nearest);

    VisualAssets {
        scene,
        gorilla_left,
        gorilla_right,
        gorilla_down,
        #[cfg(not(target_arch = "wasm32"))]
        virtual_target,
        virtual_camera,
        ui_font,
        fullscreen_enabled: Cell::new(false),
    }
}

fn texture_from_checkpoint(checkpoint: &RenderCheckpoint, transparent_zero: bool) -> Texture2D {
    let rgba = checkpoint_rgba(checkpoint, transparent_zero);
    let texture = Texture2D::from_rgba8(checkpoint.width as u16, checkpoint.height as u16, &rgba);
    texture.set_filter(FilterMode::Nearest);
    texture
}

pub(crate) async fn init_audio() -> AudioFx {
    let sequence = |name: &str| {
        canonical_play_sequence(name)
            .unwrap_or_else(|error| panic!("missing canonical PLAY sequence '{name}': {error}"))
    };
    let synth = |name: &str, volume: f32| {
        synth_play_wav(sequence(name), volume).unwrap_or_else(|error| {
            panic!("failed to synthesize canonical PLAY sequence '{name}': {error}")
        })
    };
    let duration = |name: &str| {
        trace_named_sequence(name)
            .unwrap_or_else(|error| {
                panic!("failed to trace canonical PLAY sequence '{name}': {error}")
            })
            .total_duration_seconds
    };

    let intro_theme = load_sound_from_bytes(&synth("intro_theme", 0.28))
        .await
        .expect("failed to create intro theme");
    let throw = load_sound_from_bytes(&synth("throw", 0.30))
        .await
        .expect("failed to create throw sound");
    let building_explode = load_sound_from_bytes(&synth("building_explosion", 0.40))
        .await
        .expect("failed to create building explosion sound");
    let gorilla_explode = load_sound_from_bytes(&synth("gorilla_explosion", 0.40))
        .await
        .expect("failed to create gorilla explosion sound");
    let intro_fast = load_sound_from_bytes(&synth("intro_fast_left", 0.28))
        .await
        .expect("failed to create intro fast sound");
    let victory_dance = load_sound_from_bytes(&synth("victory_dance_left", 0.28))
        .await
        .expect("failed to create victory dance sound");

    let intro_riff_1 = load_sound_from_bytes(&synth("intro_riff_1", 0.28))
        .await
        .expect("failed to create intro riff 1");
    let intro_riff_2 = load_sound_from_bytes(&synth("intro_riff_2", 0.28))
        .await
        .expect("failed to create intro riff 2");
    let intro_riff_3 = load_sound_from_bytes(&synth("intro_riff_3", 0.28))
        .await
        .expect("failed to create intro riff 3");
    let intro_riff_4 = load_sound_from_bytes(&synth("intro_riff_4", 0.28))
        .await
        .expect("failed to create intro riff 4");

    AudioFx {
        intro_theme,
        throw,
        building_explode,
        gorilla_explode,
        intro_fast,
        victory_dance,
        intro_riff_1,
        intro_riff_2,
        intro_riff_3,
        intro_riff_4,
        intro_fast_duration: duration("intro_fast_left"),
        victory_dance_duration: duration("victory_dance_left"),
        intro_riff_1_duration: duration("intro_riff_1"),
        intro_riff_2_duration: duration("intro_riff_2"),
        intro_riff_3_duration: duration("intro_riff_3"),
        intro_riff_4_duration: duration("intro_riff_4"),
    }
}
