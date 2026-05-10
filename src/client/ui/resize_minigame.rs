use bevy::{
    asset::RenderAssetUsages,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
    window::PrimaryWindow,
};
use bevy_egui::{egui, EguiContexts};
use rand::Rng;

use crate::client::settings::{ClientSettings, ResizeMinigameMode};
use super::interactive_resize::InteractiveResizeState;

#[derive(Default)]
struct BallState {
    active: bool,
    ball_pos: Vec2,
    ball_vel: Vec2,
    radius: f32,
    obstacles: Vec<egui::Rect>,
    bounce_count: u32,
}

#[derive(Default)]
struct VoxelDdaState {
    active: bool,
    buffer: Vec<u8>,
    width: u32,
    height: u32,
    texture_id: Option<egui::TextureId>,
    image_handle: Option<Handle<Image>>,
    last_update_time: f32,
    last_window_size: Vec2,
    resize_energy: f32,
}

#[derive(Resource, Default)]
pub struct ResizeMinigameState {
    ball: BallState,
    voxel: VoxelDdaState,
}

const MIN_OBSTACLES: usize = 4;
const MAX_OBSTACLES: usize = 8;
const MAX_SPEED: f32 = 420.0;

pub fn resize_minigame_system(
    mut contexts: EguiContexts,
    resize: Res<InteractiveResizeState>,
    cfg: Res<ClientSettings>,
    mut state: ResMut<ResizeMinigameState>,
    time: Res<Time>,
    mut images: ResMut<Assets<Image>>,
    query_window: Query<&Window, With<PrimaryWindow>>,
) {
    let Ok(window) = query_window.single() else {
        return;
    };

    let size = Vec2::new(window.resolution.width(), window.resolution.height());
    if size.x <= 1.0 || size.y <= 1.0 {
        return;
    }

    if resize.just_exited {
        state.ball.active = false;
        state.ball.obstacles.clear();
        state.voxel.active = false;
        return;
    }

    if !resize.in_progress {
        return;
    }

    match cfg.resize_minigame_mode {
        ResizeMinigameMode::Ball => {
            let Ok(ctx) = contexts.ctx_mut() else {
                return;
            };
            run_ball_minigame(ctx, time.delta_secs(), size, &mut state.ball);
        }
        ResizeMinigameMode::VoxelDda => {
            run_voxel_minigame(&mut contexts, &mut images, time.elapsed_secs(), time.delta_secs(), size, &mut state.voxel);
        }
    }
}

fn init_ball_state(state: &mut BallState, size: Vec2) {
    let mut rng = rand::rng();
    let radius: f32 = 18.0;

    let start_x = size.x * 0.5;
    let start_y = size.y * 0.45;

    let mut vel = Vec2::new(
        rng.random_range(-1.0..1.0),
        rng.random_range(-1.0..1.0),
    );
    if vel.length_squared() < 0.01 {
        vel = Vec2::new(0.7, 0.4);
    }
    vel = vel.normalize() * rng.random_range(240.0..MAX_SPEED);

    let count = rng.random_range(MIN_OBSTACLES..=MAX_OBSTACLES);
    let mut obstacles = Vec::with_capacity(count);

    for _ in 0..count {
        let rect = random_obstacle(&mut rng, size, radius * 2.0);
        obstacles.push(rect);
    }

    state.active = true;
    state.ball_pos = Vec2::new(start_x, start_y);
    state.ball_vel = vel;
    state.radius = radius;
    state.obstacles = obstacles;
    state.bounce_count = 0;
}

fn random_obstacle(rng: &mut impl Rng, size: Vec2, margin: f32) -> egui::Rect {
    let min_w: f32 = 60.0;
    let max_w: f32 = 160.0;
    let min_h: f32 = 26.0;
    let max_h: f32 = 80.0;

    let w = rng.random_range(min_w..max_w).min(size.x * 0.45).max(min_w);
    let h = rng.random_range(min_h..max_h).min(size.y * 0.25).max(min_h);

    let x = rng.random_range(margin..(size.x - w - margin).max(margin));
    let y = rng.random_range(margin..(size.y - h - margin).max(margin));

    egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(w, h))
}

