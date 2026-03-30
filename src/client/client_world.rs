use std::f32::consts::PI;

use bevy::light::VolumetricLight;
use bevy_renet::netcode::NetcodeClientTransport;
use bevy_renet::renet::RenetClient;

use crate::client::prelude::*;
use crate::net::{CPacket, RenetClientHelper};
use crate::prelude::*;
use crate::util::TimeIntervals;
use avian3d::prelude::*;

pub fn init(app: &mut App) {
    app.register_type::<WorldInfo>();

    app.insert_resource(ClientPlayerInfo::default());
    app.register_type::<ClientPlayerInfo>();

    app.add_systems(Update, reinterpret_skybox_cubemap);

    // World Setup/Cleanup, Tick
    app.add_systems(First, on_world_init.run_if(condition::load_world)); // Camera, Player, Sun
    app.add_systems(Last, on_world_exit.run_if(condition::unload_world()));
    app.add_systems(Update, tick_world.run_if(condition::in_world)); // Sun, World Timing.
    // Apply planet-mode radial gravity to dynamic bodies
    app.add_systems(Update, apply_planet_gravity.run_if(condition::in_world));
}

#[derive(Resource, Reflect)]
#[reflect(Resource)]
pub struct ClientPlayerInfo {
    #[reflect(ignore)]
    pub inventory: Inventory,

    pub hotbar_index: u32,

    pub health: u32,
    pub health_max: u32,
}

impl ClientPlayerInfo {
    pub const HOTBAR_SLOTS: u32 = 9;
}

impl Default for ClientPlayerInfo {
    fn default() -> Self {
        let mut inventory = Inventory::new(36);
        // 将所有已注册物品都放入物品栏，每种1个。
        // 由于Items资源未在此处可用，只能硬编码物品数量和顺序，需与item/mod.rs同步。
        // 当前注册顺序见item/mod.rs setup_items，注意avocado未加入defs，仅注册。
        let item_ids = [
            0, // stone
            1, // dirt
            2, // grass
            3, // sand
            4, // log
            5, // leaves
            6, // water
            7, // apple
            9, // coal
            10, // stick
            11, // frame
            12, // lantern
            13, // pickaxe
            14, // shears
            15, // grapple
            16, // circuit_board
            17, // iron_ingot
        ];
        for (i, &item_id) in item_ids.iter().enumerate() {
            if i < inventory.items.len() {
                inventory.items[i] = crate::item::ItemStack { count: 1, item_id: item_id as u8 };
            }
        }
        Self {
            inventory,
            hotbar_index: 0,
            health: 20,
            health_max: 20,
        }
    }
}

/// the resource only exixts when world is loaded

#[derive(Resource, Reflect, Clone)]
#[reflect(Resource)]
pub struct WorldInfo {
    pub seed: u64,

    pub name: String,

    pub daytime: f32,

    // seconds a day time long
    pub daytime_length: f32,

    // seconds
    pub time_inhabited: f32,

    time_created: u64,
    time_modified: u64,

    tick_timer: Timer,

    pub is_paused: bool,
    pub paused_steps: i32,

    // World / Planet parameters
    pub terrain_mode: crate::client::settings::TerrainMode,
    pub planet_center: bevy::prelude::Vec3,
    pub planet_radius: f32,
    pub planet_shell_thickness: f32,
    pub gravity_accel: f32,

    // Generator/version metadata for compatibility checks
    pub generator_version: String,
    pub generator_params_hash: String,
    pub world_format_version: u32,
}

impl Default for WorldInfo {
    fn default() -> Self {
        let mut wi = WorldInfo {
            seed: 0,
            name: "None Name".into(),
            daytime: 0.15,
            daytime_length: 60. * 24.,

            time_inhabited: 0.,
            time_created: 0,
            time_modified: 0,

            tick_timer: Timer::new(std::time::Duration::from_secs_f32(1. / 20.), TimerMode::Repeating),

            is_paused: false,
            paused_steps: 0,

            terrain_mode: crate::client::settings::TerrainMode::Planet,
            planet_center: Vec3::new(0.0, 512.0, 0.0),
            planet_radius: 512.0,
            planet_shell_thickness: 96.0,
            gravity_accel: 9.81,

            generator_version: env!("CARGO_PKG_VERSION").to_string(),
            generator_params_hash: String::new(),
            world_format_version: 1,
        };

        wi.recompute_params_hash();
        wi
    }
}

