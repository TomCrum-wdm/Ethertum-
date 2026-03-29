use bevy::{
    prelude::*,
    platform::collections::{HashMap, HashSet},
};
use bevy_renet::renet::ClientId;

use crate::{
    net::{EntityId, NetItemStack, ServerNetworkPlugin},
    voxel::ServerVoxelPlugin,
};

pub struct DedicatedServerPlugin;

impl Plugin for DedicatedServerPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ServerInfo::default());
        app.insert_resource(ServerSettings::default());

        // Network
        app.add_plugins(ServerNetworkPlugin);

        // ChunkSystem
        app.add_plugins(ServerVoxelPlugin);

        // Physics
        // app.add_plugins(PhysicsPlugins::default());

        app.add_systems(PreStartup, on_init); // load settings.
        app.add_systems(Last, on_exit); // save settings.

        let rcon_port = 8001;
        match tiny_http::Server::http(format!("0.0.0.0:{}", rcon_port)) {
            Ok(http_server) => {
                if let Some(addr) = http_server.server_addr().to_ip() {
                    info!("Start RCON endpoint on {}", addr);
                } else {
                    info!("Start RCON endpoint");
                }
                app.insert_resource(rcon::HttpServer { server: http_server });
                app.add_systems(Update, rcon::on_http_recv);
            }
            Err(err) => {
                warn!("Failed to start RCON endpoint on {}: {}", rcon_port, err);
            }
        }
    }
}

const SERVER_SETTINGS_FILE: &str = "server.settings.json";
const SERVER_PLAYERDATA_FILE: &str = "server.playerdata.json";

fn on_init(mut cfg: ResMut<ServerSettings>, mut server_info: ResMut<ServerInfo>) {
    info!("Loading server settings from {SERVER_SETTINGS_FILE}");

    if let Ok(content) = std::fs::read_to_string(SERVER_SETTINGS_FILE) {
        if let Ok(c) = serde_json::from_str(&content) {
            *cfg = c;
        }
    }

    if let Ok(content) = std::fs::read_to_string(SERVER_PLAYERDATA_FILE) {
        match serde_json::from_str::<HashMap<String, Vec<NetItemStack>>>(&content) {
            Ok(data) => {
                server_info.saved_inventories = data;
                info!("Loaded {} player inventories", server_info.saved_inventories.len());
            }
            Err(err) => warn!("Failed to parse {}: {}", SERVER_PLAYERDATA_FILE, err),
        }
    }
}

fn on_exit(mut exit_events: MessageReader<bevy::app::AppExit>, cfg: Res<ServerSettings>, mut server_info: ResMut<ServerInfo>) {
    for _ in exit_events.read() {
        info!("Saving server settings to {SERVER_SETTINGS_FILE}");
        match serde_json::to_string_pretty(&*cfg) {
            Ok(content) => {
                if let Err(err) = std::fs::write(SERVER_SETTINGS_FILE, content) {
                    warn!("Failed to save server settings: {}", err);
                }
            }
            Err(err) => warn!("Failed to serialize server settings: {}", err),
        }

        let online_snapshots = server_info
            .online_players
            .values()
            .map(|p| (p.username.clone(), p.inventory.clone()))
            .collect::<Vec<_>>();

        for (username, inventory) in online_snapshots {
            server_info
                .saved_inventories
                .insert(username, inventory);
        }

        match serde_json::to_string_pretty(&server_info.saved_inventories) {
            Ok(content) => {
                if let Err(err) = std::fs::write(SERVER_PLAYERDATA_FILE, content) {
                    warn!("Failed to save player data: {}", err);
                }
            }
            Err(err) => warn!("Failed to serialize player data: {}", err),
        }
    }
}

pub mod rcon {
    use super::*;

    #[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
    pub struct Motd {
        pub motd: String,
        pub game_addr: String,
        pub num_player_online: u32,
        pub num_player_limit: u32,
        pub protocol_version: u64,
        pub favicon_url: String,
    }

    #[derive(Resource)]
    pub struct HttpServer {
        pub server: tiny_http::Server,
    }

    pub fn on_http_recv(http: Res<HttpServer>, serv: Res<ServerInfo>, cfg: Res<ServerSettings>) {
        if let Ok(Some(req)) = http.server.try_recv() {
            info!("Req URL: {}", req.url());
            let motd = Motd {
                motd: cfg.motd.clone(),
                num_player_limit: cfg.num_player_limit,
                num_player_online: serv.online_players.len() as u32,
                protocol_version: 0,
                favicon_url: "".into(),
                game_addr: format!(":{}", cfg.port),
            };
            match serde_json::to_string(&motd) {
                Ok(body) => {
                    if let Err(err) = req.respond(tiny_http::Response::from_string(body)) {
                        warn!("Failed to respond RCON request: {}", err);
                    }
                }
                Err(err) => warn!("Failed to serialize RCON motd: {}", err),
            }
        }
    }
}

#[derive(Resource, serde::Deserialize, serde::Serialize, Asset, TypePath, Clone)]
pub struct ServerSettings {
    pub port: u16,
    pub num_player_limit: u32,
    pub motd: String,
}

impl Default for ServerSettings {
    fn default() -> Self {
        Self {
            port: 4060,
            num_player_limit: 80,
            motd: "An Ethertum Server".into(),
        }
    }
}

#[derive(Resource, Default)]
pub struct ServerInfo {
    // PlayerList
    pub online_players: HashMap<ClientId, PlayerInfo>,
    pub saved_inventories: HashMap<String, Vec<NetItemStack>>,
}

pub struct PlayerInfo {
    pub username: String,
    pub user_id: u64,

    pub client_id: ClientId, // network client id. renet

    pub entity_id: EntityId,
    pub position: Vec3,
    pub ping_rtt: u32,

    pub chunks_load_distance: IVec2,

    pub chunks_loaded: HashSet<IVec3>,

    pub inventory: Vec<NetItemStack>,
}

impl PlayerInfo {
    // fn update(&self) {
    // }
}
