// sidereal-core/src/ecs/plugins/networking/messages.rs
use bevy::prelude::*;
use std::io::{Read, Write};
use std::net::TcpStream;

use super::{NetworkMessage, SerializedEntityBinary, EntityDelta};

/// Helper trait for sending and receiving network messages
pub trait MessageHandler {
    fn send_message(&mut self, message: &NetworkMessage) -> Result<(), String>;
    fn receive_message(&mut self) -> Result<Option<NetworkMessage>, String>;
}

/// Implementation for TCP streams
impl MessageHandler for TcpStream {
    fn send_message(&mut self, message: &NetworkMessage) -> Result<(), String> {
        // Serialize the message using JSON
        let bytes = serde_json::to_vec(message)
            .map_err(|e| format!("Failed to encode message: {}", e))?;
        
        // Write the message length as a u32 prefix
        let len = (bytes.len() as u32).to_be_bytes();
        self.write_all(&len)
            .map_err(|e| format!("Failed to write message length: {}", e))?;
        
        // Write the message bytes
        self.write_all(&bytes)
            .map_err(|e| format!("Failed to write message: {}", e))?;
        
        Ok(())
    }
    
    fn receive_message(&mut self) -> Result<Option<NetworkMessage>, String> {
        // Read the message length
        let mut len_bytes = [0u8; 4];
        match self.read_exact(&mut len_bytes) {
            Ok(_) => {},
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => return Ok(None),
            Err(e) => return Err(format!("Failed to read message length: {}", e)),
        }
        
        let len = u32::from_be_bytes(len_bytes) as usize;
        
        // Read the message
        let mut bytes = vec![0u8; len];
        self.read_exact(&mut bytes)
            .map_err(|e| format!("Failed to read message: {}", e))?;
        
        // Deserialize the message using JSON
        let message: NetworkMessage = serde_json::from_slice(&bytes)
            .map_err(|e| format!("Failed to decode message: {}", e))?;
        
        Ok(Some(message))
    }
}

/// Resource for outgoing messages
#[derive(Resource, Default)]
pub struct OutgoingMessages {
    pub messages: Vec<NetworkMessage>,
}

/// Resource for incoming messages
#[derive(Resource, Default)]
pub struct IncomingMessages {
    pub messages: Vec<NetworkMessage>,
}

/// System to handle incoming messages
pub fn process_incoming_messages(
    mut incoming: ResMut<IncomingMessages>,
    // Add other resources as needed
) {
    for message in incoming.messages.drain(..) {
        match message {
            NetworkMessage::EntityUpdate { tick: _, updates: _ } => {
                // Process entity updates
                // This would be implemented differently in the replication server vs. shard server
            },
            NetworkMessage::EntityTransfer { entity: _, from_sector: _, to_sector: _ } => {
                // Handle entity transfer between sectors
            },
            // Handle other message types
            _ => {},
        }
    }
}