use bevy::prelude::*;

#[bevy_main]
fn main() {
    let mut primary_window = Window {
        resizable: false,
        ..default()
    };

    #[cfg(not(target_os = "android"))]
    {
        primary_window.mode = bevy::window::WindowMode::BorderlessFullscreen(bevy::window::MonitorSelection::Primary);
    }

    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(primary_window),
        ..default()
    }))
    .add_plugins(ethertia::client::prelude::ClientGamePlugin);

    app.run();
}