fn simulate_ball(state: &mut BallState, bounds: Vec2, dt: f32) {
    let mut pos = state.ball_pos;
    let mut vel = state.ball_vel;
    let r = state.radius;

    pos += vel * dt;

    if pos.x - r <= 0.0 {
        pos.x = r;
        vel.x = vel.x.abs();
        state.bounce_count += 1;
    } else if pos.x + r >= bounds.x {
        pos.x = bounds.x - r;
        vel.x = -vel.x.abs();
        state.bounce_count += 1;
    }

    if pos.y - r <= 0.0 {
        pos.y = r;
        vel.y = vel.y.abs();
        state.bounce_count += 1;
    } else if pos.y + r >= bounds.y {
        pos.y = bounds.y - r;
        vel.y = -vel.y.abs();
        state.bounce_count += 1;
    }

    for rect in &state.obstacles {
        if let Some((normal, push)) = circle_rect_hit(pos, r, rect) {
            pos += normal * push;
            if normal.x.abs() > normal.y.abs() {
                vel.x = -vel.x;
            } else {
                vel.y = -vel.y;
            }
            state.bounce_count += 1;
        }
    }

    if vel.length() > MAX_SPEED {
        vel = vel.normalize() * MAX_SPEED;
    }

    state.ball_pos = pos;
    state.ball_vel = vel;
}

fn run_ball_minigame(ctx: &egui::Context, dt: f32, size: Vec2, state: &mut BallState) {
    if !state.active {
        init_ball_state(state, size);
    }

    let dt = dt.min(1.0 / 30.0);
    simulate_ball(state, size, dt);

    let screen_rect = egui::Rect::from_min_size(
        egui::pos2(0.0, 0.0),
        egui::vec2(size.x, size.y),
    );
    let layer = egui::LayerId::new(egui::Order::Foreground, egui::Id::new("resize_minigame_ball"));
    let painter = ctx.layer_painter(layer);

    painter.rect_filled(screen_rect, 0.0, egui::Color32::from_black_alpha(180));

    for rect in &state.obstacles {
        painter.rect_filled(*rect, 6.0, egui::Color32::from_rgb(40, 80, 120));
        painter.rect_stroke(
            *rect,
            6.0,
            egui::Stroke::new(1.0, egui::Color32::from_white_alpha(80)),
            egui::StrokeKind::Inside,
        );
    }

    painter.circle_filled(
        egui::pos2(state.ball_pos.x, state.ball_pos.y),
        state.radius,
        egui::Color32::from_rgb(230, 180, 40),
    );
    painter.circle_stroke(
        egui::pos2(state.ball_pos.x, state.ball_pos.y),
        state.radius,
        egui::Stroke::new(2.0, egui::Color32::from_white_alpha(200)),
    );

    let label = format!("Resize bounce: {}", state.bounce_count);
    painter.text(
        egui::pos2(18.0, 18.0),
        egui::Align2::LEFT_TOP,
        label,
        egui::FontId::proportional(18.0),
        egui::Color32::from_white_alpha(220),
    );
}

const VOXEL_BUF_W: u32 = 160;
const VOXEL_BUF_H: u32 = 90;
const VOXEL_UPDATE_INTERVAL: f32 = 1.0 / 15.0;
const VOXEL_MAX_STEPS: u32 = 48;

fn run_voxel_minigame(
    contexts: &mut EguiContexts,
    images: &mut Assets<Image>,
    t: f32,
    dt: f32,
    size: Vec2,
    state: &mut VoxelDdaState,
) {
    let screen_rect = egui::Rect::from_min_size(
        egui::pos2(0.0, 0.0),
        egui::vec2(size.x, size.y),
    );

    if !state.active {
        state.active = true;
        state.width = VOXEL_BUF_W;
        state.height = VOXEL_BUF_H;
        state.buffer.resize((state.width * state.height * 4) as usize, 0);
        state.texture_id = None;
        state.image_handle = None;
        state.last_update_time = 0.0;
        state.last_window_size = size;
        state.resize_energy = 0.0;
    }

    let delta_size = (size - state.last_window_size).length();
    state.resize_energy = (state.resize_energy * 0.85 + delta_size * 0.15).min(2000.0);
    state.last_window_size = size;

    ensure_voxel_texture(contexts, images, state);

    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };

    let layer = egui::LayerId::new(egui::Order::Foreground, egui::Id::new("resize_minigame_voxel"));
    let painter = ctx.layer_painter(layer);
    painter.rect_filled(screen_rect, 0.0, egui::Color32::from_black_alpha(210));

    let aspect_buf = state.width as f32 / state.height as f32;
    let aspect_screen = size.x / size.y.max(1.0);
    let (draw_w, draw_h) = if aspect_screen > aspect_buf {
        let h = size.y;
        (h * aspect_buf, h)
    } else {
        let w = size.x;
        (w, w / aspect_buf)
    };
    let draw_rect = egui::Rect::from_center_size(screen_rect.center(), egui::vec2(draw_w, draw_h));

    if t - state.last_update_time >= VOXEL_UPDATE_INTERVAL {
        state.last_update_time = t;
        render_voxel_frame(state, t, dt.max(0.0001));
        if let (Some(handle), Some(_)) = (state.image_handle.clone(), state.texture_id) {
            if let Some(img) = images.get_mut(handle.id()) {
                if let Some(data) = img.data.as_mut() {
                    if data.len() == state.buffer.len() {
                        data.copy_from_slice(&state.buffer);
                    }
                }
            }
        }
    }

    if let Some(tex_id) = state.texture_id {
        painter.image(tex_id, draw_rect, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), egui::Color32::WHITE);
    }

    let info = format!("Voxels  {}x{}  dt:{:>4.1}ms", state.width, state.height, dt * 1000.0);
    painter.text(
        screen_rect.left_top() + egui::vec2(14.0, 12.0),
        egui::Align2::LEFT_TOP,
        info,
        egui::FontId::proportional(16.0),
        egui::Color32::from_white_alpha(210),
    );
}

