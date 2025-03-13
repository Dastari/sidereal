# Synodic - Sidereal Universe Map

An interactive WebGL-based map of the Sidereal universe that displays entities in a top-down cartography style. The map allows for infinite scrolling and zooming, similar to Google Maps.

## Features

- WebGL rendering using Three.js
- Real-time data fetching from the Sidereal backend
- Support for infinite scrolling in any direction
- Google Maps-style zooming and panning
- Sector grid visualization with 1000x1000 sized sectors
- Entity visualization (ships appear as cyan triangles)
- Sector information on hover
- Interactive entity sidebar with search functionality
- Focus on entities by clicking them in the sidebar
- Efficient grid rendering that only displays visible sectors

## Prerequisites

- Node.js (v14+)
- npm or yarn

## Installation

1. Clone or download this repository
2. Navigate to the project directory
3. Install dependencies:

```bash
npm install
# or
yarn install
```

## Running the Development Server

To start the development server:

```bash
npm run dev
# or
yarn dev
```

This will start the development server at `http://localhost:3000`.

## Building for Production

To build the application for production:

```bash
npm run build
# or
yarn build
```

The built files will be in the `dist` directory.

## How to Use

- **Panning**: Click and drag to pan around the map
- **Zooming**: Use the mouse wheel to zoom in and out
- **Entity Information**: Use the sidebar on the right to see all entities
- **Search**: Type in the search box to filter entities by name or type
- **Focus**: Click on an entity in the sidebar to center the map on it
- **Sector Info**: Hover over a sector to see its coordinates

## Backend Requirements

The application expects a Sidereal backend server to be running at `http://localhost:15702`.

## Configuration

The API endpoint can be modified in the `src/main.ts` file if needed.

## License

See the LICENSE file for details.
