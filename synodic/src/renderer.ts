import * as THREE from "three";
import { Entity } from "./types";

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
        object.position.copy(position);
        object.quaternion.copy(rotation); // Apply rotation
      } else {
        // Create new entity based on its type
        if (objectType === "Ship") {
          // Create a triangle for ships
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

          object = new THREE.Mesh(geometry, material);

          // Apply rotation to point in the right direction
          object.quaternion.copy(rotation);

          // Add name label with dynamic sizing
          const canvas = this.createTextCanvas(name);
          if (canvas) {
            // Remove any existing label
            const existingLabel = this.entityLabels.get(id);
            if (existingLabel) {
              this.scene.remove(existingLabel);
            }

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
        } else {
          // Default object for unknown types
          const geometry = new THREE.CircleGeometry(10, 32);
          const material = new THREE.MeshBasicMaterial({ color: 0xffffff });
          object = new THREE.Mesh(geometry, material);

          // Apply rotation (though may not be visually apparent for circles)
          object.quaternion.copy(rotation);
        }

        object.position.copy(position);
        object.userData.id = id;
        object.userData.name = name;
        object.userData.type = objectType;

        this.scene.add(object);
        this.entities.set(id, object);
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
      if (entity instanceof THREE.Mesh) {
        // Update the highlight
        (entity.material as THREE.MeshBasicMaterial).color.set(0xff0000);
      }
    }

    // Update the sector borders
    this.updateSectorBorders();

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
      if (entity instanceof THREE.Mesh) {
        // Store original color to restore later if needed
        entity.userData.originalColor = (
          entity.material as THREE.MeshBasicMaterial
        ).color.clone();
        // Change to a highlighted color (e.g., bright red)
        (entity.material as THREE.MeshBasicMaterial).color.set(0xff0000);
      }

      // Focus the camera on it
      this.focusOnPosition(entity.position.x, entity.position.y);

      // Enable follow mode when focusing on an entity
      this.toggleFollowMode(true);
    }

    // Force an entities-updated event to sync the UI
    this.emitEntitiesUpdatedEvent(this.getAllRawEntities());
  }

  // Helper method to clear visual highlight
  clearSelectedEntityHighlight() {
    if (this.selectedEntityId && this.entities.has(this.selectedEntityId)) {
      const entity = this.entities.get(this.selectedEntityId)!;

      // Restore original appearance
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
  }

  onMouseDown(event: MouseEvent) {
    this.isDragging = true;
    this.lastMousePosition.set(event.clientX, event.clientY);

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
    this.updateVisibleSectors();
  }

  onMouseWheel(event: WheelEvent) {
    event.preventDefault();

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

    // Create final canvas with appropriate dimensions
    const canvas = document.createElement("canvas");
    canvas.width = textWidth + paddingX;
    canvas.height = textHeight + paddingY;

    const context = canvas.getContext("2d");
    if (!context) return null;

    // Set transparent background
    context.fillStyle = "rgba(0, 0, 0, 0.0)";
    context.fillRect(0, 0, canvas.width, canvas.height);

    // Apply same font settings
    context.font = `${fontSize}px ${fontFace}`;
    context.fillStyle = "white";
    context.textAlign = "left";
    context.textBaseline = "top";

    // Position text with consistent padding
    context.fillText(text, paddingX / 2, paddingY / 2);

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
}