impl WorldInfo {
    /// Recompute a stable hash of the generator parameters used to produce the base terrain.
    /// This uses a deterministic serialization of the small set of parameters and blake3.
    pub fn recompute_params_hash(&mut self) {
        // Serialize only the generator-related params
        use serde_json::json;
        let obj = json!({
            "terrain_mode": format!("{:?}", self.terrain_mode),
            "planet_center": [self.planet_center.x, self.planet_center.y, self.planet_center.z],
            "planet_radius": self.planet_radius,
            "planet_shell_thickness": self.planet_shell_thickness,
            "gravity_accel": self.gravity_accel,
        });
        if let Ok(s) = serde_json::to_string(&obj) {
            let hash = blake3::hash(s.as_bytes());
            self.generator_params_hash = hash.to_hex().to_string();
        } else {
            self.generator_params_hash.clear();
        }
    }
}

/// Internal synchronous implementation that writes meta.json. Accepts owned `WorldInfo` so it
/// can be safely moved into a background thread.
fn save_world_meta_to_disk_sync(w: WorldInfo) {
    use std::fs;
    use std::io::Write;
    use std::path::Path;

    let name = if w.name.trim().is_empty() {
        format!("world_{:016x}", w.seed)
    } else {
        // sanitize: replace spaces with underscore
        w.name.replace(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '-', "_")
    };

    let save_dir = crate::util::saves_root().join(&name);

    if let Err(err) = fs::create_dir_all(&save_dir) {
        warn!("Failed to create save dir {:?}: {}", save_dir, err);
        return;
    }

    let meta_path = save_dir.join("meta.json");

    // Backup existing meta
    if meta_path.exists() {
        let backup_dir = save_dir.join(format!("backup_{}", chrono::Utc::now().format("%Y%m%d%H%M%S")));
        if let Err(err) = fs::create_dir_all(&backup_dir) {
            warn!("Failed to create backup dir {:?}: {}", backup_dir, err);
        } else {
            if let Err(err) = fs::copy(&meta_path, backup_dir.join("meta.json")) {
                warn!("Failed to backup meta.json: {}", err);
            }
        }
    }

    let meta = serde_json::json!({
        "seed": w.seed,
        "name": w.name,
        "generator_version": w.generator_version,
        "generator_params_hash": w.generator_params_hash,
        "world_format_version": w.world_format_version,
        "terrain_mode": format!("{:?}", w.terrain_mode),
        "planet_center": [w.planet_center.x, w.planet_center.y, w.planet_center.z],
        "planet_radius": w.planet_radius,
        "planet_shell_thickness": w.planet_shell_thickness,
        "time_created": w.time_created,
        "time_modified": w.time_modified,
    });

    match serde_json::to_string_pretty(&meta) {
        Ok(text) => {
            let tmp = save_dir.join("meta.json.tmp");
            match fs::File::create(&tmp) {
                Ok(mut f) => {
                    if let Err(err) = f.write_all(text.as_bytes()) {
                        warn!("Failed to write meta tmp {:?}: {}", tmp, err);
                        let _ = fs::remove_file(&tmp);
                        return;
                    }
                    if let Err(err) = fs::rename(&tmp, &meta_path) {
                        warn!("Failed to rename meta tmp to meta.json: {}", err);
                    } else {
                        info!("Saved world meta to {:?}", meta_path);
                    }
                }
                Err(err) => warn!("Failed to create meta tmp {:?}: {}", tmp, err),
            }
        }
        Err(err) => warn!("Failed to serialize world meta: {}", err),
    }
}

/// Persist minimal world metadata in a background thread to avoid blocking the caller.
pub fn save_world_meta_to_disk(w: &WorldInfo) {
    let owned = w.clone();
    std::thread::spawn(move || save_world_meta_to_disk_sync(owned));
}

/// Marker: Despawn the Entity on World Unload.
#[derive(Component)]
pub struct DespawnOnWorldUnload;

// Marker: Sun
#[derive(Component)]
struct Sun;

