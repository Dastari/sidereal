use bevy::ecs::system::SystemParam;
use bevy::log::info;
use bevy::prelude::*;
use bevy::ui::FocusPolicy;
use lightyear::prelude::client::Client;
use lightyear::prelude::client::Connected;
use lightyear::prelude::{MessageReceiver, MessageSender};
use sidereal_net::{
    ClientNotificationDismissedMessage, NotificationChannel, NotificationPlacement,
    NotificationSeverity, PlayerEntityId, ServerNotificationMessage,
};
use sidereal_ui::theme::{ActiveUiTheme, UiSemanticTone, UiVisualSettings, theme_definition};
use sidereal_ui::typography::text_font;
use sidereal_ui::widgets::{
    UiButtonVariant, UiInteractionState, button_surface, panel_surface_with_tone,
    spawn_hud_frame_chrome_with_tone,
};
use std::collections::{HashMap, HashSet, VecDeque};

use super::app_state::{ClientAppState, ClientSession};
use super::assets::LocalAssetManager;
use super::ecs_util::queue_despawn_if_exists;
use super::resources::{AssetCacheAdapter, AssetRootPath, EmbeddedFonts};

const MAX_VISIBLE_PER_PLACEMENT: usize = 5;
const TOAST_WIDTH_PX: f32 = 360.0;
const TOAST_MIN_HEIGHT_PX: f32 = 86.0;
const TOAST_OFFSET_PX: f32 = 24.0;
const TOAST_STEP_PX: f32 = 112.0;
const TOAST_GAP_PX: f32 = 12.0;
const TOAST_Z_INDEX: i32 = 900;

#[derive(Component)]
struct NotificationToastRoot;

#[derive(Component)]
struct NotificationToastCard;

#[derive(Component)]
struct NotificationToastCloseButton {
    notification_id: String,
}

#[derive(Debug, Clone)]
struct NotificationToast {
    message: ServerNotificationMessage,
    received_at_s: f64,
    dismissed_sent: bool,
}

#[derive(Resource, Default)]
pub struct NotificationQueue {
    toasts: VecDeque<NotificationToast>,
    received_ids: HashSet<String>,
    dirty: bool,
}

#[derive(Resource, Default)]
struct NotificationImageCache {
    handles_by_asset_id: HashMap<String, Handle<Image>>,
}

#[derive(SystemParam)]
struct NotificationUiAssets<'w> {
    images: ResMut<'w, Assets<Image>>,
    image_cache: ResMut<'w, NotificationImageCache>,
    fonts: Res<'w, EmbeddedFonts>,
    asset_root: Res<'w, AssetRootPath>,
    asset_manager: Res<'w, LocalAssetManager>,
    cache_adapter: Res<'w, AssetCacheAdapter>,
}

impl NotificationQueue {
    pub(crate) fn push_local_notification(
        &mut self,
        message: ServerNotificationMessage,
        received_at_s: f64,
    ) -> bool {
        if !self.received_ids.insert(message.notification_id.clone()) {
            return false;
        }
        self.toasts.push_back(NotificationToast {
            message,
            received_at_s,
            dismissed_sent: false,
        });
        self.dirty = true;
        true
    }

    #[cfg(test)]
    fn push_for_test(&mut self, message: ServerNotificationMessage, received_at_s: f64) {
        self.push_local_notification(message, received_at_s);
    }

    #[cfg(test)]
    fn visible_count_for(&self, placement: NotificationPlacement) -> usize {
        visible_toasts(self)
            .into_iter()
            .filter(|toast| toast.message.placement == placement)
            .count()
            .min(MAX_VISIBLE_PER_PLACEMENT)
    }
}

pub fn register_notification_ui(app: &mut App) {
    app.init_resource::<NotificationQueue>();
    app.init_resource::<NotificationImageCache>();
    app.add_systems(
        Update,
        (
            receive_server_notifications,
            expire_notifications,
            handle_notification_interactions,
            sync_notification_ui,
        )
            .chain()
            .run_if(in_state(ClientAppState::InWorld)),
    );
    app.add_systems(OnExit(ClientAppState::InWorld), cleanup_notification_ui);
}

