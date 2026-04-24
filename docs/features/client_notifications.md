# Client Notifications

Status: Active partial implementation spec
Last updated: 2026-04-24
Owners: client runtime + replication + scripting
Scope: server-authored non-blocking notification delivery, Bevy toast presentation, and player-scoped notification history

Primary references:
- `docs/ui_design_guide.md`
- `docs/features/scripting_support.md`
- `docs/features/visibility_replication_contract.md`
- `docs/features/asset_delivery_contract.md`

## 0. Implementation Status

2026-04-24 status note:

1. Implemented: a Lightyear notification channel carries server-authored notification payloads to the authenticated client for a player.
2. Implemented: native Bevy UI renders notification toasts with `sidereal-ui` panel, button, and HUD-frame primitives. Default placement is bottom right; top/bottom left/center/right placements are supported.
3. Implemented: notifications are persisted in the SQL `player_notifications` history table keyed by canonical `player_entity_id`.
4. Implemented: landmark discovery emits a non-blocking notification after authoritative server discovery updates `DiscoveredStaticLandmarks`.
5. Implemented: authoritative Lua runtime scripts can request player notifications through `ctx:notify_player(...)`; the host validates and converts requests into server notification commands.
6. Partial/open: no notification history browser is implemented yet, and image rendering currently preserves logical `asset_id` in the payload without adding new image assets or a dedicated toast image resolver.
7. Native impact: active Bevy UI path. WASM impact: protocol and queue model are shared-client compatible; live browser validation remains part of deferred WASM parity follow-through.

## 1. Contract

Notifications are presentation/history events, not authoritative gameplay state.

Rules:

1. The server authors notification payloads.
2. The client only renders and dismisses notifications for its authenticated selected player.
3. Dismissal messages are validated against the transport session binding before database updates.
4. Critical user-actionable failures remain modal dialogs via `dialog_ui::DialogQueue::push_error()`.
5. Notification payloads may reference logical asset IDs, but replication does not stream image bytes.
6. Notification history is stored in SQL as a player-scoped read/history model. Runtime progression remains on persisted player ECS components.

## 2. Protocol Shape

The notification lane uses:

1. `ServerNotificationMessage`
2. `ClientNotificationDismissedMessage`
3. `NotificationChannel`

Supported severities:

1. `Info`
2. `Success`
3. `Warning`
4. `Error`

Supported placements:

1. `TopLeft`
2. `TopCenter`
3. `TopRight`
4. `BottomLeft`
5. `BottomCenter`
6. `BottomRight`

Current payload variants:

1. `Generic`
2. `LandmarkDiscovery`

## 3. Persistence

The SQL table is `player_notifications`.

Required fields:

1. `notification_id`
2. `player_entity_id`
3. `notification_kind`
4. `severity`
5. `title`
6. `body`
7. `placement`
8. `payload`
9. `created_at_epoch_s`

Optional fields:

1. `image_asset_id`
2. `image_alt_text`
3. `delivered_at_epoch_s`
4. `dismissed_at_epoch_s`

## 4. UI Behavior

Toasts:

1. use `sidereal-ui` panel surfaces and HUD frame chrome,
2. include an explicit close button,
3. stack up to five visible toasts per placement,
4. auto-dismiss by severity default unless a payload overrides the duration,
5. use semantic theme colors for severity accents.

Default durations:

1. info: 5 seconds,
2. success: 5 seconds,
3. warning: 7 seconds,
4. error: 9 seconds.

## 5. Lua Boundary

Scripts request notifications through validated host intent APIs only.

Example:

```lua
ctx:notify_player({
  player_entity_id = "11111111-1111-1111-1111-111111111111",
  title = "Objective Updated",
  body = "Return to station.",
  severity = "info",
  placement = "bottom_right",
  event_type = "objective_update",
  data = { objective_id = "starter_return" },
})
```

Lua does not receive UI handles, database handles, or client authority.
