use bevy::{
    app::AppExit,
    diagnostic::{DiagnosticsStore, EntityCountDiagnosticsPlugin, FrameTimeDiagnosticsPlugin},
    prelude::*,
};
use bevy::light::VolumetricFog;
use bevy_egui::{
    egui::{self, Align2, Color32, FontId, Frame, Id, LayerId, Layout, Widget},
    EguiContexts,
};
use bevy_renet::renet::{RenetClient};
use bevy_renet::{netcode::NetcodeClientTransport};
use std::sync::atomic::Ordering;

use crate::{
    client::prelude::*,
    net::{CPacket, RenetClientHelper},
    ui::{color32_of, CurrentUI, UiExtra},
    util::AsMutRef,
    voxel::{self, lighting::VoxLightQueue, worldgen, Chunk, ChunkSystem, ClientChunkSystem, HitResult, Vox, VoxLight, VoxShape},
};

pub fn ui_menu_panel(
    mut ctx: EguiContexts,
    mut worldinfo: Option<ResMut<WorldInfo>>,
    chunk_sys: Option<ResMut<ClientChunkSystem>>,
    mut cl: EthertiaClient,
    query_cam: Query<&Transform, With<CharacterControllerCamera>>,
    query_vol_fog: Query<&VolumetricFog, With<CharacterControllerCamera>>,
    query_sun: Query<(Entity, Option<&bevy::light::VolumetricLight>), With<crate::client::client_world::Sun>>,

    net_client: Option<Res<RenetClient>>,
    net_transport: Option<Res<NetcodeClientTransport>>,

    mut app_exit_events: EventWriter<AppExit>,
) {
    let Ok(ctx_mut) = ctx.ctx_mut() else {
        return;
    };

    // const BLUE: Color = Color::rgb(0.188, 0.478, 0.776);
    // const PURPLE: Color = Color::rgb(0.373, 0.157, 0.467);
    // const ORANGE: Color = Color::rgb(0.741, 0.345, 0.133);
    const DARK_RED: Srgba = Srgba::rgb(0.525, 0.106, 0.176);
    const DARK: Srgba = Srgba::new(0., 0., 0., 0.800); // 0.176, 0.176, 0.176
    let bg = if worldinfo.as_ref().is_some_and(|w| w.is_paused) {
        color32_of(DARK_RED)
    } else {
        color32_of(DARK)
    };
    // color32_of(worldinfo.map_or(DARK, |v| v.is_paused));

    egui::TopBottomPanel::top("menu_panel")
        .frame(Frame::default().fill(bg))
        .show_separator_line(false)
        // .height_range(Rangef::new(16., 16.))  // 24
        .show(ctx_mut, |ui| {
            // ui.painter().text([0., 48.].into(), Align2::LEFT_TOP, "SomeText", FontId::default(), Color32::WHITE);

            egui::menu::bar(ui, |ui| {
                ui.style_mut().spacing.button_padding.x = 6.;
                ui.style_mut().visuals.widgets.noninteractive.fg_stroke.color = Color32::from_white_alpha(130);
                ui.style_mut().visuals.widgets.inactive.fg_stroke.color = Color32::from_white_alpha(210); // MenuButton lighter

                ui.with_layout(Layout::right_to_left(egui::Align::BOTTOM), |ui| {
                    ui.add_space(16.);
                    // ui.small("108M\n30K");
                    // ui.small("10M/s\n8K/s");
                    // ui.label("·");
                    // ui.small("9ms\n12ms");
                    // ui.label("127.0.0.1:4000 · 21ms");

                    // Network Info
                    if let Some(net_transport) = net_transport {
                        let cli = cl.data();

                        let Some(net_client) = net_client else {
                            return;
                        };
                        if net_client.is_connected() {
                            use human_bytes::human_bytes;
                            let ni = net_client.network_info();
                            let ping = cli.ping;
                            let bytes_per_sec = ni.bytes_sent_per_second + ni.bytes_received_per_second;

                            ui.menu_button(format!("{}ms {}/s", ping.0, human_bytes(bytes_per_sec)), |ui| {
                                let info_bg = Color32::from_rgba_unmultiplied(20, 24, 32, 220);
                                egui::Frame::default().fill(info_bg).show(ui, |ui| {
                                    ui.vertical(|ui| {
                                        ui.colored_label(Color32::from_rgb(140, 200, 255), format!("Server: {}", &cli.server_addr))
                                            .on_hover_text("Server Addr");
                                        ui.add_space(4.);
                                        ui.horizontal(|ui| {
                                            let ping_color = if ping.0 < 80 {
                                                Color32::from_rgb(100, 220, 120)
                                            } else if ping.0 < 180 {
                                                Color32::from_rgb(255, 200, 80)
                                            } else {
                                                Color32::from_rgb(255, 100, 100)
                                            };
                                            ui.colored_label(ping_color, format!("RTT {:>4} ms", ping.0))
                                                .on_hover_text("Latency / RTT");
                                            ui.separator();
                                            ui.colored_label(Color32::from_rgb(120, 220, 255), format!("{:>8}/s", human_bytes(bytes_per_sec)))
                                                .on_hover_text("Bandwidth");
                                            ui.small(format!(
                                                "up {:>8}/s | down {:>8}/s",
                                                human_bytes(ni.bytes_sent_per_second),
                                                human_bytes(ni.bytes_received_per_second)
                                            ))
                                            .on_hover_text("Bandwidth (Upload/Download)");
                                        });
                                        ui.add_space(2.);
                                        let loss_color = if ni.packet_loss == 0.0 {
                                            Color32::from_rgb(100, 220, 120)
                                        } else if ni.packet_loss < 2.0 {
                                            Color32::from_rgb(255, 200, 80)
                                        } else {
                                            Color32::from_rgb(255, 100, 100)
                                        };
                                        ui.colored_label(loss_color, format!("loss: {}", ni.packet_loss));
                                    });
                                });
                            });
                        }
                    }

                    // World Pause
                    if let Some(worldinfo) = &mut worldinfo {
                        ui.separator();

                        if worldinfo.is_paused {
                            if egui::Button::new("▶").ui(ui).clicked() {
                                worldinfo.is_paused = false;
                            }
                            if egui::Button::new("⏩").ui(ui).clicked() {
                                //⏩
                                worldinfo.paused_steps += 1;
                            }
                        } else if egui::Button::new("⏸").ui(ui).clicked() {
                            worldinfo.is_paused = true;
                        }
                    }

                    // put inside a Layout::right_to_left(egui::Align::Center) or the Vertical Align will offset to upper.
                    ui.with_layout(Layout::left_to_right(egui::Align::BOTTOM), |ui| {
                        let is_admin_user = cl.data().is_admin;
                        ui.add_space(12.);
                        ui.menu_button("System", |ui| {
                            ui.menu_button("Connect to Server", |ui| {
                                ui.button("Add Server").clicked();
                                ui.separator();
                            });
                            ui.menu_button("Open World", |ui| {
                                if ui.btn("New World").clicked() {
                                    let cli = cl.data();
                                    cli.curr_ui = CurrentUI::LocalWorldNew;
                                }
                                ui.btn("Open World..").clicked();
                                ui.separator();
                            });
                            if ui.btn("Edit World..").clicked() {
                                let cli = cl.data();
                                if cli.is_admin {
                                    cli.curr_ui = CurrentUI::WorldEditor;
                                    cli.global_editor_view = true;
                                    cli.enable_cursor_look = false;
                                }
                            }
                            if ui.btn("Close World").clicked() {
                                cl.exit_world();
                            }
                            ui.separator();
                            if ui.btn("Settings").clicked() {
                                let cli = cl.data();
                                cli.curr_ui = CurrentUI::Settings;
                            }
                            ui.button("Mods").clicked();
                            ui.button("Assets").clicked();
                            ui.button("Controls").clicked();
                            ui.button("About").clicked();
                            ui.separator();
                            if ui.button("Terminate").clicked() {
                                app_exit_events.write(AppExit::Success);
                            }
                        });
                        ui.menu_button("Voxel", |ui| {
                            let cli = cl.data();
                            // ui.label("Gizmos:");
                            ui.toggle_value(&mut cli.dbg_gizmo_all_loaded_chunks, "Gizmo Loaded Chunks");
                            ui.toggle_value(&mut cli.dbg_gizmo_curr_chunk, "Gizmo Current Chunk");
                            ui.toggle_value(&mut cli.dbg_gizmo_remesh_chunks, "Gizmo ReMesh Chunks");
                            
                            ui.separator();

                            if let Some(mut chunk_sys) = chunk_sys {
                                let Ok(cam_transform) = query_cam.single() else {
                                    return;
                                };
                                let campos = cam_transform.translation.as_ivec3();
                                if ui.button("Compute Voxel Light").clicked() {
                                    // for chunk in chunk_sys.get_chunks().values() {
                                    //     Chunk::compute_voxel_light(chunk.as_mut());
                                    // }
                                    let mut queue = VoxLightQueue::new();

                                    if let Some(chunk) = chunk_sys.get_chunk(Chunk::as_chunkpos(campos)) {
                                        queue.push((
                                            chunk.clone(),
                                            Chunk::local_idx(Chunk::as_localpos(campos)) as u16,
                                            VoxLight::new(0, 15, 3, 4),
                                        ));
                                    }

                                    use crate::voxel::lighting;
                                    
                                    for chunkpos in chunk_sys.get_chunks().keys() {
                                        if voxel::is_chunk_in_load_distance(Chunk::as_chunkpos(campos), *chunkpos, IVec2::new(2,2)) {
                                            if let Some(chunk) = chunk_sys.get_chunk(*chunkpos) {
                                                lighting::collect_chunk_lights(chunk, &mut queue);
                                            }

                                            // lighting::compute_skylight(chunk, &mut queue);
                                        }
                                    }


                                    lighting::compute_voxel_light(&mut queue, &mut Vec::new());
                                }
                                let mut force_blocky = voxel::meshgen::DBG_FORCE_BLOCKY.load(Ordering::Relaxed);
                                if ui.toggle_value(&mut force_blocky, "Is Force Blocky").changed() {
                                    voxel::meshgen::DBG_FORCE_BLOCKY.store(force_blocky, Ordering::Relaxed);
                                }

                                if ui.button("ReMesh All Chunks").clicked() {
                                    let chunk_keys = Vec::from_iter(chunk_sys.get_chunks().keys().cloned());
                                    for chunkpos in chunk_keys {
                                        chunk_sys.mark_chunk_remesh(chunkpos);
                                    }
                                }
                                if ui.button("ReMesh Nr Chunks").clicked() {
                                    let chunk_keys = Vec::from_iter(chunk_sys.get_chunks().keys().cloned());
                                    for chunkpos in chunk_keys {
                                        if voxel::is_chunk_in_load_distance(Chunk::as_chunkpos(campos), chunkpos, IVec2::new(2,2)) {
                                            chunk_sys.mark_chunk_remesh(chunkpos);
                                        }
                                    }
                                }
                                if ui.button("Gen Tree").clicked() {
                                    if let Some(chunk) = chunk_sys.get_chunk(Chunk::as_chunkpos(campos)) {
                                        worldgen::gen_tree(chunk.as_mut(), Chunk::as_localpos(campos), 0.8);
                                    }
                                }
                                if ui.button("Gen Floor").clicked() {

                                    // crate::util::iter::iter_center_spread(10, 1, |p| {
                                    // });
                                    if let Some(chunk) = chunk_sys.get_chunk(Chunk::as_chunkpos(campos)) {
                                        let chunk = chunk.as_mut();
                                        for x in 0..16 {
                                            for z in 0..16 {
                                                *chunk.at_voxel_mut(IVec3::new(x, 0, z)) = Vox::new(1, VoxShape::Cube, 0.);
                                            }
                                        }
                                    }
                                }
                            }
                        });
                        ui.menu_button("Render", |ui| {
                            let cli = cl.data();

                            let fog_status_color = if cli.render_volumetric_fog {
                                Color32::from_rgb(100, 220, 120)
                            } else {
                                Color32::from_rgb(255, 120, 120)
                            };
                            ui.colored_label(fog_status_color, format!("Volumetric Fog: {}", if cli.render_volumetric_fog { "ON" } else { "OFF" }));
                            ui.colored_label(Color32::from_rgb(120, 220, 255), format!("Fog Density: {:.2}", cli.volumetric_fog_density));

                            match query_vol_fog.single() {
                                Ok(vol_fog) => {
                                    ui.colored_label(Color32::from_rgb(100, 220, 120), "Camera Fog Entity: PRESENT")
                                        .on_hover_text(format!("ambient_intensity = {:.3}", vol_fog.ambient_intensity));
                                }
                                Err(_) => {
                                    ui.colored_label(Color32::from_rgb(255, 120, 120), "Camera Fog Entity: MISSING");
                                }
                            }

                            match query_sun.single() {
                                Ok((_sun_entity, has_volumetric_light)) => {
                                    let light_ok = has_volumetric_light.is_some();
                                    ui.colored_label(
                                        if light_ok { Color32::from_rgb(100, 220, 120) } else { Color32::from_rgb(255, 200, 80) },
                                        format!("Sun: PRESENT | VolumetricLight: {}", if light_ok { "YES" } else { "NO" }),
                                    );
                                }
                                Err(_) => {
                                    ui.colored_label(Color32::from_rgb(255, 120, 120), "Sun: MISSING");
                                }
                            }

                            let fog_density = if cli.volumetric_fog_density.is_finite() {
                                cli.volumetric_fog_density.clamp(0.0, 3.0)
                            } else {
                                0.0
                            };
                            let fog_visibility_scale = if cli.render_volumetric_fog {
                                (1.0 / (1.0 + fog_density * fog_density * 2.0)).clamp(0.06, 1.0)
                            } else {
                                1.0
                            };
                            ui.separator();
                            ui.colored_label(Color32::from_rgb(120, 220, 255), format!("Fog Visibility: {:.1}", cli.sky_fog_visibility));
                            ui.colored_label(
                                Color32::from_rgb(130, 200, 255),
                                format!("Effective Visibility: {:.1}", cli.sky_fog_visibility * fog_visibility_scale),
                            );
                            ui.colored_label(
                                if cli.sky_fog_is_atomspheric { Color32::from_rgb(100, 220, 120) } else { Color32::from_rgb(255, 200, 80) },
                                format!("Atmospheric: {}", if cli.sky_fog_is_atomspheric { "YES" } else { "NO" }),
                            );
                            ui.colored_label(
                                if cli.render_volumetric_fog && cli.volumetric_fog_density >= 1.5 {
                                    Color32::from_rgb(255, 200, 80)
                                } else {
                                    Color32::from_rgb(120, 220, 255)
                                },
                                format!(
                                    "Dense Fallback: {}",
                                    if cli.render_volumetric_fog && cli.volumetric_fog_density >= 1.5 {
                                        "FORCED EXP2"
                                    } else {
                                        "OFF"
                                    }
                                ),
                            );
                        });
                        ui.menu_button("Fog", |ui| {
                            let cli = cl.data();
                            let fog_status_color = if cli.render_volumetric_fog {
                                Color32::from_rgb(100, 220, 120)
                            } else {
                                Color32::from_rgb(255, 120, 120)
                            };
                            ui.colored_label(fog_status_color, format!("Volumetric Fog: {}", if cli.render_volumetric_fog { "ON" } else { "OFF" }));
                            ui.colored_label(Color32::from_rgb(120, 220, 255), format!("Fog Density: {:.2}", cli.volumetric_fog_density));
                            match query_vol_fog.single() {
                                Ok(vol_fog) => {
                                    ui.colored_label(Color32::from_rgb(100, 220, 120), "Camera Fog Entity: PRESENT")
                                        .on_hover_text(format!("ambient_intensity = {:.3}", vol_fog.ambient_intensity));
                                }
                                Err(_) => {
                                    ui.colored_label(Color32::from_rgb(255, 120, 120), "Camera Fog Entity: MISSING");
                                }
                            }
                            match query_sun.single() {
                                Ok((_sun_entity, has_volumetric_light)) => {
                                    let light_ok = has_volumetric_light.is_some();
                                    ui.colored_label(
                                        if light_ok { Color32::from_rgb(100, 220, 120) } else { Color32::from_rgb(255, 200, 80) },
                                        format!("Sun: PRESENT | VolumetricLight: {}", if light_ok { "YES" } else { "NO" }),
                                    );
                                }
                                Err(_) => {
                                    ui.colored_label(Color32::from_rgb(255, 120, 120), "Sun: MISSING");
                                }
                            }
                            let fog_density = if cli.volumetric_fog_density.is_finite() {
                                cli.volumetric_fog_density.clamp(0.0, 3.0)
                            } else {
                                0.0
                            };
                            let fog_visibility_scale = if cli.render_volumetric_fog {
                                (1.0 / (1.0 + fog_density * fog_density * 2.0)).clamp(0.06, 1.0)
                            } else {
                                1.0
                            };
                            ui.separator();
                            ui.colored_label(Color32::from_rgb(120, 220, 255), format!("Fog Visibility: {:.1}", cli.sky_fog_visibility));
                            ui.colored_label(
                                Color32::from_rgb(130, 200, 255),
                                format!("Effective Visibility: {:.1}", cli.sky_fog_visibility * fog_visibility_scale),
                            );
                            ui.colored_label(
                                if cli.sky_fog_is_atomspheric { Color32::from_rgb(100, 220, 120) } else { Color32::from_rgb(255, 200, 80) },
                                format!("Atmospheric: {}", if cli.sky_fog_is_atomspheric { "YES" } else { "NO" }),
                            );
                            ui.colored_label(
                                if cli.render_volumetric_fog && cli.volumetric_fog_density >= 1.5 {
                                    Color32::from_rgb(255, 200, 80)
                                } else {
                                    Color32::from_rgb(120, 220, 255)
                                },
                                format!(
                                    "Dense Fallback: {}",
                                    if cli.render_volumetric_fog && cli.volumetric_fog_density >= 1.5 {
                                        "FORCED EXP2"
                                    } else {
                                        "OFF"
                                    }
                                ),
                            );
                        });
                        if is_admin_user {
                            ui.menu_button("Admin", |ui| {
                                let cli = cl.data();
                                ui.label(if cli.is_owner { "Role: Owner" } else { "Role: Admin" });
                                ui.label(format!(
                                    "God: {} | Noclip: {}",
                                    if cli.admin_god_enabled { "ON" } else { "OFF" },
                                    if cli.admin_noclip_enabled { "ON" } else { "OFF" }
                                ));
                                ui.label(format!(
                                    "Global Editor View: {}",
                                    if cli.global_editor_view { "ON (F7)" } else { "OFF (F7)" }
                                ));
                                if ui.button("Open Admin Panel (F8)").clicked() {
                                    cli.admin_panel_open = true;
                                }
                                ui.small("Hotkeys: F10 world editor, F7 camera view, G toggle God, V toggle Noclip");
                            });
                        }
                        ui.menu_button("Audio", |_ui| {});
                        ui.menu_button("View", |ui| {
                            ui.toggle_value(&mut true, "HUD");
                            ui.toggle_value(&mut false, "Fullscreen");
                            if ui.button("Take Screenshot").clicked() {
                                todo!();
                            }

                            ui.separator();
                            let cli = cl.data();
                            ui.toggle_value(&mut cli.dbg_text, "Debug Text");
                            ui.toggle_value(&mut cli.dbg_inspector, "Inspector");
                        });
                    });
                });
            });
        });
}

