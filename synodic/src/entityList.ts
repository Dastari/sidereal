import { Entity } from "./types";
import { SectorMapRenderer } from "./renderer";

export class EntityListSidebar {
  private container: HTMLElement;
  private renderer: SectorMapRenderer;
  private entityListElement: HTMLElement;
  private toggleButton: HTMLElement;
  private isOpen: boolean = true;
  private selectedEntityId: number | null = null;

  constructor(container: HTMLElement, renderer: SectorMapRenderer) {
    this.container = container;
    this.renderer = renderer;

    // Create sidebar container
    const sidebar = document.createElement("div");
    sidebar.className = "entity-sidebar";
    this.container.appendChild(sidebar);

    // Create toggle button
    this.toggleButton = document.createElement("button");
    this.toggleButton.className = "sidebar-toggle";
    this.toggleButton.innerHTML = "&laquo;";
    this.toggleButton.title = "Toggle Entity List";
    sidebar.appendChild(this.toggleButton);

    // Create header
    const header = document.createElement("h2");
    header.textContent = "Entities";
    sidebar.appendChild(header);

    // Create search input
    const searchContainer = document.createElement("div");
    searchContainer.className = "search-container";
    sidebar.appendChild(searchContainer);

    const searchInput = document.createElement("input");
    searchInput.type = "text";
    searchInput.placeholder = "Search entities...";
    searchInput.className = "search-input";
    searchContainer.appendChild(searchInput);

    // Create entity list container
    this.entityListElement = document.createElement("div");
    this.entityListElement.className = "entity-list";
    sidebar.appendChild(this.entityListElement);

    // Set up event listeners
    this.setupEventListeners(searchInput);

    // Add CSS
    this.addStyles();
  }

  setupEventListeners(searchInput: HTMLInputElement) {
    // Toggle sidebar on button click
    this.toggleButton.addEventListener("click", () => {
      this.isOpen = !this.isOpen;
      document.body.classList.toggle("sidebar-collapsed", !this.isOpen);
      this.toggleButton.innerHTML = this.isOpen ? "&laquo;" : "&raquo;";
    });

    // Listen for entity updates from the renderer
    document.addEventListener("entities-updated", ((event: CustomEvent) => {
      this.updateEntityList(event.detail.entities);
    }) as EventListener);

    // Filter entities when typing in search
    searchInput.addEventListener("input", () => {
      const searchTerm = searchInput.value.toLowerCase();
      const entityItems =
        this.entityListElement.querySelectorAll(".entity-item");

      entityItems.forEach((item) => {
        const entityName = item.getAttribute("data-name")?.toLowerCase() || "";
        const entityType = item.getAttribute("data-type")?.toLowerCase() || "";
        const isVisible =
          entityName.includes(searchTerm) || entityType.includes(searchTerm);

        (item as HTMLElement).style.display = isVisible ? "block" : "none";
      });
    });
  }

  selectEntity(entityId: number) {
    // Update the stored selection
    this.selectedEntityId = entityId;

    // Update the UI to reflect the selection
    const items = this.entityListElement.querySelectorAll(".entity-item");
    items.forEach((item) => {
      const itemId = parseInt(item.getAttribute("data-id") || "0");
      if (itemId === entityId) {
        item.classList.add("selected");
      } else {
        item.classList.remove("selected");
      }
    });
  }

