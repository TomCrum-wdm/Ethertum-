use bevy::prelude::*;
use bevy::window::*;
use leafwing_input_manager::prelude::*;
use leafwing_input_manager::action_state::ActionState;
use leafwing_input_manager::user_input::gamepad::GamepadStick;
use leafwing_input_manager::input_map::InputMap;
use leafwing_input_manager::plugin::InputManagerPlugin;

use crate::client::prelude::*;
use crate::client::ui::*;
use crate::prelude::*;

#[derive(Resource, Debug, Clone)]
pub struct TouchStickState {
    pub move_axis: Vec2,
    pub look_axis: Vec2,
    pub move_center: Vec2,
    pub look_center: Vec2,
    pub move_touch_id: Option<u64>,
    pub look_touch_id: Option<u64>,
    pub radius: f32,
    pub dead_zone: f32,
    pub active: bool,
}

impl Default for TouchStickState {
    fn default() -> Self {
        Self {
            move_axis: Vec2::ZERO,
            look_axis: Vec2::ZERO,
            move_center: Vec2::ZERO,
            look_center: Vec2::ZERO,
            move_touch_id: None,
            look_touch_id: None,
            radius: 108.0,
            dead_zone: 0.12,
            active: false,
        }
    }
}

pub fn init(app: &mut App) {
    app.add_systems(Startup, super::input::input_setup);
    app.add_systems(Update, super::input::input_handle);
    app.add_plugins(InputManagerPlugin::<InputAction>::default());
    app.insert_resource(TouchStickState::default());

    #[cfg(target_os = "android")]
    {
        // bevy_touch_stick 0.2 depends on bevy 0.13 and is not compatible with bevy 0.17.
        // Keep the startup hook so Android-specific input UI can be reintroduced with a compatible implementation.
        app.add_systems(Startup, setup_touch_sticks);
        app.add_systems(PreUpdate, update_touch_sticks);
    }
}

#[cfg(target_os = "android")]
fn setup_touch_sticks(_cmds: Commands, _asset_server: Res<AssetServer>) {
}

#[cfg(target_os = "android")]
fn stick_axis_from_touch(position: Vec2, center: Vec2, radius: f32, dead_zone: f32) -> Vec2 {
    if radius <= 0.0 {
        return Vec2::ZERO;
    }

    // Window/touch space is y-down; gameplay look/move uses y-up.
    let delta = position - center;
    let mut axis = Vec2::new(delta.x, -delta.y) / radius;
    let len = axis.length();
    if len > 1.0 {
        axis /= len;
    }
    if axis.length() < dead_zone {
        Vec2::ZERO
    } else {
        axis
    }
}

#[cfg(target_os = "android")]
fn update_touch_sticks(
    touches: Res<Touches>,
    mut state: ResMut<TouchStickState>,
    query_window: Query<&Window, With<PrimaryWindow>>,
    cli: Res<ClientInfo>,
    cfg: Res<ClientSettings>,
) {
    let Ok(window) = query_window.single() else {
        state.move_axis = Vec2::ZERO;
        state.look_axis = Vec2::ZERO;
        state.active = false;
        return;
    };

    let enabled = cfg.touch_ui && cli.curr_ui == CurrentUI::None;
    state.active = enabled;
    if !enabled {
        state.move_axis = Vec2::ZERO;
        state.look_axis = Vec2::ZERO;
        state.move_touch_id = None;
        state.look_touch_id = None;
        return;
    }

    let size = Vec2::new(window.resolution.width(), window.resolution.height());
    state.move_center = Vec2::new(size.x * 0.20, size.y * 0.80);
    state.look_center = Vec2::new(size.x * 0.80, size.y * 0.80);

    let mut move_touch = None;
    let mut look_touch = None;

    for touch in touches.iter() {
        let id = touch.id();
        if Some(id) == state.move_touch_id {
            move_touch = Some(touch.position());
        }
        if Some(id) == state.look_touch_id {
            look_touch = Some(touch.position());
        }
    }

    if move_touch.is_none() {
        state.move_touch_id = None;
    }
    if look_touch.is_none() {
        state.look_touch_id = None;
    }

    if state.move_touch_id.is_none() {
        if let Some(touch) = touches.iter().find(|t| t.position().x <= size.x * 0.5) {
            state.move_touch_id = Some(touch.id());
            move_touch = Some(touch.position());
        }
    }
    if state.look_touch_id.is_none() {
        if let Some(touch) = touches.iter().find(|t| t.position().x > size.x * 0.5) {
            state.look_touch_id = Some(touch.id());
            look_touch = Some(touch.position());
        }
    }

    state.move_axis = move_touch
        .map(|p| stick_axis_from_touch(p, state.move_center, state.radius, state.dead_zone))
        .unwrap_or(Vec2::ZERO);
    state.look_axis = look_touch
        .map(|p| stick_axis_from_touch(p, state.look_center, state.radius, state.dead_zone))
        .unwrap_or(Vec2::ZERO);
}

