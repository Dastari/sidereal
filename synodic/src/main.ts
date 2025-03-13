import * as THREE from "three";
import { SectorMapRenderer } from "./renderer";
import { SectorCoord } from "./types";
import { DataService } from "./dataService";
import { EntityListSidebar } from "./entityList";

// Initialize the scene
const container = document.body;
const infoElement = document.getElementById("info") as HTMLDivElement;

// Create the renderer
const renderer = new SectorMapRenderer(container);

// Initialize the data service
const dataService = new DataService("http://localhost:15702");

// Create entity list sidebar
const entityList = new EntityListSidebar(container, renderer);

// Store current viewport info
let currentSector: SectorCoord = { x: 0, y: 0 };
let lastUpdateTime = 0;
const UPDATE_INTERVAL = 5000; // Update every 5 seconds

// Start the animation loop
animate();

function animate() {
  requestAnimationFrame(animate);

  const currentTime = Date.now();

  // Update camera position information
  updatePositionInfo();

  // Fetch new data every UPDATE_INTERVAL milliseconds
  if (currentTime - lastUpdateTime > UPDATE_INTERVAL) {
    fetchData();
    lastUpdateTime = currentTime;
  }

  // Render the scene
  renderer.render();
}

function updatePositionInfo() {
  const cameraPosition = renderer.getCameraPosition();
  const zoomLevel = renderer.getZoomLevel();

  // Calculate the current sector
  const sectorSize = 1000;
  currentSector = {
    x: Math.floor(cameraPosition.x / sectorSize),
    y: Math.floor(cameraPosition.y / sectorSize),
  };

  // Update the info display
  infoElement.innerHTML = `
    Sector: (${currentSector.x}, ${currentSector.y})<br>
    Position: (${cameraPosition.x.toFixed(1)}, ${cameraPosition.y.toFixed(
    1
  )})<br>
    Zoom: ${zoomLevel.toFixed(2)}x
  `;
}

async function fetchData() {
  try {
    const entities = await dataService.fetchEntities();
    renderer.updateEntities(entities);
  } catch (error) {
    console.error("Error fetching data:", error);
  }
}
