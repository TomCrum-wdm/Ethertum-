use bevy::{
	input::mouse::MouseMotion,
	prelude::*,
};

use crate::client::prelude::*;

pub struct EditorViewPlugin;

impl Plugin for EditorViewPlugin {
	fn build(&self, app: &mut App) {
		app.add_systems(
			Update,
			(toggle_editor_view, update_editor_free_camera)
				.chain()
				.run_if(condition::in_world),
		);
		app.add_systems(Update, update_editor_viewport_camera.run_if(condition::in_world));
	}
}

#[derive(Default)]
struct EditorCameraAngles {
	initialized: bool,
	yaw: f32,
	pitch: f32,
}

fn toggle_editor_view(
	key: Res<ButtonInput<KeyCode>>,
	mut cli: ResMut<ClientInfo>,
	query_cam: Query<&Transform, With<CharacterControllerCamera>>,
	mut angles: Local<EditorCameraAngles>,
) {
	if !cli.is_admin {
		cli.global_editor_view = false;
		angles.initialized = false;
		return;
	}

	if !key.just_pressed(KeyCode::F7) {
		return;
	}

	cli.global_editor_view = !cli.global_editor_view;
	if !cli.global_editor_view {
		angles.initialized = false;
		return;
	}

	if let Ok(cam) = query_cam.single() {
		let (yaw, pitch, _) = cam.rotation.to_euler(EulerRot::YXZ);
		angles.yaw = yaw;
		angles.pitch = pitch;
		angles.initialized = true;
	}
}

fn update_editor_free_camera(
	cli: Res<ClientInfo>,
	time: Res<Time>,
	key: Res<ButtonInput<KeyCode>>,
	mut mouse_motion_events: EventReader<MouseMotion>,
	mut query_cam: Query<&mut Transform, With<CharacterControllerCamera>>,
	mut angles: Local<EditorCameraAngles>,
) {
	if !cli.global_editor_view {
		return;
	}

	let Ok(mut cam) = query_cam.single_mut() else {
		return;
	};

	if !angles.initialized {
		let (yaw, pitch, _) = cam.rotation.to_euler(EulerRot::YXZ);
		angles.yaw = yaw;
		angles.pitch = pitch;
		angles.initialized = true;
	}

	let mouse_delta = mouse_motion_events
		.read()
		.fold(Vec2::ZERO, |acc, ev| acc + ev.delta);

	let look_sensitivity = 0.003;
	angles.yaw -= mouse_delta.x * look_sensitivity;
	angles.pitch = (angles.pitch - mouse_delta.y * look_sensitivity)
		.clamp(-std::f32::consts::FRAC_PI_2 + 0.01, std::f32::consts::FRAC_PI_2 - 0.01);

	cam.rotation = Quat::from_euler(EulerRot::YXZ, angles.yaw, angles.pitch, 0.0);

	let mut move_dir = Vec3::ZERO;
	if key.pressed(KeyCode::KeyW) {
		move_dir += *cam.forward();
	}
	if key.pressed(KeyCode::KeyS) {
		move_dir -= *cam.forward();
	}
	if key.pressed(KeyCode::KeyA) {
		move_dir -= *cam.right();
	}
	if key.pressed(KeyCode::KeyD) {
		move_dir += *cam.right();
	}
	if key.pressed(KeyCode::KeyE) {
		move_dir += Vec3::Y;
	}
	if key.pressed(KeyCode::KeyQ) {
		move_dir -= Vec3::Y;
	}

	if move_dir.length_squared() > 0.0 {
		let speed = if key.pressed(KeyCode::ShiftLeft) { 200.0 } else { 60.0 };
		cam.translation += move_dir.normalize() * speed * time.delta_secs();
	}
}

#[derive(Default)]
struct EditorViewportCameraAngles {
	initialized: bool,
	yaw: f32,
	pitch: f32,
	orbit_focus: Vec3,
	orbit_distance: f32,
}

