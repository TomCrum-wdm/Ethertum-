use std::f32::consts::{FRAC_PI_2, PI};

use crate::client::prelude::*;
use crate::util::SmoothValue;

use bevy::{
    input::mouse::{MouseMotion, MouseWheel},
    prelude::*, transform::TransformSystems,
};
use avian3d::prelude::*;
use leafwing_input_manager::action_state::ActionState;

pub struct CharacterControllerPlugin;

impl Plugin for CharacterControllerPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<CharacterController>();

        app.add_systems(Update, input_move.run_if(condition::in_world));
        app.add_systems(Update, sync_noclip_collider.run_if(condition::in_world));

        app.add_systems(
            PostUpdate,
            sync_camera
                .in_set(PhysicsSet::Writeback)
                .run_if(condition::in_world),
        );
    }
}

#[derive(Bundle)]
pub struct CharacterControllerBundle {
    character_controller: CharacterController,
    rigid_body: RigidBody,
    collider: Collider,
    collider_cache: CharacterColliderCache,
    ground_caster: ShapeCaster,
    sleeping_disabled: SleepingDisabled,
    locked_axes: LockedAxes,
    gravity_scale: GravityScale,
    friction: Friction,
    restitution: Restitution,
}
impl CharacterControllerBundle {
    pub fn new(collider: Collider, character_controller: CharacterController) -> Self {
        // Create shape caster as a slightly smaller version of collider
        let mut caster_shape = collider.clone();
        caster_shape.set_scale(Vec3::ONE * 0.99, 10);

        Self {
            character_controller,
            rigid_body: RigidBody::Dynamic,
            collider: collider.clone(),
            collider_cache: CharacterColliderCache(collider),
            ground_caster: ShapeCaster::new(caster_shape, Vec3::ZERO, Quat::default(), Dir3::NEG_Y)
                .with_max_distance(0.2),
            sleeping_disabled: SleepingDisabled,
            locked_axes: LockedAxes::ROTATION_LOCKED,
            gravity_scale: GravityScale(2.),
            friction: Friction::ZERO.with_combine_rule(CoefficientCombine::Min),
            restitution: Restitution::ZERO.with_combine_rule(CoefficientCombine::Min),
        }
    }
}

/// a tag, sync transform
#[derive(Component)]
pub struct CharacterControllerCamera;

#[derive(Component, Clone)]
pub struct CharacterColliderCache(pub Collider);

#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct CharacterController {
    // State
    pub pitch: f32,
    pub yaw: f32,

    pub is_flying: bool,
    pub allow_god_mode: bool,
    pub noclip_enabled: bool,
    // sprint: bool,
    // sneak: bool,
    // jump: bool,

    // Readonly State
    pub is_grounded: bool,

    pub is_sprinting: bool,
    pub is_sneaking: bool,

    // Control Param
    pub jump_impulse: f32,
    pub acceleration: f32,
    pub max_slope_angle: f32,
    pub unfly_on_ground: bool,

    // 3rd person camera distance.
    pub cam_distance: f32,

    // Input
    pub enable_input: bool,

    /// enable:  Yaw/Pitch by CursorMove,           and make Cursor Grabbed/Invisible.  like MC-PC
    /// disable: Yaw/Pitch by CursorDrag/TouchMove. and make Cursor Visible             like MC-PE
    /// only valid on enable_input=true,
    pub enable_input_cursor_look: bool,
    // fly_speed: f32,
    // walk_speed: f32,

    // Tmp KeyConfig
    // key_forward: KeyCode,
    // key_back: KeyCode,
    // key_left: KeyCode,
    // key_right: KeyCode,
    // key_up: KeyCode,    // flymode
    // key_down: KeyCode,  // flymode
    // key_sprint: KeyCode,
    // key_sneak: KeyCode,
    // key_jump: KeyCode,

    // mouse_sensitivity: f32,
}

impl Default for CharacterController {
    fn default() -> Self {
        Self {
            yaw: 0.,
            pitch: 0.,
            is_flying: false,
            allow_god_mode: false,
            noclip_enabled: false,
            enable_input: true,
            enable_input_cursor_look: true,
            is_grounded: false,
            is_sprinting: false,
            is_sneaking: false,
            jump_impulse: 7.,
            acceleration: 50.,
            max_slope_angle: PI * 0.25,
            cam_distance: 0.,
            unfly_on_ground: true,
        }
    }
}

#[derive(Default)]
struct InputTimingState {
    last_fly_jump: f32,
    last_forward_press: f32,
    last_jump: f32,
}

