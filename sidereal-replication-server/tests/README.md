# Sidereal Replication Server Tests

This directory contains tests for the Sidereal Replication Server, focusing on the functionality related to database interactions, physics components, and scene loading.

## Test Categories

### 1. Mock Database Tests (`mock_database.rs`)

- Provides a mock implementation of the `DatabaseClient` for testing
- Tests CRUD operations on entity records
- Simulates error conditions like not found entities

### 2. Database Tests (`database_tests.rs`)

- Tests database client creation and configuration
- Tests entity record serialization and deserialization
- Tests accessing physics data from JSON
- Tests error handling for database operations

### 3. Loader Tests (`loader_tests.rs`)

- Tests the scene loader plugin initialization
- Tests scene state transitions
- Tests loading entities from the database
- Tests physics data extraction from entity records
- Tests error handling during scene loading

### 4. Physics Tests (`physics_tests.rs`)

- Tests creating `PhysicsData` objects
- Tests converting Bevy/Rapier components to `PhysicsData`
- Tests applying `PhysicsData` to entities
- Tests JSON serialization/deserialization roundtrip
- Tests different collider shapes (ball, cuboid, capsule)

### 5. Performance Tests (`performance_tests.rs`)

- Benchmarks serialization of physics data
- Benchmarks deserialization of physics JSON
- Benchmarks entity creation with physics components
- Benchmarks complete roundtrip (record -> entity -> record)

## Running Tests

### Run all tests

```bash
cd sidereal-replication-server
cargo test
```

### Run a specific test file

```bash
cargo test --test database_tests
cargo test --test loader_tests
cargo test --test physics_tests
cargo test --test performance_tests
```

### Run a specific test

```bash
cargo test --test database_tests -- test_entity_record_serialization
```

## Test Environment

These tests use:

- A minimal Bevy application setup with only the necessary plugins
- The Rapier physics engine with the `serde-serialize` feature enabled
- A mock database client to simulate database interactions

## Adding New Tests

When adding new tests:

1. Create a new test module if testing a new domain
2. Use the mock database client for database tests
3. Keep performance tests separate from correctness tests
4. For Bevy tests, use `App::new()` with minimal plugins
5. Make sure to clean up resources after tests (especially for async tests)

## Notes on Performance Tests

The performance tests are included to benchmark the efficiency of physics data operations. They can be run with:

```bash
cargo test --test performance_tests -- --nocapture
```

This will show the timing information in the console output. Note that results will vary based on your system's hardware.
