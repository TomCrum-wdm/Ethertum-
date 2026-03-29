use bevy::prelude::*;

pub struct EditorPlugin;

impl Plugin for EditorPlugin {
	fn build(&self, app: &mut App) {
		// Editor
		use bevy_editor_pls::prelude::*;
		app.add_plugins(
			EditorPlugin::default(),
		);

		// Setup Controls
		app.insert_resource(res_editor_controls());
		app.add_systems(Startup, setup_editor_camera_controls);
		// app.add_systems(Update, handle_inputs);
	}
}
fn res_editor_controls() -> bevy_editor_pls::controls::EditorControls {
	use bevy_editor_pls::controls::*;
	let mut editor_controls = EditorControls::default_bindings();
	editor_controls.unbind(Action::PlayPauseEditor);

	editor_controls.insert(
		Action::PlayPauseEditor,
		Binding {
			input: UserInput::Single(Button::Keyboard(KeyCode::Escape)),
			conditions: vec![BindingCondition::ListeningForText(false)],
		},
	);

	editor_controls
}

fn setup_editor_camera_controls(mut query: Query<&mut bevy_editor_pls::default_windows::cameras::camera_3d_free::FlycamControls>) {
	let mut controls = query.single_mut();
	controls.key_up = KeyCode::KeyE;
	controls.key_down = KeyCode::KeyQ;
}
//             for mut controller in &mut controller_query {
//                 controller.enable_input = playing;
//             }
//         }
//     }

//     // Toggle Fullscreen
//     if key.just_pressed(KeyCode::F11) || (key.pressed(KeyCode::AltLeft) && key.just_pressed(KeyCode::Return)) {
//         window.mode = if window.mode != WindowMode::Fullscreen {
//             WindowMode::Fullscreen
//         } else {
//             WindowMode::Windowed
//         };
//     }
// }