pub fn ui_admin_panel(
    mut ctx: EguiContexts,
    mut cli: ResMut<ClientInfo>,
    mut net_client: Option<ResMut<RenetClient>>,
) {
    if !cli.is_admin || !cli.admin_panel_open {
        return;
    }

    let Ok(ctx_mut) = ctx.ctx_mut() else {
        return;
    };

    let mut request: Option<crate::net::AdminRequest> = None;

    egui::Window::new("Admin Panel")
        .resizable(false)
        .collapsible(false)
        .default_width(320.0)
        .anchor(Align2::RIGHT_TOP, [-14.0, 54.0])
        .show(ctx_mut, |ui| {
            ui.label(if cli.is_owner { "Role: Owner" } else { "Role: Admin" });
            ui.separator();
            ui.label(format!("God Mode: {}", if cli.admin_god_enabled { "Enabled" } else { "Disabled" }));
            ui.label(format!(
                "Noclip: {}",
                if cli.admin_noclip_enabled { "Enabled" } else { "Disabled" }
            ));
            ui.label(format!(
                "Global Editor View: {}",
                if cli.global_editor_view { "Enabled" } else { "Disabled" }
            ));

            ui.add_space(6.0);
            if ui.button("Toggle Global Editor View [F7]").clicked() {
                cli.global_editor_view = !cli.global_editor_view;
            }
            if ui.button("Toggle God [G]").clicked() {
                request = Some(crate::net::AdminRequest::ToggleGod);
            }
            if ui.button("Toggle Noclip [V]").clicked() {
                request = Some(crate::net::AdminRequest::ToggleNoclip);
            }
            if ui.button("Request World Save").clicked() {
                request = Some(crate::net::AdminRequest::SaveWorld);
            }
            if ui.button("Open World Editor [F10]").clicked() {
                cli.curr_ui = CurrentUI::WorldEditor;
                cli.global_editor_view = true;
                cli.enable_cursor_look = false;
            }

            ui.add_space(8.0);
            ui.small("Commands: /op <user>, /deop <user>, /god, /noclip, /time set <v>, /save");
            ui.small("Server is authoritative: states update from server packets.");

            if ui.button("Close").clicked() {
                cli.admin_panel_open = false;
            }
        });

    if let Some(request) = request {
        if let Some(net_client) = net_client.as_mut() {
            net_client.send_packet(&CPacket::AdminRequest { request });
        }
    }
}

