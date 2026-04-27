use bevy::prelude::*;
use lightyear::prelude::client::Connected;
use lightyear::prelude::server::{ClientOf, LinkOf};
use lightyear::prelude::{
    MessageReceiver, NetworkTarget, RemoteId, Server, ServerMultiMessageSender,
};
use sidereal_net::{
    ClientNotificationDismissedMessage, NotificationChannel, NotificationImageRef,
    NotificationPayload, NotificationPlacement, NotificationSeverity, PlayerEntityId,
    ServerNotificationMessage,
};
use sidereal_persistence::{GraphPersistence, PlayerNotificationRecord};
use std::collections::{HashSet, VecDeque};
use std::sync::mpsc::{Receiver, SyncSender, TrySendError, sync_channel};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

use crate::replication::auth::AuthenticatedClientBindings;

const DEFAULT_INFO_DISMISS_S: f32 = 5.0;
const DEFAULT_SUCCESS_DISMISS_S: f32 = 5.0;
const DEFAULT_WARNING_DISMISS_S: f32 = 7.0;
const DEFAULT_ERROR_DISMISS_S: f32 = 9.0;
const PLAYER_ENTERED_WORLD_EVENT_TYPE: &str = "player_entered_world";

#[derive(Debug, Clone)]
pub struct NotificationCommand {
    pub player_entity_id: String,
    pub title: String,
    pub body: String,
    pub severity: NotificationSeverity,
    pub placement: NotificationPlacement,
    pub image: Option<NotificationImageRef>,
    pub payload: NotificationPayload,
    pub auto_dismiss_after_s: Option<f32>,
}

#[derive(Resource, Default)]
pub struct NotificationCommandQueue {
    pending: VecDeque<NotificationCommand>,
}

impl NotificationCommandQueue {
    pub fn push(&mut self, command: NotificationCommand) {
        self.pending.push_back(command);
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.pending.len()
    }
}

#[derive(Debug)]
enum NotificationPersistenceRequest {
    Insert(Box<PlayerNotificationRecord>),
    MarkDelivered {
        player_entity_id: String,
        notification_id: String,
        delivered_at_epoch_s: i64,
    },
    MarkDismissed {
        player_entity_id: String,
        notification_id: String,
        dismissed_at_epoch_s: i64,
    },
}

#[derive(Resource, Default)]
pub struct NotificationDeliveryState {
    pending: VecDeque<ServerNotificationMessage>,
    delivered_ids: HashSet<String>,
}

#[derive(Resource, Default)]
pub struct NotificationPersistenceWorker {
    sender: Option<SyncSender<NotificationPersistenceRequest>>,
    dropped_requests: u64,
}

pub fn init_resources(app: &mut App) {
    app.insert_resource(NotificationCommandQueue::default());
    app.insert_resource(NotificationDeliveryState::default());
    app.insert_resource(NotificationPersistenceWorker::default());
}

pub fn start_notification_persistence_worker(world: &mut World) {
    let database_url = std::env::var("REPLICATION_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://sidereal:sidereal@127.0.0.1:5432/sidereal".to_string());
    let (sender, receiver) = sync_channel::<NotificationPersistenceRequest>(1024);
    thread::Builder::new()
        .name("replication-notification-history-writer".to_string())
        .spawn(move || notification_persistence_loop(receiver, database_url))
        .expect("failed to start notification persistence worker thread");
    world.resource_mut::<NotificationPersistenceWorker>().sender = Some(sender);
}

fn notification_persistence_loop(
    receiver: Receiver<NotificationPersistenceRequest>,
    database_url: String,
) {
    let mut persistence = match GraphPersistence::connect(&database_url) {
        Ok(mut persistence) => {
            if let Err(err) = persistence.ensure_player_notifications_schema() {
                error!("notification persistence schema init failed: {err}");
                return;
            }
            persistence
        }
        Err(err) => {
            error!("notification persistence connect failed: {err}");
            return;
        }
    };

    while let Ok(request) = receiver.recv() {
        let result = match request {
            NotificationPersistenceRequest::Insert(record) => {
                persistence.insert_player_notification(&record)
            }
            NotificationPersistenceRequest::MarkDelivered {
                player_entity_id,
                notification_id,
                delivered_at_epoch_s,
            } => persistence
                .mark_player_notification_delivered(
                    &player_entity_id,
                    &notification_id,
                    delivered_at_epoch_s,
                )
                .map(|_| ()),
            NotificationPersistenceRequest::MarkDismissed {
                player_entity_id,
                notification_id,
                dismissed_at_epoch_s,
            } => persistence
                .mark_player_notification_dismissed(
                    &player_entity_id,
                    &notification_id,
                    dismissed_at_epoch_s,
                )
                .map(|_| ()),
        };
        if let Err(err) = result {
            warn!("notification persistence request failed: {err}");
        }
    }
}

