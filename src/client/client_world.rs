use std::f32::consts::PI;

use bevy::light::{VolumetricFog, VolumetricLight};
use bevy::post_process::bloom::Bloom;
use bevy::anti_alias::fxaa::Fxaa;
use bevy::core_pipeline::Skybox;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::prelude::EnvironmentMapLight;
use bevy::pbr::ScreenSpaceReflections;
use bevy_renet::netcode::NetcodeClientTransport;
use bevy_renet::renet::RenetClient;

use crate::client::prelude::*;
use crate::net::{CPacket, RenetClientHelper};
use crate::prelude::*;
use crate::util::TimeIntervals;

pub fn volumetric_fog_intensity_from_density(density: f32) -> f32 {
    let density = density.clamp(0.0, 3.0);
    // Use a stronger curve so high-density settings remain clearly visible across day/night.
    (0.18 + density * 0.9).clamp(0.08, 2.8)
}

pub fn init(app: &mut App) {
    app.register_type::<WorldInfo>();

    app.insert_resource(ClientPlayerInfo::default());
    app.register_type::<ClientPlayerInfo>();

    app.add_systems(Update, reinterpret_skybox_cubemap);

    // World Setup/Cleanup, Tick
    app.add_systems(First, on_world_init.run_if(condition::load_world)); // Camera, Player, Sun
    app.add_systems(Last, on_world_exit.run_if(condition::unload_world()));
    app.add_systems(Update, tick_world.run_if(condition::in_world)); // Sun, World Timing.
    app.add_systems(Update, ensure_volumetric_atmosphere.run_if(condition::in_world));
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
        Self {
            inventory: Inventory::new(36),
            hotbar_index: 0,
            health: 20,
            health_max: 20,
        }
    }
}

/// the resource only exixts when world is loaded

#[derive(Resource, Reflect)]
#[reflect(Resource)]
pub struct WorldInfo {
    pub seed: u64,

    pub name: String,

    pub world_config: crate::voxel::WorldGenConfig,

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
    // pub is_manipulating: bool,
}

impl Default for WorldInfo {
    fn default() -> Self {
        WorldInfo {
            seed: 0,
            name: "None Name".into(),
            world_config: crate::voxel::WorldGenConfig::default(),
            daytime: 0.15,
            daytime_length: 60. * 24.,

            time_inhabited: 0.,
            time_created: 0,
            time_modified: 0,

            tick_timer: Timer::new(std::time::Duration::from_secs_f32(1. / 20.), TimerMode::Repeating),

            is_paused: false,
            paused_steps: 0,
            // is_manipulating: true,
        }
    }
}

/// Marker: Despawn the Entity on World Unload.
#[derive(Component)]
pub struct DespawnOnWorldUnload;

// Marker: Sun
#[derive(Component)]
pub(crate) struct Sun;

