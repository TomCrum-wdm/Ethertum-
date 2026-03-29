#[cfg(feature = "ddgi")]
mod ddgi;

pub mod character_controller;
pub mod game_client;
pub mod ui;

mod client_world;
mod input;
pub mod settings;

pub mod prelude {
    use super::*;

    #[cfg(feature = "ddgi")]
    pub use ddgi::*;

    pub use character_controller::{CharacterController, CharacterControllerBundle, CharacterControllerCamera, CharacterControllerPlugin};
    pub use client_world::{ClientPlayerInfo, DespawnOnWorldUnload, WorldInfo};
    pub use game_client::{condition, ClientGamePlugin, ClientInfo, EthertiaClient};
    pub use input::{InputAction, TouchButtonState, TouchStickState};
    pub use settings::{ClientSettings, ServerListItem, TouchActionBinding};
    pub use ui::{CurrentUI, UiExtra};

    pub use crate::item::{Inventory, ItemStack};
}

// Editor plugin depends on optional crate `bevy_editor_pls`.
// Only compile the module when both the native target and the optional
// `bevy_editor_pls` dependency are enabled.
#[cfg(all(feature = "target_native_os", feature = "bevy_editor_pls"))]
pub mod editor;
