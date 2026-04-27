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

| Variable                                | Default                   | Description                                                         |
| --------------------------------------- | ------------------------- | ------------------------------------------------------------------- |
| `REPLICATION_DATABASE_URL`              | unset                     | Full DB URL used by replication/runtime                             |
| `DATABASE_URL`                          | unset                     | Alternate full DB URL                                               |
| `PGHOST`                                | `127.0.0.1`               | PostgreSQL host (fallback when URL not set)                         |
| `PGPORT`                                | `5432`                    | PostgreSQL port (fallback when URL not set)                         |
| `PGDATABASE`                            | `sidereal`                | Database name (fallback when URL not set)                           |
| `PGUSER`                                | `sidereal`                | Database user (fallback when URL not set)                           |
| `PGPASSWORD`                            | `sidereal`                | Database password (fallback when URL not set)                       |
| `GRAPH_NAME`                            | `sidereal`                | AGE graph name                                                      |
| `REPLICATION_BRP_URL`                   | `http://127.0.0.1:15713/` | Server BRP endpoint                                                 |
| `CLIENT_BRP_URL`                        | `http://127.0.0.1:15714/` | Client BRP endpoint                                                 |
| `SIDEREAL_REPLICATION_BRP_AUTH_TOKEN`   | unset                     | Optional auth token for server BRP                                  |
| `SIDEREAL_CLIENT_BRP_AUTH_TOKEN`        | unset                     | Optional auth token for client BRP                                  |
| `GATEWAY_API_URL`                       | `http://127.0.0.1:8080`   | Gateway base URL for admin spawn and script-catalog proxies         |
| `SIDEREAL_DASHBOARD_SESSION_SECRET`     | unset                     | Required encryption secret for gateway-backed dashboard session cookie |
| `SIDEREAL_DASHBOARD_ADMIN_BEARER_TOKEN` | unset                     | Legacy fallback bearer token for direct helper/test gateway calls   |
| `SIDEREAL_DASHBOARD_ADMIN_PASSWORD`     | unset                     | Legacy dashboard password, superseded by gateway account auth       |

## API Endpoints

- `GET /api/graph` - Returns all nodes and edges from the AGE graph
- `GET /api/world` - Returns world entities with positions and component counts
- `GET /api/live-world` - Returns live entities from server BRP
- `GET /api/live-client-world` - Returns live entities from client BRP
- `GET /api/dashboard-session` - Returns dashboard admin session/configuration status
- `GET /api/genesis/planets` - Returns the Genesis planet registry catalog from gateway script catalog state
- `POST /api/genesis/planets/:planetId/draft` - Saves a Genesis planet draft and matching registry draft through gateway script catalog APIs
- `POST /api/genesis/planets/:planetId/publish` - Publishes the selected planet draft and registry draft when present
- `DELETE /api/genesis/planets/:planetId/draft` - Discards the selected planet draft and registry draft when present
- `POST /api/dashboard-session` - Exchanges gateway account login/TOTP completion for an HttpOnly dashboard session cookie
- `DELETE /api/dashboard-session` - Clears the current dashboard admin session cookie
- `POST /api/admin/spawn-entity` - Proxies admin spawn requests to gateway

## Admin Mutation Auth

As of 2026-03-12, privileged dashboard mutation routes require an authenticated dashboard admin session. The current interim flow is:

1. Set `SIDEREAL_DASHBOARD_ADMIN_PASSWORD` on the dashboard server.
2. Use the header unlock control in the dashboard UI to create an HttpOnly SameSite=Strict session cookie through `/api/dashboard-session`.
3. Privileged mutation routes reject cross-origin requests and require that admin session cookie.

Password reset requests now return accepted/sent state only. Raw reset tokens are no longer returned to the browser by default.

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