pub fn enqueue_player_notification(
    queue: &mut NotificationCommandQueue,
    command: NotificationCommand,
) {
    queue.push(command);
}

pub(crate) fn enqueue_player_entered_world_notifications(
    queue: &mut NotificationCommandQueue,
    bindings: &AuthenticatedClientBindings,
    entering_player_entity_id: &str,
    entering_display_name: Option<&str>,
) -> usize {
    let entering_player_entity_id = canonical_player_entity_id(entering_player_entity_id);
    let entering_display_name = entering_display_name
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("A player");

    let mut recipient_player_entity_ids = HashSet::<String>::new();
    for bound_player_entity_id in bindings.by_client_entity.values() {
        let recipient_player_entity_id = canonical_player_entity_id(bound_player_entity_id);
        if recipient_player_entity_id != entering_player_entity_id {
            recipient_player_entity_ids.insert(recipient_player_entity_id);
        }
    }

    let recipient_count = recipient_player_entity_ids.len();
    for recipient_player_entity_id in recipient_player_entity_ids {
        queue.push(NotificationCommand {
            player_entity_id: recipient_player_entity_id,
            title: "Player Online".to_string(),
            body: format!("{entering_display_name} entered the world."),
            severity: NotificationSeverity::Info,
            placement: NotificationPlacement::BottomRight,
            image: None,
            payload: NotificationPayload::Generic {
                event_type: PLAYER_ENTERED_WORLD_EVENT_TYPE.to_string(),
                data: serde_json::json!({
                    "player_entity_id": entering_player_entity_id,
                    "display_name": entering_display_name,
                }),
            },
            auto_dismiss_after_s: None,
        });
    }

    recipient_count
}

pub fn process_notification_commands(
    mut command_queue: ResMut<'_, NotificationCommandQueue>,
    mut delivery: ResMut<'_, NotificationDeliveryState>,
    mut worker: ResMut<'_, NotificationPersistenceWorker>,
) {
    while let Some(command) = command_queue.pending.pop_front() {
        let message = notification_message_from_command(command);
        let record = notification_record_from_message(&message);
        send_persistence_request(
            &mut worker,
            NotificationPersistenceRequest::Insert(Box::new(record)),
        );
        delivery.pending.push_back(message);
    }
}

pub fn stream_notification_messages(
    server_query: Query<'_, '_, &'_ Server>,
    mut sender: ServerMultiMessageSender<'_, '_, With<Connected>>,
    bindings: Res<'_, AuthenticatedClientBindings>,
    client_remotes: Query<'_, '_, (&'_ LinkOf, &'_ RemoteId), With<ClientOf>>,
    mut delivery: ResMut<'_, NotificationDeliveryState>,
    mut worker: ResMut<'_, NotificationPersistenceWorker>,
) {
    let Ok(server) = server_query.single() else {
        return;
    };
    if delivery.pending.is_empty() {
        return;
    }

    let mut next_pending = VecDeque::new();
    while let Some(message) = delivery.pending.pop_front() {
        if delivery.delivered_ids.contains(&message.notification_id) {
            continue;
        }
        let mut sent = false;
        for (_link, remote_id) in &client_remotes {
            let Some(bound_player) = bindings.by_remote_id.get(&remote_id.0) else {
                continue;
            };
            if !same_player(bound_player, &message.player_entity_id) {
                continue;
            }
            let target = NetworkTarget::Single(remote_id.0);
            if sender
                .send::<ServerNotificationMessage, NotificationChannel>(&message, server, &target)
                .is_ok()
            {
                sent = true;
            }
        }
        if sent {
            let now = now_epoch_s();
            delivery
                .delivered_ids
                .insert(message.notification_id.clone());
            send_persistence_request(
                &mut worker,
                NotificationPersistenceRequest::MarkDelivered {
                    player_entity_id: message.player_entity_id.clone(),
                    notification_id: message.notification_id.clone(),
                    delivered_at_epoch_s: now,
                },
            );
        } else {
            next_pending.push_back(message);
        }
    }
    delivery.pending = next_pending;
}

