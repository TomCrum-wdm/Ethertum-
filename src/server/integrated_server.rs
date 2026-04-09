use rand::Rng;

use crate::{net::ServerNetworkPlugin, prelude::*, voxel::ServerVoxelPlugin};

use super::prelude::{ServerInfo, ServerSettings};

pub struct IntegratedServerPlugin;

impl Plugin for IntegratedServerPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ServerInfo::default());
        let mut rng = rand::thread_rng();
        app.insert_resource(ServerSettings {
            port: 6000 + rng.random_range(0..6000),
            local_mode: true,
            ..default()
        });

        // Network
        app.add_plugins(ServerNetworkPlugin);

        // ChunkSystem
        app.add_plugins(ServerVoxelPlugin);
    }
}
