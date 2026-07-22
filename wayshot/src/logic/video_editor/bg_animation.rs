use crate::{
    db::{BackgroundAnimationConfigData, VIDEO_EDITOR_TABLE},
    global_store,
    logic::{
        recorder::picker_directory_with_location,
        toast,
        tr::tr,
        video_editor::{playlist::import_file_to_playlist, project::BG_ANIMATION_CONFIG_ID},
    },
    logic_cb,
    slint_generatedAppWindow::{AnimationType, AppWindow, BackgroundAnimationConfig},
};
use background_animation::{
    Animation, AnimationPreviewConfig, AnimationRecordConfig, FlowDirection,
    black_hole::BlackHoleConfig,
    bokeh::BokehConfig,
    cross_line::CrossLineConfig,
    flow_field::FlowFieldConfig,
    fluid::{FluidConfig, ForceSource},
    galaxy::GalaxyConfig,
    glitch::GlitchConfig,
    grid::GridConfig,
    ink::InkDissipationConfig,
    kaleidoscope::KaleidoscopeConfig,
    light_effects::LightEffectsConfig,
    matrix_rain::MatrixRainConfig,
    moving_grid::MovingGridConfig,
    noise_flow::{ColorPalette, NoiseFlowConfig},
    particle_life::ParticleLifeConfig,
    particle_network::ParticleNetworkConfig,
    shape::ShapeConfig,
    triangle::TriangleConfig,
    wave::WaveConfig,
};
use slint::{ComponentHandle, Image, Weak};
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

static PREVIEW_STOP_SIG: Mutex<Option<Arc<AtomicBool>>> = Mutex::new(None);
static RECORD_STOP_SIG: Mutex<Option<Arc<AtomicBool>>> = Mutex::new(None);

crate::impl_slint_enum_serde!(
    AnimationType,
    Grid,
    MovingGrid,
    Glitch,
    NoiseFlow,
    Bokeh,
    MatrixRain,
    Fluid,
    Kaleidoscope,
    LightEffects,
    InkDissipation,
    ParticleLife,
    ParticleNetwork,
    FlowField,
    Shape,
    BlackHole,
    Galaxy,
    Triangle,
    Wave,
    CrossLine
);

// Helper functions for parsing enum strings (enums don't implement FromStr/Display)
fn parse_force_source(s: &str) -> ForceSource {
    match s {
        "Circular" => ForceSource::Circular,
        "Vortices" => ForceSource::Vortices,
        _ => ForceSource::Random,
    }
}

fn force_source_to_string(source: ForceSource) -> String {
    match source {
        ForceSource::Random => "Random",
        ForceSource::Circular => "Circular",
        ForceSource::Vortices => "Vortices",
        ForceSource::MouseDriven => "Random", // fallback
    }
    .to_string()
}

pub fn init(ui: &AppWindow) {
    inner_init(ui);

    logic_cb!(video_editor_bg_animation_start_preview, ui, config);
    logic_cb!(video_editor_bg_animation_stop_preview, ui);
    logic_cb!(video_editor_bg_animation_start_record, ui, config);
    logic_cb!(video_editor_bg_animation_stop_record, ui);
}

fn inner_init(ui: &AppWindow) {
    let ui_weak = ui.as_weak();

    tokio::spawn(async move {
        let config = match sqldb::entry::select(VIDEO_EDITOR_TABLE, BG_ANIMATION_CONFIG_ID).await {
            Ok(entry) => serde_json::from_str::<BackgroundAnimationConfigData>(&entry.data)
                .unwrap_or_default(),
            Err(_) => BackgroundAnimationConfigData::default(),
        };

        _ = ui_weak.upgrade_in_event_loop(move |ui| {
            let config: BackgroundAnimationConfig = config.into();
            global_store!(ui).set_video_editor_bg_animation_config(config);
        });
    });
}

fn video_editor_bg_animation_start_preview(ui: &AppWindow, config: BackgroundAnimationConfig) {
    stop_preview_thread();

    let ui_weak = ui.as_weak();
    let data: BackgroundAnimationConfigData = config.into();
    let width = data.width as u32;
    let height = data.height as u32;
    let fps = data.fps as u32;

    let anim_config = AnimationPreviewConfig::new(width, height, fps);
    let frame_receiver = anim_config.receiver();
    let stop_sig = anim_config.stop_sig();
    *PREVIEW_STOP_SIG.lock().unwrap() = Some(stop_sig.clone());

    global_store!(ui).set_video_editor_bg_animation_is_previewing(true);

    let data_clone = data.clone();
    tokio::spawn(async move {
        save_bg_animation_config(&data_clone).await;
    });

    std::thread::spawn(move || {
        run_animation_loop(&data, anim_config);
    });

    std::thread::spawn(move || {
        loop {
            if stop_sig.load(Ordering::SeqCst) {
                break;
            }

            if let Ok(frame) = frame_receiver.recv_timeout(Duration::from_millis(100)) {
                let pixels: Vec<u8> = frame.into_raw();
                let stop_sig_clone = stop_sig.clone();

                _ = ui_weak.upgrade_in_event_loop(move |ui| {
                    if !stop_sig_clone.load(Ordering::SeqCst) {
                        let image = Image::from_rgba8(
                            slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(
                                &pixels, width, height,
                            ),
                        );

                        global_store!(ui).set_video_editor_bg_animation_preview_image(image);
                    }
                });
            }
        }
    });
}

fn video_editor_bg_animation_stop_preview(ui: &AppWindow) {
    stop_preview_thread();
    global_store!(ui).set_video_editor_bg_animation_is_previewing(false);
    global_store!(ui).set_video_editor_bg_animation_preview_image(Image::default());
}

fn stop_preview_thread() {
    if let Some(sig) = PREVIEW_STOP_SIG.lock().unwrap().take() {
        sig.store(true, Ordering::SeqCst);
    }
}