pub fn receive_notification_dismissals(
    mut worker: ResMut<'_, NotificationPersistenceWorker>,
    bindings: Res<'_, AuthenticatedClientBindings>,
    mut clients: Query<
        '_,
        '_,
        (
            &'_ RemoteId,
            &'_ mut MessageReceiver<ClientNotificationDismissedMessage>,
        ),
        With<ClientOf>,
    >,
) {
    for (remote_id, mut receiver) in &mut clients {
        let Some(bound_player) = bindings.by_remote_id.get(&remote_id.0) else {
            continue;
        };
        let messages = receiver.receive().collect::<Vec<_>>();
        for message in messages {
            if !same_player(bound_player, &message.player_entity_id) {
                warn!(
                    "dropped notification dismissal for mismatched player claimed={} bound={}",
                    message.player_entity_id, bound_player
                );
                continue;
            }
            if Uuid::parse_str(&message.notification_id).is_err() {
                warn!(
                    "dropped notification dismissal with invalid notification_id={}",
                    message.notification_id
                );
                continue;
            }
            send_persistence_request(
                &mut worker,
                NotificationPersistenceRequest::MarkDismissed {
                    player_entity_id: canonical_player_entity_id(&message.player_entity_id),
                    notification_id: message.notification_id,
                    dismissed_at_epoch_s: now_epoch_s(),
                },
            );
        }
    }
}

fn notification_message_from_command(command: NotificationCommand) -> ServerNotificationMessage {
    let severity = command.severity;
    ServerNotificationMessage {
        notification_id: Uuid::new_v4().to_string(),
        player_entity_id: canonical_player_entity_id(&command.player_entity_id),
        title: command.title,
        body: command.body,
        severity,
        placement: command.placement,
        image: command.image,
        payload: command.payload,
        created_at_epoch_s: now_epoch_s(),
        auto_dismiss_after_s: command
            .auto_dismiss_after_s
            .or_else(|| default_auto_dismiss_s(severity)),
    }
}

fn notification_record_from_message(
    message: &ServerNotificationMessage,
) -> PlayerNotificationRecord {
    PlayerNotificationRecord {
        notification_id: message.notification_id.clone(),
        player_entity_id: message.player_entity_id.clone(),
        notification_kind: message.payload.kind().to_string(),
        severity: message.severity.as_str().to_string(),
        title: message.title.clone(),
        body: message.body.clone(),
        image_asset_id: message.image.as_ref().map(|image| image.asset_id.clone()),
        image_alt_text: message
            .image
            .as_ref()
            .and_then(|image| image.alt_text.clone()),
        placement: message.placement.as_str().to_string(),
        payload: serde_json::to_value(&message.payload).unwrap_or_else(|_| serde_json::json!({})),
        created_at_epoch_s: message.created_at_epoch_s,
        delivered_at_epoch_s: None,
        dismissed_at_epoch_s: None,
    }
}

fn send_persistence_request(
    worker: &mut NotificationPersistenceWorker,
    request: NotificationPersistenceRequest,
) {
    let Some(sender) = worker.sender.as_ref() else {
        worker.dropped_requests = worker.dropped_requests.saturating_add(1);
        return;
    };
    match sender.try_send(request) {
        Ok(()) => {}
        Err(TrySendError::Full(_)) | Err(TrySendError::Disconnected(_)) => {
            worker.dropped_requests = worker.dropped_requests.saturating_add(1);
        }
    }
}

fn default_auto_dismiss_s(severity: NotificationSeverity) -> Option<f32> {
    Some(match severity {
        NotificationSeverity::Info => DEFAULT_INFO_DISMISS_S,
        NotificationSeverity::Success => DEFAULT_SUCCESS_DISMISS_S,
        NotificationSeverity::Warning => DEFAULT_WARNING_DISMISS_S,
        NotificationSeverity::Error => DEFAULT_ERROR_DISMISS_S,
    })
}

