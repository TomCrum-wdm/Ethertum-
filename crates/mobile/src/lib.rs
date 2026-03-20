use bevy::prelude::*;

#[cfg(target_os = "android")]
fn boot_log(message: &str) {
    use std::fs::OpenOptions;
    use std::io::Write;

    eprintln!("[BOOT] {}", message);

    let mut candidates = Vec::new();
    if let Ok(home) = std::env::var("HOME") {
        candidates.push(format!("{}/ethertia_boot.log", home));
    }
    candidates.push("/data/data/com.ethertia.client/files/ethertia_boot.log".to_string());
    candidates.push("/data/user/0/com.ethertia.client/files/ethertia_boot.log".to_string());
    candidates.push("/data/local/tmp/ethertia_boot.log".to_string());

    for path in candidates {
        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&path) {
            let _ = writeln!(file, "{}", message);
            break;
        }
    }
}

#[bevy_main]
fn main() {
    #[cfg(target_os = "android")]
    boot_log("mobile::main entered");

    #[cfg(target_os = "android")]
    {
        // Follow project default rendering path: force Vulkan on Android.
        std::env::set_var("WGPU_BACKEND", "vulkan");
        boot_log("set WGPU_BACKEND=vulkan");
    }

    #[cfg(target_os = "android")]
    std::panic::set_hook(Box::new(|info| {
        boot_log(&format!("panic: {}", info));
        eprintln!("PANIC: {}", info);
    }));

    #[cfg(target_os = "android")]
    boot_log("panic hook installed");

    let mut primary_window = Window {
        resizable: false,
        ..default()
    };

    #[cfg(not(target_os = "android"))]
    {
        primary_window.mode = bevy::window::WindowMode::BorderlessFullscreen(bevy::window::MonitorSelection::Primary);
    }

    #[cfg(target_os = "android")]
    boot_log("primary window configured");

    let mut default_plugins = DefaultPlugins.set(WindowPlugin {
        primary_window: Some(primary_window),
        ..default()
    });

    #[cfg(target_os = "android")]
    {
        default_plugins = default_plugins.disable::<bevy::audio::AudioPlugin>();
        boot_log("android audio plugin disabled");
    }

    let mut app = App::new();
    #[cfg(target_os = "android")]
    boot_log("bevy app created");

    app.add_plugins(default_plugins)
    .add_plugins(ethertia::client::prelude::ClientGamePlugin);

    #[cfg(target_os = "android")]
    boot_log("plugins added, entering app.run");

    app.run();
}