fn on_world_init(
    mut cmds: Commands,
    _asset_server: Res<AssetServer>,
    // materials: ResMut<Assets<StandardMaterial>>,
    // meshes: ResMut<Assets<Mesh>>,
    // cli: ResMut<ClientInfo>,
) {
    info!("Load World. setup Player, Camera, Sun.");

    // crate::net::netproc_client::spawn_player(
    //     &mut cmds.spawn_empty(),
    //     true,
    //     &cli.cfg.username, &asset_server, &mut meshes, &mut materials);

    // NOTE: Camera init has moved into UI Init. since Egui now requires Camera to render and we should only have 1 camera
    /*  
    let skybox_image = asset_server.load("table_mountain_2_puresky_4k_cubemap.jpg");
    cmds.insert_resource(SkyboxCubemap {
        is_loaded: false,
        image_handle: skybox_image.clone()
    });

    // Camera
    cmds.spawn((
        Camera3d::default(),
        Camera { hdr: true, ..default() },
        /*
        bevy::pbr::Atmosphere::EARTH,
        bevy::pbr::AtmosphereSettings {
            aerial_view_lut_max_distance: 3.2e5,
            scene_units_to_m: 1e+4,
            ..Default::default()
        },
        bevy::camera::Exposure::SUNLIGHT,
        bevy::core_pipeline::tonemapping::Tonemapping::AcesFitted,
        bevy::post_process::bloom::Bloom::NATURAL,
        bevy::light::AtmosphereEnvironmentMapLight::default(),
        */
        // #[cfg(feature = "target_native_os")]
        // bevy_atmosphere::plugin::AtmosphereCamera::default(), // Marks camera as having a skybox, by default it doesn't specify the render layers the skybox can be seen on
        DistanceFog {
            // color, falloff shoud be set in ClientInfo.sky_fog_visibility, etc. due to dynamic debug reason.
            // falloff: FogFalloff::Atmospheric { extinction: Vec3::ZERO, inscattering:  Vec3::ZERO },  // mark as Atmospheric. value will be re-set by ClientInfo.sky_fog...
            ..default()
        },
        Skybox {
            image: skybox_image.clone(),
            brightness: 1000.0,
            ..Default::default()
        },
        EnvironmentMapLight {
            diffuse_map: skybox_image.clone(),
            specular_map: skybox_image.clone(),
            intensity: 1000.0,
            ..Default::default()
        },
        CharacterControllerCamera,
        Name::new("Camera"),
        DespawnOnWorldUnload,

        // ScreenSpaceReflectionsBundle::default(),
        // Fxaa::default(),
    ))
    .insert(ScreenSpaceReflections::default())
    .insert(Fxaa::default())
    .insert(Tonemapping::TonyMcMapface)
    .insert(Bloom::default())
    .insert(VolumetricFog {
        ambient_intensity: 0.,
        //density: 0.01,
        //light_tint: Color::linear_rgb(0.916, 0.941, 1.000),
        ..default()
    })
    ;
    // .insert(ScreenSpaceAmbientOcclusionBundle::default())
    // .insert(TemporalAntiAliasBundle::default());
    */

    // Sun
    cmds.spawn((
        DirectionalLight::default(),
        VolumetricLight,
        Sun, // Marks the light as Sun
        Name::new("Sun"),
        DespawnOnWorldUnload,
    ));
}

fn on_world_exit(mut cmds: Commands, query_despawn: Query<Entity, With<DespawnOnWorldUnload>>) {
    info!("Unload World");

    for entity in query_despawn.iter() {
        cmds.entity(entity).despawn();
    }

    // todo: net_client.disconnect();  即时断开 否则服务器会觉得你假死 对其他用户体验不太好
    cmds.remove_resource::<RenetClient>();
    cmds.remove_resource::<NetcodeClientTransport>();
}


#[derive(Resource)]
pub struct SkyboxCubemap {
    pub is_loaded: bool,
    pub image_handle: Handle<Image>,
}

