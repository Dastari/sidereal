use bevy::prelude::*;
use bevy_renet2::{netcode::{ClientAuthentication, NetcodeClientTransport}, prelude::RenetClient};
use sidereal::net::config::{DEFAULT_PROTOCOL_ID, DEFAULT_RENET2_PORT, create_connection_config};
use std::{
    error::Error,
    net::{SocketAddr, UdpSocket},
    time::{SystemTime, UNIX_EPOCH},
};
use tracing::{debug, error, info, warn};
use uuid::Uuid;


#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ClientState {
    #[default]
    Disconnected,
    Connecting,
    Connected,
    Error,
}
#[derive(Resource, Debug, Clone)]
pub struct Renet2ClientConfig {
    pub bind_addr: SocketAddr,
    pub server_addr: SocketAddr,
    pub shard_id: Uuid,
    pub protocol_id: u64,
}

impl Default for Renet2ClientConfig {
    fn default() -> Self {
        Self {
            bind_addr: "127.0.0.1:0".parse().expect("Invalid default bind address"),
            server_addr: format!("127.0.0.1:{}", DEFAULT_RENET2_PORT)
                .parse()
                .expect("Invalid default server address"),
            shard_id: Uuid::new_v4(),
            protocol_id: DEFAULT_PROTOCOL_ID,
        }
    }
}

#[derive(Resource)]
pub struct Renet2ClientListener {
    pub client: RenetClient,
    pub transport: NetcodeClientTransport,
}

pub struct Renet2ClientPlugin {
    config: Renet2ClientConfig,
    tracking_enabled: bool,
}

impl Default for Renet2ClientPlugin {
    fn default() -> Self {
        Self {
            config: Renet2ClientConfig::default(),
            tracking_enabled: true,
        }
    }
}

impl Renet2ClientPlugin {
    pub fn with_config(config: Renet2ClientConfig) -> Self {
        Self {
            config,
            tracking_enabled: true,
        }
    }

    pub fn with_tracking(mut self, enabled: bool) -> Self {
        self.tracking_enabled = enabled;
        self
    }
}

impl Plugin for Renet2ClientPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(self.config.clone());
        app.init_state::<ClientState>();
        app.add_systems(Startup, init_client_system);
        app.add_systems(
            Update,
            client_update.run_if(resource_exists::<Renet2ClientListener>),
        );

        if self.tracking_enabled {
            app.add_systems(Update, log_client_status.run_if(state_changed::<ClientState>));
        }

        info!("Renet2 client plugin initialized");
    }
}

fn init_client_system(world: &mut World) {
    if let Err(e) = init_renet2_client(world) {
        warn!("Failed to initialize shard client: {}", e);
    } else {
        info!("Initialized shard client for renet2 connection");
    }
}

fn init_renet2_client(world: &mut World) -> Result<(), Box<dyn Error>> {
    let server_addr = {
        let config = world.resource::<Renet2ClientConfig>();
        config.server_addr
    };

    let socket = UdpSocket::bind(world.resource::<Renet2ClientConfig>().bind_addr)?;
    let current_time = SystemTime::now().duration_since(UNIX_EPOCH)?;
    let client_id = world.resource::<Renet2ClientConfig>().shard_id.as_u128() as u64;
    let protocol_id = world.resource::<Renet2ClientConfig>().protocol_id;

    let connection_config = create_connection_config();
    let client = RenetClient::new(connection_config, false);

    let authentication = ClientAuthentication::Unsecure {
        client_id,
        protocol_id,
        server_addr,
        user_data: None,
        socket_id: 0,
    };
    
    let socket = bevy_renet2::netcode::NativeSocket::new(socket)?;
    let transport = NetcodeClientTransport::new(current_time, authentication, socket)?;

    // Insert resources separately
    world.insert_resource(Renet2ClientListener { client, transport });

    info!("Shard client initialized connecting to {}", server_addr);

    Ok(())
}

fn log_client_status(
    client_state: Res<State<ClientState>>,
) {
    info!("Client state changed: {:?}", client_state);
}

fn client_update(mut listener: ResMut<Renet2ClientListener>, time: Res<Time>, mut client_state: ResMut<NextState<ClientState>>,) {
    let Renet2ClientListener { client, transport } = listener.as_mut();
    client.update(time.delta());

    if client.is_connected() {
        client_state.set(ClientState::Connected);
    } else {
        client_state.set(ClientState::Disconnected);
    }

    if let Err(e) = transport.send_packets(client) {
        error!("Failed to send packets: {:?}", e);
    }

    if let Err(e) = transport.update(time.delta(), client) {
        error!("Client transport update error: {:?}", e);
    }
}
