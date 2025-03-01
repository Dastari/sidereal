# Sidereal

A massively multiplayer top-down space action role-playing game where players explore a vast 2D universe in their spaceships.

## Overview

Sidereal allows players to connect via web browsers, register accounts, create characters, and explore an expansive 2D space universe. The game features a distributed server architecture for handling large numbers of concurrent players and a dynamic universe.

## Architecture

The Sidereal project consists of several interconnected components:

```
┌─────────────────┐     ┌────────────────┐     ┌─────────────────┐
│                 │     │                │     │                 │
│  Web Clients    │◄───►│  Auth Server   │◄───►│  Supabase DB    │
│  (React/Babylon)│     │                │     │                 │
│                 │     └────────────────┘     └─────────────────┘
└────────┬────────┘               ▲                     ▲
         │                        │                     │
         ▼                        │                     │
┌─────────────────────────────────────────┐             │
│                                         │             │
│  Replication Server                     │             │
│  ┌─────────────────────────────────┐    │             │
│  │ Sidereal Core (Shared Library)  │    │◄────────────┘
│  └─────────────────────────────────┘    │
│                                         │
└───────────────────┬─────────────────────┘
                    │
                    │
                    ▼
    ┌───────────────────────────────────┐
    │                                   │
    ▼                                   ▼
┌─────────────────────────────┐  ┌─────────────────────────────┐
│                             │  │                             │
│  Shard Server Instance 1    │◄►│  Shard Server Instance N    │
│  ┌─────────────────────┐    │  │  ┌─────────────────────┐    │
│  │ Sidereal Core       │    │  │  │ Sidereal Core       │    │
│  │ (Shared Library)    │    │  │  │ (Shared Library)    │    │
│  └─────────────────────┘    │  │  └─────────────────────┘    │
│                             │  │                             │
└─────────────────────────────┘  └─────────────────────────────┘
```

### Components

1. **Replication Server** (`/sidereal-replication-server`)

   - Loads and persists universe state to Supabase
   - Divides universe into sectors dynamically
   - Manages shard server connections
   - Handles entity updates and boundary crossings
   - Communicates with web clients via websockets
   - Provides GraphQL API for universe queries

2. **Shard Servers** (`/sidereal-shard-server`)

   - Multiple instances for distributed processing
   - Register with replication server
   - Process game logic and physics
   - Send real-time entity updates to the replication server
   - Report system stats to the replication server

3. **Sidereal Core** (`/sidereal-core`)

   - Shared codebase between server components
   - Contains Bevy ECS Components, Entities, Plugins and Systems
   - Handles physics using bevy_rapier

4. **Authentication Server** (`/sidereal-auth-server`)

   - Processes login requests
   - Generates authentication tokens
   - Verifies user connections

5. **Supabase Database**

   - Stores entity data and universe state
   - Manages user accounts and authentication

6. **Web Client**
   - Built with React, BabylonJS, and Vite
   - Provides the game interface for players

## Technical Stack

- **Backend**: Rust with Bevy 0.15 ECS
- **Database**: Supabase (PostgreSQL)
- **Frontend**: React, BabylonJS, Vite
- **Key Dependencies**:
  - Bevy 0.15+
  - bevy_replicon 0.30+
  - bevy_replicon_renet2 0.3+
  - bevy_renet2 0.3+
  - Serde 1.0.218+
  - Tokio 1.43.0+
  - Crossbeam channel for message passing

## Development Guidelines

- Code is modularized into separate mods based on related concerns
- Consistent technology stack across all Rust workspaces
- Multithreading implemented through Tokio
- Server-to-server communication via bevy_replicon and bevy_replicon_renet2

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

## License

[Add your license information here]

## Contributors

[Add contributor information here]