fn receive_server_notifications(
    mut queue: ResMut<'_, NotificationQueue>,
    session: Res<'_, ClientSession>,
    time: Res<'_, Time>,
    mut receivers: Query<
        '_,
        '_,
        &'_ mut MessageReceiver<ServerNotificationMessage>,
        (With<Client>, With<Connected>),
    >,
) {
    let Some(local_player_id) = session.player_entity_id.as_deref() else {
        return;
    };
    for mut receiver in &mut receivers {
        let messages = receiver.receive().collect::<Vec<_>>();
        for message in messages {
            if !same_player(local_player_id, &message.player_entity_id) {
                continue;
            }
            if !queue.received_ids.insert(message.notification_id.clone()) {
                continue;
            }
            queue.toasts.push_back(NotificationToast {
                message,
                received_at_s: time.elapsed_secs_f64(),
                dismissed_sent: false,
            });
            queue.dirty = true;
            info!("client queued server notification for player={local_player_id}");
        }
    }
}

fn expire_notifications(
    mut queue: ResMut<'_, NotificationQueue>,
    session: Res<'_, ClientSession>,
    time: Res<'_, Time>,
    mut senders: Query<
        '_,
        '_,
        &'_ mut MessageSender<ClientNotificationDismissedMessage>,
        With<Client>,
    >,
) {
    let now_s = time.elapsed_secs_f64();
    let Some(player_entity_id) = session.player_entity_id.as_deref() else {
        return;
    };
    let mut expired_ids = Vec::new();
    for toast in &mut queue.toasts {
        let Some(duration_s) = toast.message.auto_dismiss_after_s else {
            continue;
        };
        if now_s - toast.received_at_s >= f64::from(duration_s) {
            if !toast.dismissed_sent {
                send_dismissal(
                    &mut senders,
                    player_entity_id,
                    &toast.message.notification_id,
                );
                toast.dismissed_sent = true;
            }
            expired_ids.push(toast.message.notification_id.clone());
        }
    }
    if !expired_ids.is_empty() {
        let expired = expired_ids.into_iter().collect::<HashSet<_>>();
        queue
            .toasts
            .retain(|toast| !expired.contains(&toast.message.notification_id));
        queue.dirty = true;
    }
}

#[allow(clippy::type_complexity)]
fn handle_notification_interactions(
    mut queue: ResMut<'_, NotificationQueue>,
    session: Res<'_, ClientSession>,
    active_theme: Res<'_, ActiveUiTheme>,
    visual_settings: Res<'_, UiVisualSettings>,
    mut senders: Query<
        '_,
        '_,
        &'_ mut MessageSender<ClientNotificationDismissedMessage>,
        With<Client>,
    >,
    mut interactions: Query<
        '_,
        '_,
        (
            &'_ Interaction,
            &'_ NotificationToastCloseButton,
            &'_ mut BackgroundColor,
            &'_ mut BorderColor,
            &'_ mut BoxShadow,
        ),
        Changed<Interaction>,
    >,
) {
    let theme = theme_definition(active_theme.0);
    let glow_intensity = visual_settings.glow_intensity();
    let Some(player_entity_id) = session.player_entity_id.as_deref() else {
        return;
    };

    for (interaction, button, mut bg_color, mut border_color, mut shadow) in &mut interactions {
        if *interaction == Interaction::Pressed {
            let mut sent = false;
            for toast in &mut queue.toasts {
                if toast.message.notification_id == button.notification_id {
                    if !toast.dismissed_sent {
                        send_dismissal(&mut senders, player_entity_id, &button.notification_id);
                        toast.dismissed_sent = true;
                    }
                    sent = true;
                    break;
                }
            }
            if sent {
                queue
                    .toasts
                    .retain(|toast| toast.message.notification_id != button.notification_id);
                queue.dirty = true;
            }
        }
        let state = match *interaction {
            Interaction::Pressed => UiInteractionState::Pressed,
            Interaction::Hovered => UiInteractionState::Hovered,
            Interaction::None => UiInteractionState::Idle,
        };
        let (next_bg, next_border, next_shadow) =
            button_surface(theme, UiButtonVariant::Ghost, state, glow_intensity);
        *bg_color = next_bg;
        *border_color = next_border;
        *shadow = next_shadow;
    }
}

