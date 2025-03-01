fn handle_shard_connection(
    mut commands: Commands,
    mut connection_events: EventReader<ShardServerConnected>,
    // other parameters...
) {
    for event in connection_events.read() {
        info!("Shard server connected! ID: {}, Address: {}", event.shard_id, event.address);
        
        // When assigning clusters to the shard
        info!("Assigning {} clusters to shard {}", clusters_to_assign.len(), event.shard_id);
        for cluster in &clusters_to_assign {
            info!("  - Cluster at coordinates {:?} with {} entities", 
                  cluster.base_coordinates, 
                  cluster.entity_count);
        }
        
        // When sending entity data
        info!("Sending {} entities to shard {}", entities_to_send.len(), event.shard_id);
        
        // ... existing assignment code ...
    }
} 