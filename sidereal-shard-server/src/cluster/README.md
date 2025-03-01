# Cluster Management Module

This module handles the management of game world clusters assigned to a shard server. It provides functionality for tracking entities near cluster boundaries, handling entity transitions between clusters, and coordinating with neighboring shards.

## Components

### ClusterManagerPlugin

The main plugin that initializes resources and registers systems for cluster management.

### ClusterManager

Resource that tracks:

- Clusters assigned to this shard
- Entities transitioning between clusters
- Neighboring clusters and their owning shards

### BoundaryEntityRegistry

Resource that tracks:

- Entities near cluster boundaries
- Foreign entities replicated from neighboring shards

### Systems

- `detect_boundary_entities`: Identifies entities that are near cluster boundaries
- `handle_cluster_transitions`: Manages entities that cross cluster boundaries
- `update_neighboring_clusters`: Updates information about neighboring clusters

## Usage

The cluster management module is automatically initialized when the shard server starts. Cluster assignments are managed through communication with the cluster service (to be implemented).

Entities near cluster boundaries are automatically detected and tracked. When an entity crosses a cluster boundary, it is marked as transitioning and the appropriate shard is notified.

## Future Improvements

- Implement communication with the cluster service for cluster assignments
- Add systems for handling entity replication between shards
- Optimize boundary detection for large numbers of entities
