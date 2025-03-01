[← Back to Documentation Index](README.md)

# Getting Started with Sidereal

This guide will help you get up and running with the Sidereal project.

## Prerequisites

Before you begin, ensure you have the following installed:

- Rust (latest stable version)
- Node.js and npm
- Docker (for local Supabase)

## Quick Setup

1. Clone the repository:

   ```bash
   git clone https://github.com/yourusername/sidereal.git
   cd sidereal
   ```

2. Set up environment variables:

   ```bash
   cp .env.example .env
   # Edit .env with your configuration
   ```

3. Start the Supabase local instance:

   ```bash
   # Command to start Supabase locally
   ```

4. Build the project:

   ```bash
   cargo build --release --workspace
   ```

5. Run the servers:
   ```bash
   # Start the servers in the correct order
   ./target/release/sidereal-auth-server
   ./target/release/sidereal-replication-server
   ./target/release/sidereal-shard-server
   ```

## Next Steps

Once you have the servers running:

- Visit the [Architecture Documentation](architecture/networking-overview.md) to understand the system
- Read about the [Game World Partitioning](architecture/game-world.md) to understand how the universe works
- Check out the [Gameplay Overview](gameplay/gameplay-overview.md) to understand the game mechanics
- Explore the [Design Phases](gameplay/design-phases.md) to see the project roadmap

## Troubleshooting

If you encounter issues during setup:

1. Check the server logs for specific error messages
2. Ensure all required ports are available
3. Verify your environment variables are correctly set

For more help, please [open an issue](https://github.com/yourusername/sidereal/issues) on GitHub.
