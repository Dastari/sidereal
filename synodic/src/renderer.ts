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

  constructor(container: HTMLElement) {
    this.container = container;
    this.scene = new THREE.Scene();
    this.scene.background = new THREE.Color(0x000814);

    // Create an orthographic camera
    const width = window.innerWidth;
    const height = window.innerHeight;
    const frustumSize = 10000;
    const aspect = width / height;
    this.camera = new THREE.OrthographicCamera(
      (frustumSize * aspect) / -2,
      (frustumSize * aspect) / 2,
      frustumSize / 2,
      frustumSize / -2,
      0.1,
      10000
    );
    this.camera.position.z = 1000;

    // Create the renderer
    this.renderer = new THREE.WebGLRenderer({ antialias: true });
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
    // Create a canvas for the hover label
    const canvas = document.createElement("canvas");
    canvas.width = 256;
    canvas.height = 64;
    const context = canvas.getContext("2d");
    if (context) {
      context.fillStyle = "rgba(255, 255, 255, 0.8)";
      context.fillRect(0, 0, canvas.width, canvas.height);
      context.font = "20px Arial";
      context.fillStyle = "rgba(0, 0, 0, 1)";
      context.textAlign = "center";
      context.textBaseline = "middle";
      context.fillText(
        "Hover over a sector",
        canvas.width / 2,
        canvas.height / 2
      );
    }

    // Create a texture from the canvas
    const texture = new THREE.CanvasTexture(canvas);
    const material = new THREE.SpriteMaterial({
      map: texture,
      transparent: true,
    });
    this.sectorLabel = new THREE.Sprite(material);
    this.sectorLabel.scale.set(200, 50, 1);
    this.sectorLabel.visible = false;
    this.scene.add(this.sectorLabel);
  }

  updateSectorLabel(x: number, y: number, worldX: number, worldY: number) {
    if (!this.sectorLabel) return;

    // Create a canvas for the label
    const canvas = document.createElement("canvas");
    canvas.width = 256;
    canvas.height = 64;
    const context = canvas.getContext("2d");
    if (context) {
      context.fillStyle = "rgba(255, 255, 255, 0.8)";
      context.fillRect(0, 0, canvas.width, canvas.height);
      context.font = "20px Arial";
      context.fillStyle = "rgba(0, 0, 0, 1)";
      context.textAlign = "center";
      context.textBaseline = "middle";
      context.fillText(
        `Sector (${x}, ${y})`,
        canvas.width / 2,
        canvas.height / 2
      );
    }

    // Update the texture
    const texture = new THREE.CanvasTexture(canvas);
    (this.sectorLabel.material as THREE.SpriteMaterial).map = texture;
    (this.sectorLabel.material as THREE.SpriteMaterial).needsUpdate = true;

    // Position the label
    this.sectorLabel.position.set(worldX, worldY, 10);
    this.sectorLabel.visible = true;
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
      color: 0x444444,
      transparent: true,
      opacity: 0.5,
    });

    const grid = new THREE.LineSegments(gridGeometry, gridMaterial);
    group.add(grid);

    // Store sector coords as user data
    group.userData.sectorX = x;
    group.userData.sectorY = y;

    return group;
  }

  updateEntities(entities: Entity[]) {
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
    });

    // Update or add entities
    entities.forEach((entity) => {
      const id = entity.entity;
      const transform =
        entity.components["bevy_transform::components::transform::Transform"];
      const objectType =
        entity.components["sidereal_core::ecs::components::object::Object"];
      const name = entity.components["bevy_core::name::Name"].name;

      // Position from server
      const position = new THREE.Vector3(
        transform.translation[0],
        transform.translation[1],
        0
      );

      let object: THREE.Object3D;

      // Check if entity already exists
      if (this.entities.has(id)) {
        object = this.entities.get(id)!;
        object.position.copy(position);
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

          // Add a label with the ship name
          const canvas = document.createElement("canvas");
          canvas.width = 256;
          canvas.height = 64;
          const context = canvas.getContext("2d");
          if (context) {
            context.fillStyle = "rgba(0, 0, 0, 0.7)";
            context.fillRect(0, 0, canvas.width, canvas.height);
            context.font = "16px Arial";
            context.fillStyle = "white";
            context.textAlign = "center";
            context.textBaseline = "middle";
            context.fillText(name, canvas.width / 2, canvas.height / 2);
          }

          const texture = new THREE.CanvasTexture(canvas);
          const spriteMaterial = new THREE.SpriteMaterial({
            map: texture,
            transparent: true,
          });
          const sprite = new THREE.Sprite(spriteMaterial);
          sprite.position.set(0, 30, 0);
          sprite.scale.set(100, 25, 1);
          object.add(sprite);
        } else {
          // Default object for unknown types
          const geometry = new THREE.CircleGeometry(10, 32);
          const material = new THREE.MeshBasicMaterial({ color: 0xffffff });
          object = new THREE.Mesh(geometry, material);
        }

        object.position.copy(position);
        object.userData.id = id;
        object.userData.name = name;
        object.userData.type = objectType;

        this.scene.add(object);
        this.entities.set(id, object);
      }
    });

    // If we have a selected entity, focus on it
    if (this.selectedEntityId && this.entities.has(this.selectedEntityId)) {
      const entity = this.entities.get(this.selectedEntityId)!;
      this.focusOnPosition(entity.position.x, entity.position.y);
    }

    // Emit an event with the updated entities list for the UI
    this.emitEntitiesUpdatedEvent(entities);
  }

  emitEntitiesUpdatedEvent(entities: Entity[]) {
    // Create a custom event with entity data for the UI to consume
    const event = new CustomEvent("entities-updated", {
      detail: { entities },
    });
    document.dispatchEvent(event);
  }

  focusOnEntity(entityId: number) {
    this.selectedEntityId = entityId;

    if (this.entities.has(entityId)) {
      const entity = this.entities.get(entityId)!;
      this.focusOnPosition(entity.position.x, entity.position.y);
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
    // Cast a ray to see if we're hovering over a sector
    this.raycaster.setFromCamera(this.mouse, this.camera);

    // Calculate world coordinates from mouse
    const worldPosition = new THREE.Vector3(
      this.camera.position.x +
        (this.mouse.x * (window.innerWidth / 2)) / this.zoom,
      this.camera.position.y +
        (this.mouse.y * (window.innerHeight / 2)) / this.zoom,
      0
    );

    // Calculate sector from world position
    const x = Math.floor(worldPosition.x / this.sectorSize);
    const y = Math.floor(worldPosition.y / this.sectorSize);

    // If sector changed, update the hover info
    if (
      !this.hoveredSector ||
      this.hoveredSector.x !== x ||
      this.hoveredSector.y !== y
    ) {
      this.hoveredSector = { x, y };

      // Position label at sector center
      const centerX = x * this.sectorSize + this.sectorSize / 2;
      const centerY = y * this.sectorSize + this.sectorSize / 2;

      this.updateSectorLabel(x, y, centerX, centerY);
    }
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
}
