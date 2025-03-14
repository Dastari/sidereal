import * as THREE from "three";
import { Entity } from "./types";
import { shipFragmentShader } from "./shaders";
import { shipVertexShader } from "./shaders";

export class SectorMapRenderer {
  private scene: THREE.Scene;
  private camera: THREE.OrthographicCamera;
  private renderer: THREE.WebGLRenderer;
  private container: HTMLElement;
  private gridHelper!: THREE.Object3D;
  private sectorGrids: Map<string, THREE.Object3D> = new Map();
  private entities: Map<number, THREE.Object3D> = new Map();
  private raycaster: THREE.Raycaster;
  private mouse: THREE.Vector2;
  private isDragging: boolean = false;
  private lastMousePosition: THREE.Vector2 = new THREE.Vector2();
  private sectorSize: number = 1000;
  private zoom: number = 1;
  private hoveredSector: { x: number; y: number } | null = null;
  private sectorLabel: THREE.Sprite | null = null;

  // Selected entity for focusing
  private selectedEntityId: number | null = null;

  // Constants for camera
  private readonly MIN_ZOOM: number = 0.1;
  private readonly MAX_ZOOM: number = 5;
  private readonly ZOOM_SPEED: number = 0.05;

  // Private variable to track last camera position for sector updates
  private lastCheckedPosition = new THREE.Vector2();

  private sectorsWithEntities: Set<string> = new Set(); // Track sectors containing entities
  private sectorBorders: Map<string, THREE.LineSegments> = new Map(); // Store border objects

  // Store a reference to the raw entity data for re-emitting
  private rawEntities: Entity[] = [];

  // Private variable to track initial render
  private hasInitialRender = false;

  // Store velocity arrows separately to avoid rotation inheritance
  private velocityArrows: Map<number, THREE.Object3D> = new Map();

  // Add a flag to track when camera should follow selected entity
  private followSelectedEntity: boolean = false;

  // Add a map to store entity labels separately
  private entityLabels: Map<number, THREE.Sprite> = new Map();

  // Mini-map properties
  private miniMapEnabled: boolean = true;
  private miniMapSize: number = 200; // Size in pixels
  private miniMapScene: THREE.Scene = new THREE.Scene();
  private miniMapCamera: THREE.OrthographicCamera =
    new THREE.OrthographicCamera(-1, 1, 1, -1, 0.1, 10000); // Default values, will be updated in init
  private miniMapRenderer: THREE.WebGLRenderer = new THREE.WebGLRenderer({
    alpha: true,
  });
  private miniMapContainer: HTMLElement = document.createElement("div");
  private miniMapEntities: Map<string, THREE.Mesh> = new Map(); // Store entities by sector
  private miniMapSectorBorders: Map<string, THREE.LineSegments> = new Map();
  private miniMapViewport: THREE.Mesh = new THREE.Mesh(); // Default mesh, will be properly initialized

  // Ship sprite properties
  private shipSpriteTexture: THREE.Texture | null = null;
  private isShipTextureLoaded: boolean = false;
  private shipTextureWidth: number = 0;
  private shipTextureHeight: number = 0;

  constructor(container: HTMLElement) {
    this.container = container;
    this.scene = new THREE.Scene();
    // Create an orthographic camera with correct initial zoom
    const width = window.innerWidth;
    const height = window.innerHeight;

    // Instead of using a fixed frustumSize, calculate based on zoom
    this.camera = new THREE.OrthographicCamera(
      -width / (2 * this.zoom),
      width / (2 * this.zoom),
      height / (2 * this.zoom),
      -height / (2 * this.zoom),
      0.1,
      10000
    );
    this.camera.position.z = 1000;

    // Create the renderer
    this.renderer = new THREE.WebGLRenderer({
      antialias: true,
      alpha: true,
    });
    this.renderer.setClearColor(0x000000, 0);
    this.renderer.setSize(width, height);
    this.container.appendChild(this.renderer.domElement);

    // Set the default cursor to a pointer (hand)
    this.renderer.domElement.style.cursor = "pointer";

    // Create grid container
    this.gridHelper = new THREE.Object3D();
    this.scene.add(this.gridHelper);

    // Setup raycasting for interaction
    this.raycaster = new THREE.Raycaster();
    this.mouse = new THREE.Vector2();

    // Add event listeners
    window.addEventListener("resize", this.onWindowResize.bind(this));

    // Add controls for panning and zooming
    this.renderer.domElement.addEventListener(
      "mousedown",
      this.onMouseDown.bind(this)
    );
    this.renderer.domElement.addEventListener(
      "mousemove",
      this.onMouseMove.bind(this)
    );
    this.renderer.domElement.addEventListener(
      "mouseup",
      this.onMouseUp.bind(this)
    );
    this.renderer.domElement.addEventListener(
      "wheel",
      this.onMouseWheel.bind(this)
    );

    // Prevent context menu
    this.renderer.domElement.addEventListener("contextmenu", (e) =>
      e.preventDefault()
    );

    // Create sector hover label
    this.createSectorLabel();

    // Initialize mini-map
    this.initializeMiniMap();

    // Load ship sprite texture
    this.loadShipTexture();

    // Initial grid setup
    this.updateVisibleSectors();
  }

  createSectorLabel() {
    // Create a hidden/empty label (not showing anything)
    const canvas = document.createElement("canvas");
    canvas.width = 1;
    canvas.height = 1;

    const texture = new THREE.CanvasTexture(canvas);
    const material = new THREE.SpriteMaterial({
      map: texture,
      transparent: true,
      opacity: 0, // Make fully transparent
    });

    this.sectorLabel = new THREE.Sprite(material);
    this.sectorLabel.visible = false; // Always hidden
    this.scene.add(this.sectorLabel);
  }

  updateSectorLabel(x: number, y: number, worldX: number, worldY: number) {
    // Do nothing - we no longer update the sector label
    return;
  }

