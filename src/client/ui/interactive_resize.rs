use bevy::prelude::*;
use bevy::window::WindowResized;
use crate::client::settings::ClientSettings;

#[derive(Resource, Default)]
pub struct FrameIndex(pub u64);

#[derive(Resource, Default)]
pub struct InteractiveResizeState {
    pub in_progress: bool,
    pub just_exited: bool,
    last_resize_frame: Option<u64>,
}


pub fn update_frame_index_system(mut frame: ResMut<FrameIndex>) {
    frame.0 = frame.0.wrapping_add(1);
}

pub fn update_resize_state_system(
    mut events: EventReader<WindowResized>,
    frame: Res<FrameIndex>,
    cfg: Res<ClientSettings>,
    mut state: ResMut<InteractiveResizeState>,
) {
    if events.read().next().is_some() {
        state.last_resize_frame = Some(frame.0);
    }

    let was_in_progress = state.in_progress;
    let threshold = cfg.interactive_resize_debounce_frames.max(1) as u64;
    state.in_progress = state
        .last_resize_frame
        .map(|last| frame.0.saturating_sub(last) <= threshold)
        .unwrap_or(false);
    state.just_exited = was_in_progress && !state.in_progress;
}
