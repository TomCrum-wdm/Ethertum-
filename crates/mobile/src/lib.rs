use bevy::prelude::*;

#[bevy_main]
fn main() {
    #[cfg(target_os = "android")]
    std::panic::set_hook(Box::new(|info| {
        eprintln!("PANIC: {}", info);
    }));

    let mut primary_window = Window {
        resizable: false,
        ..default()
    };

    #[cfg(not(target_os = "android"))]
    {
        primary_window.mode = bevy::window::WindowMode::BorderlessFullscreen(bevy::window::MonitorSelection::Primary);
    }

    let mut default_plugins = DefaultPlugins.set(WindowPlugin {
        primary_window: Some(primary_window),
        ..default()
    });

    #[cfg(target_os = "android")]
    {
        default_plugins = default_plugins.disable::<bevy::audio::AudioPlugin>();
    }

    let mut app = App::new();
    app.add_plugins(default_plugins)
    .add_plugins(ethertia::client::prelude::ClientGamePlugin);

    app.run();
}
