use std::time::Duration;

use bevy::{prelude::*, platform::collections::HashSet};
use bevy_renet::{
    renet::{ClientId, ConnectionConfig, DefaultChannel, RenetServer, ServerEvent},
    netcode::{NetcodeServerTransport, NetcodeServerPlugin},
    RenetServerPlugin,
};

use crate::{
    net::{packet::{CellData, InventoryDeltaEntry, NetItemStack}, CPacket, EntityId, RenetServerHelper, SPacket, PROTOCOL_ID},
    server::prelude::*,
    util::{current_timestamp_millis, AsMutRef},
    voxel::{ActiveWorld, ChunkSystem, ServerChunkSystem, WorldSaveRequest},
};

fn username_equals(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

fn list_contains_username(list: &[String], username: &str) -> bool {
    list.iter().any(|v| username_equals(v, username))
}

fn load_or_create_world_meta(active_world: &ActiveWorld) -> Option<crate::voxel::WorldMeta> {
    match crate::voxel::load_world_meta(&active_world.name) {
        Ok(meta) => Some(meta),
        Err(err) => {
            warn!(
                "Failed to load world meta '{}': {}. Trying to create a new one.",
                active_world.name,
                err
            );
            match crate::voxel::create_world_with_config(
                &active_world.name,
                active_world.seed,
                active_world.config.clone(),
            ) {
                Ok(meta) => Some(meta),
                Err(create_err) => {
                    warn!(
                        "Failed to create world meta '{}' after load failure: {}",
                        active_world.name,
                        create_err
                    );
                    None
                }
            }
        }
    }
}

fn send_admin_state(server: &mut RenetServer, client_id: ClientId, player: &PlayerInfo) {
    server.send_packet(
        client_id,
        &SPacket::AdminState {
            state: player.admin_snapshot(),
        },
    );
}

fn remove_online_player_session(
    server: &mut RenetServer,
    serverinfo: &mut ServerInfo,
    client_id: ClientId,
    reason: &str,
) {
    if let Some(player) = serverinfo.online_players.remove(&client_id) {
        info!(
            "Removing online player session {} ({}) due to {}",
            player.username,
            client_id,
            reason
        );

        serverinfo
            .saved_inventories
            .insert(player.username.clone(), player.inventory.clone());

        server.broadcast_packet_chat(format!(
            "Player {} left. ({}/N)",
            player.username,
            serverinfo.online_players.len()
        ));

        server.broadcast_packet(&SPacket::EntityDel {
            entity_id: player.entity_id,
        });
    }
}

fn starter_inventory() -> Vec<NetItemStack> {
    let mut slots = vec![NetItemStack::default(); 36];
    let starter = [
        (0usize, NetItemStack { count: 1, item_id: 7 }),
        (1usize, NetItemStack { count: 1, item_id: 8 }),
        (2usize, NetItemStack { count: 1, item_id: 9 }),
        (3usize, NetItemStack { count: 32, item_id: 5 }),
        (4usize, NetItemStack { count: 16, item_id: 6 }),
        (5usize, NetItemStack { count: 48, item_id: 4 }),
        (6usize, NetItemStack { count: 24, item_id: 3 }),
        (7usize, NetItemStack { count: 8, item_id: 10 }),
        (8usize, NetItemStack { count: 12, item_id: 1 }),
    ];
    for (idx, stack) in starter {
        if let Some(slot) = slots.get_mut(idx) {
            *slot = stack;
        }
    }
    slots
}

pub struct ServerNetworkPlugin;

impl Plugin for ServerNetworkPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(RenetServerPlugin);
        app.add_plugins(NetcodeServerPlugin);

        app.insert_resource(RenetServer::new(ConnectionConfig {
            server_channels_config: super::net_channel_config(64 * 1024 * 1024),
            ..default()
        }));

        app.add_systems(Startup, bind_server_endpoint);
        app.add_systems(Update, server_sys);

        // app.add_systems(Update, ui_server_net);
    }
}