  updateVisibleSectors() {
    // Remove old sector grids that are no longer visible
    this.sectorGrids.forEach((grid, key) => {
      this.gridHelper.remove(grid);
    });
    this.sectorGrids.clear();

    // Get visible region
    const cameraPosition = this.getCameraPosition();
    const visibleWidth = window.innerWidth / this.zoom;
    const visibleHeight = window.innerHeight / this.zoom;

    const startX =
      Math.floor((cameraPosition.x - visibleWidth / 2) / this.sectorSize) - 1;
    const endX =
      Math.ceil((cameraPosition.x + visibleWidth / 2) / this.sectorSize) + 1;
    const startY =
      Math.floor((cameraPosition.y - visibleHeight / 2) / this.sectorSize) - 1;
    const endY =
      Math.ceil((cameraPosition.y + visibleHeight / 2) / this.sectorSize) + 1;

    // Add grids for visible sectors
    for (let x = startX; x <= endX; x++) {
      for (let y = startY; y <= endY; y++) {
        const key = `${x},${y}`;

        if (!this.sectorGrids.has(key)) {
          // Create a grid for this sector
          const sectorMesh = this.createSectorGrid(x, y);
          this.gridHelper.add(sectorMesh);
          this.sectorGrids.set(key, sectorMesh);
        }
      }
    }

    // After updating visible sectors, also update borders
    this.updateSectorBorders();
  }

  createSectorGrid(x: number, y: number): THREE.Object3D {
    const group = new THREE.Group();

    // Calculate sector boundaries
    const startX = x * this.sectorSize;
    const startY = y * this.sectorSize;

    // Create the grid lines
    const gridGeometry = new THREE.BufferGeometry();
    const vertices = [];

    // Horizontal lines
    for (let i = 0; i <= 10; i++) {
      const y = startY + (i / 10) * this.sectorSize;
      vertices.push(startX, y, 0);
      vertices.push(startX + this.sectorSize, y, 0);
    }

    // Vertical lines
    for (let i = 0; i <= 10; i++) {
      const x = startX + (i / 10) * this.sectorSize;
      vertices.push(x, startY, 0);
      vertices.push(x, startY + this.sectorSize, 0);
    }

    gridGeometry.setAttribute(
      "position",
      new THREE.Float32BufferAttribute(vertices, 3)
    );

    const gridMaterial = new THREE.LineBasicMaterial({
      color: 0xffffff,
      transparent: true,
      opacity: 0.1,
    });

    const grid = new THREE.LineSegments(gridGeometry, gridMaterial);
    group.add(grid);

    // Store sector coords as user data
    group.userData.sectorX = x;
    group.userData.sectorY = y;

    return group;
  }

  updateEntities(entities: Entity[]) {
    // Store the raw entities for later use
    this.rawEntities = [...entities];

    // Clear the sectors with entities tracking
    this.sectorsWithEntities.clear();

    // Keep track of entities to remove
    const entityIds = new Set(entities.map((e) => e.entity));
    const entitiesToRemove = new Set(
      [...this.entities.keys()].filter((id) => !entityIds.has(id))
    );

    // Remove entities that are no longer present
    entitiesToRemove.forEach((id) => {
      const object = this.entities.get(id);
      if (object) {
        this.scene.remove(object);
        this.entities.delete(id);
      }

      // Also remove their velocity arrows
      const arrow = this.velocityArrows.get(id);
      if (arrow) {
        this.scene.remove(arrow);
        this.velocityArrows.delete(id);
      }

      // Remove their labels
      const label = this.entityLabels.get(id);
      if (label) {
        this.scene.remove(label);
        this.entityLabels.delete(id);
      }
    });

    // Update or add entities
    entities.forEach((entity) => {
      const id = entity.entity;
      const transform =
        entity.components["bevy_transform::components::transform::Transform"];
      const objectType =
        entity.components["sidereal_core::ecs::components::object::Object"];
      const name = entity.components["bevy_core::name::Name"].name;

      // Extract velocity if it exists
      const linearVelocity =
        entity.components["avian2d::dynamics::rigid_body::LinearVelocity"];

      // Position from server
      const position = new THREE.Vector3(
        transform.translation[0],
        transform.translation[1],
        0
      );

      // Extract rotation quaternion
      const rotation = new THREE.Quaternion(
        transform.rotation[0],
        transform.rotation[1],
        transform.rotation[2],
        transform.rotation[3]
      );

      // Track which sector this entity belongs to
      const sectorX = Math.floor(position.x / this.sectorSize);
      const sectorY = Math.floor(position.y / this.sectorSize);
      const sectorKey = `${sectorX},${sectorY}`;
      this.sectorsWithEntities.add(sectorKey);

      let object: THREE.Object3D;

      // Check if entity already exists
      if (this.entities.has(id)) {
        object = this.entities.get(id)!;

        // Check if we need to update the representation based on zoom level
        const shouldUseTexture =
          this.zoom > 1.0 && objectType === "Ship" && this.isShipTextureLoaded;
        const isCurrentlyTexturedPlane =
          object.userData.isTexturedPlane === true;
        const isCurrentlySprite = object.userData.isSprite === true;

        // If zoom level crossed the threshold, recreate the object
        if (
          shouldUseTexture !== (isCurrentlyTexturedPlane || isCurrentlySprite)
        ) {
          // Remove the old object
          this.scene.remove(object);

          // Create a new representation based on the current zoom level
          object = this.createEntityObject(objectType, id, name);
          this.scene.add(object);
          this.entities.set(id, object);
        }

        // Update position
        object.position.copy(position);
        object.quaternion.copy(rotation);
      } else {
        // Create new entity
        object = this.createEntityObject(objectType, id, name);
        object.position.copy(position);

        // Apply rotation with special handling for textured ships
        if (object.userData.isTexturedPlane && objectType === "Ship") {
          // Apply entity's rotation to the main group
          object.quaternion.copy(rotation);

          // Update direction indicator
          this.updateDirectionIndicator(object, rotation);
        } else {
          // For other objects, apply quaternion directly
          object.quaternion.copy(rotation);
        }

        this.scene.add(object);
        this.entities.set(id, object);

        // Add name label with dynamic sizing
        const canvas = this.createTextCanvas(name);
        if (canvas) {
          const texture = new THREE.CanvasTexture(canvas);
          const spriteMaterial = new THREE.SpriteMaterial({
            map: texture,
            transparent: true,
          });
          const sprite = new THREE.Sprite(spriteMaterial);

          // Set the sprite position to be above the entity
          sprite.position.set(position.x, position.y + 30, position.z);

          // Scale based on the canvas dimensions to maintain proportions
          const scaleX = canvas.width / 10;
          const scaleY = canvas.height / 10;
          sprite.scale.set(scaleX, scaleY, 1);

          // Add to scene independently and track it
          this.scene.add(sprite);
          this.entityLabels.set(id, sprite);
        }
      }

      // Update velocity arrow as a separate object (not affected by entity rotation)
      this.updateVelocityArrow(id, position, linearVelocity);

      // For entities that already exist, update their label positions
      if (this.entityLabels.has(id)) {
        const label = this.entityLabels.get(id)!;
        label.position.set(position.x, position.y + 30, position.z);
      }
    });

    // When finished updating all entities, reapply selection highlight if needed
    if (this.selectedEntityId && this.entities.has(this.selectedEntityId)) {
      const entity = this.entities.get(this.selectedEntityId)!;
      this.applySelectionHighlight(entity);
    }

    // Update the sector borders on main map
    this.updateSectorBorders();

    // Update the mini-map entities and borders
    if (this.miniMapEnabled) {
      this.updateMiniMapEntities();
    }

    // Emit the updated entities event
    this.emitEntitiesUpdatedEvent(entities);
  }