fn ensure_voxel_texture(contexts: &mut EguiContexts, images: &mut Assets<Image>, state: &mut VoxelDdaState) {
    if state.image_handle.is_none() {
        let size = Extent3d {
            width: state.width,
            height: state.height,
            depth_or_array_layers: 1,
        };
        let image = Image::new_fill(
            size,
            TextureDimension::D2,
            &state.buffer,
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
        );
        let handle = images.add(image);
        state.image_handle = Some(handle);
    }

    if state.texture_id.is_none() {
        if let Some(handle) = state.image_handle.clone() {
            let tex_id = contexts.add_image(bevy_egui::EguiTextureHandle::Strong(handle));
            state.texture_id = Some(tex_id);
        }
    }
}

fn render_voxel_frame(state: &mut VoxelDdaState, t: f32, dt: f32) {
    let w = state.width as usize;
    let h = state.height as usize;
    let inv_w = 1.0 / state.width as f32;
    let inv_h = 1.0 / state.height as f32;
    let aspect = state.width as f32 / state.height as f32;
    let fov = 60.0_f32.to_radians();
    let scale = (fov * 0.5).tan();

    let yaw = t * 0.35 + dt * 0.1;
    let (sy, cy) = yaw.sin_cos();
    let origin = Vec3::new(0.0, 1.6, -6.0);

    let energy = (state.resize_energy * 0.003).clamp(0.0, 1.0);
    let light_dir = Vec3::new(0.6, 1.0 + energy * 0.6, 0.4).normalize();

    for y in 0..h {
        let ndc_y = 1.0 - (y as f32 + 0.5) * inv_h * 2.0;
        for x in 0..w {
            let ndc_x = (x as f32 + 0.5) * inv_w * 2.0 - 1.0;
            let mut dir = Vec3::new(ndc_x * aspect * scale, ndc_y * scale, 1.0).normalize();
            dir = Vec3::new(dir.x * cy + dir.z * sy, dir.y, -dir.x * sy + dir.z * cy).normalize();

            let (hit, normal, voxel_y) = dda_trace(origin, dir);
            let (r, g, b) = if hit {
                let shade = (normal.dot(light_dir).max(0.0) * 0.7 + 0.3).clamp(0.0, 1.0);
                let base = if voxel_y <= 0 {
                    Vec3::new(0.3, 0.35 + energy * 0.2, 0.25)
                } else {
                    Vec3::new(0.6, 0.5, 0.35 + energy * 0.3)
                };
                let c = base * shade;
                ((c.x * 255.0) as u8, (c.y * 255.0) as u8, (c.z * 255.0) as u8)
            } else {
                let sky = (dir.y * 0.5 + 0.5).clamp(0.0, 1.0);
                let c = Vec3::new(0.08 + 0.12 * sky, 0.10 + 0.14 * sky, 0.16 + 0.24 * sky);
                ((c.x * 255.0) as u8, (c.y * 255.0) as u8, (c.z * 255.0) as u8)
            };

            let idx = (y * w + x) * 4;
            state.buffer[idx] = r;
            state.buffer[idx + 1] = g;
            state.buffer[idx + 2] = b;
            state.buffer[idx + 3] = 255;
        }
    }
}