fn sync_notification_ui(
    mut commands: Commands<'_, '_>,
    mut queue: ResMut<'_, NotificationQueue>,
    mut ui_assets: NotificationUiAssets<'_>,
    active_theme: Res<'_, ActiveUiTheme>,
    visual_settings: Res<'_, UiVisualSettings>,
    roots: Query<'_, '_, Entity, With<NotificationToastRoot>>,
) {
    if !queue.dirty {
        return;
    }
    for root in &roots {
        queue_despawn_if_exists(&mut commands, root);
    }
    queue.dirty = false;

    let visible = visible_toasts(&queue);
    if visible.is_empty() {
        return;
    }

    let theme = theme_definition(active_theme.0);
    let glow_intensity = visual_settings.glow_intensity();

    commands
        .spawn((
            Name::new("NotificationToastRoot"),
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            ZIndex(TOAST_Z_INDEX),
            NotificationToastRoot,
            FocusPolicy::Pass,
        ))
        .with_children(|root| {
            let mut placement_counts = HashMap::<NotificationPlacement, usize>::new();
            for toast in visible {
                let index = placement_counts.entry(toast.message.placement).or_default();
                if *index >= MAX_VISIBLE_PER_PLACEMENT {
                    continue;
                }
                let node = toast_node(toast.message.placement, *index);
                *index += 1;
                let tone = severity_tone(toast.message.severity);
                let accent_color = tone.accent_color(theme);
                let foreground_color = tone.foreground_color(theme);
                let (panel_bg, panel_border, panel_shadow) =
                    panel_surface_with_tone(theme, glow_intensity, tone);
                let (button_bg, button_border, button_shadow) = button_surface(
                    theme,
                    UiButtonVariant::Ghost,
                    UiInteractionState::Idle,
                    glow_intensity,
                );
                root.spawn((
                    Name::new("NotificationToastCard"),
                    node,
                    panel_bg,
                    panel_border,
                    panel_shadow.clone(),
                    NotificationToastCard,
                    FocusPolicy::Pass,
                ))
                .with_children(|card| {
                    spawn_hud_frame_chrome_with_tone(
                        card,
                        &mut ui_assets.images,
                        theme,
                        Some(severity_label(toast.message.severity)),
                        &ui_assets.fonts.mono,
                        glow_intensity,
                        tone,
                    );
                    card.spawn((
                        Node {
                            width: Val::Percent(100.0),
                            flex_direction: FlexDirection::Row,
                            align_items: AlignItems::FlexStart,
                            column_gap: Val::Px(TOAST_GAP_PX),
                            ..default()
                        },
                        FocusPolicy::Pass,
                    ))
                    .with_children(|row| {
                        if let Some(image_handle) = toast.message.image.as_ref().and_then(|image| {
                            notification_image_handle(
                                image.asset_id.as_str(),
                                &ui_assets.asset_manager,
                                &ui_assets.asset_root.0,
                                *ui_assets.cache_adapter,
                                &mut ui_assets.image_cache,
                                &mut ui_assets.images,
                            )
                        }) {
                            row.spawn((
                                Node {
                                    width: Val::Px(54.0),
                                    height: Val::Px(54.0),
                                    min_width: Val::Px(54.0),
                                    border: UiRect::all(Val::Px(theme.metrics.control_border_px)),
                                    ..default()
                                },
                                ImageNode::new(image_handle),
                                BorderColor::all(accent_color),
                                FocusPolicy::Pass,
                            ));
                        }
                        row.spawn((
                            Node {
                                flex_direction: FlexDirection::Column,
                                row_gap: Val::Px(4.0),
                                flex_grow: 1.0,
                                ..default()
                            },
                            FocusPolicy::Pass,
                        ))
                        .with_children(|text_column| {
                            text_column.spawn((
                                Text::new(toast.message.title.clone()),
                                text_font(ui_assets.fonts.display.clone(), 16.0),
                                TextColor(foreground_color),
                                FocusPolicy::Pass,
                            ));
                            text_column.spawn((
                                Text::new(toast.message.body.clone()),
                                text_font(ui_assets.fonts.regular.clone(), 15.0),
                                TextColor(foreground_color),
                                FocusPolicy::Pass,
                            ));
                        });
                        row.spawn((
                            Button,
                            NotificationToastCloseButton {
                                notification_id: toast.message.notification_id.clone(),
                            },
                            Node {
                                min_width: Val::Px(30.0),
                                width: Val::Px(30.0),
                                height: Val::Px(30.0),
                                border: UiRect::all(Val::Px(theme.metrics.control_border_px)),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                ..default()
                            },
                            button_bg,
                            button_border,
                            button_shadow,
                        ))
                        .with_children(|button| {
                            button.spawn((
                                Text::new("X"),
                                text_font(ui_assets.fonts.mono_bold.clone(), 14.0),
                                TextColor(foreground_color),
                            ));
                        });
                    });
                });
            }
        });
}

