[← Back to Documentation Index](../README.md) | [Gameplay Overview](./gameplay-overview.md) | [Networking Overview](../architecture/networking-overview.md)

# Sidereal: Design & Development Phases

## Overview

This document outlines the phased approach to developing Sidereal, focusing on logical stages of implementation that build upon each other. Each phase has specific goals and requirements, taking into account the technical architecture and gameplay vision.

## Implementation Status Legend

- ✅ Completed
- 🔄 In Progress
- ❌ Not Started

## Phase 0: Foundation & Architecture Setup

**Duration Estimate:** 2-3 months  
**Focus:** Establishing core technical architecture
**Status:** ✅ Completed

### Technical Goals

- ✅ Set up workspace and project structure
- ✅ Implement basic Bevy ECS framework across all server components
- ✅ Establish CI/CD pipelines
- ✅ Create initial database schema in Supabase
- ✅ Implement basic logging and monitoring

### Components to Develop

- ✅ `sidereal-core`: Initial shared code framework
- ✅ `sidereal-replication-server`: Basic server skeleton
- ✅ `sidereal-shard-server`: Basic server skeleton
- ✅ `sidereal-auth-server`: Basic authentication endpoints

### Deliverables

- ✅ Working development environment
- ✅ Basic server startup and shutdown
- ✅ Initial database connectivity
- ✅ Authentication flow with token generation
- ✅ Technical documentation

### Dependencies

- None (initial phase)

## Phase 1: Basic Networked Universe

**Duration Estimate:** 3-4 months  
**Focus:** Creating a minimal universe with basic movement and visualization
**Status:** 🔄 In Progress, Implement Phase 2

### Technical Goals

- ✅ Implement WebSocket connections between client and replication server
- ✅ Establish bevy_replicon connectivity between replication and shard servers
- ✅ Create basic entity replication system
- 🔄 Implement basic login flow and player session management
- ✅ Develop minimal physics system in shard servers

### Gameplay Features

- ✅ Simple ship representation and movement
- ✅ Basic top-down 2D space environment
- ✅ Single player character entity
- ✅ Primitive universe boundaries
- ✅ Basic collision detection

### Components to Develop

- ✅ `sidereal-web-client`: Initial implementation with login and basic rendering
- ✅ Enhanced communication protocols in existing server components
- 🔄 Basic player entity management in shard servers

### Current Development Focus

1. 🔄 **Shadow Entity Framework**: Implementing the system for cross-boundary entity awareness to provide seamless gameplay across shard boundaries:

   - Shadow entity registration and tracking
   - Position and velocity updates for boundary entities
   - Component serialization for shadow entities

2. 🔄 **Entity Transition Between Shards**: Developing the process for handing off entities between different shard servers:

   - Handover coordination through the replication server
   - Entity state serialization and transfer
   - Ownership management during transitions

3. 🔄 **Event-Based Communication**: Standardizing on Bevy's EventWriter/EventReader system for cleaner communication between systems.

4. 🔄 **Database Persistence**: Implementing regular state saving to Supabase database and refining the persistence strategy for different entity types.

### Deliverables

- ✅ Players can create an account and log in
- ✅ Players can spawn with a simple default ship
- ✅ Basic movement in a small contained region
- ✅ Multiple players can see each other moving
- 🔄 Simple persistence of player position

### Dependencies

- ✅ Functional auth server from Phase 0
- ✅ Supabase integration from Phase 0

## Phase 2: Ship Systems & Enhanced Physics

**Duration Estimate:** 3-4 months  
**Focus:** Ship customization and improved physics
**Status:** 🔄 Partial implementation started

### Technical Goals

- 🔄 Implement component-based ship system
- ✅ Enhance physics simulation with gravity and inertia
- 🔄 Create entity component serialization and persistence
- 🔄 Implement basic cross-shard entity transfer
- 🔄 Enhance WebSocket performance for real-time updates

### Gameplay Features

- 🔄 Basic grid-based ship construction
- 🔄 Simple components (engines, weapons, shields)
- ✅ Improved movement physics with inertia
- ✅ Gravity wells around celestial objects
- ❌ Simple combat mechanics with hit detection

### Components to Enhance

- 🔄 `sidereal-core`: Add ship component definitions and physics models
- 🔄 `sidereal-shard-server`: Implement enhanced physics simulation
- 🔄 `sidereal-replication-server`: Add component state synchronization
- ❌ `sidereal-web-client`: Add ship customization interface