#[derive(Default, Reflect, Hash, Clone, PartialEq, Eq)]
pub enum InputStickId {
    #[default]
    LeftMove,
    RightLook,
}

#[derive(leafwing_input_manager::Actionlike, PartialEq, Eq, Clone, Copy, Hash, Debug, Reflect)]
pub enum InputAction {
    #[actionlike(DualAxis)]
    Move,
    #[actionlike(DualAxis)]
    Look,

    Jump,
    Sprint,
    Sneak,

    Attack,  // or Break Block
    UseItem, // or Place Block

    // HUD
    ESC, // PauseMenu or MainMenu (not in game)
    Fullscreen,

    TabPlayerList,
    Hotbar1,
    Hotbar2,
    Hotbar3,
    Hotbar4,
    Hotbar5,
    Hotbar6,
    Hotbar7,
    Hotbar8,
    ToggleLook, // toggle Grab-Crosshair or UnGrab-Pointer
}

impl InputAction {
    pub fn default_input_map() -> InputMap<Self> {
        let mut input_map = InputMap::default();

        // Default gamepad input bindings
        input_map.insert_dual_axis(Self::Move, GamepadStick::LEFT);
        input_map.insert_dual_axis(Self::Look, GamepadStick::RIGHT);

        // Default kbm input bindings
        input_map.insert_dual_axis(Self::Move, VirtualDPad::wasd());
        input_map.insert_dual_axis(Self::Move, VirtualDPad::arrow_keys());
        // input_map.insert(Self::Look, VirtualDPad::mouse_motion());  // Don't use MouseMotion for Look, the experimence is quite bad.

        input_map.insert(Self::Jump, KeyCode::Space);
        input_map.insert(Self::Sprint, KeyCode::ControlLeft);
        input_map.insert(Self::Sneak, KeyCode::ShiftLeft);

        input_map.insert(Self::Attack, MouseButton::Left);
        input_map.insert(Self::UseItem, MouseButton::Right);

        input_map.insert(Self::ESC, KeyCode::Escape);
        input_map.insert(Self::Fullscreen, KeyCode::F11);
        input_map.insert(Self::ToggleLook, KeyCode::Comma);

        input_map // .build()?
    }
}

pub fn input_setup(mut cmds: Commands) {
    cmds.spawn(InputAction::default_input_map());
}

pub fn input_handle(
    key: Res<ButtonInput<KeyCode>>,
    query_input: Query<&ActionState<InputAction>>,

    mut mouse_wheel_events: EventReader<bevy::input::mouse::MouseWheel>,
    mut query_window: Query<&mut Window, With<bevy::window::PrimaryWindow>>,
    mut query_cursor_options: Query<&mut CursorOptions, With<PrimaryWindow>>,
    mut query_controller: Query<&mut CharacterController>,

    worldinfo: Option<ResMut<WorldInfo>>,
    player: Option<ResMut<ClientPlayerInfo>>,
    mut cli: ResMut<ClientInfo>,
    cfg: Res<ClientSettings>,
) {
    let Ok(action_state) = query_input.single() else {
        return;
    };
    let Ok(mut window) = query_window.single_mut() else {
        return;
    };
    let Ok(mut primary_cursor_options) = query_cursor_options.single_mut() else {
        return;
    };

    // ESC
    if action_state.just_pressed(&InputAction::ESC) {
        if worldinfo.is_some() {
            cli.curr_ui = if cli.curr_ui == CurrentUI::None {
                CurrentUI::PauseMenu
            } else {
                CurrentUI::None
            };
        } else {
            cli.curr_ui = CurrentUI::MainMenu;
        }
    }
    // Toggle Game-Manipulating (grabbing mouse / character controlling) when CurrentUi!=None.
    let curr_manipulating = cli.curr_ui == CurrentUI::None;

    // Apply Cursor Grab
    #[cfg(target_os = "android")]
    let cursor_grab = false;
    #[cfg(not(target_os = "android"))]
    let cursor_grab = curr_manipulating && cli.enable_cursor_look;

    primary_cursor_options.grab_mode = if cursor_grab { CursorGrabMode::Locked } else { CursorGrabMode::None };
    primary_cursor_options.visible = !cursor_grab;

    // Enable Character Controlling
    if let Ok(ctr) = &mut query_controller.single_mut() {
        ctr.enable_input = curr_manipulating;
        ctr.enable_input_cursor_look = cursor_grab;
    }

    // Toggle Cursor-Look
    if curr_manipulating && action_state.just_pressed(&InputAction::ToggleLook) {
        cli.enable_cursor_look = !cli.enable_cursor_look;
    }

    if curr_manipulating && !key.pressed(KeyCode::AltLeft) && player.is_some() {
        let wheel_delta = mouse_wheel_events.read().fold(0.0, |acc, v| acc + v.x + v.y);
        if let Some(mut player) = player {
            player.hotbar_index = (player.hotbar_index as i32 + -wheel_delta as i32).rem_euclid(ClientPlayerInfo::HOTBAR_SLOTS as i32) as u32;
        }
    }

    // Temporary F4 Debug Settings
    if key.just_pressed(KeyCode::F4) {
        cli.curr_ui = CurrentUI::Settings;
    }

    if key.just_pressed(KeyCode::F6) {
        cli.dbg_tex = !cli.dbg_tex;
    }

    // Temporary Toggle F9 Debug Inspector
    if key.just_pressed(KeyCode::F9) {
        cli.dbg_inspector = !cli.dbg_inspector;
    }

    // Toggle F3 Debug TextInfo
    if key.just_pressed(KeyCode::F3) {
        cli.dbg_text = !cli.dbg_text;
    }

    // Toggle F12 Debug MenuBar
    if key.just_pressed(KeyCode::F12) {
        cli.dbg_menubar = !cli.dbg_menubar;
    }

    // Toggle Fullscreen
    #[cfg(not(target_os = "android"))]
    {
        if action_state.just_pressed(&InputAction::Fullscreen) || (key.pressed(KeyCode::AltLeft) && key.just_pressed(KeyCode::Enter)) {
            window.mode = if window.mode != WindowMode::Windowed {
                WindowMode::Windowed
            } else {
                WindowMode::Fullscreen(MonitorSelection::Current, VideoModeSelection::Current)
            };
        }
    }
    // Vsync
    window.present_mode = if cfg.vsync { PresentMode::AutoVsync } else { PresentMode::AutoNoVsync };

    crate::ui::set_window_size(Vec2::new(window.resolution.width(), window.resolution.height()));
}