fn dda_trace(origin: Vec3, dir: Vec3) -> (bool, Vec3, i32) {
    let mut voxel = origin.floor().as_ivec3();
    let step = IVec3::new(
        if dir.x >= 0.0 { 1 } else { -1 },
        if dir.y >= 0.0 { 1 } else { -1 },
        if dir.z >= 0.0 { 1 } else { -1 },
    );

    let mut t_max = Vec3::new(
        next_boundary(origin.x, dir.x, voxel.x),
        next_boundary(origin.y, dir.y, voxel.y),
        next_boundary(origin.z, dir.z, voxel.z),
    );
    let t_delta = Vec3::new(inv_dir(dir.x), inv_dir(dir.y), inv_dir(dir.z));

    let mut normal = Vec3::ZERO;
    for _ in 0..VOXEL_MAX_STEPS {
        if voxel_solid(voxel.x, voxel.y, voxel.z) {
            return (true, normal, voxel.y);
        }

        if t_max.x < t_max.y {
            if t_max.x < t_max.z {
                voxel.x += step.x;
                t_max.x += t_delta.x;
                normal = Vec3::new(-step.x as f32, 0.0, 0.0);
            } else {
                voxel.z += step.z;
                t_max.z += t_delta.z;
                normal = Vec3::new(0.0, 0.0, -step.z as f32);
            }
        } else if t_max.y < t_max.z {
            voxel.y += step.y;
            t_max.y += t_delta.y;
            normal = Vec3::new(0.0, -step.y as f32, 0.0);
        } else {
            voxel.z += step.z;
            t_max.z += t_delta.z;
            normal = Vec3::new(0.0, 0.0, -step.z as f32);
        }

        if voxel.x < -18 || voxel.x > 18 || voxel.y < -2 || voxel.y > 10 || voxel.z < -18 || voxel.z > 18 {
            break;
        }
    }

    (false, Vec3::ZERO, voxel.y)
}

fn next_boundary(origin: f32, dir: f32, voxel: i32) -> f32 {
    if dir.abs() < 0.0001 {
        return f32::INFINITY;
    }
    let next = if dir >= 0.0 { voxel as f32 + 1.0 } else { voxel as f32 };
    (next - origin) / dir
}

fn inv_dir(dir: f32) -> f32 {
    if dir.abs() < 0.0001 {
        f32::INFINITY
    } else {
        (1.0 / dir).abs()
    }
}

fn voxel_solid(x: i32, y: i32, z: i32) -> bool {
    if y < 0 {
        return true;
    }
    if y > 8 {
        return false;
    }

    let h = voxel_hash(x, z);
    let pillar = (h & 7) < 2;
    let height = 2 + ((h >> 3) & 3) as i32;
    if pillar && y < height {
        return true;
    }

    let ridge = (h & 31) == 0;
    ridge && y < 5
}

fn voxel_hash(x: i32, z: i32) -> u32 {
    let mut h = (x as u32).wrapping_mul(1973) ^ (z as u32).wrapping_mul(9277) ^ 0x7f4a_7c15;
    h ^= h >> 16;
    h ^= h << 5;
    h
}

fn circle_rect_hit(pos: Vec2, r: f32, rect: &egui::Rect) -> Option<(Vec2, f32)> {
    let nearest_x = pos.x.clamp(rect.min.x, rect.max.x);
    let nearest_y = pos.y.clamp(rect.min.y, rect.max.y);
    let nearest = Vec2::new(nearest_x, nearest_y);
    let delta = pos - nearest;
    let dist2 = delta.length_squared();

    if dist2 > r * r {
        return None;
    }

    if dist2 > 0.0001 {
        let dist = dist2.sqrt();
        let normal = delta / dist;
        return Some((normal, r - dist));
    }

    let left = (pos.x - rect.min.x).abs();
    let right = (rect.max.x - pos.x).abs();
    let top = (pos.y - rect.min.y).abs();
    let bottom = (rect.max.y - pos.y).abs();

    let (normal, push) = if left.min(right) < top.min(bottom) {
        if left < right {
            (Vec2::new(-1.0, 0.0), r)
        } else {
            (Vec2::new(1.0, 0.0), r)
        }
    } else if top < bottom {
        (Vec2::new(0.0, -1.0), r)
    } else {
        (Vec2::new(0.0, 1.0), r)
    };

    Some((normal, push))
}