fn cleanup_notification_ui(
    mut commands: Commands<'_, '_>,
    mut queue: ResMut<'_, NotificationQueue>,
    roots: Query<'_, '_, Entity, With<NotificationToastRoot>>,
) {
    for root in &roots {
        queue_despawn_if_exists(&mut commands, root);
    }
    queue.toasts.clear();
    queue.received_ids.clear();
    queue.dirty = false;
}

fn visible_toasts(queue: &NotificationQueue) -> Vec<&NotificationToast> {
    queue.toasts.iter().rev().collect::<Vec<_>>()
}

fn toast_node(placement: NotificationPlacement, index: usize) -> Node {
    let mut node = Node {
        position_type: PositionType::Absolute,
        width: Val::Px(TOAST_WIDTH_PX),
        min_height: Val::Px(TOAST_MIN_HEIGHT_PX),
        padding: UiRect::all(Val::Px(16.0)),
        border: UiRect::all(Val::Px(1.0)),
        overflow: Overflow::visible(),
        flex_direction: FlexDirection::Column,
        row_gap: Val::Px(8.0),
        ..default()
    };
    let offset = TOAST_OFFSET_PX + TOAST_STEP_PX * index as f32;
    match placement {
        NotificationPlacement::TopLeft => {
            node.left = Val::Px(TOAST_OFFSET_PX);
            node.top = Val::Px(offset);
        }
        NotificationPlacement::TopCenter => {
            node.left = Val::Percent(50.0);
            node.margin = UiRect::left(Val::Px(-(TOAST_WIDTH_PX * 0.5)));
            node.top = Val::Px(offset);
        }
        NotificationPlacement::TopRight => {
            node.right = Val::Px(TOAST_OFFSET_PX);
            node.top = Val::Px(offset);
        }
        NotificationPlacement::BottomLeft => {
            node.left = Val::Px(TOAST_OFFSET_PX);
            node.bottom = Val::Px(offset);
        }
        NotificationPlacement::BottomCenter => {
            node.left = Val::Percent(50.0);
            node.margin = UiRect::left(Val::Px(-(TOAST_WIDTH_PX * 0.5)));
            node.bottom = Val::Px(offset);
        }
        NotificationPlacement::BottomRight => {
            node.right = Val::Px(TOAST_OFFSET_PX);
            node.bottom = Val::Px(offset);
        }
    }
    node
}