  emitEntitiesUpdatedEvent(entities: Entity[]) {
    // Create a custom event with entity data for the UI to consume
    const event = new CustomEvent("entities-updated", {
      detail: {
        entities,
        selectedEntityId: this.selectedEntityId,
        forceSelection: true, // Add this flag to force UI to respect our selection
      },
    });
    document.dispatchEvent(event);
  }

  focusOnEntity(entityId: number) {
    // Clear any previous selection visual state
    this.clearSelectedEntityHighlight();

    this.selectedEntityId = entityId;

    if (this.entities.has(entityId)) {
      const entity = this.entities.get(entityId)!;

      // Highlight the selected entity (change its appearance)
      this.applySelectionHighlight(entity);

      // Focus the camera on it
      this.focusOnPosition(entity.position.x, entity.position.y);

      // Enable follow mode when focusing on an entity
      this.toggleFollowMode(true);
    }

    // Force an entities-updated event to sync the UI
    this.emitEntitiesUpdatedEvent(this.getAllRawEntities());
  }

  // Helper method to apply selection highlight to an entity
  applySelectionHighlight(entity: THREE.Object3D) {
    // First clear any existing highlight

    if (entity instanceof THREE.Mesh && !entity.userData.isTexturedPlane) {
      // For mesh-based entities (triangle ships when zoomed out)
      // Store original color if not already stored
      if (!entity.userData.originalColor) {
        entity.userData.originalColor = (
          entity.material as THREE.MeshBasicMaterial
        ).color.clone();
      }
      // Change to a highlighted color (bright red)
      (entity.material as THREE.MeshBasicMaterial).color.set(0xff0000);
    } else if (entity.userData.isTexturedPlane) {
    }
  }

  // Helper method to clear visual highlight
  clearSelectedEntityHighlight() {
    if (this.selectedEntityId && this.entities.has(this.selectedEntityId)) {
      const entity = this.entities.get(this.selectedEntityId)!;

      // Restore original appearance for mesh entities
      if (entity instanceof THREE.Mesh && entity.userData.originalColor) {
        (entity.material as THREE.MeshBasicMaterial).color.copy(
          entity.userData.originalColor
        );
      }
    }
  }

  focusOnPosition(x: number, y: number) {
    // Animate camera movement to focus on the position
    const startX = this.camera.position.x;
    const startY = this.camera.position.y;
    const endX = x;
    const endY = y;

    const startTime = Date.now();
    const duration = 1000; // 1 second animation

    const animate = () => {
      const elapsed = Date.now() - startTime;
      const progress = Math.min(elapsed / duration, 1);

      // Ease in-out function for smoother animation
      const easeInOut = (t: number) =>
        t < 0.5 ? 2 * t * t : -1 + (4 - 2 * t) * t;

      const eased = easeInOut(progress);

      this.camera.position.x = startX + (endX - startX) * eased;
      this.camera.position.y = startY + (endY - startY) * eased;

      if (progress < 1) {
        requestAnimationFrame(animate);
      } else {
        // When animation completes, update sectors
        this.updateVisibleSectors();
      }
    };

    animate();
  }

  // Update in the render method to continuously check position and rotation
  render() {
    // Force camera reset and multiple refresh attempts on first render
    if (!this.hasInitialRender) {
      this.hasInitialRender = true;

      // Force camera projection to correct initial state
      const width = window.innerWidth;
      const height = window.innerHeight;
      this.camera.left = -width / (2 * this.zoom);
      this.camera.right = width / (2 * this.zoom);
      this.camera.top = height / (2 * this.zoom);
      this.camera.bottom = -height / (2 * this.zoom);
      this.camera.updateProjectionMatrix();

      // Try a series of refreshes with increasing delays to ensure everything renders
      setTimeout(() => this.refreshView(), 50);
      setTimeout(() => this.refreshView(), 200);
      setTimeout(() => this.refreshView(), 500);
    }

    // Update camera position to follow selected entity if follow mode is enabled
    if (this.followSelectedEntity && this.selectedEntityId) {
      const entity = this.entities.get(this.selectedEntityId);
      if (entity) {
        // Smoothly move camera to follow entity
        const lerpFactor = 0.1; // Adjust for smoother or more immediate following
        this.camera.position.x +=
          (entity.position.x - this.camera.position.x) * lerpFactor;
        this.camera.position.y +=
          (entity.position.y - this.camera.position.y) * lerpFactor;
      }
    }

    // Update visible sectors if needed
    this.checkSectorUpdate();

    // Handle sector hover
    this.checkSectorHover();

    // Render the scene
    this.renderer.render(this.scene, this.camera);

    // Render mini-map if enabled
    if (this.miniMapEnabled) {
      // Update the mini-map viewport to represent the main camera's visible area
      this.updateMiniMapViewport();

      // Update entity dots on mini-map
      this.updateMiniMapEntities();

      // Render mini-map
      this.miniMapRenderer.render(this.miniMapScene, this.miniMapCamera);
    }
  }