fn run_animation_loop(config: &BackgroundAnimationConfigData, anim_config: AnimationPreviewConfig) {
    match config.animation_type {
        AnimationType::Grid => {
            let mut grid_config = GridConfig::new()
                .with_rows(config.grid.rows as usize)
                .with_cols(config.grid.cols as usize)
                .with_amplitude(config.grid.amplitude)
                .with_node_amplitude(config.grid.node_amplitude)
                .with_frequency(config.grid.frequency)
                .with_node_radius(config.grid.node_radius)
                .with_line_color((&config.grid).into())
                .with_line_width(config.grid.line_width)
                .with_bg_color((&config.grid).into())
                .with_node_color((
                    config.grid.node_color_r as u8,
                    config.grid.node_color_g as u8,
                    config.grid.node_color_b as u8,
                    config.grid.node_color_a as u8,
                ));
            _ = grid_config.animate_preview(anim_config);
        }
        AnimationType::MovingGrid => {
            let mut moving_grid_config = MovingGridConfig::new()
                .with_rows(config.moving_grid.rows as usize)
                .with_cols(config.moving_grid.cols as usize)
                .with_speed(config.moving_grid.speed)
                .with_direction(
                    config
                        .moving_grid
                        .direction
                        .parse()
                        .unwrap_or(FlowDirection::Up),
                )
                .with_line_color((
                    config.moving_grid.line_color_r as u8,
                    config.moving_grid.line_color_g as u8,
                    config.moving_grid.line_color_b as u8,
                    config.moving_grid.line_color_a as u8,
                ))
                .with_line_width(config.moving_grid.line_width)
                .with_bg_color((
                    config.moving_grid.bg_color_r as u8,
                    config.moving_grid.bg_color_g as u8,
                    config.moving_grid.bg_color_b as u8,
                ))
                .with_supersample(config.moving_grid.supersample as u32);
            _ = moving_grid_config.animate_preview(anim_config);
        }
        AnimationType::Glitch => {
            let mut glitch_config = GlitchConfig::new()
                .with_intensity(config.glitch.intensity)
                .with_scan_lines_enabled(config.glitch.scan_lines_enabled)
                .with_scan_line_spacing(config.glitch.scan_line_spacing as u32)
                .with_rgb_split_enabled(config.glitch.rgb_split_enabled)
                .with_rgb_split_offset(config.glitch.rgb_split_offset)
                .with_block_shift_enabled(config.glitch.block_shift_enabled)
                .with_block_shift_max_offset(config.glitch.block_shift_max_offset)
                .with_noise_enabled(config.glitch.noise_enabled)
                .with_animation_speed(config.glitch.animation_speed)
                .with_bg_color((
                    config.glitch.bg_color_r as u8,
                    config.glitch.bg_color_g as u8,
                    config.glitch.bg_color_b as u8,
                ));
            _ = glitch_config.animate_preview(anim_config);
        }
        AnimationType::NoiseFlow => {
            let palette = ColorPalette::new(
                config
                    .noise_flow
                    .palette_r
                    .iter()
                    .enumerate()
                    .map(|(i, r)| {
                        (
                            *r as u8,
                            config.noise_flow.palette_g.get(i).copied().unwrap_or(0) as u8,
                            config.noise_flow.palette_b.get(i).copied().unwrap_or(0) as u8,
                            config.noise_flow.palette_a.get(i).copied().unwrap_or(255) as u8,
                        )
                    })
                    .collect(),
            );
            let mut noise_config = NoiseFlowConfig::new()
                .with_noise_scale(config.noise_flow.noise_scale)
                .with_animation_speed(config.noise_flow.animation_speed)
                .with_color_palette(palette)
                .with_bg_color((
                    config.noise_flow.bg_color_r as u8,
                    config.noise_flow.bg_color_g as u8,
                    config.noise_flow.bg_color_b as u8,
                ))
                .with_flow_direction(
                    config
                        .noise_flow
                        .flow_direction
                        .parse()
                        .unwrap_or(FlowDirection::Right),
                );
            _ = noise_config.animate_preview(anim_config);
        }
        AnimationType::Bokeh => {
            let colors: Vec<(u8, u8, u8, u8)> = config
                .bokeh
                .colors_r
                .iter()
                .enumerate()
                .map(|(i, r)| {
                    (
                        *r as u8,
                        config.bokeh.colors_g.get(i).copied().unwrap_or(255) as u8,
                        config.bokeh.colors_b.get(i).copied().unwrap_or(255) as u8,
                        config.bokeh.colors_a.get(i).copied().unwrap_or(255) as u8,
                    )
                })
                .collect();
            let mut bokeh_config = BokehConfig::new()
                .with_spot_count(config.bokeh.spot_count as usize)
                .with_min_size(config.bokeh.min_size)
                .with_max_size(config.bokeh.max_size)
                .with_blur_radius(config.bokeh.blur_radius)
                .with_animation_speed(config.bokeh.animation_speed)
                .with_hexagonal_enabled(config.bokeh.hexagonal_enabled)
                .with_colors(colors)
                .with_bg_color((
                    config.bokeh.bg_color_r as u8,
                    config.bokeh.bg_color_g as u8,
                    config.bokeh.bg_color_b as u8,
                ));
            _ = bokeh_config.animate_preview(anim_config);
        }
        AnimationType::MatrixRain => {
            let mut matrix_config = MatrixRainConfig::new()
                .with_columns(config.matrix_rain.columns as usize)
                .with_cell_size(config.matrix_rain.cell_size as u32)
                .with_min_speed(config.matrix_rain.min_speed)
                .with_max_speed(config.matrix_rain.max_speed)
                .with_trail_length(config.matrix_rain.trail_length as usize)
                .with_fade_speed(config.matrix_rain.fade_speed)
                .with_color((
                    config.matrix_rain.color_r as u8,
                    config.matrix_rain.color_g as u8,
                    config.matrix_rain.color_b as u8,
                ))
                .with_bg_color((
                    config.matrix_rain.bg_color_r as u8,
                    config.matrix_rain.bg_color_g as u8,
                    config.matrix_rain.bg_color_b as u8,
                ))
                .with_glow_intensity(config.matrix_rain.glow_intensity)
                .with_char_change_prob(config.matrix_rain.char_change_prob)
                .with_flicker_prob(config.matrix_rain.flicker_prob)
                .with_particle_density(config.matrix_rain.particle_density as u32);
            _ = matrix_config.animate_preview(anim_config);
        }
        AnimationType::Fluid => {
            let colors: Vec<(u8, u8, u8)> = config
                .fluid
                .colors_r
                .iter()
                .enumerate()
                .map(|(i, r)| {
                    (
                        *r as u8,
                        config.fluid.colors_g.get(i).copied().unwrap_or(0) as u8,
                        config.fluid.colors_b.get(i).copied().unwrap_or(0) as u8,
                    )
                })
                .collect();
            let force_source = parse_force_source(&config.fluid.force_source);
            let mut fluid_config = FluidConfig::new()
                .with_resolution_divisor(config.fluid.resolution_divisor as u32)
                .with_viscosity(config.fluid.viscosity)
                .with_diffusion(config.fluid.diffusion)
                .with_force_source(force_source)
                .with_num_sources(config.fluid.num_sources as usize)
                .with_steps_per_frame(config.fluid.steps_per_frame as usize)
                .with_color_injection(config.fluid.color_injection)
                .with_colors(colors)
                .with_bg_color((
                    config.fluid.bg_color_r as u8,
                    config.fluid.bg_color_g as u8,
                    config.fluid.bg_color_b as u8,
                ));
            _ = fluid_config.animate_preview(anim_config);
        }
        AnimationType::Kaleidoscope => {
            let colors: Vec<(u8, u8, u8)> = config
                .kaleidoscope
                .colors_r
                .iter()
                .enumerate()
                .map(|(i, r)| {
                    (
                        *r as u8,
                        config.kaleidoscope.colors_g.get(i).copied().unwrap_or(0) as u8,
                        config.kaleidoscope.colors_b.get(i).copied().unwrap_or(0) as u8,
                    )
                })
                .collect();
            let mut kaleidoscope_config = KaleidoscopeConfig::new()
                .with_segments(config.kaleidoscope.segments as usize)
                .with_rotation_speed(config.kaleidoscope.rotation_speed)
                .with_center((config.kaleidoscope.center_x, config.kaleidoscope.center_y))
                .with_scale(config.kaleidoscope.scale)
                .with_complexity(config.kaleidoscope.complexity as usize)
                .with_colors(colors)
                .with_bg_color((
                    config.kaleidoscope.bg_color_r as u8,
                    config.kaleidoscope.bg_color_g as u8,
                    config.kaleidoscope.bg_color_b as u8,
                ));
            _ = kaleidoscope_config.animate_preview(anim_config);
        }
        AnimationType::LightEffects => {
            let colors: Vec<(u8, u8, u8, u8)> = config
                .light_effects
                .colors_r
                .iter()
                .enumerate()
                .map(|(i, r)| {
                    (
                        *r as u8,
                        config.light_effects.colors_g.get(i).copied().unwrap_or(255) as u8,
                        config.light_effects.colors_b.get(i).copied().unwrap_or(255) as u8,
                        config.light_effects.colors_a.get(i).copied().unwrap_or(255) as u8,
                    )
                })
                .collect();
            let mut light_effects_config = LightEffectsConfig::new()
                .with_flare_count(config.light_effects.flare_count as usize)
                .with_min_size(config.light_effects.min_size)
                .with_max_size(config.light_effects.max_size)
                .with_movement_speed(config.light_effects.movement_speed)
                .with_elliptical_enabled(config.light_effects.elliptical_enabled)
                .with_bands_enabled(config.light_effects.bands_enabled)
                .with_colors(colors)
                .with_bg_color((
                    config.light_effects.bg_color_r as u8,
                    config.light_effects.bg_color_g as u8,
                    config.light_effects.bg_color_b as u8,
                ));
            _ = light_effects_config.animate_preview(anim_config);
        }
        AnimationType::InkDissipation => {
            let mut ink_config = InkDissipationConfig::new()
                .with_style(config.ink_dissipation.style)
                .with_source_count(config.ink_dissipation.source_count as usize)
                .with_spawn_rate(config.ink_dissipation.spawn_rate)
                .with_source_lifetime(config.ink_dissipation.source_lifetime as u32)
                .with_initial_radius(config.ink_dissipation.initial_radius)
                .with_max_radius(config.ink_dissipation.max_radius)
                .with_spread_rate(config.ink_dissipation.spread_rate)
                .with_diffusion_strength(config.ink_dissipation.diffusion_strength)
                .with_fade_speed(config.ink_dissipation.fade_speed)
                .with_max_drops(config.ink_dissipation.max_drops as usize)
                .with_resolution_divisor(config.ink_dissipation.resolution_divisor as u32);
            _ = ink_config.animate_preview(anim_config);
        }
        AnimationType::ParticleLife => {
            let mut particle_life_config = ParticleLifeConfig::new()
                .with_particle_count(config.particle_life.particle_count as usize)
                .with_type_count(config.particle_life.type_count as usize)
                .with_rmax(config.particle_life.rmax)
                .with_friction(config.particle_life.friction)
                .with_force(config.particle_life.force)
                .with_dt(config.particle_life.dt)
                .with_wrap(config.particle_life.wrap)
                .with_particle_size(config.particle_life.particle_size)
                .with_bg_color((
                    config.particle_life.bg_color_r as u8,
                    config.particle_life.bg_color_g as u8,
                    config.particle_life.bg_color_b as u8,
                ))
                .with_matrix_seed(config.particle_life.matrix_seed as u64);
            _ = particle_life_config.animate_preview(anim_config);
        }
        AnimationType::ParticleNetwork => {
            let mut particle_config = ParticleNetworkConfig::new()
                .with_density(config.particle_network.density as u32)
                .with_line_color((
                    config.particle_network.line_color_r as u8,
                    config.particle_network.line_color_g as u8,
                    config.particle_network.line_color_b as u8,
                    config.particle_network.line_color_a as u8,
                ))
                .with_particle_color((
                    config.particle_network.particle_color_r as u8,
                    config.particle_network.particle_color_g as u8,
                    config.particle_network.particle_color_b as u8,
                    config.particle_network.particle_color_a as u8,
                ))
                .with_bg_color((
                    config.particle_network.bg_color_r as u8,
                    config.particle_network.bg_color_g as u8,
                    config.particle_network.bg_color_b as u8,
                ))
                .with_pointer_enabled(config.particle_network.pointer_enabled)
                .with_pointer_range(config.particle_network.pointer_range)
                .with_pointer_count(config.particle_network.pointer_count as usize);
            _ = particle_config.animate_preview(anim_config);
        }
        AnimationType::FlowField => {
            let mut flow_config = FlowFieldConfig::new()
                .with_color((
                    config.flow_field.color_r as u8,
                    config.flow_field.color_g as u8,
                    config.flow_field.color_b as u8,
                    config.flow_field.color_a as u8,
                ))
                .with_bg_color((
                    config.flow_field.bg_color_r as u8,
                    config.flow_field.bg_color_g as u8,
                    config.flow_field.bg_color_b as u8,
                ))
                .with_trail_opacity(config.flow_field.trail_opacity)
                .with_particle_count(config.flow_field.particle_count as u32)
                .with_speed(config.flow_field.speed)
                .with_pointer_enabled(config.flow_field.pointer_enabled)
                .with_pointer_count(config.flow_field.pointer_count as usize);
            _ = flow_config.animate_preview(anim_config);
        }
        AnimationType::Shape => {
            let mut shape_config = ShapeConfig::new()
                .with_max_circles(config.shape.max_circles as usize)
                .with_rad_min(config.shape.rad_min)
                .with_rad_max(config.shape.rad_max)
                .with_filled_circle_pct(config.shape.filled_circle_pct as u32)
                .with_concentric_circle_pct(config.shape.concentric_circle_pct as u32)
                .with_rad_threshold(config.shape.rad_threshold)
                .with_speed_min(config.shape.speed_min)
                .with_speed_max(config.shape.speed_max)
                .with_max_opacity(config.shape.max_opacity)
                .with_circle_border(config.shape.circle_border)
                .with_background_mult(config.shape.background_mult)
                .with_line_border(config.shape.line_border)
                .with_link_dist_fraction(config.shape.link_dist_fraction)
                .with_bg_color((
                    config.shape.bg_color_r as u8,
                    config.shape.bg_color_g as u8,
                    config.shape.bg_color_b as u8,
                ));
            _ = shape_config.animate_preview(anim_config);
        }
        AnimationType::BlackHole => {
            let mut black_hole_config = BlackHoleConfig::new()
                .with_star_count(config.black_hole.star_count as usize)
                .with_black_hole_size(config.black_hole.black_hole_size)
                .with_event_horizon_offset(config.black_hole.event_horizon_offset)
                .with_max_consume_frames(config.black_hole.max_consume_frames as usize)
                .with_hue_speed(config.black_hole.hue_speed)
                .with_star_saturation(config.black_hole.star_saturation)
                .with_star_lightness(config.black_hole.star_lightness)
                .with_trail_alpha(config.black_hole.trail_alpha)
                .with_trail_color((
                    config.black_hole.trail_color_r as u8,
                    config.black_hole.trail_color_g as u8,
                    config.black_hole.trail_color_b as u8,
                ))
                .with_bg_color((
                    config.black_hole.bg_color_r as u8,
                    config.black_hole.bg_color_g as u8,
                    config.black_hole.bg_color_b as u8,
                ))
                .with_center_x(config.black_hole.center_x)
                .with_center_y(config.black_hole.center_y)
                .with_hole_stroke_color((
                    config.black_hole.hole_stroke_color_r as u8,
                    config.black_hole.hole_stroke_color_g as u8,
                    config.black_hole.hole_stroke_color_b as u8,
                ))
                .with_hole_inner_color((
                    config.black_hole.hole_inner_color_r as u8,
                    config.black_hole.hole_inner_color_g as u8,
                    config.black_hole.hole_inner_color_b as u8,
                ))
                .with_hole_mid_color((
                    config.black_hole.hole_mid_color_r as u8,
                    config.black_hole.hole_mid_color_g as u8,
                    config.black_hole.hole_mid_color_b as u8,
                ))
                .with_hole_outer_color((
                    config.black_hole.hole_outer_color_r as u8,
                    config.black_hole.hole_outer_color_g as u8,
                    config.black_hole.hole_outer_color_b as u8,
                ));
            _ = black_hole_config.animate_preview(anim_config);
        }
        AnimationType::Galaxy => {
            let mut galaxy_config = GalaxyConfig::new()
                .with_star_count(config.galaxy.star_count as usize)
                .with_rotation_period(config.galaxy.rotation_period)
                .with_appear_duration(config.galaxy.appear_duration)
                .with_breathing_period(config.galaxy.breathing_period)
                .with_breathing_min(config.galaxy.breathing_min)
                .with_perspective(config.galaxy.perspective)
                .with_glow_intensity(config.galaxy.glow_intensity)
                .with_min_distance(config.galaxy.min_distance)
                .with_max_distance(config.galaxy.max_distance)
                .with_min_size(config.galaxy.min_size)
                .with_max_size(config.galaxy.max_size)
                .with_bg_color((
                    config.galaxy.bg_color_r as u8,
                    config.galaxy.bg_color_g as u8,
                    config.galaxy.bg_color_b as u8,
                ));
            _ = galaxy_config.animate_preview(anim_config);
        }
        AnimationType::Triangle => {
            let mut triangle_config = TriangleConfig::new()
                .with_triangle_size(config.triangle.triangle_size)
                .with_bleed(config.triangle.bleed)
                .with_noise(config.triangle.noise)
                .with_color1((
                    config.triangle.color1_r as u8,
                    config.triangle.color1_g as u8,
                    config.triangle.color1_b as u8,
                ))
                .with_color2((
                    config.triangle.color2_r as u8,
                    config.triangle.color2_g as u8,
                    config.triangle.color2_b as u8,
                ))
                .with_stroke_color((
                    config.triangle.stroke_color_r as u8,
                    config.triangle.stroke_color_g as u8,
                    config.triangle.stroke_color_b as u8,
                    config.triangle.stroke_color_a as u8,
                ))
                .with_stroke_width(config.triangle.stroke_width)
                .with_point_variation_x(config.triangle.point_variation_x)
                .with_point_variation_y(config.triangle.point_variation_y)
                .with_point_animation_speed(config.triangle.point_animation_speed)
                .with_particle_count(config.triangle.particle_count as usize)
                .with_bg_color((
                    config.triangle.bg_color_r as u8,
                    config.triangle.bg_color_g as u8,
                    config.triangle.bg_color_b as u8,
                ));
            _ = triangle_config.animate_preview(anim_config);
        }
        AnimationType::Wave => {
            let mut wave_config = WaveConfig::new()
                .with_wave_count(config.wave.wave_count as usize)
                .with_wave_height(config.wave.wave_height)
                .with_duration(config.wave.duration)
                .with_wave_color((
                    config.wave.wave_color_r as u8,
                    config.wave.wave_color_g as u8,
                    config.wave.wave_color_b as u8,
                ))
                .with_wave_opacity(config.wave.wave_opacity)
                .with_gradient_duration(config.wave.gradient_duration)
                .with_bg_color((
                    config.wave.bg_color_r as u8,
                    config.wave.bg_color_g as u8,
                    config.wave.bg_color_b as u8,
                ));
            _ = wave_config.animate_preview(anim_config);
        }
        AnimationType::CrossLine => {
            let mut cross_line_config = CrossLineConfig::new()
                .with_lines_num(config.cross_line.lines_num as usize)
                .with_speed_min(config.cross_line.speed_min)
                .with_speed_max(config.cross_line.speed_max)
                .with_line_color((
                    config.cross_line.line_color_r as u8,
                    config.cross_line.line_color_g as u8,
                    config.cross_line.line_color_b as u8,
                    config.cross_line.line_color_a as u8,
                ))
                .with_line_width(config.cross_line.line_width)
                .with_point_color((
                    config.cross_line.point_color_r as u8,
                    config.cross_line.point_color_g as u8,
                    config.cross_line.point_color_b as u8,
                    config.cross_line.point_color_a as u8,
                ))
                .with_point_radius(config.cross_line.point_radius)
                .with_bg_color((
                    config.cross_line.bg_color_r as u8,
                    config.cross_line.bg_color_g as u8,
                    config.cross_line.bg_color_b as u8,
                ));
            _ = cross_line_config.animate_preview(anim_config);
        }
    }
}

