# Sidereal Explorer

A graph exploration dashboard for the Sidereal game world. Visualize entities on a coordinate grid, explore graph relationships from Apache AGE, and inspect components from the Bevy ECS.

## Features

- **Coordinate Grid** - Entities displayed at their world coordinates on a zoomable, pannable WebGL grid with multi-level subdivision
- **Graph Exploration** - Click entities to select, double-click to expand and reveal connected nodes from the AGE graph
- **Entity Tree** - Sidebar navigation organized by entity type
- **Detail Panel** - Inspect properties and components of selected entities
- **Theme Support** - Dark and light modes with smooth transitions
- **Real-time Updates** - Auto-refreshes data every 5 seconds

## Controls

- **Scroll** - Zoom in/out (centered on cursor)
- **Drag** - Pan the viewport
- **Click** - Select an entity
- **Double-click** - Expand entity to show connected nodes
- **Click again** - Clicking a selected entity also expands it

## Tech Stack

- React 19 with TanStack Router
- Tailwind CSS v4 with custom theming
- shadcn/ui components (Radix primitives)
- WebGL2 for grid and entity rendering
- Apache AGE for graph storage
- PostgreSQL for persistence

## Development

```bash
# Install dependencies
pnpm install

# Start development server
pnpm dev

# Type check
pnpm exec tsc --noEmit

# Lint and format
pnpm check

# Build for production
pnpm build
```

## Environment Variables

| Variable                         | Default                         | Description                                  |
| -------------------------------- | ------------------------------- | -------------------------------------------- |
| `REPLICATION_DATABASE_URL`       | unset                           | Full DB URL used by replication/runtime      |
| `DATABASE_URL`                   | unset                           | Alternate full DB URL                        |
| `PGHOST`                         | `127.0.0.1`                     | PostgreSQL host (fallback when URL not set)  |
| `PGPORT`                         | `5432`                          | PostgreSQL port (fallback when URL not set)  |
| `PGDATABASE`                     | `sidereal`                      | Database name (fallback when URL not set)    |
| `PGUSER`                         | `sidereal`                      | Database user (fallback when URL not set)    |
| `PGPASSWORD`                     | `sidereal`                      | Database password (fallback when URL not set) |
| `GRAPH_NAME`                     | `sidereal`                      | AGE graph name                               |
| `REPLICATION_BRP_URL`            | `http://127.0.0.1:15713/`       | Server BRP endpoint                          |
| `CLIENT_BRP_URL`                 | `http://127.0.0.1:15714/`       | Client BRP endpoint                          |
| `SIDEREAL_REPLICATION_BRP_AUTH_TOKEN` | unset                    | Optional auth token for server BRP           |
| `SIDEREAL_CLIENT_BRP_AUTH_TOKEN` | unset                           | Optional auth token for client BRP           |

## API Endpoints

- `GET /api/graph` - Returns all nodes and edges from the AGE graph
- `GET /api/world` - Returns world entities with positions and component counts
- `GET /api/live-world` - Returns live entities from server BRP
- `GET /api/live-client-world` - Returns live entities from client BRP

## Project Structure

```
src/
├── components/
│   ├── grid/           # WebGL grid canvas and rendering
│   ├── layout/         # App layout components
│   ├── sidebar/        # Entity tree, detail panel, toolbar
│   └── ui/             # shadcn/ui primitives
├── hooks/              # Custom React hooks
├── lib/                # Utility functions
└── routes/             # TanStack Router routes and API handlers
```