fn same_player(left: &str, right: &str) -> bool {
    canonical_player_entity_id(left) == canonical_player_entity_id(right)
}

fn canonical_player_entity_id(raw: &str) -> String {
    PlayerEntityId::parse(raw)
        .map(PlayerEntityId::canonical_wire_id)
        .unwrap_or_else(|| raw.to_string())
}

fn now_epoch_s() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enqueue_player_notification_adds_command() {
        let mut queue = NotificationCommandQueue::default();
        enqueue_player_notification(
            &mut queue,
            NotificationCommand {
                player_entity_id: "11111111-1111-1111-1111-111111111111".to_string(),
                title: "Title".to_string(),
                body: "Body".to_string(),
                severity: NotificationSeverity::Info,
                placement: NotificationPlacement::BottomRight,
                image: None,
                payload: NotificationPayload::Generic {
                    event_type: "test".to_string(),
                    data: serde_json::json!({}),
                },
                auto_dismiss_after_s: None,
            },
        );

        assert_eq!(queue.len(), 1);
    }

    #[test]
    fn canonical_player_matching_rejects_mismatches() {
        assert!(same_player(
            "11111111-1111-1111-1111-111111111111",
            "11111111-1111-1111-1111-111111111111"
        ));
        assert!(!same_player(
            "11111111-1111-1111-1111-111111111111",
            "22222222-2222-2222-2222-222222222222"
        ));
    }

    #[test]
    fn command_defaults_to_severity_dismiss_duration() {
        let message = notification_message_from_command(NotificationCommand {
            player_entity_id: "11111111-1111-1111-1111-111111111111".to_string(),
            title: "Warning".to_string(),
            body: "Careful".to_string(),
            severity: NotificationSeverity::Warning,
            placement: NotificationPlacement::BottomRight,
            image: None,
            payload: NotificationPayload::Generic {
                event_type: "warning".to_string(),
                data: serde_json::json!({}),
            },
            auto_dismiss_after_s: None,
        });

        assert_eq!(
            message.auto_dismiss_after_s,
            Some(DEFAULT_WARNING_DISMISS_S)
        );
    }

    #[test]
    fn player_entered_world_notifications_target_other_bound_players_once() {
        let entering_player = "11111111-1111-1111-1111-111111111111";
        let recipient_a = "22222222-2222-2222-2222-222222222222";
        let recipient_b = "33333333-3333-3333-3333-333333333333";
        let mut bindings = AuthenticatedClientBindings::default();
        bindings
            .by_client_entity
            .insert(Entity::from_bits(1), entering_player.to_string());
        bindings
            .by_client_entity
            .insert(Entity::from_bits(2), recipient_a.to_string());
        bindings
            .by_client_entity
            .insert(Entity::from_bits(3), recipient_a.to_string());
        bindings
            .by_client_entity
            .insert(Entity::from_bits(4), recipient_b.to_string());

        let mut queue = NotificationCommandQueue::default();
        let queued = enqueue_player_entered_world_notifications(
            &mut queue,
            &bindings,
            entering_player,
            Some("Talanah"),
        );

        assert_eq!(queued, 2);
        assert_eq!(queue.len(), 2);
        let recipients = queue
            .pending
            .iter()
            .map(|command| command.player_entity_id.as_str())
            .collect::<std::collections::HashSet<_>>();
        assert!(!recipients.contains(entering_player));
        assert!(recipients.contains(recipient_a));
        assert!(recipients.contains(recipient_b));
        for command in &queue.pending {
            assert_eq!(command.title, "Player Online");
            assert_eq!(command.body, "Talanah entered the world.");
            assert_eq!(command.severity, NotificationSeverity::Info);
            assert_eq!(command.placement, NotificationPlacement::BottomRight);
            match &command.payload {
                NotificationPayload::Generic { event_type, data } => {
                    assert_eq!(event_type, PLAYER_ENTERED_WORLD_EVENT_TYPE);
                    assert_eq!(data["player_entity_id"], entering_player);
                    assert_eq!(data["display_name"], "Talanah");
                }
                _ => panic!("expected generic player-entered-world payload"),
            }
        }
    }
}