pub fn ui_world_editor_panel(
    mut ctx: EguiContexts,
    mut cli: ResMut<ClientInfo>,
    mut cfg: ResMut<ClientSettings>,
    mut editor_runtime: ResMut<EditorRuntime>,
    mut rtt_state: ResMut<EditorViewportRttState>,
    mut vox_brush: ResMut<crate::voxel::VoxelBrush>,
    chunk_sys: Res<ClientChunkSystem>,
    meshing_stats: Option<Res<crate::voxel::VoxelMeshingStats>>,
    worldgen_stats: Option<Res<crate::voxel::VoxelWorldGenStats>>,
    diagnostics: Res<DiagnosticsStore>,
    mut editor_queries: ParamSet<(
        Query<&mut Transform, With<CharacterControllerCamera>>,
        Query<(Entity, Option<&Name>, Option<&Transform>, Option<&GlobalTransform>)>,
    )>,
) {
    if !cli.is_admin {
        if cli.curr_ui == CurrentUI::WorldEditor {
            cli.curr_ui = CurrentUI::None;
        }
        cli.global_editor_view = false;
        return;
    }

    if rtt_state.texture_id.is_none() && rtt_state.image_handle.id() != Handle::<Image>::default().id() {
        rtt_state.texture_id = Some(ctx.add_image(bevy_egui::EguiTextureHandle::Strong(
            rtt_state.image_handle.clone(),
        )));
    }

    let Ok(ctx_mut) = ctx.ctx_mut() else {
        return;
    };

    egui::TopBottomPanel::top("editor_workspace_top").show(ctx_mut, |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.heading("Editor Workspace");
            ui.separator();
            if ui
                .selectable_label(editor_runtime.view_mode == EditorViewMode::View3D, "Viewport 3D")
                .clicked()
            {
                editor_runtime.view_mode = EditorViewMode::View3D;
            }
            if ui
                .selectable_label(editor_runtime.view_mode == EditorViewMode::View2D, "Viewport 2D")
                .clicked()
            {
                editor_runtime.view_mode = EditorViewMode::View2D;
            }
            ui.separator();

            egui::ComboBox::from_id_salt("editor_camera_mode")
                .selected_text(match editor_runtime.camera_mode {
                    EditorCameraMode::Fly => "Camera: Fly",
                    EditorCameraMode::Orbit => "Camera: Orbit",
                    EditorCameraMode::TopDown => "Camera: TopDown",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut editor_runtime.camera_mode, EditorCameraMode::Fly, "Fly");
                    ui.selectable_value(&mut editor_runtime.camera_mode, EditorCameraMode::Orbit, "Orbit");
                    ui.selectable_value(&mut editor_runtime.camera_mode, EditorCameraMode::TopDown, "TopDown");
                });

            egui::ComboBox::from_id_salt("editor_render_mode")
                .selected_text(match editor_runtime.render_mode {
                    EditorRenderMode::Lit => "Render: Lit",
                    EditorRenderMode::Flat => "Render: Flat",
                    EditorRenderMode::Performance => "Render: Performance",
                    EditorRenderMode::Wireframe => "Render: Wireframe",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut editor_runtime.render_mode, EditorRenderMode::Lit, "Lit");
                    ui.selectable_value(&mut editor_runtime.render_mode, EditorRenderMode::Flat, "Flat");
                    ui.selectable_value(&mut editor_runtime.render_mode, EditorRenderMode::Performance, "Performance");
                    ui.selectable_value(&mut editor_runtime.render_mode, EditorRenderMode::Wireframe, "Wireframe");
                });

            if ui.checkbox(&mut cli.dbg_gizmo_all_loaded_chunks, "Chunk Bounds").changed() {
                cli.dbg_gizmo_curr_chunk = cli.dbg_gizmo_all_loaded_chunks;
                cli.dbg_gizmo_remesh_chunks = cli.dbg_gizmo_all_loaded_chunks;
            }

            ui.separator();
            ui.checkbox(&mut editor_runtime.show_help, "Help");
            ui.separator();
            ui.label("F10: Exit Editor");
            ui.label("F9: Inspector");
            ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Exit [F10]").clicked() {
                    cli.curr_ui = CurrentUI::None;
                    cli.global_editor_view = false;
                    cli.enable_cursor_look = true;
                    editor_runtime.view_mode = EditorViewMode::View3D;
                }
            });
        });
    });

    egui::TopBottomPanel::bottom("editor_workspace_bottom")
        .resizable(true)
        .default_height(145.0)
        .show(ctx_mut, |ui| {
            ui.horizontal(|ui| {
                if ui
                    .selectable_label(editor_runtime.bottom_tab == EditorBottomTab::Resources, "Resources")
                    .clicked()
                {
                    editor_runtime.bottom_tab = EditorBottomTab::Resources;
                }
                if ui
                    .selectable_label(editor_runtime.bottom_tab == EditorBottomTab::Diagnostics, "Diagnostics")
                    .clicked()
                {
                    editor_runtime.bottom_tab = EditorBottomTab::Diagnostics;
                }
                if ui
                    .selectable_label(editor_runtime.bottom_tab == EditorBottomTab::Assets, "Assets")
                    .clicked()
                {
                    editor_runtime.bottom_tab = EditorBottomTab::Assets;
                }
            });
            ui.separator();

            match editor_runtime.bottom_tab {
                EditorBottomTab::Resources => {
                    ui.label("Voxel Brush");
                    ui.add(egui::Slider::new(&mut vox_brush.size, 0.0..=32.0).text("Size"));
                    ui.add(egui::Slider::new(&mut vox_brush.strength, 0.0..=1.0).text("Intensity"));
                    ui.add(egui::Slider::new(&mut vox_brush.tex, 0..=64).text("Material ID"));
                    ui.horizontal(|ui| {
                        if ui
                            .selectable_label(vox_brush.shape == VoxShape::Isosurface, "Smooth")
                            .clicked()
                        {
                            vox_brush.shape = VoxShape::Isosurface;
                        }
                        if ui
                            .selectable_label(vox_brush.shape == VoxShape::Cube, "Cube")
                            .clicked()
                        {
                            vox_brush.shape = VoxShape::Cube;
                        }
                    });
                }
                EditorBottomTab::Diagnostics => {
                    let fps = diagnostics
                        .get(&FrameTimeDiagnosticsPlugin::FPS)
                        .and_then(|d| d.smoothed())
                        .unwrap_or(0.0);
                    let frame_ms = diagnostics
                        .get(&FrameTimeDiagnosticsPlugin::FRAME_TIME)
                        .and_then(|d| d.smoothed())
                        .unwrap_or(0.0);
                    ui.label(format!("FPS: {:.1}", fps));
                    ui.label(format!("Frame: {:.3} ms", frame_ms));
                    ui.label(format!("Loaded Chunks: {}", chunk_sys.get_chunks().len()));
                    ui.label(format!("Surface-First: {}", if cfg.surface_first_meshing { "ON" } else { "OFF" }));
                    ui.label(format!("Surface-Only: {}", if cfg.surface_only_meshing { "ON" } else { "OFF" }));
                    ui.label(format!("GPU WorldGen: {}", if cfg.gpu_worldgen { "ON" } else { "OFF" }));
                    if let Some(stats) = meshing_stats {
                        ui.separator();
                        ui.label(format!("Remesh Queue: {}", stats.remesh_queue));
                        ui.label(format!("Meshing Inflight: {}", stats.meshing_inflight));
                        ui.label(format!("Fast Pending Upgrade: {}", stats.fast_pending_upgrade));
                        ui.label(format!(
                            "Submitted (S/F): {}/{}",
                            stats.submitted_surface_this_frame,
                            stats.submitted_full_this_frame
                        ));
                        ui.label(format!(
                            "Completed Total (S/F): {}/{}",
                            stats.completed_surface_total,
                            stats.completed_full_total
                        ));
                    }
                    if let Some(stats) = worldgen_stats {
                        ui.separator();
                        ui.label(format!("WorldGen Loading Queue: {}", stats.loading_queue));
                        ui.label(format!("WorldGen Inflight: {}", stats.loading_inflight));
                        ui.label(format!("GPU Batch Size: {}", stats.batch_size));
                        ui.label(format!(
                            "Submitted This Frame (GPU/CPU): {}/{}",
                            stats.submitted_gpu_this_frame,
                            stats.submitted_cpu_this_frame
                        ));
                        ui.label(format!(
                            "Completed Total (GPU/CPU): {}/{}",
                            stats.completed_gpu_total,
                            stats.completed_cpu_total
                        ));
                    }
                }
                EditorBottomTab::Assets => {
                    ui.label("Assets browser placeholder (phase 2).");
                    ui.small("Next step: add texture/material list and quick-assign actions.");
                }
            }
        });

    egui::SidePanel::left("editor_workspace_hierarchy")
        .default_width(260.0)
        .show(ctx_mut, |ui| {
            ui.heading("Hierarchy");
            ui.separator();

            let mut rows: Vec<(Entity, String)> = editor_queries
                .p1()
                .iter()
                .map(|(entity, name, _, _)| {
                    let label = name
                        .map(|n| n.to_string())
                        .unwrap_or_else(|| format!("Entity {:?}", entity));
                    (entity, label)
                })
                .collect();
            rows.sort_by(|a, b| a.1.cmp(&b.1));

            egui::ScrollArea::vertical().show(ui, |ui| {
                for (entity, label) in rows {
                    let selected = editor_runtime.selected_entity.is_some_and(|v| v == entity);
                    if ui.selectable_label(selected, label).clicked() {
                        editor_runtime.selected_entity = Some(entity);
                    }
                }
            });
        });

    egui::SidePanel::right("editor_workspace_inspector")
        .default_width(300.0)
        .show(ctx_mut, |ui| {
            ui.heading("Inspector");
            ui.separator();

            if let Some(entity) = editor_runtime.selected_entity {
                if let Ok((_, name, trans, gtrans)) = editor_queries.p1().get(entity) {
                    let display_name = name
                        .map(|n| n.to_string())
                        .unwrap_or_else(|| format!("Entity {:?}", entity));
                    ui.label(display_name);
                    ui.small(format!("Entity ID: {:?}", entity));
                    ui.separator();

                    if let Some(trans) = trans {
                        ui.label("Transform");
                        ui.small(format!(
                            "pos: {:.2}, {:.2}, {:.2}",
                            trans.translation.x, trans.translation.y, trans.translation.z
                        ));
                    }
                    if let Some(gtrans) = gtrans {
                        let pos = gtrans.translation();
                        ui.label("Global Transform");
                        ui.small(format!("pos: {:.2}, {:.2}, {:.2}", pos.x, pos.y, pos.z));
                    }
                } else {
                    ui.small("Selected entity is no longer valid.");
                    editor_runtime.selected_entity = None;
                }
            } else {
                ui.small("Select an entity from Hierarchy.");
            }

            ui.separator();
            ui.label("Admin State");
            ui.small(format!("Role: {}", if cli.is_owner { "Owner" } else { "Admin" }));
            ui.small(format!(
                "God: {} | Noclip: {}",
                if cli.admin_god_enabled { "ON" } else { "OFF" },
                if cli.admin_noclip_enabled { "ON" } else { "OFF" }
            ));
        });

    egui::CentralPanel::default().show(ctx_mut, |ui| {
        ui.heading("Viewport");
        if editor_runtime.show_help {
            ui.small("W/A/S/D + Mouse: fly camera in 3D mode");
            ui.small("Ctrl + Left/Right Click: sculpt terrain while editor UI is open");
        }
        ui.separator();

        // Keep legacy flag disabled: viewport now uses an independent RTT camera.
        cli.global_editor_view = false;

        let loaded_count = chunk_sys.get_chunks().len();
        ui.label(format!("Loaded Chunks: {}", loaded_count));

        if loaded_count == 0 {
            ui.small("No chunks loaded yet.");
            return;
        }

        let mut min = IVec3::new(i32::MAX, i32::MAX, i32::MAX);
        let mut max = IVec3::new(i32::MIN, i32::MIN, i32::MIN);
        for cp in chunk_sys.get_chunks().keys() {
            min = min.min(*cp);
            max = max.max(*cp);
        }

        ui.small(format!(
            "Loaded Bounds XZ: x {}..{}, z {}..{}",
            min.x, max.x, min.z, max.z
        ));

        ui.horizontal(|ui| {
            ui.label("Load Radius X");
            ui.add(egui::Slider::new(&mut cfg.chunks_load_distance.x, 2..=64));
            ui.label("Y");
            ui.add(egui::Slider::new(&mut cfg.chunks_load_distance.y, 1..=32));
            ui.checkbox(&mut editor_runtime.show_lod_overlay, "LOD Overlay");

            if ui.button("Focus Loaded Bounds").clicked() {
                if let Ok(mut cam) = editor_queries.p0().single_mut() {
                    let cx = ((min.x + max.x) as f32) * 0.5;
                    let cz = ((min.z + max.z) as f32) * 0.5;
                    let span = (max - min).abs().max_element() as f32;
                    cam.translation = Vec3::new(cx, (span + 32.0).max(64.0), cz);
                }
            }
        });

        let desired = egui::vec2(ui.available_width().max(300.0), ui.available_height().max(220.0));
        let (response, painter) = ui.allocate_painter(desired, egui::Sense::hover());
        let rect = response.rect;
        painter.rect_filled(rect, 4.0, Color32::from_black_alpha(120));

        if editor_runtime.view_mode == EditorViewMode::View3D {
            let pixels_per_point = ctx_mut.pixels_per_point().max(0.5);
            let requested = UVec2::new(
                (rect.width() * pixels_per_point).round().max(1.0) as u32,
                (rect.height() * pixels_per_point).round().max(1.0) as u32,
            );
            if requested != rtt_state.requested_size {
                rtt_state.requested_size = requested;
            }

            if let Some(texture_id) = rtt_state.texture_id {
                painter.image(
                    texture_id,
                    rect,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    Color32::WHITE,
                );
            } else {
                painter.text(
                    rect.center(),
                    Align2::CENTER_CENTER,
                    "Preparing offscreen viewport...",
                    FontId::proportional(14.0),
                    Color32::from_gray(210),
                );
            }

            painter.text(
                rect.left_top() + egui::vec2(8.0, 8.0),
                Align2::LEFT_TOP,
                format!("RTT {}x{}", rtt_state.allocated_size.x, rtt_state.allocated_size.y),
                FontId::monospace(12.0),
                Color32::from_white_alpha(220),
            );
        } else {
            let cols = (max.x - min.x + 1).max(1) as f32;
            let rows = (max.z - min.z + 1).max(1) as f32;
            let cell_w = rect.width() / cols;
            let cell_h = rect.height() / rows;

            for cp in chunk_sys.get_chunks().keys() {
                let x = (cp.x - min.x) as f32;
                let z = (cp.z - min.z) as f32;
                let c_rect = egui::Rect::from_min_size(
                    egui::pos2(rect.left() + x * cell_w, rect.top() + z * cell_h),
                    egui::vec2(cell_w.max(1.0), cell_h.max(1.0)),
                );
                let color = if editor_runtime.show_lod_overlay {
                    Color32::from_rgb(55, 144, 86)
                } else {
                    Color32::from_gray(95)
                };
                painter.rect_filled(c_rect.shrink(0.5), 0.0, color);
            }

            if let Ok(cam) = editor_queries.p0().single_mut() {
                let cam_cp = Chunk::as_chunkpos(cam.translation.as_ivec3());
                let x = (cam_cp.x - min.x) as f32 + 0.5;
                let z = (cam_cp.z - min.z) as f32 + 0.5;
                let marker = egui::pos2(rect.left() + x * cell_w, rect.top() + z * cell_h);
                painter.circle_filled(marker, 4.0, Color32::YELLOW);
            }
        }
    });
}