fn video_editor_bg_animation_start_record(ui: &AppWindow, config: BackgroundAnimationConfig) {
    stop_preview_thread();
    stop_record_thread();

    let ui_weak = ui.as_weak();
    let mut data: BackgroundAnimationConfigData = config.into();
    let animation_type_str =
        crate::global_logic!(ui).invoke_animation_type_to_string(data.animation_type);

    tokio::spawn(async move {
        let Some(dir) = picker_directory_with_location(
            ui_weak.clone(),
            &tr("Choose save directory"),
            &data.save_dir,
        ) else {
            return;
        };

        data.save_dir = dir.to_string_lossy().to_string();
        save_bg_animation_config(&data).await;

        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
        let filename = format!("bg_animation_{}_{}.mp4", animation_type_str, timestamp);
        generate_animation_video(ui_weak, data, dir.join(&filename)).await;
    });
}

fn video_editor_bg_animation_stop_record(ui: &AppWindow) {
    stop_record_thread();
    global_store!(ui).set_video_editor_bg_animation_is_recording(false);
}

fn stop_record_thread() {
    if let Some(sig) = RECORD_STOP_SIG.lock().unwrap().take() {
        sig.store(true, Ordering::SeqCst);
    }
}

async fn generate_animation_video(
    ui_weak: Weak<AppWindow>,
    config: BackgroundAnimationConfigData,
    output_path: PathBuf,
) {
    _ = ui_weak.upgrade_in_event_loop(|ui| {
        global_store!(ui).set_video_editor_bg_animation_is_recording(true);
        global_store!(ui).set_video_editor_bg_animation_record_progress(0.0);
    });

    let anim_config = AnimationRecordConfig::new(
        config.width as u32,
        config.height as u32,
        config.fps as u32,
        Duration::from_secs(config.duration as u64),
        &output_path,
    );

    let progress_receiver = anim_config.progress_receiver();
    let stop_sig = anim_config.stop_sig();
    *RECORD_STOP_SIG.lock().unwrap() = Some(stop_sig.clone());

    let monitor_weak = ui_weak.clone();
    std::thread::spawn(move || {
        loop {
            if stop_sig.load(Ordering::SeqCst) {
                break;
            }

            if let Ok(progress) = progress_receiver.recv_timeout(Duration::from_millis(100)) {
                let stop_sig_inner = stop_sig.clone();
                _ = monitor_weak.upgrade_in_event_loop(move |ui| {
                    if !stop_sig_inner.load(Ordering::SeqCst) {
                        global_store!(ui).set_video_editor_bg_animation_record_progress(progress);
                    }
                });

                if progress >= 1.0 {
                    break;
                }
            }
        }

        _ = RECORD_STOP_SIG.lock().unwrap().take();

        _ = monitor_weak.clone().upgrade_in_event_loop(|ui| {
            global_store!(ui).set_video_editor_bg_animation_is_previewing(false);
            global_store!(ui).set_video_editor_bg_animation_is_recording(false);
        });

        if !stop_sig.load(Ordering::SeqCst) {
            toast::async_toast_success(
                monitor_weak.clone(),
                format!("Background animation saved to {}", output_path.display()),
            );

            _ = slint::invoke_from_event_loop(move || {
                tokio::spawn(async move {
                    import_file_to_playlist(monitor_weak, output_path, None).await;
                });
            });
        }
    });

    std::thread::spawn(move || {
        let result = match config.animation_type {
            AnimationType::Grid => {
                let mut grid_config = GridConfig::new()
                    .with_rows(config.grid.rows as usize)
                    .with_cols(config.grid.cols as usize)
                    .with_amplitude(config.grid.amplitude)
                    .with_node_amplitude(config.grid.node_amplitude)
                    .with_frequency(config.grid.frequency)
                    .with_node_radius(config.grid.node_radius)
                    .with_line_color((&config.grid).into())
                    .with_line_width(config.grid.line_width)
                    .with_bg_color((&config.grid).into())
                    .with_node_color((
                        config.grid.node_color_r as u8,
                        config.grid.node_color_g as u8,
                        config.grid.node_color_b as u8,
                        config.grid.node_color_a as u8,
                    ));
                grid_config.animate_record(anim_config)
            }
            AnimationType::MovingGrid => {
                let mut moving_grid_config = MovingGridConfig::new()
                    .with_rows(config.moving_grid.rows as usize)
                    .with_cols(config.moving_grid.cols as usize)
                    .with_speed(config.moving_grid.speed)
                    .with_direction(
                        config
                            .moving_grid
                            .direction
                            .parse()
                            .unwrap_or(FlowDirection::Up),
                    )
                    .with_line_color((
                        config.moving_grid.line_color_r as u8,
                        config.moving_grid.line_color_g as u8,
                        config.moving_grid.line_color_b as u8,
                        config.moving_grid.line_color_a as u8,
                    ))
                    .with_line_width(config.moving_grid.line_width)
                    .with_bg_color((
                        config.moving_grid.bg_color_r as u8,
                        config.moving_grid.bg_color_g as u8,
                        config.moving_grid.bg_color_b as u8,
                    ))
                    .with_supersample(config.moving_grid.supersample as u32);
                moving_grid_config.animate_record(anim_config)
            }
            AnimationType::Glitch => {
                let mut glitch_config = GlitchConfig::new()
                    .with_intensity(config.glitch.intensity)
                    .with_scan_lines_enabled(config.glitch.scan_lines_enabled)
                    .with_scan_line_spacing(config.glitch.scan_line_spacing as u32)
                    .with_rgb_split_enabled(config.glitch.rgb_split_enabled)
                    .with_rgb_split_offset(config.glitch.rgb_split_offset)
                    .with_block_shift_enabled(config.glitch.block_shift_enabled)
                    .with_block_shift_max_offset(config.glitch.block_shift_max_offset)
                    .with_noise_enabled(config.glitch.noise_enabled)
                    .with_animation_speed(config.glitch.animation_speed)
                    .with_bg_color((
                        config.glitch.bg_color_r as u8,
                        config.glitch.bg_color_g as u8,
                        config.glitch.bg_color_b as u8,
                    ));
                glitch_config.animate_record(anim_config)
            }
            AnimationType::NoiseFlow => {
                let palette = ColorPalette::new(
                    config
                        .noise_flow
                        .palette_r
                        .iter()
                        .enumerate()
                        .map(|(i, r)| {
                            (
                                *r as u8,
                                config.noise_flow.palette_g.get(i).copied().unwrap_or(0) as u8,
                                config.noise_flow.palette_b.get(i).copied().unwrap_or(0) as u8,
                                config.noise_flow.palette_a.get(i).copied().unwrap_or(255) as u8,
                            )
                        })
                        .collect(),
                );
                let mut noise_config = NoiseFlowConfig::new()
                    .with_noise_scale(config.noise_flow.noise_scale)
                    .with_animation_speed(config.noise_flow.animation_speed)
                    .with_color_palette(palette)
                    .with_bg_color((
                        config.noise_flow.bg_color_r as u8,
                        config.noise_flow.bg_color_g as u8,
                        config.noise_flow.bg_color_b as u8,
                    ))
                    .with_flow_direction(
                        config
                            .noise_flow
                            .flow_direction
                            .parse()
                            .unwrap_or(FlowDirection::Right),
                    );
                noise_config.animate_record(anim_config)
            }
            AnimationType::Bokeh => {
                let colors: Vec<(u8, u8, u8, u8)> = config
                    .bokeh
                    .colors_r
                    .iter()
                    .enumerate()
                    .map(|(i, r)| {
                        (
                            *r as u8,
                            config.bokeh.colors_g.get(i).copied().unwrap_or(255) as u8,
                            config.bokeh.colors_b.get(i).copied().unwrap_or(255) as u8,
                            config.bokeh.colors_a.get(i).copied().unwrap_or(255) as u8,
                        )
                    })
                    .collect();
                let mut bokeh_config = BokehConfig::new()
                    .with_spot_count(config.bokeh.spot_count as usize)
                    .with_min_size(config.bokeh.min_size)
                    .with_max_size(config.bokeh.max_size)
                    .with_blur_radius(config.bokeh.blur_radius)
                    .with_animation_speed(config.bokeh.animation_speed)
                    .with_hexagonal_enabled(config.bokeh.hexagonal_enabled)
                    .with_colors(colors)
                    .with_bg_color((
                        config.bokeh.bg_color_r as u8,
                        config.bokeh.bg_color_g as u8,
                        config.bokeh.bg_color_b as u8,
                    ));
                bokeh_config.animate_record(anim_config)
            }
            AnimationType::MatrixRain => {
                let mut matrix_config = MatrixRainConfig::new()
                    .with_columns(config.matrix_rain.columns as usize)
                    .with_cell_size(config.matrix_rain.cell_size as u32)
                    .with_min_speed(config.matrix_rain.min_speed)
                    .with_max_speed(config.matrix_rain.max_speed)
                    .with_trail_length(config.matrix_rain.trail_length as usize)
                    .with_fade_speed(config.matrix_rain.fade_speed)
                    .with_color((
                        config.matrix_rain.color_r as u8,
                        config.matrix_rain.color_g as u8,
                        config.matrix_rain.color_b as u8,
                    ))
                    .with_bg_color((
                        config.matrix_rain.bg_color_r as u8,
                        config.matrix_rain.bg_color_g as u8,
                        config.matrix_rain.bg_color_b as u8,
                    ))
                    .with_glow_intensity(config.matrix_rain.glow_intensity)
                    .with_char_change_prob(config.matrix_rain.char_change_prob)
                    .with_flicker_prob(config.matrix_rain.flicker_prob)
                    .with_particle_density(config.matrix_rain.particle_density as u32);
                matrix_config.animate_record(anim_config)
            }
            AnimationType::Fluid => {
                let colors: Vec<(u8, u8, u8)> = config
                    .fluid
                    .colors_r
                    .iter()
                    .enumerate()
                    .map(|(i, r)| {
                        (
                            *r as u8,
                            config.fluid.colors_g.get(i).copied().unwrap_or(0) as u8,
                            config.fluid.colors_b.get(i).copied().unwrap_or(0) as u8,
                        )
                    })
                    .collect();
                let force_source = parse_force_source(&config.fluid.force_source);
                let mut fluid_config = FluidConfig::new()
                    .with_resolution_divisor(config.fluid.resolution_divisor as u32)
                    .with_viscosity(config.fluid.viscosity)
                    .with_diffusion(config.fluid.diffusion)
                    .with_force_source(force_source)
                    .with_num_sources(config.fluid.num_sources as usize)
                    .with_steps_per_frame(config.fluid.steps_per_frame as usize)
                    .with_color_injection(config.fluid.color_injection)
                    .with_colors(colors)
                    .with_bg_color((
                        config.fluid.bg_color_r as u8,
                        config.fluid.bg_color_g as u8,
                        config.fluid.bg_color_b as u8,
                    ));
                fluid_config.animate_record(anim_config)
            }
            AnimationType::Kaleidoscope => {
                let colors: Vec<(u8, u8, u8)> = config
                    .kaleidoscope
                    .colors_r
                    .iter()
                    .enumerate()
                    .map(|(i, r)| {
                        (
                            *r as u8,
                            config.kaleidoscope.colors_g.get(i).copied().unwrap_or(0) as u8,
                            config.kaleidoscope.colors_b.get(i).copied().unwrap_or(0) as u8,
                        )
                    })
                    .collect();
                let mut kaleidoscope_config = KaleidoscopeConfig::new()
                    .with_segments(config.kaleidoscope.segments as usize)
                    .with_rotation_speed(config.kaleidoscope.rotation_speed)
                    .with_center((config.kaleidoscope.center_x, config.kaleidoscope.center_y))
                    .with_scale(config.kaleidoscope.scale)
                    .with_complexity(config.kaleidoscope.complexity as usize)
                    .with_colors(colors)
                    .with_bg_color((
                        config.kaleidoscope.bg_color_r as u8,
                        config.kaleidoscope.bg_color_g as u8,
                        config.kaleidoscope.bg_color_b as u8,
                    ));
                kaleidoscope_config.animate_record(anim_config)
            }
            AnimationType::LightEffects => {
                let colors: Vec<(u8, u8, u8, u8)> = config
                    .light_effects
                    .colors_r
                    .iter()
                    .enumerate()
                    .map(|(i, r)| {
                        (
                            *r as u8,
                            config.light_effects.colors_g.get(i).copied().unwrap_or(255) as u8,
                            config.light_effects.colors_b.get(i).copied().unwrap_or(255) as u8,
                            config.light_effects.colors_a.get(i).copied().unwrap_or(255) as u8,
                        )
                    })
                    .collect();
                let mut light_effects_config = LightEffectsConfig::new()
                    .with_flare_count(config.light_effects.flare_count as usize)
                    .with_min_size(config.light_effects.min_size)
                    .with_max_size(config.light_effects.max_size)
                    .with_movement_speed(config.light_effects.movement_speed)
                    .with_elliptical_enabled(config.light_effects.elliptical_enabled)
                    .with_bands_enabled(config.light_effects.bands_enabled)
                    .with_colors(colors)
                    .with_bg_color((
                        config.light_effects.bg_color_r as u8,
                        config.light_effects.bg_color_g as u8,
                        config.light_effects.bg_color_b as u8,
                    ));
                light_effects_config.animate_record(anim_config)
            }
            AnimationType::InkDissipation => {
                let mut ink_config = InkDissipationConfig::new()
                    .with_style(config.ink_dissipation.style)
                    .with_source_count(config.ink_dissipation.source_count as usize)
                    .with_spawn_rate(config.ink_dissipation.spawn_rate)
                    .with_source_lifetime(config.ink_dissipation.source_lifetime as u32)
                    .with_initial_radius(config.ink_dissipation.initial_radius)
                    .with_max_radius(config.ink_dissipation.max_radius)
                    .with_spread_rate(config.ink_dissipation.spread_rate)
                    .with_diffusion_strength(config.ink_dissipation.diffusion_strength)
                    .with_fade_speed(config.ink_dissipation.fade_speed)
                    .with_max_drops(config.ink_dissipation.max_drops as usize)
                    .with_resolution_divisor(config.ink_dissipation.resolution_divisor as u32);
                ink_config.animate_record(anim_config)
            }
            AnimationType::ParticleLife => {
                let mut particle_life_config = ParticleLifeConfig::new()
                    .with_particle_count(config.particle_life.particle_count as usize)
                    .with_type_count(config.particle_life.type_count as usize)
                    .with_rmax(config.particle_life.rmax)
                    .with_friction(config.particle_life.friction)
                    .with_force(config.particle_life.force)
                    .with_dt(config.particle_life.dt)
                    .with_wrap(config.particle_life.wrap)
                    .with_particle_size(config.particle_life.particle_size)
                    .with_bg_color((
                        config.particle_life.bg_color_r as u8,
                        config.particle_life.bg_color_g as u8,
                        config.particle_life.bg_color_b as u8,
                    ))
                    .with_matrix_seed(config.particle_life.matrix_seed as u64);
                particle_life_config.animate_record(anim_config)
            }
            AnimationType::ParticleNetwork => {
                let mut particle_config = ParticleNetworkConfig::new()
                    .with_density(config.particle_network.density as u32)
                    .with_line_color((
                        config.particle_network.line_color_r as u8,
                        config.particle_network.line_color_g as u8,
                        config.particle_network.line_color_b as u8,
                        config.particle_network.line_color_a as u8,
                    ))
                    .with_particle_color((
                        config.particle_network.particle_color_r as u8,
                        config.particle_network.particle_color_g as u8,
                        config.particle_network.particle_color_b as u8,
                        config.particle_network.particle_color_a as u8,
                    ))
                    .with_bg_color((
                        config.particle_network.bg_color_r as u8,
                        config.particle_network.bg_color_g as u8,
                        config.particle_network.bg_color_b as u8,
                    ))
                    .with_pointer_enabled(config.particle_network.pointer_enabled)
                    .with_pointer_range(config.particle_network.pointer_range)
                    .with_pointer_count(config.particle_network.pointer_count as usize);
                particle_config.animate_record(anim_config)
            }
            AnimationType::FlowField => {
                let mut flow_config = FlowFieldConfig::new()
                    .with_color((
                        config.flow_field.color_r as u8,
                        config.flow_field.color_g as u8,
                        config.flow_field.color_b as u8,
                        config.flow_field.color_a as u8,
                    ))
                    .with_bg_color((
                        config.flow_field.bg_color_r as u8,
                        config.flow_field.bg_color_g as u8,
                        config.flow_field.bg_color_b as u8,
                    ))
                    .with_trail_opacity(config.flow_field.trail_opacity)
                    .with_particle_count(config.flow_field.particle_count as u32)
                    .with_speed(config.flow_field.speed)
                    .with_pointer_enabled(config.flow_field.pointer_enabled)
                    .with_pointer_count(config.flow_field.pointer_count as usize);
                flow_config.animate_record(anim_config)
            }
            AnimationType::Shape => {
                let mut shape_config = ShapeConfig::new()
                    .with_max_circles(config.shape.max_circles as usize)
                    .with_rad_min(config.shape.rad_min)
                    .with_rad_max(config.shape.rad_max)
                    .with_filled_circle_pct(config.shape.filled_circle_pct as u32)
                    .with_concentric_circle_pct(config.shape.concentric_circle_pct as u32)
                    .with_rad_threshold(config.shape.rad_threshold)
                    .with_speed_min(config.shape.speed_min)
                    .with_speed_max(config.shape.speed_max)
                    .with_max_opacity(config.shape.max_opacity)
                    .with_circle_border(config.shape.circle_border)
                    .with_background_mult(config.shape.background_mult)
                    .with_line_border(config.shape.line_border)
                    .with_link_dist_fraction(config.shape.link_dist_fraction)
                    .with_bg_color((
                        config.shape.bg_color_r as u8,
                        config.shape.bg_color_g as u8,
                        config.shape.bg_color_b as u8,
                    ));
                shape_config.animate_record(anim_config)
            }
            AnimationType::BlackHole => {
                let mut black_hole_config = BlackHoleConfig::new()
                    .with_star_count(config.black_hole.star_count as usize)
                    .with_black_hole_size(config.black_hole.black_hole_size)
                    .with_event_horizon_offset(config.black_hole.event_horizon_offset)
                    .with_max_consume_frames(config.black_hole.max_consume_frames as usize)
                    .with_hue_speed(config.black_hole.hue_speed)
                    .with_star_saturation(config.black_hole.star_saturation)
                    .with_star_lightness(config.black_hole.star_lightness)
                    .with_trail_alpha(config.black_hole.trail_alpha)
                    .with_trail_color((
                        config.black_hole.trail_color_r as u8,
                        config.black_hole.trail_color_g as u8,
                        config.black_hole.trail_color_b as u8,
                    ))
                    .with_bg_color((
                        config.black_hole.bg_color_r as u8,
                        config.black_hole.bg_color_g as u8,
                        config.black_hole.bg_color_b as u8,
                    ))
                    .with_center_x(config.black_hole.center_x)
                    .with_center_y(config.black_hole.center_y)
                    .with_hole_stroke_color((
                        config.black_hole.hole_stroke_color_r as u8,
                        config.black_hole.hole_stroke_color_g as u8,
                        config.black_hole.hole_stroke_color_b as u8,
                    ))
                    .with_hole_inner_color((
                        config.black_hole.hole_inner_color_r as u8,
                        config.black_hole.hole_inner_color_g as u8,
                        config.black_hole.hole_inner_color_b as u8,
                    ))
                    .with_hole_mid_color((
                        config.black_hole.hole_mid_color_r as u8,
                        config.black_hole.hole_mid_color_g as u8,
                        config.black_hole.hole_mid_color_b as u8,
                    ))
                    .with_hole_outer_color((
                        config.black_hole.hole_outer_color_r as u8,
                        config.black_hole.hole_outer_color_g as u8,
                        config.black_hole.hole_outer_color_b as u8,
                    ));
                black_hole_config.animate_record(anim_config)
            }
            AnimationType::Galaxy => {
                let mut galaxy_config = GalaxyConfig::new()
                    .with_star_count(config.galaxy.star_count as usize)
                    .with_rotation_period(config.galaxy.rotation_period)
                    .with_appear_duration(config.galaxy.appear_duration)
                    .with_breathing_period(config.galaxy.breathing_period)
                    .with_breathing_min(config.galaxy.breathing_min)
                    .with_perspective(config.galaxy.perspective)
                    .with_glow_intensity(config.galaxy.glow_intensity)
                    .with_min_distance(config.galaxy.min_distance)
                    .with_max_distance(config.galaxy.max_distance)
                    .with_min_size(config.galaxy.min_size)
                    .with_max_size(config.galaxy.max_size)
                    .with_bg_color((
                        config.galaxy.bg_color_r as u8,
                        config.galaxy.bg_color_g as u8,
                        config.galaxy.bg_color_b as u8,
                    ));
                galaxy_config.animate_record(anim_config)
            }
            AnimationType::Triangle => {
                let mut triangle_config = TriangleConfig::new()
                    .with_triangle_size(config.triangle.triangle_size)
                    .with_bleed(config.triangle.bleed)
                    .with_noise(config.triangle.noise)
                    .with_color1((
                        config.triangle.color1_r as u8,
                        config.triangle.color1_g as u8,
                        config.triangle.color1_b as u8,
                    ))
                    .with_color2((
                        config.triangle.color2_r as u8,
                        config.triangle.color2_g as u8,
                        config.triangle.color2_b as u8,
                    ))
                    .with_stroke_color((
                        config.triangle.stroke_color_r as u8,
                        config.triangle.stroke_color_g as u8,
                        config.triangle.stroke_color_b as u8,
                        config.triangle.stroke_color_a as u8,
                    ))
                    .with_stroke_width(config.triangle.stroke_width)
                    .with_point_variation_x(config.triangle.point_variation_x)
                    .with_point_variation_y(config.triangle.point_variation_y)
                    .with_point_animation_speed(config.triangle.point_animation_speed)
                    .with_particle_count(config.triangle.particle_count as usize)
                    .with_bg_color((
                        config.triangle.bg_color_r as u8,
                        config.triangle.bg_color_g as u8,
                        config.triangle.bg_color_b as u8,
                    ));
                triangle_config.animate_record(anim_config)
            }
            AnimationType::Wave => {
                let mut wave_config = WaveConfig::new()
                    .with_wave_count(config.wave.wave_count as usize)
                    .with_wave_height(config.wave.wave_height)
                    .with_duration(config.wave.duration)
                    .with_wave_color((
                        config.wave.wave_color_r as u8,
                        config.wave.wave_color_g as u8,
                        config.wave.wave_color_b as u8,
                    ))
                    .with_wave_opacity(config.wave.wave_opacity)
                    .with_gradient_duration(config.wave.gradient_duration)
                    .with_bg_color((
                        config.wave.bg_color_r as u8,
                        config.wave.bg_color_g as u8,
                        config.wave.bg_color_b as u8,
                    ));
                wave_config.animate_record(anim_config)
            }
            AnimationType::CrossLine => {
                let mut cross_line_config = CrossLineConfig::new()
                    .with_lines_num(config.cross_line.lines_num as usize)
                    .with_speed_min(config.cross_line.speed_min)
                    .with_speed_max(config.cross_line.speed_max)
                    .with_line_color((
                        config.cross_line.line_color_r as u8,
                        config.cross_line.line_color_g as u8,
                        config.cross_line.line_color_b as u8,
                        config.cross_line.line_color_a as u8,
                    ))
                    .with_line_width(config.cross_line.line_width)
                    .with_point_color((
                        config.cross_line.point_color_r as u8,
                        config.cross_line.point_color_g as u8,
                        config.cross_line.point_color_b as u8,
                        config.cross_line.point_color_a as u8,
                    ))
                    .with_point_radius(config.cross_line.point_radius)
                    .with_bg_color((
                        config.cross_line.bg_color_r as u8,
                        config.cross_line.bg_color_g as u8,
                        config.cross_line.bg_color_b as u8,
                    ));
                cross_line_config.animate_record(anim_config)
            }
        };

        if let Err(e) = result {
            log::error!("bg_animation animate_record failed: {}", e);
        }
    });
}