fn tangent_basis_from_up(up: Vec3, yaw: f32) -> (Vec3, Vec3) {
    let up = safe_unit_vec3(up, Vec3::Y);
    let up_align = Quat::from_rotation_arc(Vec3::Y, up);
    let mut base_forward = safe_unit_vec3(up_align * Vec3::Z, Vec3::Z);
    if base_forward.dot(up).abs() > 0.999 {
        base_forward = safe_unit_vec3(up_align * Vec3::X, Vec3::X);
    }
    let yaw_rot = Quat::from_axis_angle(up, yaw);
    let forward = safe_unit_vec3(yaw_rot * base_forward, base_forward);
    let right = safe_unit_vec3(up.cross(forward), Vec3::X);

    (forward, right)
}

fn safe_unit_vec3(v: Vec3, fallback: Vec3) -> Vec3 {
    if !v.is_finite() {
        return fallback;
    }
    let len_sq = v.length_squared();
    if !len_sq.is_finite() || len_sq <= 1e-8 {
        return fallback;
    }

    let normalized = v / len_sq.sqrt();
    if !normalized.is_finite() || normalized.length_squared() <= 1e-8 {
        fallback
    } else {
        normalized
    }
}

fn safe_dt_secs(dt: f32) -> f32 {
    if !dt.is_finite() {
        return 1.0 / 60.0;
    }
    dt.clamp(1.0 / 240.0, 0.1)
}

// fn handle_input(

// ) {

// }