### Deliverables

- 🔄 Players can customize their ship with basic components
- ✅ Physics-based movement with realistic inertia
- ❌ Simple weapons that can target and affect other ships
- ❌ Basic shield mechanics
- 🔄 Persistence of ship configurations

### Dependencies

- ✅ Networked universe from Phase 1
- ✅ Entity replication system from Phase 1

## Phase 3: Universe Expansion & Resource Systems

**Duration Estimate:** 4-5 months  
**Focus:** Creating a larger universe with resources and basic economic activities
**Status:** ❌ Not Started

### Technical Goals

- Implement universe sector/shard management
- Develop procedural celestial object generation
- Create resource entity system
- Implement inventory and cargo systems
- Enhance database schema for economic elements
- Develop shard load balancing

### Gameplay Features

- Expanded universe with multiple sectors
- Basic resource gathering (asteroid mining)
- Simple inventory and cargo management
- Jump points for travel between distant sectors
- Fog of war / sensor system

### Components to Enhance

- `sidereal-core`: Add inventory and resource systems
- `sidereal-shard-server`: Implement sector management and procedural generation
- `sidereal-replication-server`: Enhance cross-shard coordination
- `sidereal-web-client`: Add inventory UI and expanded navigation

### Deliverables

- Multi-sector universe with varied celestial objects
- Resource gathering mechanics
- Basic inventory management
- Long-distance travel via jump points
- Sensor ranges affecting visibility

### Dependencies

- Ship systems from Phase 2
- Enhanced physics from Phase 2
- Cross-shard entity transfer from Phase 2

## Phase 4: Economy & Progression Systems

**Duration Estimate:** 4-5 months  
**Focus:** Trading, crafting, and player progression
**Status:** ❌ Not Started

### Technical Goals

- Implement market data synchronization system
- Create blueprint and research database
- Develop character progression tracking
- Implement NPC faction state management
- Add GraphQL API for complex universe queries

### Gameplay Features

- Trading system with buy/sell interfaces
- Basic crafting/production mechanics
- Character skills and progression
- Technology research
- NPC factions with basic reputation system
- Simple mission system

### Components to Enhance

- `sidereal-core`: Add progression and economic models
- `sidereal-replication-server`: Implement GraphQL API and market synchronization
- `sidereal-auth-server`: Enhance player profile management
- `sidereal-web-client`: Add economic and progression interfaces

### New Components

- `sidereal-metrics-server`: Initial implementation for economic tracking

### Deliverables

- Working economy with supply and demand
- Trading posts where players can buy and sell goods
- Skills that players can develop over time
- Research system to unlock new technologies
- Basic reputation system with NPC factions
- Simple missions from NPCs

### Dependencies

- Resource systems from Phase 3
- Multi-sector universe from Phase 3
- Inventory system from Phase 3

## Phase 5: Social & Organization Systems

**Duration Estimate:** 3-4 months  
**Focus:** Player organizations, communication, and coordination
**Status:** ❌ Not Started

### Technical Goals

- Implement guild/corporation data structures
- Create communication channels system
- Develop shared asset management
- Implement friend and reputation tracking
- Enhance security for organizational permissions

### Gameplay Features

- Guild/corporation system
- Text chat channels (global, local, guild)
- Fleet formations for group movement
- Friend list and player reputation tracking
- Shared asset ownership (stations, territories)

### Components to Enhance

- `sidereal-core`: Add organization models
- `sidereal-replication-server`: Implement communication systems
- `sidereal-auth-server`: Add organization permissions
- `sidereal-web-client`: Add guild and communication interfaces

### Deliverables

- Player-created organizations with hierarchy
- Communication system for player coordination
- Fleet mechanics for group activities
- Friend system for finding and tracking other players
- Organization asset ownership

### Dependencies

- Economy systems from Phase 4
- Progression systems from Phase 4

## Phase 6: Advanced Combat & Territory Control

**Duration Estimate:** 4-5 months  
**Focus:** Enhanced combat and territorial gameplay
**Status:** ❌ Not Started

### Technical Goals

- Implement complex damage models
- Create territory state management system
- Develop specialized combat event handling
- Implement boarding and capture mechanics
- Enhance security for PvP interactions

### Gameplay Features

- Component-based damage system
- Electronic warfare capabilities
- PvP flagging system
- Territory control mechanisms
- Boarding actions
- Fleet combat coordination

### Components to Enhance