async fn save_bg_animation_config(config: &BackgroundAnimationConfigData) {
    let data = serde_json::to_string(config).expect("serialize bg animation config failed");
    if sqldb::entry::insert(VIDEO_EDITOR_TABLE, BG_ANIMATION_CONFIG_ID, &data)
        .await
        .is_err()
    {
        if let Err(e) =
            sqldb::entry::update(VIDEO_EDITOR_TABLE, BG_ANIMATION_CONFIG_ID, &data).await
        {
            log::warn!("Failed to save bg animation config: {:?}", e);
        }
    }
}

impl From<&crate::db::GridAnimConfigData> for (u8, u8, u8, u8) {
    fn from(c: &crate::db::GridAnimConfigData) -> Self {
        (
            c.line_color_r as u8,
            c.line_color_g as u8,
            c.line_color_b as u8,
            c.line_color_a as u8,
        )
    }
}

impl From<&crate::db::GridAnimConfigData> for (u8, u8, u8) {
    fn from(c: &crate::db::GridAnimConfigData) -> Self {
        (c.bg_color_r as u8, c.bg_color_g as u8, c.bg_color_b as u8)
    }
}

impl From<GridConfig> for crate::db::GridAnimConfigData {
    fn from(c: GridConfig) -> Self {
        Self {
            rows: c.rows as i32,
            cols: c.cols as i32,
            amplitude: c.amplitude,
            node_amplitude: c.node_amplitude,
            frequency: c.frequency,
            node_radius: c.node_radius,
            line_color_r: c.line_color.0 as i32,
            line_color_g: c.line_color.1 as i32,
            line_color_b: c.line_color.2 as i32,
            line_color_a: c.line_color.3 as i32,
            line_width: c.line_width,
            bg_color_r: c.bg_color.0 as i32,
            bg_color_g: c.bg_color.1 as i32,
            bg_color_b: c.bg_color.2 as i32,
            node_color_r: c.node_color.0 as i32,
            node_color_g: c.node_color.1 as i32,
            node_color_b: c.node_color.2 as i32,
            node_color_a: c.node_color.3 as i32,
        }
    }
}