  checkSectorUpdate() {
    // Check if camera has moved enough to warrant updating the sectors
    const currentPosition = this.getCameraPosition();

    // Check distance moved
    const distance = this.lastCheckedPosition.distanceTo(currentPosition);

    // If moved significantly, update the sectors
    if (distance > this.sectorSize / 4) {
      this.updateVisibleSectors();
      this.lastCheckedPosition.copy(currentPosition);
    }
  }

  checkSectorHover() {
    // Disable sector hover functionality
    return;
  }

  onWindowResize() {
    const width = window.innerWidth;
    const height = window.innerHeight;

    this.camera.left = -width / (2 * this.zoom);
    this.camera.right = width / (2 * this.zoom);
    this.camera.top = height / (2 * this.zoom);
    this.camera.bottom = -height / (2 * this.zoom);

    this.camera.updateProjectionMatrix();
    this.renderer.setSize(width, height);

    // Update the visible sectors
    this.updateVisibleSectors();

    // Update mini-map if enabled
    if (this.miniMapEnabled) {
      // Update the mini-map viewport to represent the new visible area
      this.updateMiniMapViewport();
    }
  }

  onMouseDown(event: MouseEvent) {
    this.isDragging = true;
    this.lastMousePosition.set(event.clientX, event.clientY);

    // Change cursor to grabbing hand while dragging
    this.renderer.domElement.style.cursor = "grabbing";

    // Disable follow mode when manually dragging
    if (this.followSelectedEntity) {
      this.followSelectedEntity = false;
    }
  }

  onMouseMove(event: MouseEvent) {
    // Update mouse position for raycasting
    this.mouse.x = (event.clientX / window.innerWidth) * 2 - 1;
    this.mouse.y = -(event.clientY / window.innerHeight) * 2 + 1;

    if (this.isDragging) {
      const deltaX = event.clientX - this.lastMousePosition.x;
      const deltaY = event.clientY - this.lastMousePosition.y;

      // Move camera in the opposite direction of mouse movement
      this.camera.position.x -= deltaX / this.zoom;
      this.camera.position.y += deltaY / this.zoom; // Invert Y axis

      this.lastMousePosition.set(event.clientX, event.clientY);
    }
  }

  onMouseUp() {
    this.isDragging = false;

    // Change cursor back to pointer (hand) when done dragging
    this.renderer.domElement.style.cursor = "pointer";

    this.updateVisibleSectors();
  }

  onMouseWheel(event: WheelEvent) {
    event.preventDefault();

    // Store previous zoom for comparison
    const prevZoom = this.zoom;

    // Calculate new zoom level
    const zoomDelta = event.deltaY > 0 ? -this.ZOOM_SPEED : this.ZOOM_SPEED;
    this.zoom = Math.max(
      this.MIN_ZOOM,
      Math.min(this.MAX_ZOOM, this.zoom + zoomDelta)
    );

    // Adjust camera
    const width = window.innerWidth;
    const height = window.innerHeight;

    this.camera.left = -width / (2 * this.zoom);
    this.camera.right = width / (2 * this.zoom);
    this.camera.top = height / (2 * this.zoom);
    this.camera.bottom = -height / (2 * this.zoom);

    this.camera.updateProjectionMatrix();

    // Update visible sectors
    this.updateVisibleSectors();

    // Check if we crossed the zoom threshold for ship representations (1.0)
    // Only update if we have loaded ship texture and actually crossed the threshold
    if (
      this.isShipTextureLoaded &&
      ((prevZoom <= 1.0 && this.zoom > 1.0) ||
        (prevZoom > 1.0 && this.zoom <= 1.0))
    ) {
      // Re-render entities to update ship representations
      if (this.rawEntities.length > 0) {
        this.updateEntities(this.rawEntities);
      }
    }
  }

  getCameraPosition(): THREE.Vector2 {
    return new THREE.Vector2(this.camera.position.x, this.camera.position.y);
  }

  getZoomLevel(): number {
    return this.zoom;
  }

  getAllEntities(): Array<{
    id: number;
    name: string;
    type: string;
    position: { x: number; y: number };
  }> {
    const result = [];

    for (const [id, object] of this.entities.entries()) {
      result.push({
        id,
        name: object.userData.name,
        type: object.userData.type,
        position: {
          x: object.position.x,
          y: object.position.y,
        },
      });
    }

    return result;
  }

  updateSectorBorders() {
    // Remove borders for sectors that no longer have entities
    this.sectorBorders.forEach((border, key) => {
      if (!this.sectorsWithEntities.has(key)) {
        this.scene.remove(border);
        border.geometry.dispose();
        (border.material as THREE.Material).dispose();
        this.sectorBorders.delete(key);
      }
    });

    // Add borders for sectors with entities that don't have borders yet
    this.sectorsWithEntities.forEach((sectorKey) => {
      if (!this.sectorBorders.has(sectorKey)) {
        const [x, y] = sectorKey.split(",").map(Number);
        const border = this.createSectorBorder(x, y);
        this.scene.add(border);
        this.sectorBorders.set(sectorKey, border);
      }
    });
  }

  createSectorBorder(x: number, y: number): THREE.LineSegments {
    // Calculate sector boundaries
    const startX = x * this.sectorSize;
    const startY = y * this.sectorSize;

    // Create a box to represent the border
    const borderGeometry = new THREE.BufferGeometry();
    const vertices = [];

    // Create a square outline
    vertices.push(startX, startY, 0.5);
    vertices.push(startX + this.sectorSize, startY, 0.5);

    vertices.push(startX + this.sectorSize, startY, 0.5);
    vertices.push(startX + this.sectorSize, startY + this.sectorSize, 0.5);

    vertices.push(startX + this.sectorSize, startY + this.sectorSize, 0.5);
    vertices.push(startX, startY + this.sectorSize, 0.5);

    vertices.push(startX, startY + this.sectorSize, 0.5);
    vertices.push(startX, startY, 0.5);

    borderGeometry.setAttribute(
      "position",
      new THREE.Float32BufferAttribute(vertices, 3)
    );

    const borderMaterial = new THREE.LineBasicMaterial({
      color: 0xffff00, // Yellow color
      linewidth: 2,
      transparent: false,
      depthTest: false,
    });

    return new THREE.LineSegments(borderGeometry, borderMaterial);
  }