fn update_editor_viewport_camera(
	cli: Res<ClientInfo>,
	editor_runtime: Res<EditorRuntime>,
	time: Res<Time>,
	key: Res<ButtonInput<KeyCode>>,
	mut mouse_motion_events: EventReader<MouseMotion>,
	mut query_cam: Query<(&mut Transform, &mut Projection), With<EditorViewportCamera>>,
	mut angles: Local<EditorViewportCameraAngles>,
) {
	let active = cli.curr_ui == CurrentUI::WorldEditor && editor_runtime.view_mode == EditorViewMode::View3D;
	if !active {
		angles.initialized = false;
		return;
	}

	let Ok((mut cam, mut projection)) = query_cam.single_mut() else {
		return;
	};

	if !angles.initialized {
		let (yaw, pitch, _) = cam.rotation.to_euler(EulerRot::YXZ);
		angles.yaw = yaw;
		angles.pitch = pitch;
		angles.orbit_distance = 128.0;
		angles.orbit_focus = cam.translation + *cam.forward() * angles.orbit_distance;
		angles.initialized = true;
	}

	let mouse_delta = mouse_motion_events
		.read()
		.fold(Vec2::ZERO, |acc, ev| acc + ev.delta);

	let look_sensitivity = 0.003;

	match editor_runtime.camera_mode {
		EditorCameraMode::Fly => {
			if !matches!(*projection, Projection::Perspective(_)) {
				*projection = Projection::Perspective(PerspectiveProjection::default());
			}

			angles.yaw -= mouse_delta.x * look_sensitivity;
			angles.pitch = (angles.pitch - mouse_delta.y * look_sensitivity)
				.clamp(-std::f32::consts::FRAC_PI_2 + 0.01, std::f32::consts::FRAC_PI_2 - 0.01);
			cam.rotation = Quat::from_euler(EulerRot::YXZ, angles.yaw, angles.pitch, 0.0);

			let mut move_dir = Vec3::ZERO;
			if key.pressed(KeyCode::KeyW) {
				move_dir += *cam.forward();
			}
			if key.pressed(KeyCode::KeyS) {
				move_dir -= *cam.forward();
			}
			if key.pressed(KeyCode::KeyA) {
				move_dir -= *cam.right();
			}
			if key.pressed(KeyCode::KeyD) {
				move_dir += *cam.right();
			}
			if key.pressed(KeyCode::KeyE) {
				move_dir += Vec3::Y;
			}
			if key.pressed(KeyCode::KeyQ) {
				move_dir -= Vec3::Y;
			}

			if move_dir.length_squared() > 0.0 {
				let speed = if key.pressed(KeyCode::ShiftLeft) { 220.0 } else { 70.0 };
				cam.translation += move_dir.normalize() * speed * time.delta_secs();
			}
		}
		EditorCameraMode::Orbit => {
			if !matches!(*projection, Projection::Perspective(_)) {
				*projection = Projection::Perspective(PerspectiveProjection::default());
			}

			angles.yaw -= mouse_delta.x * look_sensitivity;
			angles.pitch = (angles.pitch - mouse_delta.y * look_sensitivity)
				.clamp(-1.5, 1.5);

			if key.pressed(KeyCode::KeyE) {
				angles.orbit_distance = (angles.orbit_distance - 80.0 * time.delta_secs()).max(8.0);
			}
			if key.pressed(KeyCode::KeyQ) {
				angles.orbit_distance = (angles.orbit_distance + 80.0 * time.delta_secs()).min(2000.0);
			}

			let rot = Quat::from_euler(EulerRot::YXZ, angles.yaw, angles.pitch, 0.0);
			let offset = rot * Vec3::new(0.0, 0.0, angles.orbit_distance);
			cam.translation = angles.orbit_focus + offset;
			cam.look_at(angles.orbit_focus, Vec3::Y);
		}
		EditorCameraMode::TopDown => {
			let ortho = match &*projection {
				Projection::Orthographic(v) => {
					let mut out = v.clone();
					out.scale = out.scale.clamp(0.4, 12.0);
					out
				}
				_ => OrthographicProjection {
					scale: 1.6,
					near: -4000.0,
					far: 4000.0,
					..OrthographicProjection::default_3d()
				},
			};
			*projection = Projection::Orthographic(ortho);

			let mut pan = Vec3::ZERO;
			if key.pressed(KeyCode::KeyW) {
				pan += Vec3::new(0.0, 0.0, -1.0);
			}
			if key.pressed(KeyCode::KeyS) {
				pan += Vec3::new(0.0, 0.0, 1.0);
			}
			if key.pressed(KeyCode::KeyA) {
				pan += Vec3::new(-1.0, 0.0, 0.0);
			}
			if key.pressed(KeyCode::KeyD) {
				pan += Vec3::new(1.0, 0.0, 0.0);
			}
			if pan.length_squared() > 0.0 {
				let speed = if key.pressed(KeyCode::ShiftLeft) { 180.0 } else { 70.0 };
				cam.translation += pan.normalize() * speed * time.delta_secs();
			}

			angles.yaw -= mouse_delta.x * look_sensitivity * 0.35;
			cam.rotation = Quat::from_axis_angle(Vec3::Y, angles.yaw)
				* Quat::from_euler(EulerRot::XYZ, -std::f32::consts::FRAC_PI_2 + 0.001, 0.0, 0.0);
		}
	}
}