impl From<MovingGridConfig> for crate::db::MovingGridAnimConfigData {
    fn from(c: MovingGridConfig) -> Self {
        Self {
            rows: c.rows as i32,
            cols: c.cols as i32,
            speed: c.speed,
            direction: c.direction.to_string(),
            line_color_r: c.line_color.0 as i32,
            line_color_g: c.line_color.1 as i32,
            line_color_b: c.line_color.2 as i32,
            line_color_a: c.line_color.3 as i32,
            line_width: c.line_width,
            bg_color_r: c.bg_color.0 as i32,
            bg_color_g: c.bg_color.1 as i32,
            bg_color_b: c.bg_color.2 as i32,
            supersample: c.supersample as i32,
        }
    }
}

impl From<GlitchConfig> for crate::db::GlitchAnimConfigData {
    fn from(c: GlitchConfig) -> Self {
        Self {
            intensity: c.intensity,
            scan_lines_enabled: c.scan_lines_enabled,
            scan_line_spacing: c.scan_line_spacing as i32,
            rgb_split_enabled: c.rgb_split_enabled,
            rgb_split_offset: c.rgb_split_offset,
            block_shift_enabled: c.block_shift_enabled,
            block_shift_max_offset: c.block_shift_max_offset,
            noise_enabled: c.noise_enabled,
            animation_speed: c.animation_speed,
            bg_color_r: c.bg_color.0 as i32,
            bg_color_g: c.bg_color.1 as i32,
            bg_color_b: c.bg_color.2 as i32,
        }
    }
}