fn reinterpret_skybox_cubemap(
    asset_server: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
    cubemap: Option<ResMut<SkyboxCubemap>>,
) {
    let Some(mut cubemap) = cubemap else {
        return;
    };
    if !cubemap.is_loaded {
        if let Some(load_state) = asset_server.get_load_state(&cubemap.image_handle) {
            if load_state.is_loaded() {
                let Some(image) = images.get_mut(&cubemap.image_handle) else {
                    return;
                };
                // NOTE: PNGs do not have any metadata that could indicate they contain a cubemap texture,
                // so they appear as one texture. The following code reconfigures the texture as necessary.
                if image.texture_descriptor.array_layer_count() == 1 {
                    info!("Reinterpret 2D image into Cubemap");
                    image.reinterpret_stacked_2d_as_array(
                        image.texture_descriptor.size.height / image.texture_descriptor.size.width,
                    );
                    image.texture_view_descriptor = Some(bevy::render::render_resource::TextureViewDescriptor {
                        dimension: Some(bevy::render::render_resource::TextureViewDimension::Cube),
                        ..default()
                    });
                }

                cubemap.is_loaded = true;
            }
        }
    }
}

fn tick_world(
    // #[cfg(feature = "target_native_os")] mut atmosphere: bevy_atmosphere::system_param::AtmosphereMut<bevy_atmosphere::prelude::Nishita>,
    mut query_sun: Query<(&mut Transform, &mut DirectionalLight), With<Sun>>,
    mut worldinfo: ResMut<WorldInfo>,
    time: Res<Time>,

    query_player: Query<&Transform, (With<CharacterController>, Without<Sun>)>,
    mut net_client: Option<ResMut<RenetClient>>,
    mut last_player_pos: Local<Vec3>,

    mut query_fog: Query<&mut DistanceFog>,
    cli: Res<ClientInfo>,
) {
    // worldinfo.tick_timer.tick(time.delta());
    // if !worldinfo.tick_timer.just_finished() {
    //     return;
    // }
    // let dt_sec = worldinfo.tick_timer.duration().as_secs_f32();  // constant time step?

    // Pause & Steps
    if worldinfo.is_paused {
        if worldinfo.paused_steps > 0 {
            worldinfo.paused_steps -= 1;
        } else {
            return;
        }
    }
    let dt_sec = time.delta_secs();

    worldinfo.time_inhabited += dt_sec;

    // DayTime
    let daytime_length = if worldinfo.daytime_length.is_finite() && worldinfo.daytime_length > f32::EPSILON {
        worldinfo.daytime_length
    } else {
        60.0 * 24.0
    };
    worldinfo.daytime += dt_sec / daytime_length;
    worldinfo.daytime -= worldinfo.daytime.trunc(); // trunc to [0-1]

    // Send PlayerPos
    if let Ok(player_loc) = query_player.single() {
        let player_pos = player_loc.translation;

        if player_pos.distance_squared(*last_player_pos) > 0.01 * 0.01 {
            *last_player_pos = player_pos;
            if let Some(net_client) = net_client.as_mut() {
                net_client.send_packet(&CPacket::PlayerPos { position: player_pos });
            }
        }
    }
    // net_client.send_packet(&CPacket::LoadDistance {
    //     load_distance: cli.chunks_load_distance,
    // }); // todo: Only Send after Edit Dist Config

    // Ping Network
    if time.at_interval(1.0) {
        if let Some(net_client) = net_client.as_mut() {
            net_client.send_packet(&CPacket::Ping {
                client_time: crate::util::current_timestamp_millis(),
                last_rtt: cli.ping.0 as u32,
            });
        }
    }

    // Fog
    if let Ok(mut fog) = query_fog.single_mut() {
        fog.color = cli.sky_fog_color;
        let visibility = if cli.sky_fog_visibility.is_finite() {
            cli.sky_fog_visibility.max(0.001)
        } else {
            1200.0
        };
        if cli.sky_fog_is_atomspheric {
            // let FogFalloff::Atmospheric { .. } = fog.falloff {
            fog.falloff = FogFalloff::from_visibility_colors(visibility, cli.sky_extinction_color, cli.sky_inscattering_color);
        } else {
            fog.falloff = FogFalloff::from_visibility_squared(visibility / 4.0);
        }
    }

    // Sun Pos
    let sun_angle = worldinfo.daytime * PI * 2.;

    // if !time.at_interval(0.5) {
    //     return;
    // }
    // #[cfg(feature = "target_native_os")]
    // atmosphere.sun_position = Vec3::new(sun_angle.cos(), sun_angle.sin(), 0.);

    if let Ok((mut light_trans, mut directional)) = query_sun.single_mut() {
        directional.illuminance = sun_angle.sin().max(0.0).powf(2.0) * cli.skylight_illuminance * 1000.0;
        directional.shadows_enabled = cli.skylight_shadow;

        // or from000.looking_at()
        light_trans.rotation = Quat::from_rotation_z(sun_angle) * Quat::from_rotation_y(PI / 2.3);
    }
}