fn input_move(
    input_key: Res<ButtonInput<KeyCode>>,
    input_mouse_button: Res<ButtonInput<MouseButton>>,
    mut mouse_motion_events: EventReader<MouseMotion>,
    mut mouse_wheel_events: EventReader<MouseWheel>,
    touches: Res<Touches>,
    cfg: Res<ClientSettings>,
    touch_sticks: Res<TouchStickState>,
    touch_buttons: Res<TouchButtonState>,
    worldinfo: Res<WorldInfo>,

    time: Res<Time>,
    query_input: Query<&ActionState<InputAction>>,
    mut query: Query<(
        &mut Transform,
        &mut CharacterController,
        &mut LinearVelocity,
        &mut GravityScale,
        &mut ShapeCaster,
        &ShapeHits,
        &Position,
    )>,
    mut cam_dist_smoothed: Local<SmoothValue>,
    mut input_timing: Local<InputTimingState>,
) {
    let mouse_delta = mouse_motion_events.read().fold(Vec2::ZERO, |acc, v| acc + v.delta);
    let wheel_delta = mouse_wheel_events.read().fold(0.0, |acc, v| acc + v.x + v.y);
    let dt_sec = safe_dt_secs(time.delta_secs());

    let Ok(action_state) = query_input.single() else {
        return;
    };

    let world_cfg = worldinfo.world_config.clone();

    for (mut trans, mut ctl, mut linvel, mut gravity_scale, mut ground_caster, hits, position) in query.iter_mut() {
        if !ctl.yaw.is_finite() {
            ctl.yaw = 0.0;
        }
        if !ctl.pitch.is_finite() {
            ctl.pitch = 0.0;
        }
        if !ctl.cam_distance.is_finite() {
            ctl.cam_distance = 0.0;
        }

        // A Local-Space Movement.  Speed/Acceleration/Delta will applied later on this.
        let mut movement = Vec3::ZERO;
        let effective_flying = ctl.is_flying || ctl.noclip_enabled;
        let local_up = safe_unit_vec3(world_cfg.world_up_at(position.0), Vec3::Y);
        let is_planet_world = world_cfg.terrain_mode == crate::voxel::WorldTerrainMode::Planet;

        ground_caster.direction = if is_planet_world {
            Dir3::new(-local_up).unwrap_or(Dir3::NEG_Y)
        } else {
            Dir3::NEG_Y
        };

        // Flying
        if effective_flying {
            gravity_scale.0 = 0.0;
        } else if is_planet_world {
            gravity_scale.0 = 0.0;
        } else {
            gravity_scale.0 = (world_cfg.gravity_acceleration / 9.81).max(0.0);
        }

        if ctl.enable_input {
            // View Rotation
            let look_sensitivity = 0.003;
            let mouse_delta = mouse_delta * look_sensitivity; //ctl.mouse_sensitivity;

            if ctl.enable_input_cursor_look || input_mouse_button.pressed(MouseButton::Left) {
                ctl.pitch -= mouse_delta.y;
                ctl.yaw -= mouse_delta.x;
            }

            // Touch look handling.
            if cfg.touch_ui {
                let block_touch_look = touch_buttons.attack_pressed
                    || touch_buttons.use_pressed
                    || touch_buttons.jump_pressed
                    || touch_buttons.sprint_pressed
                    || touch_buttons.crouch_pressed
                    || touch_buttons.vertical_active;
                if !block_touch_look {
                for touch in touches.iter() {
                    if touch_sticks.move_touch_id == Some(touch.id()) {
                        continue;
                    }
                    let mov = touch.delta();
                    ctl.pitch -= look_sensitivity * mov.y;
                    ctl.yaw -= look_sensitivity * mov.x;
                }
                }
            } else {
                for touch in touches.iter() {
                    let mov = touch.delta();
                    ctl.pitch -= look_sensitivity * mov.y;
                    ctl.yaw -= look_sensitivity * mov.x;
                }
            }

            // TouchStickUi / Gamepad: Look
            {
                let axis_value = action_state.clamped_axis_pair(&InputAction::Look).xy();

                let look_sensitivity = look_sensitivity * 10.;
                ctl.pitch += look_sensitivity * axis_value.y;
                ctl.yaw -= look_sensitivity * axis_value.x;

                if touch_sticks.active {
                    // For Android touch UI, look is handled by direct finger drag; no right-touch stick.
                    // Keep move axis only from left joystick.
                }
            }

            let mut is_move_forward = false;
            // TouchStickUi / Gamepad: Move
            {
                let axis_value = action_state.clamped_axis_pair(&InputAction::Move).xy();
                if axis_value.y > 0. {
                    is_move_forward = true;
                }

                // info!("moving: {axis_value}");
                movement.x += axis_value.x;
                movement.z -= axis_value.y;

                if touch_sticks.active {
                    if touch_sticks.move_axis.y > 0.0 {
                        is_move_forward = true;
                    }
                    movement.x += touch_sticks.move_axis.x;
                    movement.z -= touch_sticks.move_axis.y;
                }
            }

            // Clamp/Normalize
            let pitch_limit = FRAC_PI_2 - 1e-3;
            ctl.pitch = ctl.pitch.clamp(-pitch_limit, pitch_limit);
            if ctl.yaw.abs() > PI {
                ctl.yaw = ctl.yaw.rem_euclid(2. * PI);
            }

            // 3rd Person Camera: Distance Control.
            if input_key.pressed(KeyCode::AltLeft) {
                let d = (ctl.cam_distance * 0.18).max(0.3) * -wheel_delta;
                if cam_dist_smoothed.target < 4. {
                    if cam_dist_smoothed.target != 0. {
                        cam_dist_smoothed.target = 0.;
                    } else if d > 0. {
                        cam_dist_smoothed.target = 4.;
                    }
                } else {
                    cam_dist_smoothed.target += d;
                }
                cam_dist_smoothed.target = cam_dist_smoothed.target.clamp(0., 1_000.);

                cam_dist_smoothed.update(dt_sec * 18.);
                ctl.cam_distance = cam_dist_smoothed.current;
            }

            // if action_state.pressed(&InputAction::Move) {
            //     let axis_value = action_state.clamped_axis_pair(&InputAction::Move).unwrap().xy();

            // }
            // // Move: WSAD
            // if input_key.pressed(KeyCode::KeyA) {
            //     movement.x -= 1.;
            // }
            // if input_key.pressed(KeyCode::KeyD) {
            //     movement.x += 1.;
            // }
            // if input_key.pressed(KeyCode::KeyW) {
            //     movement.z -= 1.;
            // }
            // if input_key.pressed(KeyCode::KeyS) {
            //     movement.z += 1.;
            // }

            let touch_sneak_pressed = cfg.touch_ui && touch_buttons.crouch_pressed;
            ctl.is_sneaking = action_state.pressed(&InputAction::Sneak) || touch_sneak_pressed;

            let touch_jump_pressed = cfg.touch_ui && touch_buttons.jump_pressed;
            let touch_jump_just_pressed = cfg.touch_ui && touch_buttons.jump_just_pressed;

            let is_jump_just_pressed = action_state.just_pressed(&InputAction::Jump) || touch_jump_just_pressed;
            let is_jump_hold = action_state.pressed(&InputAction::Jump) || touch_jump_pressed;

            // Is Grouned
            // The character is grounded if the shape caster has a hit with a normal that isn't too steep.
            ctl.is_grounded = hits.iter().any(|hit| {
                (-hit.normal2).angle_between(local_up).abs() <= ctl.max_slope_angle
            });

            // Fly Move
            if effective_flying {
                if ctl.is_sneaking {
                    movement.y -= 1.;
                }
                if is_jump_hold {
                    movement.y += 1.;
                }
                if cfg.touch_ui {
                    movement.y += touch_buttons.vertical_axis.clamp(-1.0, 1.0);
                }
            }
            // Fly Toggle: Double Space
            let time_now = time.elapsed_secs();
            if ctl.allow_god_mode && is_jump_just_pressed {
                if time_now - input_timing.last_fly_jump < 0.3 {
                    ctl.is_flying = !ctl.is_flying;
                }
                input_timing.last_fly_jump = time_now;
            }
            // UnFly on Touch Ground.
            if ctl.unfly_on_ground && ctl.is_grounded && ctl.is_flying && !ctl.noclip_enabled && !cfg.touch_ui {
                ctl.is_flying = false;
            }

            // Input Sprint
            if is_move_forward {
                let sprint_pressed = action_state.pressed(&InputAction::Sprint)
                    || (cfg.touch_ui
                        && (touch_buttons.sprint_pressed || touch_sticks.sprint_locked));
                if sprint_pressed {
                    ctl.is_sprinting = true;
                }
            } else {
                ctl.is_sprinting = false;
            }
            // Sprint: Double W
            if input_key.just_pressed(KeyCode::KeyW) {
                // todo: LastForward.
                if time_now - input_timing.last_forward_press < 0.3 {
                    ctl.is_sprinting = true;
                }
                input_timing.last_forward_press = time_now;
            }

            // Jump
            if is_jump_hold && ctl.is_grounded && !effective_flying {
                if time_now - input_timing.last_jump > 0.3 {
                    if is_planet_world {
                        let curr_up_speed = linvel.0.dot(local_up);
                        linvel.0 += local_up * (ctl.jump_impulse - curr_up_speed);
                    } else {
                        linvel.0.y = ctl.jump_impulse;
                    }
                }
                input_timing.last_jump = time_now;
                // info!("JMP {:?}", linvel.0);
            }

            // Apply Yaw to Movement & Rotation
            {
                let (forward, right) = tangent_basis_from_up(local_up, ctl.yaw);
                let forward_amount = -movement.z;
                movement = right * movement.x + forward * forward_amount + local_up * movement.y;

                let up_align = Quat::from_rotation_arc(Vec3::Y, local_up);
                let yaw_rot = Quat::from_axis_angle(local_up, ctl.yaw);
                trans.rotation = yaw_rot * up_align;
            }
        }

        if !effective_flying && is_planet_world {
            linvel.0 += -local_up * world_cfg.gravity_acceleration * dt_sec;
        }

        // Movement
        let mut acceleration = ctl.acceleration;
        if ctl.is_sprinting {
            acceleration *= 2.;
        }

        if effective_flying {
            linvel.0 += movement * acceleration * dt_sec;
        } else {
            if ctl.is_sneaking {
                // !Minecraft [Sneak] * 0.3
                acceleration *= 0.3;
            } // else if using item: // Minecraft [UsingItem] * 0.2

            if !ctl.is_grounded {
                acceleration *= 0.2; // LessMove on air MC-Like 0.2
            }

            if is_planet_world {
                linvel.0 += movement * acceleration * dt_sec;
            } else {
                linvel.x += movement.x * acceleration * dt_sec;
                linvel.z += movement.z * acceleration * dt_sec;
            }
        }

        // Damping
        if effective_flying {
            linvel.0 *= 0.01f32.powf(dt_sec);
        } else {
            let mut damping_factor = 0.0001f32.powf(dt_sec);
            if !ctl.is_grounded {
                damping_factor = 0.07f32.powf(dt_sec);
            }

            if is_planet_world {
                let v_up = local_up * linvel.0.dot(local_up);
                let v_tangent = linvel.0 - v_up;
                linvel.0 = v_tangent * damping_factor + v_up;
            } else {
                // We could use `LinearDamping`, but we don't want to dampen movement along the Y axis
                linvel.x *= damping_factor;
                linvel.z *= damping_factor;
            }
        }
        // if ctl.flying {
        //     linvel.0 *= damping_factor;
        // } else if ctl.is_grounded {
        //     linvel.x *= damping_factor;
        //     linvel.z *= damping_factor;
        // }
    }
}

