# Android adaptation notes

This document describes the Android-specific wiring for the current codebase.

## Current architecture (mobile path)

- Desktop entry: `src/main.rs`
- Shared game logic: `src/lib.rs` + `src/client/*`
- Mobile entry and package metadata: `crates/mobile/`
- Android packaging scripts: `build/android_pack.sh`, `build/android_pack.bat`

Mobile uses the shared client plugin from the main crate and only changes app/window bootstrap.

## Adaptations applied

1. Fixed mobile plugin path:
   - `crates/mobile/src/lib.rs` now uses `ethertia::client::prelude::ClientGamePlugin`.
   - This matches the actual exported module tree in `src/client/mod.rs`.

2. Hardened client settings persistence for Android:
   - `src/client/settings.rs` no longer panics on file read/write failures.
   - On Android, settings save is skipped to avoid shutdown panic from sandbox path differences.

## Build (Android)

1. Install target and cargo-apk:

```bash
rustup target add aarch64-linux-android
cargo install --force cargo-apk
```

2. Ensure Android SDK/NDK env is set:

```bash
export ANDROID_HOME=/path/to/Android/Sdk
export ANDROID_NDK_ROOT=$ANDROID_HOME/ndk/<version>
```

3. Build apk:

```bash
cargo +stable apk build --profile android-debug --package mobile
```

## Next recommended improvements

- Add touch-native input layer (on-screen controls) behind `cfg(target_os = "android")`.
- Persist settings to app-private storage on Android instead of skipping save.
- Add a CI smoke check to compile `-p mobile` on non-Android host target for early API mismatch detection.