fn apply_planet_gravity(
    mut query: Query<(&mut LinearVelocity, &Transform, &RigidBody, Option<&GravityScale>)>,
    worldinfo: Option<Res<WorldInfo>>,
    time: Res<Time>,
) {
    let Some(w) = worldinfo else { return; };
    if w.terrain_mode != crate::client::settings::TerrainMode::Planet {
        return;
    }

    let dt = time.delta_secs();

    for (mut linvel, transform, rb, grav_scale_opt) in query.iter_mut() {
        // only affect dynamic bodies
        if *rb != RigidBody::Dynamic {
            continue;
        }

        let gscale = grav_scale_opt.map(|g| g.0).unwrap_or(1.0);
        let to_center = w.planet_center - transform.translation;
        let dist2 = to_center.length_squared();
        if dist2 <= f32::EPSILON {
            continue;
        }
        let dir = to_center / dist2.sqrt();

        linvel.0 += dir * w.gravity_accel * dt * gscale;
    }
}

/// Try to load meta.json from disk for this world (by name or by seed) and apply to `w`.
/// Returns Some(message) describing compatibility status, or None if no meta found.
pub fn load_world_meta_from_disk(w: &mut WorldInfo) -> Option<String> {
    use std::fs;
    use std::path::Path;

    let try_names = vec![
        w.name.replace(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '-', "_"),
        format!("world_{:016x}", w.seed),
    ];

    for name in try_names {
        let meta_path = crate::util::saves_root().join(&name).join("meta.json");
        if !meta_path.exists() {
            continue;
        }
        match fs::read_to_string(&meta_path) {
            Ok(text) => match serde_json::from_str::<serde_json::Value>(&text) {
                Ok(val) => {
                    // apply known fields if present
                    if let Some(seed) = val.get("seed").and_then(|v| v.as_u64()) {
                        w.seed = seed;
                    }
                    if let Some(namev) = val.get("name").and_then(|v| v.as_str()) {
                        w.name = namev.to_string();
                    }
                    if let Some(gv) = val.get("generator_version").and_then(|v| v.as_str()) {
                        // store meta value separately
                        let meta_gen = gv.to_string();
                        if meta_gen != w.generator_version {
                            let msg = format!("Generator version mismatch: meta={} current={}", meta_gen, w.generator_version);
                            warn!("{}", msg);
                            // still apply the meta value
                            w.generator_version = meta_gen;
                            if let Some(param_hash) = val.get("generator_params_hash").and_then(|v| v.as_str()) {
                                w.generator_params_hash = param_hash.to_string();
                            }
                            if let Some(fmt) = val.get("world_format_version").and_then(|v| v.as_u64()) {
                                w.world_format_version = fmt as u32;
                            }
                            return Some(msg);
                        } else {
                            // same version, check params hash
                            if let Some(param_hash) = val.get("generator_params_hash").and_then(|v| v.as_str()) {
                                if param_hash != w.generator_params_hash {
                                    let msg = format!("Generator params hash mismatch: meta={} current={}", param_hash, w.generator_params_hash);
                                    warn!("{}", msg);
                                    w.generator_params_hash = param_hash.to_string();
                                    if let Some(fmt) = val.get("world_format_version").and_then(|v| v.as_u64()) {
                                        w.world_format_version = fmt as u32;
                                    }
                                    return Some(msg);
                                }
                            }
                        }
                    }
                    // update timestamps if present
                    if let Some(tc) = val.get("time_created").and_then(|v| v.as_u64()) {
                        w.time_created = tc;
                    }
                    if let Some(tm) = val.get("time_modified").and_then(|v| v.as_u64()) {
                        w.time_modified = tm;
                    }
                    info!("Loaded world meta from {:?}", meta_path);
                    return Some("Loaded meta and compatible".to_string());
                }
                Err(err) => {
                    warn!("Failed to parse meta.json {:?}: {}", meta_path, err);
                    return Some(format!("failed_parse: {}", err));
                }
            },
            Err(err) => {
                warn!("Failed to read meta.json {:?}: {}", meta_path, err);
                return Some(format!("failed_read: {}", err));
            }
        }
    }
    None
}