fn sync_noclip_collider(
    mut cmds: Commands,
    query: Query<(Entity, &CharacterController, &CharacterColliderCache, Option<&Collider>), Changed<CharacterController>>,
) {
    for (entity, ctl, cache, collider) in query.iter() {
        if ctl.noclip_enabled {
            if collider.is_some() {
                if let Ok(mut ec) = cmds.get_entity(entity) {
                    ec.remove::<Collider>();
                }
            }
        } else if collider.is_none() {
            if let Ok(mut ec) = cmds.get_entity(entity) {
                ec.insert(cache.0.clone());
            }
        }
    }
}

fn sync_camera(
    mut query_cam: Query<(&mut Transform, &mut Projection), With<CharacterControllerCamera>>,
    query_char: Query<(&Position, &CharacterController), Without<CharacterControllerCamera>>,
    mut fov_val: Local<SmoothValue>,
    mut smoothed_up: Local<Vec3>,
    mut smoothed_camera_up: Local<Vec3>,
    time: Res<Time>,

    input_key: Res<ButtonInput<KeyCode>>,
    cli: Res<ClientInfo>,
    cfg: Res<ClientSettings>,
    worldinfo: Res<WorldInfo>,
) {
    if cli.global_editor_view {
        return;
    }

    if let Ok((char_pos, ctl)) = query_char.single() {
        if let Ok((mut cam_trans, mut proj)) = query_cam.single_mut() {
            let dt_sec = safe_dt_secs(time.delta_secs());
            let target_up = safe_unit_vec3(worldinfo.world_config.world_up_at(char_pos.0), Vec3::Y);
            if smoothed_up.length_squared() <= 1e-6 || !smoothed_up.is_finite() {
                *smoothed_up = target_up;
            }

            // Smoothly follow planet up-vector to avoid sudden camera flips on very small planets.
            let up_follow = 1.0 - (-10.0 * dt_sec).exp();
            *smoothed_up = safe_unit_vec3(smoothed_up.lerp(target_up, up_follow), target_up);
            let local_up = *smoothed_up;

            let safe_yaw = if ctl.yaw.is_finite() { ctl.yaw } else { 0.0 };
            let safe_pitch = if ctl.pitch.is_finite() { ctl.pitch } else { 0.0 };
            let (yaw_forward, right) = tangent_basis_from_up(local_up, safe_yaw);
            let safe_right = safe_unit_vec3(right, Vec3::X);
            let pitch_rot = Quat::from_axis_angle(safe_right, -safe_pitch.clamp(-FRAC_PI_2 + 1e-3, FRAC_PI_2 - 1e-3));
            let look_dir = safe_unit_vec3(pitch_rot * yaw_forward, yaw_forward);

            // Build a stable camera-up even when looking near vertical, and keep it continuous over time.
            let projected_up = local_up - look_dir * local_up.dot(look_dir);
            let side = safe_unit_vec3(look_dir.cross(local_up), safe_right);
            let fallback_up = safe_unit_vec3(side.cross(look_dir), local_up);
            let mut target_camera_up = if projected_up.length_squared() > 1e-6 {
                safe_unit_vec3(projected_up, fallback_up)
            } else {
                fallback_up
            };

            if smoothed_camera_up.length_squared() <= 1e-6 || !smoothed_camera_up.is_finite() {
                *smoothed_camera_up = target_camera_up;
            }
            if smoothed_camera_up.dot(target_camera_up) < 0.0 {
                target_camera_up = -target_camera_up;
            }

            let up_blend = 1.0 - (-18.0 * dt_sec).exp();
            *smoothed_camera_up = safe_unit_vec3(
                smoothed_camera_up.lerp(target_camera_up, up_blend),
                target_camera_up,
            );
            let camera_up = *smoothed_camera_up;

            cam_trans.rotation = Transform::from_translation(Vec3::ZERO)
                .looking_to(look_dir, camera_up)
                .rotation;
            let safe_cam_distance = if ctl.cam_distance.is_finite() {
                ctl.cam_distance.clamp(0.0, 1000.0)
            } else {
                0.0
            };
            cam_trans.translation = char_pos.0 + local_up * 0.8 - look_dir * safe_cam_distance;

            // Smoothed FOV on sprinting
            fov_val.target = if input_key.pressed(KeyCode::KeyC) {
                24.
            } else if ctl.is_sprinting {
                cfg.fov + 20.
            } else {
                cfg.fov
            };
            fov_val.target = if fov_val.target.is_finite() {
                fov_val.target.clamp(10.0, 170.0)
            } else {
                85.0
            };
            if !fov_val.current.is_finite() {
                fov_val.current = fov_val.target;
            }
            fov_val.update(dt_sec * 16.);

            if let Projection::Perspective(pp) = proj.as_mut() {
                let fov_deg = if fov_val.current.is_finite() {
                    fov_val.current.clamp(10.0, 170.0)
                } else {
                    85.0
                };
                pp.fov = fov_deg.to_radians();
            }
        }
    }
}