impl From<NoiseFlowConfig> for crate::db::NoiseFlowAnimConfigData {
    fn from(c: NoiseFlowConfig) -> Self {
        Self {
            noise_scale: c.noise_scale,
            animation_speed: c.animation_speed,
            palette_r: c
                .color_palette
                .colors
                .iter()
                .map(|(r, _, _, _)| *r as i32)
                .collect(),
            palette_g: c
                .color_palette
                .colors
                .iter()
                .map(|(_, g, _, _)| *g as i32)
                .collect(),
            palette_b: c
                .color_palette
                .colors
                .iter()
                .map(|(_, _, b, _)| *b as i32)
                .collect(),
            palette_a: c
                .color_palette
                .colors
                .iter()
                .map(|(_, _, _, a)| *a as i32)
                .collect(),
            bg_color_r: c.bg_color.0 as i32,
            bg_color_g: c.bg_color.1 as i32,
            bg_color_b: c.bg_color.2 as i32,
            flow_direction: c.flow_direction.to_string(),
        }
    }
}

impl From<BokehConfig> for crate::db::BokehAnimConfigData {
    fn from(c: BokehConfig) -> Self {
        Self {
            spot_count: c.spot_count as i32,
            min_size: c.min_size,
            max_size: c.max_size,
            blur_radius: c.blur_radius,
            animation_speed: c.animation_speed,
            hexagonal_enabled: c.hexagonal_enabled,
            colors_r: c.colors.iter().map(|(r, _, _, _)| *r as i32).collect(),
            colors_g: c.colors.iter().map(|(_, g, _, _)| *g as i32).collect(),
            colors_b: c.colors.iter().map(|(_, _, b, _)| *b as i32).collect(),
            colors_a: c.colors.iter().map(|(_, _, _, a)| *a as i32).collect(),
            bg_color_r: c.bg_color.0 as i32,
            bg_color_g: c.bg_color.1 as i32,
            bg_color_b: c.bg_color.2 as i32,
        }
    }
}

