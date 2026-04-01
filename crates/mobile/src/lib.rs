use bevy::prelude::*;

#[cfg(target_os = "android")]
fn boot_log(message: &str) {
    use std::fs::OpenOptions;
    use std::io::Write;
    use std::ffi::CString;
    use std::os::raw::c_char;

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

    // Also try to write to Android's logcat using the native __android_log_write symbol.
    // This is a best-effort call; if the symbol isn't available the call will simply be skipped.
    #[cfg(target_os = "android")]
    {
        extern "C" {
            fn __android_log_write(prio: i32, tag: *const c_char, text: *const c_char) -> i32;
        }
        if let Ok(tag_c) = CString::new("ethertia") {
            if let Ok(msg_c) = CString::new(message) {
                unsafe {
                    let _ = __android_log_write(4, tag_c.as_ptr(), msg_c.as_ptr());
                }
            }
        }
    }
}

#[bevy_main]
fn main() {
    #[cfg(target_os = "android")]
    boot_log("mobile::main entered");

    #[cfg(target_os = "android")]
    {
        // Keep backend selection adaptive on Android to reduce startup crashes on devices
        // with incomplete Vulkan support. Respect explicit user/runtime override if provided.
        match std::env::var("WGPU_BACKEND") {
            Ok(current) if !current.trim().is_empty() => {
                boot_log(&format!("keep WGPU_BACKEND={}", current));
            }
            _ => {
                boot_log("WGPU_BACKEND not set, using runtime default backend selection");
            }
        }
    }

    #[cfg(target_os = "android")]
    std::panic::set_hook(Box::new(|info| {
        boot_log(&format!("panic: {}", info));
        eprintln!("PANIC: {}", info);
    }));

    #[cfg(target_os = "android")]
    boot_log("panic hook installed");

    // On Android, avoid forcing a monitor-based fullscreen mode during startup.
    // Some devices can stall before first frame when fullscreen monitor selection
    // is requested too early in NativeActivity initialization.
    let primary_window = if cfg!(target_os = "android") {
        Window {
            resizable: false,
            ..default()
        }
    } else {
        Window {
            resizable: false,
            mode: bevy::window::WindowMode::BorderlessFullscreen(bevy::window::MonitorSelection::Primary),
            ..default()
        }
    };

    #[cfg(target_os = "android")]
    boot_log("primary window configured fullscreen");

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