fn bind_server_endpoint(mut cmds: Commands, cfg: Res<ServerSettings>) {
    match super::new_netcode_server_transport(cfg.port, 64) {
        Ok(transport) => {
            cmds.insert_resource(transport);
            info!("Server bind endpoint at port {}", cfg.port);
        }
        Err(err) => {
            error!("Failed to bind server endpoint at port {}: {}", cfg.port, err);
            error!("Integrated server will be unavailable. Multiplayer features only.");
            // Don't panic - allow app to continue without integrated server
        }
    }
}

pub fn server_sys(
    mut server_events: EventReader<ServerEvent>,
    mut server: ResMut<RenetServer>,
    transport: Option<Res<NetcodeServerTransport>>,

    mut serverinfo: ResMut<ServerInfo>,
    active_world: Res<ActiveWorld>,
    mut chunk_sys: ResMut<ServerChunkSystem>,
    mut save_req: ResMut<WorldSaveRequest>,
    mut cmds: Commands,
) {
    // If server failed to bind, skip processing
    let Some(transport) = transport else {
        return;
    };

    for event in server_events.read() {
        match event {
            ServerEvent::ClientConnected { client_id } => {
                let result_string: String = transport
                    .user_data(*client_id)
                    .unwrap_or([0; bevy_renet::netcode::NETCODE_USER_DATA_BYTES])
                    .iter()
                    .map(|&byte| byte as char)
                    .collect();

                info!("Cli Connected {} {}", client_id, result_string);
            }
            ServerEvent::ClientDisconnected { client_id, reason } => {
                info!("Cli Disconnected {} {}", client_id, reason);
                remove_online_player_session(&mut server, &mut serverinfo, *client_id, "disconnect event");
            }
        }
    }

    // Reconcile stale sessions when disconnect events are missed.
    let connected_client_ids: HashSet<ClientId> = server.clients_id().into_iter().collect();
    let stale_client_ids: Vec<ClientId> = serverinfo
        .online_players
        .keys()
        .copied()
        .filter(|id| !connected_client_ids.contains(id))
        .collect();
    for stale_id in stale_client_ids {
        remove_online_player_session(&mut server, &mut serverinfo, stale_id, "missing from active client list");
    }

    // Receive message from all clients
    for client_id in server.clients_id() {
        while let Some(bytes) = server.receive_message(client_id, DefaultChannel::ReliableOrdered) {
            // info!("Server Received: {}", String::from_utf8_lossy(&bytes));
            let packet: CPacket = match bincode::deserialize(&bytes[..]) {
                Ok(packet) => packet,
                Err(err) => {
                    warn!("Failed to deserialize CPacket from {}: {}", client_id, err);
                    continue;
                }
            };
            match packet {
                CPacket::Handshake { protocol_version } => {
                    if protocol_version < PROTOCOL_ID {
                        server.send_packet_disconnect(client_id, "Client outdated.".into());
                    } else if protocol_version > PROTOCOL_ID {
                        server.send_packet_disconnect(client_id, "Server outdated.".into());
                    }
                }
                CPacket::ServerQuery {} => {
                    // Reserved for future server-info query path.
                }
                CPacket::Login {
                    uuid,
                    access_token,
                    username,
                } => {
                    info!("Login Requested: {} {} {}", uuid, access_token, username);

                    // If a duplicated username session exists, prefer the latest login
                    // and evict the older session to avoid permanent lockout.
                    let duplicate_ids: Vec<ClientId> = serverinfo
                        .online_players
                        .iter()
                        .filter_map(|(online_id, v)| {
                            if *online_id != client_id && username_equals(&v.username, &username) {
                                Some(*online_id)
                            } else {
                                None
                            }
                        })
                        .collect();
                    for duplicate_id in duplicate_ids {
                        server.send_packet_disconnect(
                            duplicate_id,
                            "You were disconnected because this account logged in from another session.".into(),
                        );
                        remove_online_player_session(
                            &mut server,
                            &mut serverinfo,
                            duplicate_id,
                            "duplicate username replaced by newer login",
                        );
                    }

                    if serverinfo.online_players.contains_key(&client_id) {
                        remove_online_player_session(
                            &mut server,
                            &mut serverinfo,
                            client_id,
                            "client re-login refresh",
                        );
                    }
                    // 模拟登录验证
                    std::thread::sleep(Duration::from_millis(800));

                    let mut is_owner = false;
                    let mut is_admin = false;
                    if let Some(mut meta) = load_or_create_world_meta(&active_world) {
                        let mut meta_changed = false;
                        if meta
                            .owner_username
                            .as_ref()
                            .is_none_or(|owner| owner.trim().is_empty())
                        {
                            meta.owner_username = Some(username.clone());
                            meta_changed = true;
                            info!(
                                "Assigned initial world owner '{}' for world '{}'",
                                username,
                                active_world.name
                            );
                        }

                        if meta_changed {
                            if let Err(err) = crate::voxel::save_world_meta(&meta) {
                                warn!(
                                    "Failed to persist owner bootstrap for world '{}': {}",
                                    active_world.name,
                                    err
                                );
                            }
                        }

                        is_owner = meta
                            .owner_username
                            .as_ref()
                            .is_some_and(|owner| username_equals(owner, &username));
                        is_admin = is_owner || list_contains_username(&meta.admin_usernames, &username);
                    }

                    let spawn_position = active_world.config.default_spawn_position_with_seed(active_world.seed);
                    info!(
                        "Spawn computed for {}: {} (seed={}, mode={:?})",
                        username,
                        spawn_position,
                        active_world.seed,
                        active_world.config.terrain_mode
                    );
                    let entity_id = EntityId::from_server(cmds.spawn(Transform::from_translation(spawn_position)).id());

                    // Login Success
                    server.send_packet(
                        client_id,
                        &SPacket::LoginSuccess {
                            player_entity: entity_id,
                            spawn_position,
                        },
                    );
                    server.send_packet(
                        client_id,
                        &SPacket::WorldInit {
                            world_name: active_world.name.clone(),
                            seed: active_world.seed,
                            world_config: active_world.config.clone(),
                        },
                    );
                    server.send_packet(
                        client_id,
                        &SPacket::EntityPos {
                            entity_id,
                            position: spawn_position,
                        },
                    );

                    let inventory = serverinfo
                        .saved_inventories
                        .get(&username)
                        .cloned()
                        .unwrap_or_else(starter_inventory);
                    server.send_packet(client_id, &SPacket::InventorySync { slots: inventory.clone() });

                    server.broadcast_packet_chat(format!("Player {} joined. ({}/N)", &username, serverinfo.online_players.len() + 1));

                    server.broadcast_packet_except(
                        client_id,
                        &SPacket::EntityNew {
                            entity_id,
                            name: username.clone(),
                            position: spawn_position,
                        },
                    );

                    // Send Server Players to the client. Note: Before insert of online_players
                    for player in serverinfo.online_players.values() {
                        server.send_packet(
                            client_id,
                            &SPacket::EntityNew {
                                entity_id: player.entity_id,
                                name: player.username.clone(),
                                position: player.position,
                            },
                        );
                        server.send_packet(
                            client_id,
                            &SPacket::EntityPos {
                                entity_id: player.entity_id,
                                position: player.position,
                            },
                        );
                    }

                    serverinfo.online_players.insert(
                        client_id,
                        PlayerInfo {
                            username,
                            user_id: uuid,
                            is_owner,
                            is_admin,
                            god_enabled: false,
                            noclip_enabled: false,
                            client_id,
                            entity_id,
                            position: spawn_position,
                            last_valid_chunkpos: crate::voxel::Chunk::as_chunkpos(spawn_position.as_ivec3()),
                            chunks_loaded: HashSet::default(),
                            chunks_load_distance: IVec2::new(4, 3),
                            ping_rtt: 0,
                            inventory,
                        },
                    );

                    if let Some(player) = serverinfo.online_players.get(&client_id) {
                        send_admin_state(&mut server, client_id, player);
                    }
                }
                CPacket::ChatMessage { message } => {
                    let Some(issuer) = serverinfo.online_players.get(&client_id) else {
                        server.send_packet_disconnect(client_id, "illegal play-stage packet. you have not login yet".into());
                        continue;
                    };
                    let issuer_name = issuer.username.clone();
                    let issuer_is_owner = issuer.is_owner;
                    let issuer_is_admin = issuer.is_admin;

                    if message.starts_with('/') {
                        let Some(args) = shlex::split(&message[1..]) else {
                            server.send_packet_chat(client_id, "Invalid command format".into());
                            continue;
                        };
                        if args.is_empty() {
                            server.send_packet_chat(client_id, "Empty command".into());
                            continue;
                        }

                        match args[0].as_str() {
                            "time" => {
                                if !issuer_is_admin {
                                    server.send_packet_chat(client_id, "Permission denied".into());
                                    continue;
                                }
                                if args.get(1).is_some_and(|v| v == "set") {
                                    let Some(daytime_raw) = args.get(2) else {
                                        server.send_packet_chat(client_id, "Usage: /time set <value>".into());
                                        continue;
                                    };
                                    let Ok(daytime) = daytime_raw.parse::<f32>() else {
                                        server.send_packet_chat(client_id, "Invalid daytime value".into());
                                        continue;
                                    };
                                    server.broadcast_packet(&SPacket::WorldTime { daytime });
                                } else {
                                    server.send_packet_chat(client_id, "Usage: /time set <value>".into());
                                }
                            }
                            "op" => {
                                if !issuer_is_owner {
                                    server.send_packet_chat(client_id, "Only the world owner can grant admin.".into());
                                    continue;
                                }
                                let Some(target_name) = args.get(1) else {
                                    server.send_packet_chat(client_id, "Usage: /op <username>".into());
                                    continue;
                                };

                                match crate::voxel::set_world_admin(&active_world.name, target_name, true) {
                                    Ok(_) => {
                                        let mut target_online: Option<ClientId> = None;
                                        for (target_id, target_player) in serverinfo.online_players.iter_mut() {
                                            if username_equals(&target_player.username, target_name) {
                                                target_player.is_admin = true;
                                                target_online = Some(*target_id);
                                                break;
                                            }
                                        }
                                        if let Some(target_id) = target_online {
                                            if let Some(target_player) = serverinfo.online_players.get(&target_id) {
                                                send_admin_state(&mut server, target_id, target_player);
                                            }
                                        }
                                        server.broadcast_packet_chat(format!(
                                            "[Admin] {} granted admin to {}",
                                            issuer_name,
                                            target_name
                                        ));
                                    }
                                    Err(err) => {
                                        server.send_packet_chat(client_id, format!("Failed to grant admin: {}", err));
                                    }
                                }
                            }
                            "deop" => {
                                if !issuer_is_owner {
                                    server.send_packet_chat(client_id, "Only the world owner can revoke admin.".into());
                                    continue;
                                }
                                let Some(target_name) = args.get(1) else {
                                    server.send_packet_chat(client_id, "Usage: /deop <username>".into());
                                    continue;
                                };

                                match crate::voxel::load_world_meta(&active_world.name) {
                                    Ok(meta) => {
                                        if meta
                                            .owner_username
                                            .as_ref()
                                            .is_some_and(|owner| username_equals(owner, target_name))
                                        {
                                            server.send_packet_chat(client_id, "Cannot revoke owner privileges.".into());
                                            continue;
                                        }
                                    }
                                    Err(err) => {
                                        server.send_packet_chat(client_id, format!("Failed to read world meta: {}", err));
                                        continue;
                                    }
                                }

                                match crate::voxel::set_world_admin(&active_world.name, target_name, false) {
                                    Ok(_) => {
                                        let mut target_online: Option<ClientId> = None;
                                        for (target_id, target_player) in serverinfo.online_players.iter_mut() {
                                            if username_equals(&target_player.username, target_name) {
                                                target_player.is_admin = target_player.is_owner;
                                                if !target_player.is_admin {
                                                    target_player.god_enabled = false;
                                                    target_player.noclip_enabled = false;
                                                }
                                                target_online = Some(*target_id);
                                                break;
                                            }
                                        }
                                        if let Some(target_id) = target_online {
                                            if let Some(target_player) = serverinfo.online_players.get(&target_id) {
                                                send_admin_state(&mut server, target_id, target_player);
                                            }
                                        }
                                        server.broadcast_packet_chat(format!(
                                            "[Admin] {} revoked admin from {}",
                                            issuer_name,
                                            target_name
                                        ));
                                    }
                                    Err(err) => {
                                        server.send_packet_chat(client_id, format!("Failed to revoke admin: {}", err));
                                    }
                                }
                            }
                            "god" => {
                                if !issuer_is_admin {
                                    server.send_packet_chat(client_id, "Permission denied".into());
                                    continue;
                                }
                                if let Some(player) = serverinfo.online_players.get_mut(&client_id) {
                                    player.god_enabled = if let Some(arg) = args.get(1) {
                                        match arg.as_str() {
                                            "on" | "1" | "true" => true,
                                            "off" | "0" | "false" => false,
                                            _ => !player.god_enabled,
                                        }
                                    } else {
                                        !player.god_enabled
                                    };
                                    if !player.god_enabled {
                                        player.noclip_enabled = false;
                                    }
                                }
                                if let Some(player) = serverinfo.online_players.get(&client_id) {
                                    send_admin_state(&mut server, client_id, player);
                                    server.send_packet_chat(
                                        client_id,
                                        format!("God mode {}", if player.god_enabled { "enabled" } else { "disabled" }),
                                    );
                                }
                            }
                            "noclip" => {
                                if !issuer_is_admin {
                                    server.send_packet_chat(client_id, "Permission denied".into());
                                    continue;
                                }
                                if let Some(player) = serverinfo.online_players.get_mut(&client_id) {
                                    let requested = if let Some(arg) = args.get(1) {
                                        match arg.as_str() {
                                            "on" | "1" | "true" => true,
                                            "off" | "0" | "false" => false,
                                            _ => !player.noclip_enabled,
                                        }
                                    } else {
                                        !player.noclip_enabled
                                    };

                                    if requested && !player.god_enabled {
                                        player.god_enabled = true;
                                    }
                                    player.noclip_enabled = requested;
                                }
                                if let Some(player) = serverinfo.online_players.get(&client_id) {
                                    send_admin_state(&mut server, client_id, player);
                                    server.send_packet_chat(
                                        client_id,
                                        format!(
                                            "Noclip {}",
                                            if player.noclip_enabled { "enabled" } else { "disabled" }
                                        ),
                                    );
                                }
                            }
                            "save" => {
                                if !issuer_is_admin {
                                    server.send_packet_chat(client_id, "Permission denied".into());
                                    continue;
                                }
                                save_req.save_now = true;
                                server.send_packet_chat(client_id, "World save requested".into());
                            }
                            _ => {
                                server.send_packet_chat(client_id, format!("Unknown command: {}", args[0]));
                            }
                        }

                        info!("[CMD]: {:?}", args);
                    } else {
                        server.broadcast_packet_chat(format!("<{}>: {}", issuer_name, message));
                    }
                }
                CPacket::LoadDistance { load_distance } => {
                    let Some(player) = serverinfo.online_players.get_mut(&client_id) else {
                        server.send_packet_disconnect(client_id, "illegal play-stage packet. you have not login yet".into());
                        continue;
                    };
                    player.chunks_load_distance = IVec2::new(load_distance.x.max(2), load_distance.y.max(1));
                }
                CPacket::PlayerPos { position } => {
                    let Some(player) = serverinfo.online_players.get_mut(&client_id) else {
                        server.send_packet_disconnect(client_id, "illegal play-stage packet. you have not login yet".into());
                        continue;
                    };

                    if !position.is_finite() {
                        warn!("Ignored invalid PlayerPos from {}: {}", player.username, position);
                        continue;
                    }

                    player.position = position;
                    player.last_valid_chunkpos = crate::voxel::Chunk::as_chunkpos(position.as_ivec3());
                    server.broadcast_packet_except(
                        client_id,
                        &SPacket::EntityPos {
                            entity_id: player.entity_id,
                            position,
                        },
                    );
                }
                CPacket::Ping {
                    client_time,
                    last_rtt,
                } => {
                    let Some(player) = serverinfo.online_players.get_mut(&client_id) else {
                        server.send_packet_disconnect(client_id, "illegal play-stage packet. you have not login yet".into());
                        continue;
                    };

                    player.ping_rtt = last_rtt;
                    server.send_packet(
                        client_id,
                        &SPacket::Pong {
                            client_time,
                            server_time: current_timestamp_millis(),
                        },
                    );
                }
                CPacket::PlayerList => {
                    if !serverinfo.online_players.contains_key(&client_id) {
                        server.send_packet_disconnect(client_id, "illegal play-stage packet. you have not login yet".into());
                        continue;
                    }
                    let playerlist = serverinfo
                        .online_players
                        .iter()
                        .map(|e| (e.1.username.clone(), e.1.ping_rtt))
                        .collect();
                    server.send_packet(client_id, &SPacket::PlayerList { playerlist });
                }
                CPacket::InventorySwap { a, b } => {
                    let Some(player) = serverinfo.online_players.get_mut(&client_id) else {
                        server.send_packet_disconnect(client_id, "illegal play-stage packet. you have not login yet".into());
                        continue;
                    };

                    let a_idx = a as usize;
                    let b_idx = b as usize;
                    if a_idx >= player.inventory.len() || b_idx >= player.inventory.len() {
                        warn!("InventorySwap out of range: {} <-> {}", a, b);
                        continue;
                    }

                    player.inventory.swap(a_idx, b_idx);

                    server.send_packet(
                        client_id,
                        &SPacket::InventoryDelta {
                            changes: vec![
                                InventoryDeltaEntry {
                                    slot: a,
                                    stack: player.inventory[a_idx],
                                },
                                InventoryDeltaEntry {
                                    slot: b,
                                    stack: player.inventory[b_idx],
                                },
                            ],
                        },
                    );
                }
                CPacket::AdminRequest { request } => {
                    let Some(player) = serverinfo.online_players.get_mut(&client_id) else {
                        server.send_packet_disconnect(client_id, "illegal play-stage packet. you have not login yet".into());
                        continue;
                    };

                    match request {
                        crate::net::AdminRequest::RequestState => {}
                        crate::net::AdminRequest::ToggleGod => {
                            if !player.is_admin {
                                server.send_packet_chat(client_id, "Permission denied".into());
                                continue;
                            }
                            player.god_enabled = !player.god_enabled;
                            if !player.god_enabled {
                                player.noclip_enabled = false;
                            }
                        }
                        crate::net::AdminRequest::SetGod { enabled } => {
                            if !player.is_admin {
                                server.send_packet_chat(client_id, "Permission denied".into());
                                continue;
                            }
                            player.god_enabled = enabled;
                            if !player.god_enabled {
                                player.noclip_enabled = false;
                            }
                        }
                        crate::net::AdminRequest::ToggleNoclip => {
                            if !player.is_admin {
                                server.send_packet_chat(client_id, "Permission denied".into());
                                continue;
                            }
                            if !player.god_enabled {
                                player.god_enabled = true;
                            }
                            player.noclip_enabled = !player.noclip_enabled;
                        }
                        crate::net::AdminRequest::SetNoclip { enabled } => {
                            if !player.is_admin {
                                server.send_packet_chat(client_id, "Permission denied".into());
                                continue;
                            }
                            if enabled && !player.god_enabled {
                                player.god_enabled = true;
                            }
                            player.noclip_enabled = enabled;
                        }
                        crate::net::AdminRequest::SaveWorld => {
                            if !player.is_admin {
                                server.send_packet_chat(client_id, "Permission denied".into());
                                continue;
                            }
                            save_req.save_now = true;
                            server.send_packet_chat(client_id, "World save requested".into());
                        }
                    }

                    if let Some(player) = serverinfo.online_players.get(&client_id) {
                        send_admin_state(&mut server, client_id, player);
                    }
                }
                CPacket::ChunkModify { chunkpos, voxel } => {
                    let Some(player) = serverinfo.online_players.get(&client_id) else {
                        server.send_packet_disconnect(client_id, "illegal play-stage packet. you have not login yet".into());
                        continue;
                    };
                    if !player.is_admin {
                        server.send_packet_chat(client_id, "Permission denied: editing requires admin.".into());
                        continue;
                    }

                    if let Some(chunk) = chunk_sys.get_chunk(chunkpos) {
                        CellData::to_chunk(&voxel, chunk.as_mut());
                        server.broadcast_packet_on(
                            bevy_renet::renet::DefaultChannel::ReliableUnordered,
                            &SPacket::ChunkModify { chunkpos, voxel },
                        );
                    } else {
                        server.send_packet_chat(client_id, format!("Chunk {} is not loaded on server", chunkpos));
                    }
                }
            }
        }
    }
}