impl From<MatrixRainConfig> for crate::db::MatrixRainAnimConfigData {
    fn from(c: MatrixRainConfig) -> Self {
        Self {
            columns: c.columns as i32,
            cell_size: c.cell_size as i32,
            min_speed: c.min_speed,
            max_speed: c.max_speed,
            trail_length: c.trail_length as i32,
            fade_speed: c.fade_speed,
            color_r: c.color.0 as i32,
            color_g: c.color.1 as i32,
            color_b: c.color.2 as i32,
            bg_color_r: c.bg_color.0 as i32,
            bg_color_g: c.bg_color.1 as i32,
            bg_color_b: c.bg_color.2 as i32,
            glow_intensity: c.glow_intensity,
            char_change_prob: c.char_change_prob,
            flicker_prob: c.flicker_prob,
            particle_density: c.particle_density as i32,
        }
    }
}

impl From<FluidConfig> for crate::db::FluidAnimConfigData {
    fn from(c: FluidConfig) -> Self {
        Self {
            resolution_divisor: c.resolution_divisor as i32,
            viscosity: c.viscosity,
            diffusion: c.diffusion,
            force_source: force_source_to_string(c.force_source),
            num_sources: c.num_sources as i32,
            steps_per_frame: c.steps_per_frame as i32,
            color_injection: c.color_injection,
            colors_r: c.colors.iter().map(|(r, _, _)| *r as i32).collect(),
            colors_g: c.colors.iter().map(|(_, g, _)| *g as i32).collect(),
            colors_b: c.colors.iter().map(|(_, _, b)| *b as i32).collect(),
            bg_color_r: c.bg_color.0 as i32,
            bg_color_g: c.bg_color.1 as i32,
            bg_color_b: c.bg_color.2 as i32,
        }
    }
}

impl From<KaleidoscopeConfig> for crate::db::KaleidoscopeAnimConfigData {
    fn from(c: KaleidoscopeConfig) -> Self {
        Self {
            segments: c.segments as i32,
            rotation_speed: c.rotation_speed,
            center_x: c.center.0,
            center_y: c.center.1,
            scale: c.scale,
            complexity: c.complexity as i32,
            colors_r: c.colors.iter().map(|(r, _, _)| *r as i32).collect(),
            colors_g: c.colors.iter().map(|(_, g, _)| *g as i32).collect(),
            colors_b: c.colors.iter().map(|(_, _, b)| *b as i32).collect(),
            bg_color_r: c.bg_color.0 as i32,
            bg_color_g: c.bg_color.1 as i32,
            bg_color_b: c.bg_color.2 as i32,
        }
    }
}

impl From<LightEffectsConfig> for crate::db::LightEffectsAnimConfigData {
    fn from(c: LightEffectsConfig) -> Self {
        Self {
            flare_count: c.flare_count as i32,
            min_size: c.min_size,
            max_size: c.max_size,
            movement_speed: c.movement_speed,
            elliptical_enabled: c.elliptical_enabled,
            bands_enabled: c.bands_enabled,
            colors_r: c.colors.iter().map(|(r, _, _, _)| *r as i32).collect(),
            colors_g: c.colors.iter().map(|(_, g, _, _)| *g as i32).collect(),
            colors_b: c.colors.iter().map(|(_, _, b, _)| *b as i32).collect(),
            colors_a: c.colors.iter().map(|(_, _, _, a)| *a as i32).collect(),
            bg_color_r: c.bg_color.0 as i32,
            bg_color_g: c.bg_color.1 as i32,
            bg_color_b: c.bg_color.2 as i32,
        }
    }
}

impl From<InkDissipationConfig> for crate::db::InkDissipationAnimConfigData {
    fn from(c: InkDissipationConfig) -> Self {
        Self {
            style: c.style,
            source_count: c.source_count as i32,
            spawn_rate: c.spawn_rate,
            source_lifetime: c.source_lifetime as i32,
            initial_radius: c.initial_radius,
            max_radius: c.max_radius,
            spread_rate: c.spread_rate,
            diffusion_strength: c.diffusion_strength,
            fade_speed: c.fade_speed,
            max_drops: c.max_drops as i32,
            resolution_divisor: c.resolution_divisor as i32,
        }
    }
}