  // Helper method to get the stored raw entities
  getAllRawEntities(): Entity[] {
    return this.rawEntities;
  }

  // Force a complete update of the view
  refreshView() {
    this.updateVisibleSectors();

    // If we have entities, re-apply them to trigger visual updates
    if (this.rawEntities.length > 0) {
      this.updateEntities(this.rawEntities);
    }

    // Make sure sector label is positioned correctly
    if (this.hoveredSector) {
      const centerX =
        this.hoveredSector.x * this.sectorSize + this.sectorSize / 2;
      const centerY =
        this.hoveredSector.y * this.sectorSize + this.sectorSize / 2;
      this.updateSectorLabel(
        this.hoveredSector.x,
        this.hoveredSector.y,
        centerX,
        centerY
      );
    }

    // Refresh mini-map if enabled
    if (this.miniMapEnabled) {
      this.updateMiniMapViewport();
      this.updateMiniMapEntities();
    }
  }

  // Helper method to create or update velocity arrow as an independent object
  updateVelocityArrow(
    entityId: number,
    entityPosition: THREE.Vector3,
    velocity: number[] | undefined
  ) {
    // Remove existing arrow if present
    const existingArrow = this.velocityArrows.get(entityId);
    if (existingArrow) {
      this.scene.remove(existingArrow);
      this.velocityArrows.delete(entityId);
    }

    // If no velocity or zero velocity, don't add an arrow
    if (!velocity || (velocity[0] === 0 && velocity[1] === 0)) {
      return;
    }

    // Create velocity arrow
    const velocityVector = new THREE.Vector2(velocity[0], velocity[1]);
    const velocityMagnitude = velocityVector.length();

    // Scale factor to make arrow visible (adjust as needed based on typical velocities)
    const scaleFactor = 2;
    const arrowLength = velocityMagnitude * scaleFactor;

    // Only show arrow if velocity is significant
    if (arrowLength < 5) {
      return;
    }

    // Create arrow group
    const arrowGroup = new THREE.Group();
    arrowGroup.position.copy(entityPosition);
    arrowGroup.position.z = 2; // Put slightly above the entity

    // Calculate direction
    const direction = velocityVector.clone().normalize();

    // Create line for arrow shaft
    const lineGeometry = new THREE.BufferGeometry();
    const lineVertices = new Float32Array([
      0,
      0,
      0, // Start at entity center
      direction.x * arrowLength,
      direction.y * arrowLength,
      0, // End at scaled velocity direction
    ]);
    lineGeometry.setAttribute(
      "position",
      new THREE.BufferAttribute(lineVertices, 3)
    );

    const lineMaterial = new THREE.LineBasicMaterial({ color: 0xff8000 }); // Orange color for velocity
    const line = new THREE.Line(lineGeometry, lineMaterial);
    arrowGroup.add(line);

    // Create arrowhead (small triangle at the tip)
    const arrowHeadSize = Math.min(10, arrowLength * 0.2); // Cap size and make proportional
    const arrowHeadGeometry = new THREE.BufferGeometry();

    // Calculate perpendicular direction for arrowhead
    const perpDirection = new THREE.Vector2(-direction.y, direction.x);

    const tipX = direction.x * arrowLength;
    const tipY = direction.y * arrowLength;

    const arrowHeadVertices = new Float32Array([
      tipX,
      tipY,
      0, // Tip
      tipX -
        direction.x * arrowHeadSize -
        (perpDirection.x * arrowHeadSize) / 2,
      tipY -
        direction.y * arrowHeadSize -
        (perpDirection.y * arrowHeadSize) / 2,
      0, // Left wing
      tipX -
        direction.x * arrowHeadSize +
        (perpDirection.x * arrowHeadSize) / 2,
      tipY -
        direction.y * arrowHeadSize +
        (perpDirection.y * arrowHeadSize) / 2,
      0, // Right wing
    ]);

    arrowHeadGeometry.setAttribute(
      "position",
      new THREE.BufferAttribute(arrowHeadVertices, 3)
    );

    const arrowHeadMaterial = new THREE.MeshBasicMaterial({
      color: 0xff8000,
      side: THREE.DoubleSide,
    });

    const arrowHead = new THREE.Mesh(arrowHeadGeometry, arrowHeadMaterial);
    arrowGroup.add(arrowHead);

    // Add completed arrow to scene and track it
    this.scene.add(arrowGroup);
    this.velocityArrows.set(entityId, arrowGroup);
  }

  // Helper method to create text canvas with dynamic sizing
  createTextCanvas(text: string) {
    // Create temporary canvas for measuring text
    const measureCanvas = document.createElement("canvas");
    const measureContext = measureCanvas.getContext("2d");

    if (!measureContext) return null;

    // Set font to measure
    const fontSize = 128;
    const fontFace = "Consolas";
    measureContext.font = `${fontSize}px ${fontFace}`;

    // Measure text width
    const metrics = measureContext.measureText(text);
    const textWidth = metrics.width;

    // Account for font height (approx 70% of fontSize for most fonts)
    const textHeight = fontSize * 0.7;

    // Add padding (50px on each side horizontally, 30px vertically)
    const paddingX = 100;
    const paddingY = 60;

    // Add extra padding for the outline
    const outlineWidth = 8; // Width of the outline
    const canvasPadding = outlineWidth * 2;

    // Create final canvas with appropriate dimensions
    const canvas = document.createElement("canvas");
    canvas.width = textWidth + paddingX + canvasPadding;
    canvas.height = textHeight + paddingY + canvasPadding;

    const context = canvas.getContext("2d");
    if (!context) return null;

    // Set transparent background
    context.fillStyle = "rgba(0, 0, 0, 0.0)";
    context.fillRect(0, 0, canvas.width, canvas.height);

    // Apply same font settings
    context.font = `${fontSize}px ${fontFace}`;
    context.textAlign = "left";
    context.textBaseline = "top";

    // Position text with consistent padding (adjusted for outline)
    const textX = paddingX / 2 + outlineWidth;
    const textY = paddingY / 2 + outlineWidth;

    // Draw the text outline by drawing the text multiple times in black with offsets
    context.fillStyle = "black";

    // Draw the outline by repeating the text at slight offsets
    for (let x = -outlineWidth; x <= outlineWidth; x += outlineWidth) {
      for (let y = -outlineWidth; y <= outlineWidth; y += outlineWidth) {
        if (x !== 0 || y !== 0) {
          // Skip the center position (that's for the white text)
          context.fillText(text, textX + x, textY + y);
        }
      }
    }

    // Now draw the main text in white on top
    context.fillStyle = "white";
    context.fillText(text, textX, textY);

    return canvas;
  }