fn on_world_init(
    mut cmds: Commands,
    asset_server: Res<AssetServer>,
    cli: Res<ClientInfo>,
    query_cam: Query<Entity, With<CharacterControllerCamera>>,
) {
    info!("Load World. setup Player, Camera, Sun.");

    // Sun
    cmds.spawn((
        DirectionalLight {
            shadows_enabled: true,
            illuminance: 40_000.0,
            ..default()
        },
        VolumetricLight,
        Sun, // Marks the light as Sun
        Name::new("Sun"),
        DespawnOnWorldUnload,
    ));

    // Load skybox cubemap and attach to existing camera (if any) or spawn a world camera.
    // This ensures the cubemap is only loaded when a world actually initializes, and
    // avoids creating heavy GPU textures while in menus.
    let skybox_image = asset_server.load("table_mountain_2_puresky_4k_cubemap.jpg");
    cmds.insert_resource(SkyboxCubemap {
        is_loaded: false,
        image_handle: skybox_image.clone(),
    });

    if let Some(cam_entity) = query_cam.iter().next() {
        // Attach skybox and effects to existing camera (e.g. menu fallback camera)
        cmds.entity(cam_entity)
            .insert(Skybox {
                image: skybox_image.clone(),
                brightness: 1000.0,
                ..Default::default()
            })
            .insert(EnvironmentMapLight {
                diffuse_map: skybox_image.clone(),
                specular_map: skybox_image.clone(),
                intensity: 1000.0,
                ..Default::default()
            })
            .insert(ScreenSpaceReflections::default())
            .insert(Fxaa::default())
            .insert(Tonemapping::TonyMcMapface)
            .insert(Bloom::default())
            .insert(VolumetricFog {
                ambient_color: Color::linear_rgb(
                    cli.volumetric_fog_color.x.clamp(0.0, 1.0),
                    cli.volumetric_fog_color.y.clamp(0.0, 1.0),
                    cli.volumetric_fog_color.z.clamp(0.0, 1.0),
                ),
                ambient_intensity: volumetric_fog_intensity_from_density(cli.volumetric_fog_density),
                ..default()
            });
    } else {
        // No existing camera; spawn a full-featured world camera with skybox and effects.
        let mut camera_entity = cmds.spawn((
            Camera3d::default(),
            Camera {
                order: 0,
                ..default()
            },
            bevy::render::view::Hdr,
            bevy::core_pipeline::prepass::DepthPrepass,
            bevy::core_pipeline::prepass::DeferredPrepass,
            bevy::core_pipeline::prepass::NormalPrepass,
            DistanceFog {
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
            Msaa::Off,
        ));

        camera_entity
            .insert(ScreenSpaceReflections::default())
            .insert(Fxaa::default())
            .insert(Tonemapping::TonyMcMapface)
            .insert(Bloom::default())
            .insert(VolumetricFog {
                ambient_color: Color::linear_rgb(
                    cli.volumetric_fog_color.x.clamp(0.0, 1.0),
                    cli.volumetric_fog_color.y.clamp(0.0, 1.0),
                    cli.volumetric_fog_color.z.clamp(0.0, 1.0),
                ),
                ambient_intensity: volumetric_fog_intensity_from_density(cli.volumetric_fog_density),
                ..default()
            });
    }
}

fn on_world_exit(
    mut cmds: Commands,
    query_despawn: Query<Entity, With<DespawnOnWorldUnload>>,
    mut net_client: Option<ResMut<RenetClient>>,
) {
    info!("Unload World");

    for entity in query_despawn.iter() {
        cmds.entity(entity).despawn();
    }

    if let Some(net_client) = net_client.as_mut() {
        net_client.disconnect();
    }

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
    mut cubemap: Option<ResMut<SkyboxCubemap>>,
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
        mut cmds: Commands,
    // #[cfg(feature = "target_native_os")] mut atmosphere: bevy_atmosphere::system_param::AtmosphereMut<bevy_atmosphere::prelude::Nishita>,
    mut query_sun: Query<(&mut Transform, &mut DirectionalLight), With<Sun>>,
    mut worldinfo: ResMut<WorldInfo>,
    time: Res<Time>,

    query_player: Query<&Transform, (With<CharacterController>, Without<Sun>)>,
    mut net_client: Option<ResMut<RenetClient>>,
    mut last_player_pos: Local<Vec3>,
    mut last_load_distance: Local<Option<IVec2>>,

    mut query_fog: Query<
        (Entity, Option<&mut DistanceFog>),
        Or<(With<CharacterControllerCamera>, With<EditorViewportCamera>)>,
    >,
    mut query_vol_fog: Query<
        (Entity, Option<&mut VolumetricFog>),
        Or<(With<CharacterControllerCamera>, With<EditorViewportCamera>)>,
    >,
    mut ambient: ResMut<AmbientLight>,
    cli: Res<ClientInfo>,
    cfg: Res<ClientSettings>,
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

    let is_planet_world = worldinfo.world_config.terrain_mode == crate::voxel::WorldTerrainMode::Planet;
    let planet_radius = worldinfo.world_config.planet_radius.max(16.0);
    let small_planet_boost = if is_planet_world {
        (256.0 / planet_radius).clamp(1.0, 4.0)
    } else {
        1.0
    };

    let base_ambient = if cfg!(target_os = "android") {
        if cfg.high_quality_rendering { 1.8 } else { 1.3 }
    } else if cfg.high_quality_rendering {
        1.25
    } else {
        0.8
    };
    ambient.brightness = (base_ambient * small_planet_boost.sqrt()).clamp(base_ambient, base_ambient * 2.5);

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

    if last_load_distance.as_ref() != Some(&cfg.chunks_load_distance) {
        *last_load_distance = Some(cfg.chunks_load_distance);
        if let Some(net_client) = net_client.as_mut() {
            net_client.send_packet(&CPacket::LoadDistance {
                load_distance: cfg.chunks_load_distance,
            });
        }
    }

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
    let fog_density = if cli.volumetric_fog_density.is_finite() {
        cli.volumetric_fog_density.clamp(0.0, 3.0)
    } else {
        0.0
    };
    let volumetric_fog_color = Color::linear_rgb(
        cli.volumetric_fog_color.x.clamp(0.0, 1.0),
        cli.volumetric_fog_color.y.clamp(0.0, 1.0),
        cli.volumetric_fog_color.z.clamp(0.0, 1.0),
    );
    // Keep a visible fog floor even when volumetric scattering is too subtle on some GPUs.
    // Density=0 keeps original visibility; density=2.2 => ~9%, density=3 => clamped to ~6%.
    let density_visibility_scale = if cli.render_volumetric_fog {
        (1.0 / (1.0 + fog_density * fog_density * 2.0)).clamp(0.06, 1.0)
    } else {
        1.0
    };
    let force_dense_fallback = cli.render_volumetric_fog && fog_density >= 1.5;
    let mut needs_distance_fog_insert = Vec::new();
    for (cam_entity, maybe_fog) in &mut query_fog {
        let mut visibility = if cli.sky_fog_visibility.is_finite() {
            cli.sky_fog_visibility.max(0.001)
        } else {
            1200.0
        } * small_planet_boost * density_visibility_scale;
        if force_dense_fallback {
            visibility = visibility.min((220.0 / (fog_density + 0.1)).max(24.0));
        }

        let Some(mut fog) = maybe_fog else {
            needs_distance_fog_insert.push(cam_entity);
            continue;
        };

        fog.color = cli.sky_fog_color;
        if cli.sky_fog_is_atomspheric && !force_dense_fallback {
            // let FogFalloff::Atmospheric { .. } = fog.falloff {
            fog.falloff = FogFalloff::from_visibility_colors(visibility, cli.sky_extinction_color, cli.sky_inscattering_color);
        } else {
            fog.falloff = FogFalloff::from_visibility_squared(visibility / 1.4);
        }
    }

    for cam_entity in needs_distance_fog_insert {
        let mut visibility = if cli.sky_fog_visibility.is_finite() {
            cli.sky_fog_visibility.max(0.001)
        } else {
            1200.0
        } * small_planet_boost * density_visibility_scale;
        if force_dense_fallback {
            visibility = visibility.min((220.0 / (fog_density + 0.1)).max(24.0));
        }
        let falloff = if cli.sky_fog_is_atomspheric && !force_dense_fallback {
            FogFalloff::from_visibility_colors(visibility, cli.sky_extinction_color, cli.sky_inscattering_color)
        } else {
            FogFalloff::from_visibility_squared(visibility / 1.4)
        };
        cmds.entity(cam_entity).insert(DistanceFog {
            color: cli.sky_fog_color,
            falloff,
            ..default()
        });
    }

    let mut needs_insert = Vec::new();
    for (cam_entity, maybe_vol_fog) in &mut query_vol_fog {
        if !cli.render_volumetric_fog {
            continue;
        }
        let Some(mut vol_fog) = maybe_vol_fog else {
            needs_insert.push(cam_entity);
            continue;
        };

        let base_ambient = volumetric_fog_intensity_from_density(fog_density);
        vol_fog.ambient_intensity = (base_ambient * small_planet_boost.sqrt()).clamp(0.08, 3.0);
        vol_fog.ambient_color = volumetric_fog_color;
    }

    for cam_entity in needs_insert {
        cmds.entity(cam_entity).insert(VolumetricFog {
            ambient_color: volumetric_fog_color,
            ambient_intensity: (volumetric_fog_intensity_from_density(fog_density)
                * small_planet_boost.sqrt())
                .clamp(0.08, 3.0),
            ..default()
        });
    }

    // Sun Pos
    let sun_angle = worldinfo.daytime * PI * 2.;

    // if !time.at_interval(0.5) {
    //     return;
    // }
    // #[cfg(feature = "target_native_os")]
    // atmosphere.sun_position = Vec3::new(sun_angle.cos(), sun_angle.sin(), 0.);

    if let Ok((mut light_trans, mut directional)) = query_sun.single_mut() {
        let daylight = sun_angle.sin().max(0.0).powf(2.0);
        let fill = if is_planet_world {
            0.02 + (small_planet_boost - 1.0) / 3.0 * 0.18
        } else {
            0.01
        };
        directional.illuminance = (daylight + fill) * cli.skylight_illuminance * 1000.0;
        directional.shadows_enabled = cli.skylight_shadow || cli.render_volumetric_fog;

        // or from000.looking_at()
        light_trans.rotation = Quat::from_rotation_z(sun_angle) * Quat::from_rotation_y(PI / 2.3);
    }
}

fn ensure_volumetric_atmosphere(
    mut cmds: Commands,
    cli: Res<ClientInfo>,
    query_cam: Query<
        (Entity, Option<&VolumetricFog>, Option<&DistanceFog>),
        Or<(With<CharacterControllerCamera>, With<EditorViewportCamera>)>,
    >,
    query_sun: Query<(Entity, Option<&VolumetricLight>), With<Sun>>,
) {
    match query_sun.single() {
        Ok((sun_entity, vol_light)) => {
            // Self-heal missing volumetric-light component on existing sun.
            if vol_light.is_none() {
                cmds.entity(sun_entity).insert(VolumetricLight);
            }
        }
        Err(_) => {
            // Self-heal missing sun/light entity; without VolumetricLight fog scattering disappears.
            cmds.spawn((
                DirectionalLight {
                    shadows_enabled: true,
                    illuminance: 40_000.0,
                    ..default()
                },
                VolumetricLight,
                Sun,
                Name::new("Sun"),
                DespawnOnWorldUnload,
            ));
        }
    }

    if !cli.render_volumetric_fog {
        return;
    }

    for (cam_entity, has_vol_fog, has_distance_fog) in query_cam.iter() {
        if has_distance_fog.is_none() {
            let fog_density = if cli.volumetric_fog_density.is_finite() {
                cli.volumetric_fog_density.clamp(0.0, 3.0)
            } else {
                0.0
            };
            let force_dense_fallback = cli.render_volumetric_fog && fog_density >= 1.5;

            let mut visibility = if cli.sky_fog_visibility.is_finite() {
                cli.sky_fog_visibility.max(0.001)
            } else {
                1200.0
            };
            if force_dense_fallback {
                visibility = visibility.min((220.0 / (fog_density + 0.1)).max(24.0));
            }
            let falloff = if cli.sky_fog_is_atomspheric && !force_dense_fallback {
                FogFalloff::from_visibility_colors(visibility, cli.sky_extinction_color, cli.sky_inscattering_color)
            } else {
                FogFalloff::from_visibility_squared(visibility / 1.4)
            };
            cmds.entity(cam_entity).insert(DistanceFog {
                color: cli.sky_fog_color,
                falloff,
                ..default()
            });
        }

        if has_vol_fog.is_none() {
            cmds.entity(cam_entity).insert(VolumetricFog {
                ambient_color: Color::linear_rgb(
                    cli.volumetric_fog_color.x.clamp(0.0, 1.0),
                    cli.volumetric_fog_color.y.clamp(0.0, 1.0),
                    cli.volumetric_fog_color.z.clamp(0.0, 1.0),
                ),
                ambient_intensity: volumetric_fog_intensity_from_density(cli.volumetric_fog_density),
                ..default()
            });
        }
    }
}