#[cfg(not(target_arch = "wasm32"))]
use std::net::ToSocketAddrs;

use bevy::{
    anti_alias::fxaa::Fxaa,
    asset::RenderAssetUsages,
    camera::RenderTarget,
    core_pipeline::tonemapping::Tonemapping,
    ecs::system::SystemParam,
    light::DirectionalLightShadowMap,
    math::vec3,
    post_process::bloom::Bloom,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages},
};
use bevy::pbr::wireframe::{Wireframe, WireframePlugin};
use bevy_renet::renet::RenetClient;
use avian3d::prelude::*;

#[cfg(feature = "target_native_os")]
use bevy_atmosphere::prelude::*;

use crate::client::prelude::*;
use crate::item::ItemPlugin;
use crate::net::{CPacket, ClientNetworkPlugin, RenetClientHelper};
#[cfg(not(target_arch = "wasm32"))]
use crate::server::prelude::IntegratedServerPlugin;
use crate::ui::prelude::*;
use crate::voxel::{ActiveWorld, ClientVoxelPlugin, VoxelChunkRenderMesh, WorldSaveRequest};

pub struct ClientGamePlugin;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorViewMode {
    View3D,
    View2D,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorBottomTab {
    Resources,
    Diagnostics,
    Assets,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorCameraMode {
    Fly,
    Orbit,
    TopDown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorRenderMode {
    Lit,
    Flat,
    Performance,
    Wireframe,
}

#[derive(Component)]
pub struct EditorViewportCamera;

#[derive(Resource, Debug)]
pub struct EditorViewportRttState {
    pub image_handle: Handle<Image>,
    pub texture_id: Option<bevy_egui::egui::TextureId>,
    pub requested_size: UVec2,
    pub allocated_size: UVec2,
}

impl Default for EditorViewportRttState {
    fn default() -> Self {
        Self {
            image_handle: Handle::default(),
            texture_id: None,
            requested_size: UVec2::new(1280, 720),
            allocated_size: UVec2::ZERO,
        }
    }
}

#[derive(Resource, Debug)]
pub struct EditorRuntime {
    pub view_mode: EditorViewMode,
    pub camera_mode: EditorCameraMode,
    pub render_mode: EditorRenderMode,
    pub show_help: bool,
    pub selected_entity: Option<Entity>,
    pub bottom_tab: EditorBottomTab,
    pub show_lod_overlay: bool,
}

impl Default for EditorRuntime {
    fn default() -> Self {
        Self {
            view_mode: EditorViewMode::View3D,
            camera_mode: EditorCameraMode::Fly,
            render_mode: EditorRenderMode::Lit,
            show_help: true,
            selected_entity: None,
            bottom_tab: EditorBottomTab::Resources,
            show_lod_overlay: true,
        }
    }
}

impl Plugin for ClientGamePlugin {
    fn build(&self, app: &mut App) {
        // Render
        {
            app.insert_resource(AmbientLight { brightness: 1.0, ..default() });
            app.insert_resource(ClearColor(Color::BLACK));

            // Atmosphere
            #[cfg(feature = "target_native_os")]
            {
                //app.add_plugins(AtmospherePlugin);
                //app.insert_resource(AtmosphereModel::default());
            }
            
            // Voxel materials currently provide deferred shaders; forcing forward on Android
            // bypasses those code paths and breaks terrain texture sampling.
            app.insert_resource(bevy::pbr::DefaultOpaqueRendererMethod::deferred());

            #[cfg(target_os = "android")]
            {
                app.insert_resource(AmbientLight { brightness: 1.8, ..default() });
                app.insert_resource(ClearColor(Color::srgb(0.06, 0.09, 0.12)));
            }
            
            // SSAO
            // app.add_plugins(TemporalAntiAliasPlugin);
            // app.insert_resource(AmbientLight { brightness: 0.05, ..default() });
        }
        // .obj model loader.
        app.add_plugins(bevy_obj::ObjPlugin);
        app.add_plugins(WireframePlugin::default());
        app.insert_resource(GlobalVolume::new(bevy::audio::Volume::Linear(1.0))); // Audio GlobalVolume
        
        // Physics
        app.add_plugins(PhysicsPlugins::default());
        
        // UI
        app.add_plugins(crate::ui::UiPlugin);
        
        // Gameplay
        app.add_plugins(CharacterControllerPlugin); // CharacterController
        app.add_plugins(ClientVoxelPlugin); // Voxel
        app.add_plugins(ItemPlugin); // Items
        #[cfg(feature = "target_native_os")]
        app.add_plugins(super::editor::EditorViewPlugin);
        
        // Network
        app.add_plugins(ClientNetworkPlugin); // Client Network
        // Integrated local server is unavailable on wasm runtime.
        #[cfg(not(target_arch = "wasm32"))]
        app.add_plugins(IntegratedServerPlugin);
        
        // ClientInfo
        app.insert_resource(ClientInfo::default());
        app.insert_resource(EditorRuntime::default());
        app.insert_resource(EditorViewportRttState::default());
        app.register_type::<ClientInfo>();
        
        super::settings::build_plugin(app); // Config
        super::input::init(app); // Input
        
        // World
        super::client_world::init(app);

        // Debug
        {
            // app.add_systems(Update, wfc_test);
            
            // Draw Basis
            app.add_systems(PostUpdate, debug_draw_gizmo.in_set(PhysicsSet::Writeback).run_if(condition::in_world));
            
            // World Inspector
            #[cfg(feature = "target_native_os")]
            app.add_plugins(bevy_inspector_egui::quick::WorldInspectorPlugin::new().run_if(|cli: Res<ClientInfo>| cli.dbg_inspector));
        }

        #[cfg(target_os = "android")]
        app.add_systems(Update, handle_android_lifecycle);

        #[cfg(feature = "ddgi")]
        app.add_plugins(DDGIPlugin);

        app.add_systems(Startup, apply_graphics_settings);
        app.add_systems(Update, ensure_editor_viewport_camera.run_if(condition::in_world));
        app.add_systems(Update, resize_editor_viewport_target.run_if(condition::in_world));
        app.add_systems(Update, apply_editor_render_mode.run_if(condition::in_world));
        app.add_systems(Update, apply_editor_viewport_render_mode.run_if(condition::in_world));
    }
}

fn apply_editor_render_mode(
    cli: ResMut<ClientInfo>,
    editor_runtime: Res<EditorRuntime>,
) {
    if !editor_runtime.is_changed() {
        return;
    }

    let mut cli = cli;
    match editor_runtime.render_mode {
        EditorRenderMode::Lit => {
            cli.render_fxaa = true;
            cli.render_tonemapping = true;
            cli.render_bloom = true;
            cli.render_ssr = true;
            cli.render_volumetric_fog = true;
            cli.render_skybox = true;
        }
        EditorRenderMode::Flat => {
            cli.render_fxaa = false;
            cli.render_tonemapping = true;
            cli.render_bloom = false;
            cli.render_ssr = false;
            cli.render_volumetric_fog = false;
            cli.render_skybox = false;
        }
        EditorRenderMode::Performance => {
            cli.render_fxaa = false;
            cli.render_tonemapping = false;
            cli.render_bloom = false;
            cli.render_ssr = false;
            cli.render_volumetric_fog = false;
            cli.render_skybox = false;
        }
        EditorRenderMode::Wireframe => {
            cli.render_fxaa = false;
            cli.render_tonemapping = false;
            cli.render_bloom = false;
            cli.render_ssr = false;
            cli.render_volumetric_fog = false;
            cli.render_skybox = false;
        }
    }
}

fn apply_editor_viewport_render_mode(
    mut commands: Commands,
    editor_runtime: Res<EditorRuntime>,
    cli: Res<ClientInfo>,
    query_cam: Query<(
        Entity,
        Option<&Fxaa>,
        Option<&Tonemapping>,
        Option<&Bloom>,
        Option<&bevy::light::VolumetricFog>,
    ), With<EditorViewportCamera>>,
    query_voxel_meshes: Query<(Entity, Option<&Wireframe>), With<VoxelChunkRenderMesh>>,
) {
    if !editor_runtime.is_changed() {
        return;
    }

    let Ok((entity, has_fxaa, has_tonemapping, has_bloom, has_vol_fog)) = query_cam.single() else {
        return;
    };
    let mut ent = commands.entity(entity);

    match editor_runtime.render_mode {
        EditorRenderMode::Lit => {
            if has_fxaa.is_none() {
                ent.insert(Fxaa::default());
            }
            if has_tonemapping.is_none() {
                ent.insert(Tonemapping::TonyMcMapface);
            }
            if has_bloom.is_none() {
                ent.insert(Bloom::default());
            }
            if has_vol_fog.is_none() {
                ent.insert(bevy::light::VolumetricFog {
                    ambient_color: Color::linear_rgb(
                        cli.volumetric_fog_color.x.clamp(0.0, 1.0),
                        cli.volumetric_fog_color.y.clamp(0.0, 1.0),
                        cli.volumetric_fog_color.z.clamp(0.0, 1.0),
                    ),
                    ambient_intensity: crate::client::client_world::volumetric_fog_intensity_from_density(
                        cli.volumetric_fog_density,
                    ),
                    ..default()
                });
            }
        }
        EditorRenderMode::Flat => {
            if has_fxaa.is_none() {
                ent.insert(Fxaa::default());
            }
            if has_tonemapping.is_none() {
                ent.insert(Tonemapping::TonyMcMapface);
            }
            if has_bloom.is_some() {
                ent.remove::<Bloom>();
            }
            if has_vol_fog.is_some() {
                ent.remove::<bevy::light::VolumetricFog>();
            }
        }
        EditorRenderMode::Performance => {
            if has_fxaa.is_some() {
                ent.remove::<Fxaa>();
            }
            if has_tonemapping.is_some() {
                ent.remove::<Tonemapping>();
            }
            if has_bloom.is_some() {
                ent.remove::<Bloom>();
            }
            if has_vol_fog.is_some() {
                ent.remove::<bevy::light::VolumetricFog>();
            }
        }
        EditorRenderMode::Wireframe => {
            if has_fxaa.is_some() {
                ent.remove::<Fxaa>();
            }
            if has_tonemapping.is_some() {
                ent.remove::<Tonemapping>();
            }
            if has_bloom.is_some() {
                ent.remove::<Bloom>();
            }
            if has_vol_fog.is_some() {
                ent.remove::<bevy::light::VolumetricFog>();
            }
        }
    }

    let enable_wireframe = editor_runtime.render_mode == EditorRenderMode::Wireframe;
    for (mesh_entity, has_wireframe) in query_voxel_meshes.iter() {
        let mut mesh_cmd = commands.entity(mesh_entity);
        if enable_wireframe {
            if has_wireframe.is_none() {
                mesh_cmd.insert(Wireframe);
            }
        } else if has_wireframe.is_some() {
            mesh_cmd.remove::<Wireframe>();
        }
    }
}

fn alloc_editor_viewport_image(images: &mut Assets<Image>, size: UVec2) -> Handle<Image> {
    let extent = Extent3d {
        width: size.x.max(1),
        height: size.y.max(1),
        depth_or_array_layers: 1,
    };
    let mut image = Image::new_fill(
        extent,
        TextureDimension::D2,
        &[0, 0, 0, 255],
        TextureFormat::bevy_default(),
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    image.texture_descriptor.usage = TextureUsages::RENDER_ATTACHMENT
        | TextureUsages::TEXTURE_BINDING
        | TextureUsages::COPY_SRC;
    images.add(image)
}

fn ensure_editor_viewport_camera(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut rtt_state: ResMut<EditorViewportRttState>,
    mut query_rtt_cam: Query<(Entity, &mut Camera, &mut Transform), With<EditorViewportCamera>>,
    query_main_cam: Query<(&Transform, &Projection), (With<CharacterControllerCamera>, Without<EditorViewportCamera>)>,
    cli: Res<ClientInfo>,
    editor_runtime: Res<EditorRuntime>,
) {
    if rtt_state.image_handle.id() == Handle::<Image>::default().id() {
        let initial = rtt_state.requested_size.max(UVec2::new(64, 64));
        rtt_state.image_handle = alloc_editor_viewport_image(&mut images, initial);
        rtt_state.allocated_size = initial;
        rtt_state.texture_id = None;
    }

    let is_editor_view_3d = cli.curr_ui == CurrentUI::WorldEditor && editor_runtime.view_mode == EditorViewMode::View3D;

    if let Ok((_entity, mut camera, mut cam_transform)) = query_rtt_cam.single_mut() {
        camera.target = RenderTarget::Image(rtt_state.image_handle.clone().into());
        camera.is_active = is_editor_view_3d;

        if is_editor_view_3d {
            if let Ok((main_transform, _)) = query_main_cam.single() {
                cam_transform.translation = main_transform.translation;
                cam_transform.rotation = main_transform.rotation;
            }
        }
        return;
    }

    let mut transform = Transform::from_xyz(0.0, 120.0, 0.0).looking_at(Vec3::ZERO, Vec3::Y);
    let mut projection = Projection::Perspective(PerspectiveProjection::default());
    if let Ok((main_transform, main_projection)) = query_main_cam.single() {
        transform = *main_transform;
        projection = main_projection.clone();
    }

    commands.spawn((
        Camera3d::default(),
        Camera {
            target: RenderTarget::Image(rtt_state.image_handle.clone().into()),
            order: -10,
            is_active: is_editor_view_3d,
            ..default()
        },
        bevy::render::view::Hdr,
        bevy::core_pipeline::prepass::DepthPrepass,
        bevy::core_pipeline::prepass::DeferredPrepass,
        bevy::core_pipeline::prepass::NormalPrepass,
        projection,
        transform,
        EditorViewportCamera,
        Name::new("EditorViewportCamera"),
        Msaa::Off,
    ));
}

fn resize_editor_viewport_target(
    mut images: ResMut<Assets<Image>>,
    mut rtt_state: ResMut<EditorViewportRttState>,
    mut query_rtt_cam: Query<&mut Camera, With<EditorViewportCamera>>,
) {
    let requested = rtt_state.requested_size.max(UVec2::new(64, 64));
    if requested == rtt_state.allocated_size {
        return;
    }

    rtt_state.image_handle = alloc_editor_viewport_image(&mut images, requested);
    rtt_state.allocated_size = requested;
    rtt_state.texture_id = None;

    if let Ok(mut camera) = query_rtt_cam.single_mut() {
        camera.target = RenderTarget::Image(rtt_state.image_handle.clone().into());
    }
}

fn apply_graphics_settings(
    cfg: Res<ClientSettings>,
    mut ambient: ResMut<AmbientLight>,
) {
    ambient.brightness = if cfg!(target_os = "android") {
        if cfg.high_quality_rendering { 1.8 } else { 1.3 }
    } else if cfg.high_quality_rendering {
        1.25
    } else {
        0.8
    };
}

#[cfg(target_os = "android")]
fn handle_android_lifecycle(
    mut lifecycle_events: MessageReader<bevy::window::AppLifecycle>,
    mut cli: ResMut<ClientInfo>,
    mut worldinfo: Option<ResMut<WorldInfo>>,
) {
    for event in lifecycle_events.read() {
        match event {
            bevy::window::AppLifecycle::WillSuspend | bevy::window::AppLifecycle::Suspended => {
                if let Some(world) = &mut worldinfo {
                    world.is_paused = true;
                    world.paused_steps = 0;
                }
                // Ensure app returns to a stable UI state when resuming from background.
                if cli.curr_ui == CurrentUI::None {
                    cli.curr_ui = CurrentUI::PauseMenu;
                }
                cli.enable_cursor_look = false;
            }
            bevy::window::AppLifecycle::WillResume => {
                if let Some(world) = &mut worldinfo {
                    world.is_paused = false;
                }
                if cli.curr_ui == CurrentUI::None {
                    cli.curr_ui = CurrentUI::PauseMenu;
                }
            }
            _ => {}
        }
    }
}

pub mod condition {
    use crate::client::prelude::*;
    use bevy::ecs::{change_detection::DetectChanges, schedule::common_conditions::resource_removed, system::Res};

    // a.k.a. loaded_world
    pub fn in_world(res: Option<Res<WorldInfo>>, res_vox: Option<Res<crate::voxel::ClientChunkSystem>>) -> bool {
        res.is_some() && res_vox.is_some()
    }
    pub fn load_world(res: Option<Res<WorldInfo>>) -> bool {
        res.is_some_and(|r| r.is_added())
    }
    pub fn unload_world() -> impl FnMut(Option<Res<WorldInfo>>, bevy::prelude::Local<bool>) -> bool + Clone {
        resource_removed::<WorldInfo>
    }
    pub fn manipulating(cli: Res<ClientInfo>) -> bool {
        cli.curr_ui == CurrentUI::None
    }
    pub fn in_ui(ui: CurrentUI) -> impl FnMut(Res<ClientInfo>) -> bool + Clone {
        move |cli: Res<ClientInfo>| cli.curr_ui == ui
    }
}

fn debug_draw_gizmo(
    mut gizmo: Gizmos,
    // mut gizmo_config: ResMut<GizmoConfigStore>,
    query_cam: Query<&Transform, With<CharacterControllerCamera>>,
) {
    // gizmo.config.depth_bias = -1.; // always in front

    // World Basis Axes
    let n = 5;
    gizmo.line(Vec3::ZERO, Vec3::X * 2. * n as f32, Srgba::RED);
    gizmo.line(Vec3::ZERO, Vec3::Y * 2. * n as f32, Srgba::GREEN);
    gizmo.line(Vec3::ZERO, Vec3::Z * 2. * n as f32, Srgba::BLUE);

    let color = Srgba::gray(0.4);
    for x in -n..=n {
        gizmo.ray(vec3(x as f32, 0., -n as f32), Vec3::Z * n as f32 * 2., color);
    }
    for z in -n..=n {
        gizmo.ray(vec3(-n as f32, 0., z as f32), Vec3::X * n as f32 * 2., color);
    }

    // View Basis
    if let Ok(cam_trans) = query_cam.single() {
        // let cam_trans = query_cam.single();
        let p = cam_trans.translation;
        let rot = cam_trans.rotation;
        let n = 0.03;
        let offset = vec3(0., 0., -0.5);
        gizmo.ray(p + rot * offset, Vec3::X * n, Srgba::RED);
        gizmo.ray(p + rot * offset, Vec3::Y * n, Srgba::GREEN);
        gizmo.ray(p + rot * offset, Vec3::Z * n, Srgba::BLUE);
    }
}

#[derive(Resource, Reflect)]
#[reflect(Resource)]
pub struct ClientInfo {
    // Networking
    pub server_addr: String, // just a record
    pub disconnected_reason: String,
    pub ping: (u64, i64, i64, u64),     // ping. (rtt, c2s, ping-begin) in ms.
    pub playerlist: Vec<(String, u32)>, // as same as SPacket::PlayerList. username, ping.

    // Debug Draw
    pub dbg_text: bool,
    pub dbg_menubar: bool,
    pub dbg_inspector: bool,
    pub dbg_gizmo_remesh_chunks: bool,
    pub dbg_gizmo_curr_chunk: bool,
    pub dbg_gizmo_all_loaded_chunks: bool,
    pub dbg_tex: bool,

    // Render Sky
    pub sky_fog_color: Color,
    pub sky_fog_visibility: f32,
    pub sky_inscattering_color: Color,
    pub sky_extinction_color: Color,
    pub sky_fog_is_atomspheric: bool,
    pub skylight_shadow: bool,
    pub skylight_illuminance: f32,

    pub render_fxaa: bool,
    pub render_tonemapping: bool,
    pub render_bloom: bool,
    pub render_ssr: bool,
    pub render_volumetric_fog: bool,
    pub volumetric_fog_density: f32,
    pub volumetric_fog_color: Vec3,
    pub render_skybox: bool,

    pub touch_controls_edit_mode: bool,

    // Server-authoritative admin capabilities/state
    pub is_owner: bool,
    pub is_admin: bool,
    pub admin_god_enabled: bool,
    pub admin_noclip_enabled: bool,
    pub admin_panel_open: bool,
    pub global_editor_view: bool,

    // Control
    pub enable_cursor_look: bool,

    // UI
    #[reflect(ignore)]
    pub curr_ui: CurrentUI,
}

impl Default for ClientInfo {
    fn default() -> Self {
        Self {
            disconnected_reason: String::new(),
            ping: (0, 0, 0, 0),
            playerlist: Vec::new(),
            server_addr: String::new(),

            dbg_text: false,
            // On Android, menu bar debug UI is disabled by default to avoid startup-time UI context assumptions.
            dbg_menubar: !cfg!(target_os = "android"),
            dbg_inspector: false,
            dbg_gizmo_remesh_chunks: true,
            dbg_gizmo_curr_chunk: false,
            dbg_gizmo_all_loaded_chunks: false,
            dbg_tex: false,

            sky_fog_color: Color::srgba(0.0, 0.666, 1.0, 1.0),
            sky_fog_visibility: 1200.0, // 280 for ExpSq, 1200 for Atmo
            sky_fog_is_atomspheric: true,
            sky_inscattering_color: Color::srgb(110.0 / 255.0, 230.0 / 255.0, 1.0), // bevy demo: Color::rgb(0.7, 0.844, 1.0),
            sky_extinction_color: Color::srgb(0.35, 0.5, 0.66),

            skylight_shadow: true,
            skylight_illuminance: 20.,

            render_fxaa: true,
            render_tonemapping: true,
            render_bloom: true,
            render_ssr: true,
            render_volumetric_fog: true,
            volumetric_fog_density: 2.2,
            volumetric_fog_color: Vec3::ONE,
            render_skybox: true,

            touch_controls_edit_mode: false,

            is_owner: false,
            is_admin: false,
            admin_god_enabled: false,
            admin_noclip_enabled: false,
            admin_panel_open: false,
            global_editor_view: false,

            enable_cursor_look: true,

            curr_ui: CurrentUI::MainMenu,
        }
    }
}

// A helper on Client

#[derive(SystemParam)]
pub struct EthertiaClient<'w, 's> {
    clientinfo: ResMut<'w, ClientInfo>,
    pub cfg: ResMut<'w, ClientSettings>,

    cmds: Commands<'w, 's>,
}

impl<'w, 's> EthertiaClient<'w, 's> {
    /// for Singleplayer
    // pub fn load_world(&mut self, cmds: &mut Commands, server_addr: String)

    pub fn data(&mut self) -> &mut ClientInfo {
        self.clientinfo.as_mut()
    }

    pub fn connect_server(&mut self, server_addr: String) {
        info!("Connecting to {}", server_addr);

        #[cfg(target_arch = "wasm32")]
        {
            let _ = server_addr;
            self.data().disconnected_reason = "Networking is unavailable on this runtime".to_string();
            self.data().curr_ui = CurrentUI::DisconnectedReason;
            warn!("connect_server is not supported on wasm32 runtime");
            return;
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            let mut addrs = match server_addr.trim().to_socket_addrs() {
                Ok(addrs) => addrs.collect::<Vec<_>>(),
                Err(err) => {
                    error!("Failed to resolve DNS of server_addr: {}", err);
                    self.data().curr_ui = CurrentUI::DisconnectedReason;
                    return;
                }
            };
            let addr = match addrs.pop() {
                Some(addr) => addr,
                None => {
                    self.data().curr_ui = CurrentUI::DisconnectedReason;
                    return;
                }
            };

            self.data().curr_ui = CurrentUI::ConnectingServer;
            self.clientinfo.server_addr.clone_from(&server_addr);

            let mut net_client = RenetClient::new(bevy_renet::renet::ConnectionConfig::default());

            let username = &self.cfg.username;
            net_client.send_packet(&CPacket::Login {
                uuid: crate::util::hashcode(username),
                access_token: 123,
                username: username.clone(),
            });

            self.cmds.insert_resource(net_client);

            match crate::net::new_netcode_client_transport(
                addr,
                Some("userData123".to_string().into_bytes()),
            ) {
                Ok(transport) => {
                    self.cmds.insert_resource(transport);
                }
                Err(err) => {
                    error!("Failed to establish connection to {}: {}", server_addr, err);
                    self.data().disconnected_reason = format!("Connection failed: {}", err);
                    self.data().curr_ui = CurrentUI::DisconnectedReason;
                    return;
                }
            }

            // clear DisconnectReason on new connect, to prevents display old invalid reason.
            self.clientinfo.disconnected_reason.clear();

            // 提前初始化世界资源，避免后续网络包先到导致缺资源。
            self.cmds.queue(|world: &mut World| {
                if !world.contains_resource::<WorldInfo>() {
                    world.insert_resource(WorldInfo::default());
                }
            });
        }
    }

    pub fn select_active_world(&mut self, meta: crate::voxel::WorldMeta) {
        let mut world_info = WorldInfo::default();
        world_info.name = meta.name.clone();
        world_info.seed = meta.seed;
        world_info.world_config = meta.config.clone();

        self.cmds.insert_resource(ActiveWorld {
            name: meta.name,
            seed: meta.seed,
            config: meta.config,
        });
        self.cmds.insert_resource(world_info);
    }

    pub fn connect_local_world(&mut self, meta: crate::voxel::WorldMeta, port: u16) {
        #[cfg(target_arch = "wasm32")]
        {
            let _ = port;
            self.select_active_world(meta);
            self.data().disconnected_reason.clear();
            self.data().curr_ui = CurrentUI::None;
            return;
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            self.select_active_world(meta);
            self.connect_server(format!("127.0.0.1:{}", port));
        }
    }

    pub fn request_save_world(&mut self) {
        self.cmds.insert_resource(WorldSaveRequest { save_now: true });
    }

    pub fn enter_world(&mut self) {
        self.cmds.insert_resource(WorldInfo::default());
        self.data().curr_ui = CurrentUI::None;
    }

    pub fn exit_world(&mut self) {
        self.request_save_world();
        self.cmds.remove_resource::<WorldInfo>();
        self.data().curr_ui = CurrentUI::MainMenu;
    }
}