  // Add a method to toggle camera follow mode
  toggleFollowMode(enabled?: boolean) {
    if (enabled !== undefined) {
      this.followSelectedEntity = enabled;
    } else {
      this.followSelectedEntity = !this.followSelectedEntity;
    }

    // If enabling follow mode and we have a selected entity, immediately center on it
    if (
      this.followSelectedEntity &&
      this.selectedEntityId &&
      this.entities.has(this.selectedEntityId)
    ) {
      const entity = this.entities.get(this.selectedEntityId)!;
      this.camera.position.x = entity.position.x;
      this.camera.position.y = entity.position.y;
      this.updateVisibleSectors();
    }

    return this.followSelectedEntity;
  }

  // Add a method to get current follow mode state for UI
  isFollowingEntity(): boolean {
    return this.followSelectedEntity;
  }

  // Initialize mini-map
  initializeMiniMap() {
    // Create mini-map scene
    this.miniMapScene = new THREE.Scene();

    // Set up fixed-size orthographic camera for mini-map
    const miniMapAspect = 1; // Square mini-map
    // Show 1000 sectors in each direction (1000 * 1000 units per sector)
    const miniMapFrustumSize = 2000000;
    this.miniMapCamera = new THREE.OrthographicCamera(
      -miniMapFrustumSize / 2,
      miniMapFrustumSize / 2,
      miniMapFrustumSize / 2,
      -miniMapFrustumSize / 2,
      0.1,
      10000
    );
    this.miniMapCamera.position.z = 1000;

    // Create mini-map renderer
    this.miniMapRenderer = new THREE.WebGLRenderer({
      antialias: true,
      alpha: true,
    });
    this.miniMapRenderer.setClearColor(0x111111, 1); // Dark background with some transparency
    this.miniMapRenderer.setSize(this.miniMapSize, this.miniMapSize);

    // Position mini-map in bottom left corner
    this.miniMapContainer = document.createElement("div");
    this.miniMapContainer.style.position = "absolute";
    this.miniMapContainer.style.left = "10px";
    this.miniMapContainer.style.bottom = "10px";
    this.miniMapContainer.style.width = `${this.miniMapSize}px`;
    this.miniMapContainer.style.height = `${this.miniMapSize}px`;
    this.miniMapContainer.style.border = "2px solid #555555";
    this.miniMapContainer.style.borderRadius = "3px";
    this.miniMapContainer.style.overflow = "hidden";
    this.miniMapContainer.style.pointerEvents = "auto"; // Allow interaction

    // Add mini-map to container
    this.miniMapContainer.appendChild(this.miniMapRenderer.domElement);
    this.container.appendChild(this.miniMapContainer);

    // Create a mesh to represent the main camera's viewport in the mini-map
    const viewportGeometry = new THREE.PlaneGeometry(
      window.innerWidth / this.zoom,
      window.innerHeight / this.zoom
    );
    const viewportMaterial = new THREE.MeshBasicMaterial({
      color: 0xffffff,
      transparent: true,
      opacity: 0.2,
      wireframe: true,
    });
    this.miniMapViewport = new THREE.Mesh(viewportGeometry, viewportMaterial);
    this.miniMapScene.add(this.miniMapViewport);

    // Add click event listener to mini-map
    this.miniMapRenderer.domElement.addEventListener(
      "click",
      this.onMiniMapClick.bind(this)
    );

    // Create a grid for the mini-map to show sector boundaries
    this.createMiniMapGrid();
  }

  // Handle clicks on the mini-map
  onMiniMapClick(event: MouseEvent) {
    // Calculate the position within the mini-map (in pixels)
    const rect = this.miniMapRenderer.domElement.getBoundingClientRect();
    const x = event.clientX - rect.left;
    const y = event.clientY - rect.top;

    // Convert to normalized coordinates (-1 to 1)
    const normX = (x / this.miniMapSize) * 2 - 1;
    const normY = -((y / this.miniMapSize) * 2 - 1); // Invert Y for Three.js

    // Raycasting not needed for orthographic mini-map; we can directly convert to world position
    const worldX =
      this.miniMapCamera.left +
      (this.miniMapCamera.right - this.miniMapCamera.left) * ((normX + 1) / 2);
    const worldY =
      this.miniMapCamera.bottom +
      (this.miniMapCamera.top - this.miniMapCamera.bottom) * ((normY + 1) / 2);

    // Focus the main camera on this position
    this.focusOnPosition(worldX, worldY);

    // Toggle off follow mode when directly navigating
    if (this.followSelectedEntity) {
      this.followSelectedEntity = false;
    }
  }