pub fn hud_debug_text(
    mut ctx: EguiContexts,
    time: Res<Time>,
    diagnostics: Res<DiagnosticsStore>,

    #[cfg(feature = "target_native_os")] mut sys: Local<sysinfo::System>,
    render_adapter_info: Res<bevy::render::renderer::RenderAdapterInfo>,

    // cli: Res<ClientInfo>,
    worldinfo: Option<Res<WorldInfo>>,
    chunk_sys: Option<Res<ClientChunkSystem>>,
    worldgen_stats: Option<Res<crate::voxel::VoxelWorldGenStats>>,
    hit_result: Res<HitResult>,
    query_cam: Query<(&Transform, &bevy::camera::visibility::VisibleEntities), With<CharacterControllerCamera>>,
    query_vol_fog: Query<&bevy::light::VolumetricFog, With<CharacterControllerCamera>>,
    query_editor_cam: Query<&Camera, With<crate::client::game_client::EditorViewportCamera>>,
    query_sun: Query<(Entity, Option<&bevy::light::VolumetricLight>), With<crate::client::client_world::Sun>>,
    cli: Res<ClientInfo>,
    mut last_cam_pos: Local<Vec3>,
) {
    let mut str_sys = String::default();
    #[cfg(feature = "target_native_os")]
    {
        use crate::util::TimeIntervals;

        if time.at_interval(2.0) || sys.cpus().is_empty() {
            sys.refresh_cpu_all();
            sys.refresh_memory();
        }
        // "HOMEPATH", "\\Users\\Dreamtowards",
        // "LANG", "en_US.UTF-8",
        // "USERNAME", "Dreamtowards",

        let num_concurrency = std::thread::available_parallelism().map(|v| v.get()).unwrap_or(1);

        // use sysinfo::{CpuExt, SystemExt};

        let cpu_arch = std::env::consts::ARCH;
        let dist_id = std::env::consts::OS;
        let os_ver = sysinfo::System::long_os_version().unwrap_or_default();
        let os_ver_sm = sysinfo::System::os_version().unwrap_or_default();

        // let curr_path = std::env::current_exe().unwrap().display().to_string();
        let os_lang = std::env::var("LANG").unwrap_or("?lang".into()); // "en_US.UTF-8"
                                                                       //let user_name = std::env::var("USERNAME").unwrap();  // "Dreamtowards"

        let Some(cpu) = sys.cpus().first() else {
            return;
        };
        let cpu_cores = sysinfo::System::physical_core_count().unwrap_or_default();
        let cpu_name = cpu.brand().trim().to_string();
        let cpu_usage = cpu.cpu_usage();

        let mem_used = sys.used_memory() as f64 * BYTES_TO_GIB;
        let mem_total = sys.total_memory() as f64 * BYTES_TO_GIB;

        const BYTES_TO_MIB: f64 = 1.0 / 1024.0 / 1024.0;
        const BYTES_TO_GIB: f64 = 1.0 / 1024.0 / 1024.0 / 1024.0;

        let mut mem_usage_phys = 0.;
        let mut mem_usage_virtual = 0.;

        let gpu_name = &render_adapter_info.0.name;
        let gpu_backend = &render_adapter_info.0.backend.to_str();
        let gpu_driver_name = &render_adapter_info.0.driver;
        let gpu_driver_info = &render_adapter_info.0.driver_info;

        // #[cfg(feature = "target_native_os")]
        if let Some(usage) = memory_stats::memory_stats() {
            // println!("Current physical memory usage: {}", byte_unit::Byte::from_bytes(usage.physical_mem as u128).get_appropriate_unit(false).to_string());
            // println!("Current virtual memory usage: {}", byte_unit::Byte::from_bytes(usage.virtual_mem as u128).get_appropriate_unit(false).to_string());

            mem_usage_phys = usage.physical_mem as f64 * BYTES_TO_MIB;
            mem_usage_virtual = usage.virtual_mem as f64 * BYTES_TO_MIB;
        }

        str_sys = format!(
            "\nOS:  {dist_id}.{cpu_arch}, {num_concurrency} concurrency, {cpu_cores} cores; {os_lang}. {os_ver}, {os_ver_sm}.
CPU: {cpu_name}, usage {cpu_usage:.1}%
GPU: {gpu_name}, {gpu_backend}. {gpu_driver_name} {gpu_driver_info}
RAM: {mem_usage_phys:.2} MB, vir {mem_usage_virtual:.2} MB | {mem_used:.2} / {mem_total:.2} GB\n"
        );
    }

    let mut cam_visible_entities_num = 0;
    let mut str_world = String::default();
    if chunk_sys.is_some() && worldinfo.is_some() {
        let Some(chunk_sys) = chunk_sys else {
            return;
        };
        let Some(worldinfo) = worldinfo else {
            return;
        };

        let Ok((cam_trans, cam_visible_entities)) = query_cam.single() else {
            return;
        };
        let cam_pos = cam_trans.translation;
        let cam_pos_spd = (cam_pos - *last_cam_pos).length() / time.delta_secs();
        *last_cam_pos = cam_pos;
        cam_visible_entities_num = cam_visible_entities.entities.len();

        let num_chunks_loading = -1; //chunk_sys.chunks_loading.len();
        let num_chunks_remesh = chunk_sys.chunks_remesh.len();
        let num_chunks_meshing = chunk_sys.chunks_meshing.len();

        let mut hit_str = "none".into();
        if hit_result.is_hit {
            hit_str = format!(
                "p: {}, n: {}, d: {}, vox: {}",
                hit_result.position, hit_result.normal, hit_result.distance, hit_result.is_voxel
            );
        }

        let mut cam_cell_str = "none".into();
        let campos_v = cam_pos.floor().as_ivec3();
        if let Some(chunk) = chunk_sys.get_chunk(Chunk::as_chunkpos(campos_v)) {
            let vox = chunk.at_voxel(Chunk::as_localpos(campos_v));
            
            cam_cell_str = format!(
"Vox: tex: {}, shape: {:?}, isoval: {}, light: [{}]
Chunk: is_populated: {}", vox.tex_id, vox.shape_id, vox.isovalue(), vox.light, chunk.is_populated);
        }

        str_world = format!(
            "
Cam: ({:.1}, {:.2}, {:.3}). spd: {:.2} mps, {:.2} kph. 
{cam_cell_str}

Hit: {hit_str},
World: '{}', daytime: {:.2}. inhabited: {:.1}, seed: {}
ChunkSys: {} loaded, {num_chunks_loading} loading, {num_chunks_remesh} remesh, {num_chunks_meshing} meshing, -- saving.",
            cam_pos.x,
            cam_pos.y,
            cam_pos.z,
            cam_pos_spd,
            cam_pos_spd * 3.6,
            worldinfo.name,
            worldinfo.daytime,
            worldinfo.time_inhabited,
            worldinfo.seed,
            chunk_sys.num_chunks()
        );
    }

    let frame_time = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FRAME_TIME)
        .map_or(time.delta_secs_f64(), |d| d.smoothed().unwrap_or_default());

    let fps = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .map_or(frame_time / 1.0, |d| d.smoothed().unwrap_or_default());

    let num_entity = diagnostics
        .get(&EntityCountDiagnosticsPlugin::ENTITY_COUNT)
        .map_or(0., |f| f.smoothed().unwrap_or_default()) as usize;

    let fog_entity_status = if query_vol_fog.single().is_ok() { "PRESENT" } else { "MISSING" };
    let sun_status = match query_sun.single() {
        Ok((_e, vol)) => {
            if vol.is_some() {
                "PRESENT + VOL_LIGHT"
            } else {
                "PRESENT (NO VOL_LIGHT)"
            }
        }
        Err(_) => "MISSING",
    };

    let fog_density = if cli.volumetric_fog_density.is_finite() {
        cli.volumetric_fog_density.clamp(0.0, 3.0)
    } else {
        0.0
    };
    let fog_visibility_scale = if cli.render_volumetric_fog {
        (1.0 / (1.0 + fog_density * fog_density * 2.0)).clamp(0.06, 1.0)
    } else {
        1.0
    };
    let fog_visibility_effective = cli.sky_fog_visibility * fog_visibility_scale;
    let fog_dense_fallback = cli.render_volumetric_fog && fog_density >= 1.5;
    let editor_cam_status = match query_editor_cam.single() {
        Ok(cam) => {
            if cam.is_active {
                "ON"
            } else {
                "OFF"
            }
        }
        Err(_) => "MISSING",
    };

    let str = format!(
        "fps: {fps:.1}, dt: {frame_time:.4}ms
entity: vis {cam_visible_entities_num} / all {num_entity}
    FogDbg: enabled={} density={:.2} cam_fog={} sun={} vis={:.1}->{:.1} fallback={} editor_view={} editor_cam={}
{str_sys}
{str_world}
"
    ,
        if cli.render_volumetric_fog { "ON" } else { "OFF" },
        cli.volumetric_fog_density,
        fog_entity_status,
        sun_status,
        cli.sky_fog_visibility,
        fog_visibility_effective,
        if fog_dense_fallback { "FORCED_EXP2" } else { "OFF" },
        if cli.global_editor_view { "ON" } else { "OFF" },
        editor_cam_status,
    );

    let mut wg_banner = "WORLDGEN: UNKNOWN";
    let mut wg_color = Color32::from_rgb(120, 120, 120);
    if let Some(stats) = worldgen_stats {
        wg_banner = if stats.force_cpu_persisted_world {
            "WORLDGEN: CPU (Persisted Save Compatibility Lock)"
        } else if stats.last_backend_label == "GPU->CPU FALLBACK" {
            "WORLDGEN: GPU->CPU FALLBACK"
        } else if stats.last_backend_label == "GPU" {
            "WORLDGEN: GPU"
        } else if stats.last_backend_label == "CPU" {
            "WORLDGEN: CPU"
        } else {
            stats.last_backend_label
        };

        wg_color = if stats.force_cpu_persisted_world {
            Color32::from_rgb(255, 190, 60)
        } else if stats.last_backend_label == "GPU->CPU FALLBACK" {
            Color32::from_rgb(255, 90, 90)
        } else if stats.last_backend_label == "GPU" {
            Color32::from_rgb(70, 230, 120)
        } else {
            Color32::from_rgb(255, 220, 90)
        };
    }

    let Ok(ctx_mut) = ctx.ctx_mut() else {
        return;
    };
    let painter = ctx_mut.layer_painter(LayerId::new(egui::Order::Middle, Id::NULL));
    let banner_rect = egui::Rect::from_min_size([0.0, 28.0].into(), egui::vec2(560.0, 18.0));
    painter.rect_filled(banner_rect, 2.0, Color32::from_black_alpha(170));
    painter.text(
        [6.0, 30.0].into(),
        Align2::LEFT_TOP,
        wg_banner,
        FontId::proportional(13.),
        wg_color,
    );
    painter.text(
        [0., 50.].into(),
        Align2::LEFT_TOP,
        str,
        FontId::proportional(12.),
        Color32::WHITE,
    );
}