fn send_dismissal(
    senders: &mut Query<
        '_,
        '_,
        &'_ mut MessageSender<ClientNotificationDismissedMessage>,
        With<Client>,
    >,
    player_entity_id: &str,
    notification_id: &str,
) {
    let message = ClientNotificationDismissedMessage {
        player_entity_id: canonical_player_entity_id(player_entity_id),
        notification_id: notification_id.to_string(),
    };
    for mut sender in senders {
        sender.send::<NotificationChannel>(message.clone());
    }
}

fn notification_image_handle(
    asset_id: &str,
    asset_manager: &LocalAssetManager,
    asset_root: &str,
    cache_adapter: AssetCacheAdapter,
    image_cache: &mut NotificationImageCache,
    images: &mut Assets<Image>,
) -> Option<Handle<Image>> {
    if let Some(handle) = image_cache.handles_by_asset_id.get(asset_id) {
        return Some(handle.clone());
    }
    let handle = super::assets::cached_image_handle(
        asset_id,
        asset_manager,
        asset_root,
        cache_adapter,
        images,
    )?;
    image_cache
        .handles_by_asset_id
        .insert(asset_id.to_string(), handle.clone());
    Some(handle)
}

fn same_player(left: &str, right: &str) -> bool {
    canonical_player_entity_id(left) == canonical_player_entity_id(right)
}

fn canonical_player_entity_id(raw: &str) -> String {
    PlayerEntityId::parse(raw)
        .map(PlayerEntityId::canonical_wire_id)
        .unwrap_or_else(|| raw.to_string())
}

fn severity_tone(severity: NotificationSeverity) -> UiSemanticTone {
    match severity {
        NotificationSeverity::Info => UiSemanticTone::Info,
        NotificationSeverity::Success => UiSemanticTone::Success,
        NotificationSeverity::Warning => UiSemanticTone::Warning,
        NotificationSeverity::Error => UiSemanticTone::Danger,
    }
}

fn severity_label(severity: NotificationSeverity) -> &'static str {
    match severity {
        NotificationSeverity::Info => "Info",
        NotificationSeverity::Success => "Status",
        NotificationSeverity::Warning => "Warning",
        NotificationSeverity::Error => "Error",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sidereal_net::NotificationPayload;

    fn message(id: &str, placement: NotificationPlacement) -> ServerNotificationMessage {
        ServerNotificationMessage {
            notification_id: id.to_string(),
            player_entity_id: "11111111-1111-1111-1111-111111111111".to_string(),
            title: "Title".to_string(),
            body: "Body".to_string(),
            severity: NotificationSeverity::Info,
            placement,
            image: None,
            payload: NotificationPayload::Generic {
                event_type: "test".to_string(),
                data: serde_json::json!({}),
            },
            created_at_epoch_s: 1,
            auto_dismiss_after_s: Some(5.0),
        }
    }

    #[test]
    fn queue_dedupes_received_ids() {
        let mut queue = NotificationQueue::default();
        queue.push_for_test(message("a", NotificationPlacement::BottomRight), 0.0);
        queue.push_for_test(message("b", NotificationPlacement::BottomRight), 0.0);

        assert_eq!(
            queue.visible_count_for(NotificationPlacement::BottomRight),
            2
        );
    }

    #[test]
    fn visible_count_caps_per_placement() {
        let mut queue = NotificationQueue::default();
        for index in 0..8 {
            queue.push_for_test(
                message(&format!("id-{index}"), NotificationPlacement::TopLeft),
                0.0,
            );
        }

        assert_eq!(
            queue.visible_count_for(NotificationPlacement::TopLeft),
            MAX_VISIBLE_PER_PLACEMENT
        );
    }

    #[test]
    fn default_placement_is_bottom_right() {
        assert_eq!(
            NotificationPlacement::default(),
            NotificationPlacement::BottomRight
        );
    }
}