  // Create grid for mini-map
  createMiniMapGrid() {
    // Create a basic grid for the mini-map
    const gridGeometry = new THREE.BufferGeometry();
    // Grid should cover the entire viewable area (1000 sectors * 1000 units per sector * 2 for both directions)
    const gridSize = 2000000;
    const vertices = [];

    // Only create horizontal and vertical center lines through the origin
    // Horizontal line
    vertices.push(-gridSize / 2, 0, 0);
    vertices.push(gridSize / 2, 0, 0);

    // Vertical line
    vertices.push(0, -gridSize / 2, 0);
    vertices.push(0, gridSize / 2, 0);

    gridGeometry.setAttribute(
      "position",
      new THREE.Float32BufferAttribute(vertices, 3)
    );

    const gridMaterial = new THREE.LineBasicMaterial({
      color: 0x555555, // Slightly brighter than before to make center lines more visible
      transparent: true,
      opacity: 0.5,
      linewidth: 1.5, // Make slightly thicker for better visibility
    });

    const grid = new THREE.LineSegments(gridGeometry, gridMaterial);
    this.miniMapScene.add(grid);

    // Add a marker for the origin (0,0) to help with orientation
    const originGeometry = new THREE.CircleGeometry(5000, 16);
    const originMaterial = new THREE.MeshBasicMaterial({
      color: 0x888888,
      transparent: true,
      opacity: 0.7,
    });
    const originMarker = new THREE.Mesh(originGeometry, originMaterial);
    originMarker.position.set(0, 0, 0.5);
    this.miniMapScene.add(originMarker);

    // Remove the cross lines since we now have the center grid lines
  }

  // Add a method to update the mini-map viewport to represent the main camera's visible area
  updateMiniMapViewport() {
    // Get the current camera position
    const cameraPosition = this.getCameraPosition();

    // Calculate the visible area dimensions based on current zoom
    const visibleWidth = window.innerWidth / this.zoom;
    const visibleHeight = window.innerHeight / this.zoom;

    // Position the viewport representation at the camera position
    this.miniMapViewport.position.set(cameraPosition.x, cameraPosition.y, 2); // Keep it above other elements

    // Update the geometry to match current visible area
    if (this.miniMapViewport.geometry) {
      this.miniMapViewport.geometry.dispose();
    }

    // Create new geometry for the viewport indicator
    const viewportGeometry = new THREE.PlaneGeometry(
      visibleWidth,
      visibleHeight
    );
    this.miniMapViewport.geometry = viewportGeometry;

    // Create/update material for the viewport indicator - semi-transparent white
    const viewportMaterial = new THREE.MeshBasicMaterial({
      color: 0xffffff,
      transparent: true,
      opacity: 0.2,
      wireframe: true,
    });

    // Apply the material, disposing of old one if it exists
    if (
      this.miniMapViewport.material &&
      this.miniMapViewport.material instanceof THREE.Material
    ) {
      this.miniMapViewport.material.dispose();
    }
    this.miniMapViewport.material = viewportMaterial;
  }

  // Add a method to update entity dots on the mini-map
  updateMiniMapEntities() {
    // Clear existing entity dots on mini-map
    this.miniMapEntities.forEach((dot) => {
      this.miniMapScene.remove(dot);
      dot.geometry.dispose();
      (dot.material as THREE.Material).dispose();
    });
    this.miniMapEntities.clear();

    // Clear existing sector borders on mini-map
    this.miniMapSectorBorders.forEach((border) => {
      this.miniMapScene.remove(border);
      border.geometry.dispose();
      (border.material as THREE.Material).dispose();
    });
    this.miniMapSectorBorders.clear();

    // Create pixel dot for each entity
    this.entities.forEach((entity, entityId) => {
      // Create a small dot to represent the entity
      // Much smaller dots for the larger view (1000 sectors)
      const dotGeometry = new THREE.CircleGeometry(10000, 8);

      // Use different colors based on entity type
      let color = 0xffffff; // Default white
      if (entity.userData.type === "Ship") {
        color = 0x00ffff; // Cyan for ships
      }

      // Highlight selected entity
      if (entityId === this.selectedEntityId) {
        color = 0xff0000; // Red for selected entity
      }

      const dotMaterial = new THREE.MeshBasicMaterial({ color });
      const dot = new THREE.Mesh(dotGeometry, dotMaterial);

      // Position at entity's coordinates
      dot.position.copy(entity.position);
      dot.position.z = 1; // Ensure dot is above the grid

      // Add to mini-map scene
      this.miniMapScene.add(dot);
      this.miniMapEntities.set(entityId.toString(), dot);
    });

    // Create yellow borders for sectors with entities
    this.sectorsWithEntities.forEach((sectorKey) => {
      const [x, y] = sectorKey.split(",").map(Number);
      const border = this.createMiniMapSectorBorder(x, y);
      this.miniMapScene.add(border);
      this.miniMapSectorBorders.set(sectorKey, border);
    });
  }

  // Add a method to create a mini-map sector border with yellow outline
  createMiniMapSectorBorder(x: number, y: number): THREE.LineSegments {
    // Calculate sector boundaries
    const startX = x * this.sectorSize;
    const startY = y * this.sectorSize;

    // Create a box to represent the border
    const borderGeometry = new THREE.BufferGeometry();
    const vertices = [];

    // Create a square outline (same as createSectorBorder but for mini-map)
    vertices.push(startX, startY, 0.5);
    vertices.push(startX + this.sectorSize, startY, 0.5);

    vertices.push(startX + this.sectorSize, startY, 0.5);
    vertices.push(startX + this.sectorSize, startY + this.sectorSize, 0.5);

    vertices.push(startX + this.sectorSize, startY + this.sectorSize, 0.5);
    vertices.push(startX, startY + this.sectorSize, 0.5);

    vertices.push(startX, startY + this.sectorSize, 0.5);
    vertices.push(startX, startY, 0.5);

    borderGeometry.setAttribute(
      "position",
      new THREE.Float32BufferAttribute(vertices, 3)
    );

    const borderMaterial = new THREE.LineBasicMaterial({
      color: 0xffff00, // Yellow color (same as main map)
      linewidth: 2,
      transparent: false,
      depthTest: false,
    });

    return new THREE.LineSegments(borderGeometry, borderMaterial);
  }