- `sidereal-core`: Add advanced combat models
- `sidereal-shard-server`: Implement territory control mechanisms
- `sidereal-replication-server`: Enhance combat event handling
- `sidereal-web-client`: Add advanced combat interfaces

### Deliverables

- Tactical combat with component targeting
- Electronic warfare affecting ship systems
- Territory control for organizations
- Safe zones vs. open PvP areas
- Ship boarding and capture mechanics
- Coordinated fleet combat

### Dependencies

- Social systems from Phase 5
- Organization structures from Phase 5

## Phase 7: Advanced Universe & Events

**Duration Estimate:** 4-6 months  
**Focus:** Dynamic universe with events and deeper interactions
**Status:** ❌ Not Started

### Technical Goals

- Implement procedural event system
- Create environmental hazard mechanics
- Develop advanced wormhole/anomaly system
- Implement storyline mission framework
- Enhance cross-server event coordination

### Gameplay Features

- Dynamic events (pirate invasions, anomalies)
- Space weather and environmental hazards
- Wormhole exploration to special regions
- Story arcs with connected missions
- Reactive universe based on player actions

### Components to Enhance

- All existing components to support dynamic universe

### New Components

- Event orchestration system

### Deliverables

- Universe that changes based on player and NPC actions
- Environmental challenges that affect gameplay
- Special exploration opportunities via wormholes
- Connected story missions for narrative progression
- Large-scale events affecting multiple sectors

### Dependencies

- Combat systems from Phase 6
- Territory control from Phase 6
- All previous systems

## Phase 8: Polish, Balance & Advanced Features

**Duration Estimate:** Ongoing  
**Focus:** Refinement, optimization, and advanced feature implementation
**Status:** ❌ Not Started

### Technical Goals

- Optimize network performance
- Enhance security features
- Implement advanced analytics
- Explore potential for WebRTC for P2P features
- Consider Redis integration for distributed caching

### Gameplay Features

- User experience improvements
- Advanced customization options
- Specialized ship classes
- Prestige mechanics
- Legacy systems

### Components to Enhance

- All components for optimization and polish

### Deliverables

- Refined gameplay balance
- Enhanced user interfaces
- Improved performance
- Additional customization options
- End-game systems for veteran players

### Dependencies

- All previous phases

## Implementation Milestones

### Current Implementation Focus

The project is currently transitioning between Phase 1 and Phase 2, focusing on:

#### Milestone 1: Entity Awareness (Current)

- 🔄 Complete shadow entity framework
- 🔄 Implement boundary detection system
- 🔄 Establish entity serialization standards

#### Milestone 2: Multi-Shard Foundation

- 🔄 Implement cross-shard entity transfer
- 🔄 Establish cluster management
- 🔄 Develop hybrid communication system

#### Milestone 3: Database Integration

- 🔄 Implement comprehensive persistence strategy
- ❌ Deploy regular save intervals
- ❌ Develop database schema migration plan

## Implementation Risk Factors

### Technical Challenges

- **Cross-Server Synchronization**: Ensuring consistent state across multiple servers
- **Real-Time Performance**: Maintaining low latency for fast-paced gameplay
- **Database Scaling**: Managing growing data requirements
- **Security**: Protecting against cheating and exploitation

### Development Considerations

- **Phased Testing**: Each phase should include appropriate testing strategies
- **Feedback Integration**: Systems for gathering and acting on player feedback
- **Documentation**: Maintaining technical and design documentation
- **Scalability**: Designing systems that can grow with the player base

## Milestone Planning

| Milestone              | Estimated Completion | Key Deliverable                                                        | Status |
| ---------------------- | -------------------- | ---------------------------------------------------------------------- | ------ |
| MVP Launch             | After Phase 3        | Basic playable universe with ship customization and resource gathering | 🔄     |
| Economic Update        | After Phase 4        | Trading, crafting, and progression systems                             | ❌     |
| Social Update          | After Phase 5        | Player organizations and communication systems                         | ❌     |
| Combat Update          | After Phase 6        | Enhanced PvP and territory control                                     | ❌     |
| Dynamic Universe       | After Phase 7        | Reactive world with events and deeper narrative                        | ❌     |
| Continuous Improvement | Ongoing              | Regular updates based on metrics and feedback                          | ❌     |

---

_This phased approach provides a roadmap for development while allowing flexibility to adjust based on technical challenges, player feedback, and emerging opportunities. Phases may overlap or be adjusted as development progresses._
