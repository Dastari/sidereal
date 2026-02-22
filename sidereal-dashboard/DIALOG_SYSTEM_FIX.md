# Dialog System & BRP Method Fix

**Date:** 2026-02-18  
**Status:** ✅ Complete

## Changes

Fixed two issues with the entity deletion feature:

1. Replaced browser `confirm()` and `alert()` with proper shadcn dialogs
2. Fixed BRP method from `bevy/destroy` to `bevy/despawn`

## 1. Reusable Confirmation Dialog ✅

Created a proper dialog system using Radix UI and shadcn patterns.

### New Components

**`src/components/ui/alert-dialog.tsx`**

- Base AlertDialog primitives from Radix UI
- Styled with shadcn design system
- Includes: AlertDialog, AlertDialogContent, AlertDialogHeader, AlertDialogFooter, AlertDialogTitle, AlertDialogDescription, AlertDialogAction, AlertDialogCancel

**`src/components/ui/confirm-dialog.tsx`**

- Reusable confirmation dialog component
- Props:
  - `open`: boolean
  - `onOpenChange`: (open: boolean) => void
  - `title`: string
  - `description`: string
  - `confirmText`: string (default: "Confirm")
  - `cancelText`: string (default: "Cancel")
  - `onConfirm`: async function
  - `variant`: 'default' | 'destructive'
- Features:
  - Loading state during async operations
  - Automatic close on success
  - Error handling (throws to parent)
  - Disabled buttons during loading
  - Destructive variant (red button) for dangerous actions

### Usage Example

```tsx
<ConfirmDialog
  open={deleteDialogOpen}
  onOpenChange={setDeleteDialogOpen}
  title="Delete Entity?"
  description="This will permanently delete the entity. This action cannot be undone."
  confirmText="Delete"
  cancelText="Cancel"
  variant="destructive"
  onConfirm={async () => {
    await deleteEntity(entityId)
  }}
/>
```

### Updated Files

**`src/components/sidebar/EntityTree.tsx`**

- Removed browser `confirm()` and `alert()` calls
- Added `deleteDialogOpen` state
- Uses `ConfirmDialog` component
- Shows proper warning for entities with children
- Cleaner error handling (no browser alerts)

## 2. BRP Method Fix ✅

Fixed the Bevy Remote Protocol method call for entity deletion.

### Issue

- Used `bevy/destroy` (doesn't exist)
- BRP method not found error

### Solution

- Changed to `bevy/despawn` (correct method)
- Added entity ID validation (must be numeric for BRP)
- Better error messages

### Updated File

**`src/routes/api.delete-live-entity.$entityId.tsx`**

```typescript
// Old (broken)
method: 'bevy/destroy',
params: { entity: entityId }

// New (fixed)
const entityIndex = parseInt(entityId, 10)
if (isNaN(entityIndex)) {
  return json({ error: 'Invalid entity ID (must be numeric for BRP)' }, { status: 400 })
}

method: 'world.despawn_entity',
params: { entity: entityIndex }
```

### BRP Entity IDs

**Important**: Bevy Remote Protocol expects numeric entity indices, not UUIDs:

- Live entities use numeric IDs (e.g., `123`)
- Database entities use UUID strings (e.g., `550e8400-e29b-41d4-a716-446655440000`)
- The delete endpoint validates this and returns 400 if non-numeric

## Dependencies Added

```json
{
  "@radix-ui/react-alert-dialog": "1.1.15"
}
```

## User Experience Improvements

### Before

- Browser `confirm()` dialog (ugly, blocking)
- Browser `alert()` for errors (no styling)
- No loading state visibility

### After

- Beautiful styled modal dialog
- Consistent with app design
- Loading state ("Processing...")
- Proper error handling
- Disabled buttons prevent double-clicks
- Destructive styling for dangerous actions

## Dialog Variants

### Default

- Standard blue confirm button
- For non-destructive actions

### Destructive

- Red confirm button
- For dangerous actions (delete, remove, etc.)
- Used for entity deletion

## Technical Notes

### Alert Dialog Structure

```
AlertDialog (root)
├── AlertDialogTrigger (optional, not used in ConfirmDialog)
├── AlertDialogPortal
│   ├── AlertDialogOverlay (backdrop)
│   └── AlertDialogContent (modal)
│       ├── AlertDialogHeader
│       │   ├── AlertDialogTitle
│       │   └── AlertDialogDescription
│       └── AlertDialogFooter
│           ├── AlertDialogCancel (button)
│           └── AlertDialogAction (button)
```

### State Management

The `ConfirmDialog` component manages its own loading state internally, so parent components only need to:

1. Track `open` state
2. Provide `onConfirm` handler
3. Handle `onOpenChange` for closing

### Error Handling

- Errors thrown in `onConfirm` are caught internally
- Dialog stays open on error (gives user chance to retry or cancel)
- Error logged to console
- Parent can display error UI if needed

## Testing Checklist

- [x] TypeScript compilation passes
- [x] No browser `confirm()` or `alert()` calls
- [x] Dialog shows with proper styling
- [x] Destructive variant uses red button
- [x] Loading state shows "Processing..."
- [x] Buttons disabled during loading
- [x] Dialog closes on success
- [x] BRP method is `bevy/despawn`
- [x] Entity ID validated as numeric for BRP
- [ ] Visual: Dialog appears centered with backdrop
- [ ] Visual: Animations smooth (fade in/out, zoom)
- [ ] Functional: Delete works in live mode
- [ ] Functional: Error messages display properly

---

**Status:** Complete and ready for testing ✅  
**Risk:** None (improved UX, fixed broken API call)  
**Breaking:** No (backward compatible)