  // Add a method to toggle mini-map visibility
  toggleMiniMap(enabled?: boolean): boolean {
    if (enabled !== undefined) {
      this.miniMapEnabled = enabled;
    } else {
      this.miniMapEnabled = !this.miniMapEnabled;
    }

    // Update the mini-map container visibility
    this.miniMapContainer.style.display = this.miniMapEnabled
      ? "block"
      : "none";

    // If enabling, make sure the mini-map is up to date
    if (this.miniMapEnabled) {
      this.updateMiniMapViewport();
      this.updateMiniMapEntities();
    }

    return this.miniMapEnabled;
  }

  // Load the ship sprite texture
  loadShipTexture() {
    const textureLoader = new THREE.TextureLoader();
    textureLoader.load(
      "/SpaceShip.png",
      (texture) => {
        this.shipSpriteTexture = texture;
        this.isShipTextureLoaded = true;

        // Store the texture dimensions for proper aspect ratio
        const image = texture.image;
        this.shipTextureWidth = image.width;
        this.shipTextureHeight = image.height;

        // Re-render existing entities to apply the sprite if needed
        if (this.rawEntities.length > 0 && this.zoom > 1.0) {
          this.updateEntities(this.rawEntities);
        }
      },
      undefined, // onProgress callback not needed
      (error) => {
        console.error("Error loading ship texture:", error);
      }
    );
  }

  // Helper method to create entity objects based on their type and current zoom level
  createEntityObject(
    objectType: string,
    id: number,
    name: string
  ): THREE.Object3D {
    if (objectType === "Ship") {
      // Use textured plane for ships when zoomed in and texture is loaded
      if (
        this.zoom > 1.0 &&
        this.isShipTextureLoaded &&
        this.shipSpriteTexture
      ) {
        // Create a group to hold both the ship and direction indicator
        const group = new THREE.Group();

        // Calculate the aspect ratio if we have the texture dimensions
        let width = 25;
        let height = 25;

        if (this.shipTextureWidth && this.shipTextureHeight) {
          // Keep the larger dimension at 25 units and scale the other accordingly
          const aspectRatio = this.shipTextureWidth / this.shipTextureHeight;
          if (aspectRatio >= 1) {
            // Width is larger than or equal to height
            width = 25;
            height = 25 / aspectRatio;
          } else {
            // Height is larger than width
            height = 25;
            width = 25 * aspectRatio;
          }
        }

        // Create the plane geometry with proper aspect ratio
        const planeGeometry = new THREE.PlaneGeometry(width, height);

        // Create the material with the ship texture
        const material = new THREE.ShaderMaterial({
          vertexShader: shipVertexShader,
          fragmentShader: shipFragmentShader,
          uniforms: {
            map: { value: this.shipSpriteTexture },
            brightness: { value: 0.8 },
          },
        });

        // Create the mesh
        const shipMesh = new THREE.Mesh(planeGeometry, material);

        // Create a separate mesh container group to handle the ship's orientation
        const shipContainer = new THREE.Group();
        shipContainer.add(shipMesh);

        // Add the ship container to the main group
        group.add(shipContainer);

        // Add direction indicator (green line) showing the forward direction
        const directionGeometry = new THREE.BufferGeometry();
        const directionVertices = new Float32Array([
          0,
          0,
          0, // center
          0,
          40,
          0, // front direction - points along +Y axis
        ]);
        directionGeometry.setAttribute(
          "position",
          new THREE.BufferAttribute(directionVertices, 3)
        );

        const directionMaterial = new THREE.LineBasicMaterial({
          color: 0x00ff00, // Green for direction
          linewidth: 2,
        });

        const directionLine = new THREE.Line(
          directionGeometry,
          directionMaterial
        );

        // Add the direction line to the main group
        group.add(directionLine);

        // Store the ship container for later reference
        group.userData.shipContainer = shipContainer;

        // Set user data
        group.userData.id = id;
        group.userData.name = name;
        group.userData.type = objectType;
        group.userData.isTexturedPlane = true;
        group.userData.isSprite = false;

        return group;
      } else {
        // Use triangle for ships when zoomed out or texture not loaded
        const geometry = new THREE.BufferGeometry();
        const vertices = new Float32Array([
          0,
          20,
          0, // top
          -10,
          -10,
          0, // bottom left
          10,
          -10,
          0, // bottom right
        ]);
        geometry.setAttribute(
          "position",
          new THREE.BufferAttribute(vertices, 3)
        );

        // Create a material with a brighter color for better visibility
        const material = new THREE.MeshBasicMaterial({
          color: 0x00ffff,
          side: THREE.DoubleSide,
        });

        const mesh = new THREE.Mesh(geometry, material);

        // Set user data
        mesh.userData.id = id;
        mesh.userData.name = name;
        mesh.userData.type = objectType;
        mesh.userData.isSprite = false;

        return mesh;
      }
    } else {
      // Default object for unknown types (no change)
      const geometry = new THREE.CircleGeometry(10, 32);
      const material = new THREE.MeshBasicMaterial({ color: 0xffffff });
      const object = new THREE.Mesh(geometry, material);

      // Set user data
      object.userData.id = id;
      object.userData.name = name;
      object.userData.type = objectType;
      object.userData.isSprite = false;

      return object;
    }
  }

  // Helper method to update the direction indicator based on the rotation
  updateDirectionIndicator(
    shipGroup: THREE.Object3D,
    rotation: THREE.Quaternion
  ) {
    // Find the direction line in the group
    const directionLine = shipGroup.children.find(
      (child) => child instanceof THREE.Line
    );

    if (directionLine) {
      // Calculate the forward direction vector from the quaternion
      const forward = new THREE.Vector3(0, 1, 0); // Start with forward = +Y axis
      forward.applyQuaternion(rotation); // Apply rotation

      // Scale it to the desired length
      forward.normalize().multiplyScalar(40);

      // Update the direction line's geometry
      const positions = new Float32Array([
        0,
        0,
        0, // Origin
        forward.x,
        forward.y,
        0, // Forward direction (keep z=0 for 2D)
      ]);

      // Update the buffer geometry
      const geometry = (directionLine as THREE.Line).geometry;
      geometry.setAttribute(
        "position",
        new THREE.BufferAttribute(positions, 3)
      );
      geometry.attributes.position.needsUpdate = true;
    }
  }
}