// // TouchStick  Move-Left
// cmds.spawn((
//     Name::new("InputStickMove"),
//     DespawnOnWorldUnload,
//     // map this stick as a left gamepad stick (through bevy_input)
//     // leafwing will register this as a normal gamepad
//     TouchStickGamepadMapping::LEFT_STICK,
//     TouchStickUiBundle {
//         stick: TouchStick {
//             id: InputStickId::LeftMove,
//             stick_type: TouchStickType::Fixed,
//             ..default()
//         },
//         // configure the interactable area through bevy_ui
//         style: Style {
//             width: Val::Px(150.),
//             height: Val::Px(150.),
//             position_type: PositionType::Absolute,
//             left: Val::Percent(15.),
//             bottom: Val::Percent(5.),
//             ..default()
//         },
//         ..default()
//     },
// ))
// .with_children(|parent| {
//     parent.spawn((
//         TouchStickUiKnob,
//         ImageBundle {
//             image: asset_server.load("knob.png").into(),
//             style: Style {
//                 width: Val::Px(75.),
//                 height: Val::Px(75.),
//                 ..default()
//             },
//             ..default()
//         },
//     ));
//     parent.spawn((
//         TouchStickUiOutline,
//         ImageBundle {
//             image: asset_server.load("outline.png").into(),
//             style: Style {
//                 width: Val::Px(150.),
//                 height: Val::Px(150.),
//                 ..default()
//             },
//             ..default()
//         },
//     ));
// });

// // spawn a look stick
// cmds.spawn((
//     Name::new("InputStickLook"),
//     DespawnOnWorldUnload,
//     // map this stick as a right gamepad stick (through bevy_input)
//     // leafwing will register this as a normal gamepad
//     TouchStickGamepadMapping::RIGHT_STICK,
//     TouchStickUiBundle {
//         stick: TouchStick {
//             id: InputStickId::RightLook,
//             stick_type: TouchStickType::Floating,
//             ..default()
//         },
//         // configure the interactable area through bevy_ui
//         style: Style {
//             width: Val::Px(150.),
//             height: Val::Px(150.),
//             position_type: PositionType::Absolute,
//             right: Val::Percent(15.),
//             bottom: Val::Percent(5.),
//             ..default()
//         },
//         ..default()
//     },
// ))
// .with_children(|parent| {
//     parent.spawn((
//         TouchStickUiKnob,
//         ImageBundle {
//             image: asset_server.load("knob.png").into(),
//             style: Style {
//                 width: Val::Px(75.),
//                 height: Val::Px(75.),
//                 ..default()
//             },
//             ..default()
//         },
//     ));
//     parent.spawn((
//         TouchStickUiOutline,
//         ImageBundle {
//             image: asset_server.load("outline.png").into(),
//             style: Style {
//                 width: Val::Px(150.),
//                 height: Val::Px(150.),
//                 ..default()
//             },
//             ..default()
//         },
//     ));
// });