impl From<ParticleLifeConfig> for crate::db::ParticleLifeAnimConfigData {
    fn from(c: ParticleLifeConfig) -> Self {
        Self {
            particle_count: c.particle_count as i32,
            type_count: c.type_count as i32,
            rmax: c.rmax,
            friction: c.friction,
            force: c.force,
            dt: c.dt,
            wrap: c.wrap,
            particle_size: c.particle_size,
            bg_color_r: c.bg_color.0 as i32,
            bg_color_g: c.bg_color.1 as i32,
            bg_color_b: c.bg_color.2 as i32,
            matrix_seed: c.matrix_seed as i32,
        }
    }
}

impl From<ParticleNetworkConfig> for crate::db::ParticleNetworkAnimConfigData {
    fn from(c: ParticleNetworkConfig) -> Self {
        Self {
            density: c.density as i32,
            line_color_r: c.line_color.0 as i32,
            line_color_g: c.line_color.1 as i32,
            line_color_b: c.line_color.2 as i32,
            line_color_a: c.line_color.3 as i32,
            particle_color_r: c.particle_color.0 as i32,
            particle_color_g: c.particle_color.1 as i32,
            particle_color_b: c.particle_color.2 as i32,
            particle_color_a: c.particle_color.3 as i32,
            bg_color_r: c.bg_color.0 as i32,
            bg_color_g: c.bg_color.1 as i32,
            bg_color_b: c.bg_color.2 as i32,
            pointer_enabled: c.pointer_enabled,
            pointer_range: c.pointer_range,
            pointer_count: c.pointer_count as i32,
        }
    }
}

impl From<FlowFieldConfig> for crate::db::FlowFieldAnimConfigData {
    fn from(c: FlowFieldConfig) -> Self {
        Self {
            color_r: c.color.0 as i32,
            color_g: c.color.1 as i32,
            color_b: c.color.2 as i32,
            color_a: c.color.3 as i32,
            bg_color_r: c.bg_color.0 as i32,
            bg_color_g: c.bg_color.1 as i32,
            bg_color_b: c.bg_color.2 as i32,
            trail_opacity: c.trail_opacity,
            particle_count: c.particle_count as i32,
            speed: c.speed,
            pointer_enabled: c.pointer_enabled,
            pointer_count: c.pointer_count as i32,
        }
    }
}

impl From<ShapeConfig> for crate::db::ShapeAnimConfigData {
    fn from(c: ShapeConfig) -> Self {
        Self {
            max_circles: c.max_circles as i32,
            rad_min: c.rad_min,
            rad_max: c.rad_max,
            filled_circle_pct: c.filled_circle_pct as i32,
            concentric_circle_pct: c.concentric_circle_pct as i32,
            rad_threshold: c.rad_threshold,
            speed_min: c.speed_min,
            speed_max: c.speed_max,
            max_opacity: c.max_opacity,
            circle_border: c.circle_border,
            background_mult: c.background_mult,
            line_border: c.line_border,
            link_dist_fraction: c.link_dist_fraction,
            bg_color_r: c.bg_color.0 as i32,
            bg_color_g: c.bg_color.1 as i32,
            bg_color_b: c.bg_color.2 as i32,
        }
    }
}

impl From<BlackHoleConfig> for crate::db::BlackHoleAnimConfigData {
    fn from(c: BlackHoleConfig) -> Self {
        Self {
            star_count: c.star_count as i32,
            black_hole_size: c.black_hole_size,
            event_horizon_offset: c.event_horizon_offset,
            max_consume_frames: c.max_consume_frames as i32,
            hue_speed: c.hue_speed,
            star_saturation: c.star_saturation,
            star_lightness: c.star_lightness,
            trail_alpha: c.trail_alpha,
            trail_color_r: c.trail_color.0 as i32,
            trail_color_g: c.trail_color.1 as i32,
            trail_color_b: c.trail_color.2 as i32,
            bg_color_r: c.bg_color.0 as i32,
            bg_color_g: c.bg_color.1 as i32,
            bg_color_b: c.bg_color.2 as i32,
            center_x: c.center_x,
            center_y: c.center_y,
            hole_stroke_color_r: c.hole_stroke_color.0 as i32,
            hole_stroke_color_g: c.hole_stroke_color.1 as i32,
            hole_stroke_color_b: c.hole_stroke_color.2 as i32,
            hole_inner_color_r: c.hole_inner_color.0 as i32,
            hole_inner_color_g: c.hole_inner_color.1 as i32,
            hole_inner_color_b: c.hole_inner_color.2 as i32,
            hole_mid_color_r: c.hole_mid_color.0 as i32,
            hole_mid_color_g: c.hole_mid_color.1 as i32,
            hole_mid_color_b: c.hole_mid_color.2 as i32,
            hole_outer_color_r: c.hole_outer_color.0 as i32,
            hole_outer_color_g: c.hole_outer_color.1 as i32,
            hole_outer_color_b: c.hole_outer_color.2 as i32,
        }
    }
}

impl From<GalaxyConfig> for crate::db::GalaxyAnimConfigData {
    fn from(c: GalaxyConfig) -> Self {
        Self {
            star_count: c.star_count as i32,
            rotation_period: c.rotation_period,
            appear_duration: c.appear_duration,
            breathing_period: c.breathing_period,
            breathing_min: c.breathing_min,
            perspective: c.perspective,
            glow_intensity: c.glow_intensity,
            min_distance: c.min_distance,
            max_distance: c.max_distance,
            min_size: c.min_size,
            max_size: c.max_size,
            bg_color_r: c.bg_color.0 as i32,
            bg_color_g: c.bg_color.1 as i32,
            bg_color_b: c.bg_color.2 as i32,
        }
    }
}

impl From<TriangleConfig> for crate::db::TriangleAnimConfigData {
    fn from(c: TriangleConfig) -> Self {
        Self {
            triangle_size: c.triangle_size,
            bleed: c.bleed,
            noise: c.noise,
            color1_r: c.color1.0 as i32,
            color1_g: c.color1.1 as i32,
            color1_b: c.color1.2 as i32,
            color2_r: c.color2.0 as i32,
            color2_g: c.color2.1 as i32,
            color2_b: c.color2.2 as i32,
            stroke_color_r: c.stroke_color.0 as i32,
            stroke_color_g: c.stroke_color.1 as i32,
            stroke_color_b: c.stroke_color.2 as i32,
            stroke_color_a: c.stroke_color.3 as i32,
            stroke_width: c.stroke_width,
            point_variation_x: c.point_variation_x,
            point_variation_y: c.point_variation_y,
            point_animation_speed: c.point_animation_speed,
            particle_count: c.particle_count as i32,
            bg_color_r: c.bg_color.0 as i32,
            bg_color_g: c.bg_color.1 as i32,
            bg_color_b: c.bg_color.2 as i32,
        }
    }
}

impl From<WaveConfig> for crate::db::WaveAnimConfigData {
    fn from(c: WaveConfig) -> Self {
        Self {
            wave_count: c.wave_count as i32,
            wave_height: c.wave_height,
            duration: c.duration,
            wave_color_r: c.wave_color.0 as i32,
            wave_color_g: c.wave_color.1 as i32,
            wave_color_b: c.wave_color.2 as i32,
            wave_opacity: c.wave_opacity,
            gradient_duration: c.gradient_duration,
            bg_color_r: c.bg_color.0 as i32,
            bg_color_g: c.bg_color.1 as i32,
            bg_color_b: c.bg_color.2 as i32,
        }
    }
}

impl From<CrossLineConfig> for crate::db::CrossLineAnimConfigData {
    fn from(c: CrossLineConfig) -> Self {
        Self {
            lines_num: c.lines_num as i32,
            speed_min: c.speed_min,
            speed_max: c.speed_max,
            line_color_r: c.line_color.0 as i32,
            line_color_g: c.line_color.1 as i32,
            line_color_b: c.line_color.2 as i32,
            line_color_a: c.line_color.3 as i32,
            line_width: c.line_width,
            point_color_r: c.point_color.0 as i32,
            point_color_g: c.point_color.1 as i32,
            point_color_b: c.point_color.2 as i32,
            point_color_a: c.point_color.3 as i32,
            point_radius: c.point_radius,
            bg_color_r: c.bg_color.0 as i32,
            bg_color_g: c.bg_color.1 as i32,
            bg_color_b: c.bg_color.2 as i32,
        }
    }
}
