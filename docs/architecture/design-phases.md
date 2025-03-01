[← Back to Documentation Index](../README.md) | [Architecture Documentation](./networking-overview.md)

# Sidereal Development Phases

This document outlines the planned development phases for the Sidereal project, detailing the key features and components that will be implemented in each phase.

## Phase 1: Core Engine and Single-Shard Architecture

The foundation of the game, focusing on core mechanics and single-shard server architecture.

### World Generation and Management

- ✅ Initial universe generation with basic sector grid
- ✅ Simple entity placement and spatial partitioning
- ✅ Basic physics simulation with Rapier
- ✅ N-body gravity system for simple celestial mechanics
- ✅ Required Components system for ensuring entities have proper components
- 🔄 Shadow entity framework for cross-boundary entity awareness
- 🔄 Entity serialization and deserialization

### Networking

- ✅ Single replication server connected to single shard server
- ✅ Basic entity replication with bevy_replicon
- ✅ WebSocket server for client connections
- ✅ Simple GraphQL endpoint for game state queries
- 🔄 Event-based communication using Bevy's event system
- ❌ Authentication system (Basic)

### Database Integration

- ✅ Schema design for game entities and state
- ✅ Connection to Supabase
- 🔄 Entity serialization for persistence
- ❌ Basic save/load functionality

### Clients

- ✅ Basic web client with BabylonJS
- ✅ Camera controls and entity rendering
- ❌ Ship controls and basic UI
- ❌ Simple chat system

## Phase 2: Multi-Shard Architecture

Expanding the game to support multiple shard servers and improving the distributed architecture.

### World Management

- 🔄 Cluster-based world partitioning
- 🔄 Entity transition between shards
- ❌ Dynamic cluster assignment to shards
- 🔄 Cross-shard entity transfer
- 🔄 Empty sector timeout and resource management
- ❌ Advanced physics simulation

### Networking

- 🔄 Multiple shard servers connected to replication server
- 🔄 Hybrid communication for cross-shard awareness:
  - Direct shard-to-shard for boundary entities
  - Replication server mediated for coordination
- ❌ Improved authentication and authorization
- ❌ WebSocket server for client connections
- ❌ Enhanced GraphQL API

### Database

- 🔄 Regular state persistence
- 🔄 Player account data
- ❌ Inactive sector storage and retrieval
- ❌ Analytics and monitoring data

### Clients

- ❌ Improved web client with better visuals
- ❌ Ship customization and loadouts
- ❌ Enhanced UI with game status information
- ❌ Mobile-responsive design

## Phase 3: Gameplay and Content

Adding rich gameplay features and expanding content once the technical foundation is solid.

### Gameplay Systems

- ❌ Resource gathering and processing
- ❌ Trading and economy
- ❌ Missions and objectives
- ❌ Faction system
- ❌ Research and technology progression

### World Expansion

- ❌ Procedural content generation
- ❌ More varied celestial bodies
- ❌ Points of interest and discoveries
- ❌ Environmental hazards and phenomena

### Advanced Networking

- ❌ Optimized entity state synchronization
- ❌ Load balancing between shard servers
- ❌ Seamless client transitions between shards
- ❌ Improved latency handling

### Clients

- ❌ Full-featured web client
- ❌ Native client (optional)
- ❌ Expanded UI with economy, research, etc.
- ❌ Social features: groups, communications

## Phase 4: Scaling and Optimization

Focusing on performance, scalability, and polishing the game for larger player numbers.

### Performance Optimization

- ❌ Enhanced entity filtering for relevance
- ❌ Dynamic LOD system for distant objects
- ❌ Multi-threading for physics simulation
- ❌ GPU acceleration for specific computations

### Scaling

- ❌ Auto-scaling shard servers
- ❌ Database sharding for performance
- ❌ Regional deployment for latency optimization
- ❌ CDN integration for static assets

### Player Experience

- ❌ Onboarding and tutorials
- ❌ Community features
- ❌ Advanced social systems
- ❌ Player-driven governance

## Current Development Focus

The project is currently transitioning between Phase 1 and Phase 2, focusing on:

1. 🔄 **Shadow Entity Framework**: Implementing the system for cross-boundary entity awareness to provide seamless gameplay across shard boundaries. This includes:

   - Shadow entity registration and tracking
   - Position and velocity updates for boundary entities
   - Component serialization for shadow entities

2. 🔄 **Entity Transition Between Shards**: Developing the process for handing off entities between different shard servers, including:

   - Handover coordination through the replication server
   - Entity state serialization and transfer
   - Ownership management during transitions

3. 🔄 **Event-Based Communication**: Standardizing on Bevy's EventWriter/EventReader system for cleaner communication between systems.

4. 🔄 **Database Persistence**: Implementing regular state saving to Supabase database and refining the persistence strategy for different entity types.

## Implementation Milestones

### Milestone 1: Entity Awareness (Current)

- Complete shadow entity framework
- Implement boundary detection system
- Establish entity serialization standards

### Milestone 2: Multi-Shard Foundation

- Implement cross-shard entity transfer
- Establish cluster management
- Develop hybrid communication system

### Milestone 3: Database Integration

- Implement comprehensive persistence strategy
- Deploy regular save intervals
- Develop database schema migration plan

## Key Technical Dependencies

- Bevy 0.15+ (Core ECS framework)
- bevy_replicon (Networking)
- Rapier (Physics)
- BabylonJS (Web client rendering)
- Supabase (Database)
- GraphQL (Query language)
- WebSockets (Real-time communication)