  updateEntityList(entities: Entity[]) {
    // Store the selected ID before clearing the list
    const selectedId = this.selectedEntityId;

    // Clear existing list
    this.entityListElement.innerHTML = "";

    // Group entities by type
    const entityTypes = new Map<string, Entity[]>();

    entities.forEach((entity) => {
      const type =
        entity.components["sidereal::ecs::components::object::Object"];

      if (!entityTypes.has(type)) {
        entityTypes.set(type, []);
      }

      entityTypes.get(type)!.push(entity);
    });

    // Create sections for each type
    entityTypes.forEach((entities, type) => {
      // Create type header
      const typeHeader = document.createElement("div");
      typeHeader.className = "entity-type-header";
      typeHeader.textContent = `${type}s (${entities.length})`;
      this.entityListElement.appendChild(typeHeader);

      // Create collapsible container for this type
      const typeContainer = document.createElement("div");
      typeContainer.className = "entity-type-container";
      this.entityListElement.appendChild(typeContainer);

      // Add click to expand/collapse
      typeHeader.addEventListener("click", () => {
        typeContainer.classList.toggle("collapsed");
        typeHeader.classList.toggle("collapsed");
      });

      // Add entities of this type
      entities
        .sort((a, b) => {
          const nameA = a.components["bevy_core::name::Name"];
          const nameB = b.components["bevy_core::name::Name"];
          return nameA.localeCompare(nameB);
        })
        .forEach((entity) => {
          const item = document.createElement("div");
          item.className = "entity-item";
          item.setAttribute("data-id", entity.entity.toString());
          item.setAttribute(
            "data-name",
            entity.components["bevy_core::name::Name"]
          );
          item.setAttribute("data-type", type);

          const name = entity.components["bevy_core::name::Name"];
          const transform =
            entity.components[
              "bevy_transform::components::transform::Transform"
            ];
          const position = transform.translation;

          // Get sector information if available
          const sectorInfo =
            entity.components["sidereal::ecs::components::sector::Sector"];
          const sectorText = sectorInfo
            ? `Sector: ${sectorInfo.x}, ${sectorInfo.y}`
            : "No sector";

          item.innerHTML = `
            <div class="entity-name">${name}</div>
            <div class="entity-position">Position: ${position[0].toFixed(
              0
            )}, ${position[1].toFixed(0)}</div>
            <div class="entity-sector">${sectorText}</div>
          `;

          // Check if this entity was previously selected
          if (this.selectedEntityId === entity.entity) {
            item.classList.add("selected");
          }

          // Modify the click handler to use the new selectEntity method
          item.addEventListener("click", () => {
            this.renderer.focusOnEntity(entity.entity);
            this.selectEntity(entity.entity);
          });

          typeContainer.appendChild(item);
        });
    });

    // Re-apply the selection after building the new list
    if (selectedId !== null) {
      this.selectEntity(selectedId);
    }
  }

  addStyles() {
    // Add CSS to the page
    const style = document.createElement("style");
    style.textContent = `
      body {
        transition: padding-right 0.3s ease;
      }
      
      body.sidebar-collapsed {
        padding-right: 0;
      }
      
      .entity-sidebar {
        position: fixed;
        top: 0;
        right: 0;
        width: 300px;
        height: 100vh;
        background-color: rgba(0, 0, 0, 0.8);
        color: white;
        padding: 10px;
        box-sizing: border-box;
        overflow-y: auto;
        z-index: 1000;
        transition: transform 0.3s ease;
      }
      
      body.sidebar-collapsed .entity-sidebar {
        transform: translateX(290px);
      }
      
      .sidebar-toggle {
        position: absolute;
        left: 0;
        top: 20px;
        width: 20px;
        height: 60px;
        background-color: rgba(0, 0, 0, 0.8);
        color: white;
        border: none;
        cursor: pointer;
        font-size: 14px;
        border-radius: 5px 0 0 5px;
      }
      
      .entity-sidebar h2 {
        margin-top: 0;
        padding-left: 10px;
      }
      
      .search-container {
        padding: 10px;
        margin-bottom: 10px;
      }
      
      .search-input {
        width: 100%;
        padding: 8px;
        background-color: rgba(255, 255, 255, 0.1);
        color: white;
        border: 1px solid rgba(255, 255, 255, 0.2);
        border-radius: 4px;
      }
      
      .entity-list {
        margin-bottom: 20px;
      }
      
      .entity-type-header {
        background-color: rgba(0, 80, 120, 0.5);
        padding: 8px 10px;
        font-weight: bold;
        cursor: pointer;
        border-radius: 4px;
        margin-bottom: 5px;
        position: relative;
      }
      
      .entity-type-header:after {
        content: '▼';
        position: absolute;
        right: 10px;
        transition: transform 0.3s;
      }
      
      .entity-type-header.collapsed:after {
        transform: rotate(-90deg);
      }
      
      .entity-type-container {
        margin: 0 0 10px 10px;
        max-height: 500px;
        overflow-y: auto;
      }
      
      .entity-type-container.collapsed {
        display: none;
      }
      
      .entity-item {
        padding: 8px;
        margin-bottom: 2px;
        background-color: rgba(255, 255, 255, 0.05);
        border-radius: 4px;
        cursor: pointer;
        transition: background-color 0.2s;
      }
      
      .entity-item:hover {
        background-color: rgba(255, 255, 255, 0.1);
      }
      
      .entity-item.selected {
        background-color: rgba(0, 180, 255, 0.3);
        border-left: 3px solid #00b4ff;
      }
      
      .entity-name {
        font-weight: bold;
        margin-bottom: 4px;
      }
      
      .entity-position {
        font-size: 12px;
        color: rgba(255, 255, 255, 0.7);
      }
      
      .entity-sector {
        font-size: 12px;
        color: rgba(255, 255, 255, 0.7);
        margin-top: 2px;
      }
    `;

    document.head.appendChild(style);
  }
}
