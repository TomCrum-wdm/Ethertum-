@echo off
setlocal EnableExtensions EnableDelayedExpansion

rem Android debug APK build helper (Windows).
rem It auto-detects NDK and exports C/C++ toolchain vars required by cc-rs/oboe-sys.

set "NDK_ROOT="

if defined ANDROID_NDK_ROOT set "NDK_ROOT=%ANDROID_NDK_ROOT%"
if not defined NDK_ROOT if defined ANDROID_NDK_HOME set "NDK_ROOT=%ANDROID_NDK_HOME%"

set "SDK_ROOT="
if defined ANDROID_HOME set "SDK_ROOT=%ANDROID_HOME%"
if not defined SDK_ROOT if defined ANDROID_SDK_ROOT set "SDK_ROOT=%ANDROID_SDK_ROOT%"

rem Prefer ASCII SDK path on Windows to avoid aapt Unicode path issues.
if exist "C:\AndroidSdk" set "SDK_ROOT=C:\AndroidSdk"

if not defined NDK_ROOT call :pick_latest_ndk "C:\AndroidSdk\ndk"
if not defined NDK_ROOT call :pick_latest_ndk "%ANDROID_SDK_ROOT%\ndk"
if not defined NDK_ROOT call :pick_latest_ndk "%ANDROID_HOME%\ndk"
if not defined NDK_ROOT call :pick_latest_ndk "%LOCALAPPDATA%\Android\Sdk\ndk"

if not defined NDK_ROOT (
	echo [ERROR] Could not find Android NDK.
	echo Set ANDROID_NDK_ROOT or install Android NDK under Android/Sdk/ndk.
	exit /b 1
)

set "ANDROID_NDK_ROOT=%NDK_ROOT%"
set "ANDROID_NDK_HOME=%NDK_ROOT%"

if not defined SDK_ROOT for %%P in ("%NDK_ROOT%") do set "SDK_ROOT=%%~dpP"
if defined SDK_ROOT if "%SDK_ROOT:~-1%"=="\" set "SDK_ROOT=%SDK_ROOT:~0,-1%"
if defined SDK_ROOT for %%P in ("%SDK_ROOT%") do if /I "%%~nxP"=="ndk" set "SDK_ROOT=%%~dpP"
if defined SDK_ROOT if "%SDK_ROOT:~-1%"=="\" set "SDK_ROOT=%SDK_ROOT:~0,-1%"

if defined SDK_ROOT (
	set "ANDROID_HOME=%SDK_ROOT%"
)

set "LLVM_BIN=%NDK_ROOT%\toolchains\llvm\prebuilt\windows-x86_64\bin"
set "CLANG=%LLVM_BIN%\clang.exe"
set "CLANGXX=%LLVM_BIN%\clang++.exe"
set "ANDROID_API=%ETHERTUM_ANDROID_API%"
if not defined ANDROID_API set "ANDROID_API=24"
set "AARCH64_CLANG=%LLVM_BIN%\aarch64-linux-android%ANDROID_API%-clang.cmd"
set "AARCH64_CLANGXX=%LLVM_BIN%\aarch64-linux-android%ANDROID_API%-clang++.cmd"
set "CLANG_LIB_ROOT=%NDK_ROOT%\toolchains\llvm\prebuilt\windows-x86_64\lib\clang"
set "CLANG_VER="
for /f "delims=" %%V in ('dir /b /ad "%CLANG_LIB_ROOT%" 2^>nul') do set "CLANG_VER=%%V"
set "CLANG_RT_AARCH64=%CLANG_LIB_ROOT%\%CLANG_VER%\lib\linux\aarch64"

if not exist "%CLANGXX%" (
	echo [ERROR] clang++.exe not found: "%CLANGXX%"
	exit /b 1
)

if not exist "%AARCH64_CLANG%" (
	echo [ERROR] aarch64 clang driver not found: "%AARCH64_CLANG%"
	exit /b 1
)

set "PATH=%LLVM_BIN%;%PATH%"
set "CC=%CLANG%"
set "CXX=%CLANGXX%"
set "AR=%LLVM_BIN%\llvm-ar.exe"
set "CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER=%AARCH64_CLANG%"
set "CARGO_TARGET_AARCH64_LINUX_ANDROID_RUSTFLAGS=-Clink-arg=-L%CLANG_RT_AARCH64%"
set "EXTRA_LINK_LIB_DIR=%CD%\target\cargo-apk-temp-extra-link-libraries"
if not exist "%EXTRA_LINK_LIB_DIR%" mkdir "%EXTRA_LINK_LIB_DIR%"
if exist "%CLANG_RT_AARCH64%\libunwind.a" copy /Y "%CLANG_RT_AARCH64%\libunwind.a" "%EXTRA_LINK_LIB_DIR%\libunwind.a" >nul

set "RUSTUP_DIST_SERVER=https://static.rust-lang.org"
set "RUSTUP_UPDATE_ROOT=https://static.rust-lang.org/rustup"

rem cc-rs checks both target-specific env name forms.
set "CC_aarch64-linux-android=%AARCH64_CLANG%"
set "CC_aarch64_linux_android=%AARCH64_CLANG%"
set "CXX_aarch64-linux-android=%AARCH64_CLANGXX%"
set "CXX_aarch64_linux_android=%AARCH64_CLANGXX%"

echo [INFO] NDK_ROOT=%NDK_ROOT%
echo [INFO] ANDROID_HOME=%ANDROID_HOME%
echo [INFO] CXX=%CXX%

set "RUST_TOOLCHAIN=%ETHERTUM_ANDROID_TOOLCHAIN%"
if not defined RUST_TOOLCHAIN set "RUST_TOOLCHAIN=nightly"

set "BUILD_TARGETS=%ETHERTUM_ANDROID_TARGETS%"
if not defined BUILD_TARGETS set "BUILD_TARGETS=aarch64-linux-android"

for %%T in (%BUILD_TARGETS%) do (
	rustup target list --toolchain %RUST_TOOLCHAIN% --installed | findstr /x "%%T" >nul
	if errorlevel 1 (
		echo [INFO] Installing rust target for %RUST_TOOLCHAIN%: %%T
		rustup target add --toolchain %RUST_TOOLCHAIN% %%T
		if errorlevel 1 exit /b 1
	)
)

set "CARGO_APK_TARGET_ARGS="
for %%T in (%BUILD_TARGETS%) do (
	set "CARGO_APK_TARGET_ARGS=!CARGO_APK_TARGET_ARGS! --target %%T"
)

cargo +%RUST_TOOLCHAIN% apk build --profile android-debug --package mobile %CARGO_APK_TARGET_ARGS%
set "BUILD_RC=%ERRORLEVEL%"

if not "%BUILD_RC%"=="0" (
	echo [ERROR] Android build failed with exit code %BUILD_RC%.
	exit /b %BUILD_RC%
)

echo [OK] Android APK build finished.
exit /b 0

:pick_latest_ndk
if "%~1"=="" exit /b 0
if not exist "%~1" exit /b 0
for /f "delims=" %%D in ('dir /b /ad /o-n "%~1" 2^>nul') do (
	set "NDK_ROOT=%~1\%%D"
	goto :eof
)
exit /b 0