# Sidereal

A massively multiplayer top-down space action role-playing game where players explore a vast 2D universe in their spaceships.

> **⚠️ IMPORTANT NOTE**: This codebase is currently in early development and is a work in progress. It represents an architectural vision and design documentation rather than a working product. Many features described here are planned or partially implemented, but not yet functional. The codebase should be treated as a reference implementation and design document rather than production-ready software.

## Documentation

Detailed documentation can be found in the `/docs` directory:
- [Gameplay Overview](docs/gameplay-overview.md) - Detailed game mechanics and systems
- [Network Architecture](docs/network-architecture.md) - In-depth networking implementation details

## Overview

Sidereal is a browser-based MMO space simulation that combines the depth of complex space games with the accessibility of web-based play. Players can register accounts, create characters, and explore an expansive 2D universe. The game features realistic physics, dynamic ship construction, complex economic systems, and a distributed server architecture supporting large numbers of concurrent players.

### Key Features

- **Dynamic Ship Construction**: Grid-based hull system with directional components
- **Realistic Physics**: Newtonian movement with thrust, inertia, and gravity simulation
- **Complex Economy**: Player-driven markets, resource gathering, and production chains
- **Vast Universe**: Multiple regions from safe central systems to dangerous frontier space
- **Social Systems**: Guilds, corporations, and alliance networks
- **Progression Systems**: Character skills, technology research, and reputation systems

## Game Systems

### Ship & Station Building
- Grid-based construction with directional component placement
- Dynamic ship stats based on installed components
- Advanced physics affecting ship handling characteristics
- Multiple hull sizes and specialized component types
- Power management and heat distribution systems

### Combat & Exploration
- Real-time tactical combat with skill-based aiming
- Diverse weapon systems and shield management
- Electronic warfare and boarding mechanics
- Exploration mechanics with rewards for discovering new areas
- Environmental hazards including black holes, nebulae, and ion storms

### Economy & Progression
- Resource gathering and processing
- Player-driven markets with dynamic pricing
- Contract and mission systems
- Character skills and technology research
- Reputation and faction alignment systems

## Architecture

The Sidereal project consists of several interconnected components:

### Components

1. **Replication Server** (`/replication-server`)

   - Loads and persists universe state to Supabase
   - Divides universe into sectors dynamically
   - Manages shard server connections
   - Handles entity updates and boundary crossings
   - Communicates with web clients via UDP/TCP hybrid protocol
   - Provides real-time state synchronization
   - Uses bevy_replicon for efficient entity replication

2. **Shard Servers** (`/shard-server`)

   - Multiple instances for distributed processing
   - Each shard manages one or more 1000×1000 unit sectors
   - Process game logic and physics simulation
   - Send real-time entity updates to the replication server
   - Report system stats to the replication server
   - Uses Avian2D for physics calculations

3. **Sidereal Core** (`/sidereal`)

   - Shared codebase between server components
   - Contains Bevy ECS Components, Entities, Plugins and Systems
   - Handles physics using Avian2D
   - Implements core game mechanics and systems

4. **Authentication Server** (`/auth-server`)

   - Processes login requests
   - Generates authentication tokens
   - Verifies user connections

5. **Supabase Database**

   - Stores entity data and universe state
   - Manages user accounts and authentication

6. **Web Client**
   - Built with React, BabylonJS, and Vite
   - Provides the game interface for players

7. **Synodic Inspector** (`/synodic`)
   - Web-based debugging interface for bevy_remote protocol
   - Real-time inspection of ECS world state
   - Monitors entity and component changes
   - Visualizes server-client communication
   - Assists in development and debugging

## Technical Stack

- **Backend**: Rust with Bevy 0.15 ECS
- **Database**: Supabase (PostgreSQL)
- **Frontend**: React, BabylonJS, Vite
- **Networking**: UDP/TCP hybrid using Renet
- **Key Dependencies**:
  - Bevy 0.15+
  - bevy_replicon 0.30+
  - bevy_replicon_renet2 0.3+
  - bevy_renet2 0.3+
  - Serde 1.0.218+
  - Tokio 1.43.0+
  - Avian2D physics engine

## Network Architecture

The game uses a hybrid UDP/TCP networking strategy:
- UDP for high-frequency entity state updates
- TCP (or reliable UDP) for critical game events
- Efficient batching and compression of network messages
- Entity change detection and delta updates
- Interest management for bandwidth optimization
- Client-side prediction and lag compensation

## Development Guidelines

- Code is modularized into separate mods based on related concerns
- Consistent technology stack across all Rust workspaces
- Multithreading implemented through Tokio
- Server-to-server communication via bevy_replicon and bevy_replicon_renet2
- Containerized deployment using Docker
- Scalable architecture supporting dynamic shard allocation

## Getting Started

### Prerequisites

- Rust (latest stable version)
- Node.js and npm
- Docker (for local Supabase)

### Setup

1. Clone the repository
2. Set up the local Supabase instance
3. Configure environment variables
4. Build and run the servers:
   ```bash
   cargo build --release --workspace
   ```
