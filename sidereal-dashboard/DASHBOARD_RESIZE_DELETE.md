# Dashboard Enhancements: Resizable Sidebar & Entity Deletion

**Date:** 2026-02-18  
**Status:** ✅ Complete

## Features Implemented

### 1. Resizable Sidebar ✅

The Sidereal Explorer sidebar is now horizontally resizable with a drag handle.

**Changes:**

- **`AppLayout.tsx`**: Added resize functionality with mouse drag events
  - Drag handle on right edge of sidebar
  - Min width: 200px, Max width: 600px
  - Visual feedback during drag (hover and active states)
  - Persists width via callback to parent component

- **`index.tsx`**: Added sidebar width state management
  - `sidebarWidth` state (default: 280px)
  - `onSidebarResize` callback passed to AppLayout

**UX:**

- Hover over right edge of sidebar to see resize cursor
- Click and drag to adjust width
- Visual indicator (colored bar) shows drag area
- Smooth resize with constraints

### 2. Entity Deletion ✅

Delete entities from both database and live world with visual confirmation.

**API Endpoints Created:**

1. **`/api/delete-entity/$entityId`** (Database deletion)
   - Uses Apache AGE Cypher query
   - Deletes entity and all HAS_COMPONENT relationships
   - Cascades to components

2. **`/api/delete-live-entity/$entityId`** (Live world deletion)
   - Uses Bevy Remote Protocol (BRP)
   - Calls `bevy/destroy` method
   - Removes entity from running simulation

**UI Changes:**

- **`EntityTree.tsx`**: Added delete icon next to each entity
  - Trash icon appears on hover
  - Shows for both root and child entities
  - Confirmation dialog before deletion
  - Warning if entity has children
  - Loading state during deletion (pulsing icon)
  - Disabled state prevents double-clicks

- **`index.tsx`**: Added delete handler
  - Routes to correct API based on `sourceMode`
  - Refreshes data after deletion
  - Clears selection if deleted entity was selected

**User Flow:**

1. Hover over entity in tree
2. Click trash icon
3. Confirm deletion in dialog
4. Entity removed from world/database
5. Tree refreshes automatically

## Files Modified

### Core Layout

- **`src/components/layout/AppLayout.tsx`**: Resizable sidebar implementation
- **`src/routes/index.tsx`**: State management for width and delete handler

### Entity Tree

- **`src/components/sidebar/EntityTree.tsx`**: Delete UI with trash icon

### API Routes (NEW)

- **`src/routes/api.delete-entity.$entityId.tsx`**: Database entity deletion
- **`src/routes/api.delete-live-entity.$entityId.tsx`**: Live entity deletion via BRP

## Technical Details

### Resize Implementation

```tsx
// Mouse event handling
const [isDragging, setIsDragging] = useState(false)
const [currentWidth, setCurrentWidth] = useState(sidebarWidth)

useEffect(() => {
  if (!isDragging) return

  const handleMouseMove = (e: MouseEvent) => {
    const newWidth = Math.max(200, Math.min(600, e.clientX))
    setCurrentWidth(newWidth)
  }

  const handleMouseUp = () => {
    setIsDragging(false)
    onSidebarResize?.(currentWidth)
  }

  // ... event listeners
}, [isDragging, currentWidth, onSidebarResize])
```

### Delete Flow

```tsx
const handleDeleteEntity = async (entityId: string) => {
  const endpoint =
    sourceMode === 'live'
      ? `/api/delete-live-entity/${entityId}`
      : `/api/delete-entity/${entityId}`

  const response = await fetch(endpoint, { method: 'DELETE' })
  const result = await response.json()

  if (!result.success) {
    throw new Error(result.error || 'Failed to delete entity')
  }

  await loadData() // Refresh
  if (selectedId === entityId) {
    setSelectedId(null) // Clear selection
  }
}
```

## Database Deletion Query

```cypher
MATCH (e:Entity {entity_id: $entity_id})
OPTIONAL MATCH (e)-[:HAS_COMPONENT]->(c:Component)
DETACH DELETE e, c
```

This query:

- Finds the entity by ID
- Finds all components linked via HAS_COMPONENT
- Detaches all relationships
- Deletes entity and components

## Live Deletion (BRP)

```typescript
await callBrp({
  method: 'bevy/destroy',
  params: { entity: entityId },
})
```

This calls Bevy's entity despawn command through the remote protocol.

## Safety Features

1. **Confirmation Dialog**: Prevents accidental deletion
2. **Child Warning**: Alerts user if entity has children
3. **Loading State**: Prevents duplicate requests
4. **Error Handling**: Shows user-friendly error messages
5. **Auto Refresh**: Updates UI after deletion
6. **Selection Clear**: Removes deleted entity from selection

## Testing Checklist

- [x] TypeScript compilation passes
- [x] ESLint/Prettier passes
- [ ] Visual: Sidebar resizes smoothly
- [ ] Visual: Min/max width constraints work
- [ ] Visual: Delete icon appears on hover
- [ ] Functional: Database entity deletion works
- [ ] Functional: Live entity deletion works
- [ ] Functional: Confirmation dialog shows
- [ ] Functional: Child warning displays correctly
- [ ] Functional: Tree refreshes after deletion
- [ ] Functional: Selection clears when deleted entity selected
- [ ] Edge case: Deleting parent removes children from tree
- [ ] Edge case: Cannot delete during pending deletion

---

**Status:** Complete and ready for testing ✅  
**Risk:** Low (guarded with confirmations, error handling)  
**Compatibility:** Works with both database and live modes
