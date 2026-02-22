use serde::{Deserialize, Serialize};
use sidereal_persistence::{ChannelClass, NetEnvelope, decode_envelope_json, encode_envelope_json};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct PayloadV1 {
    player_id: String,
    thrust_forward: bool,
    #[serde(default)]
    stop_requested: bool,
}

#[test]
fn envelope_json_roundtrip_preserves_fields() {
    let envelope = NetEnvelope {
        protocol_version: 1,
        channel: ChannelClass::Input,
        source_shard_id: 7,
        lease_epoch: 11,
        seq: 42,
        tick: 99,
        payload: PayloadV1 {
            player_id: "player:abc".to_string(),
            thrust_forward: true,
            stop_requested: false,
        },
    };

    let bytes = encode_envelope_json(&envelope).expect("encode should succeed");
    let decoded: NetEnvelope<PayloadV1> =
        decode_envelope_json(&bytes).expect("decode should succeed");

    assert_eq!(decoded.protocol_version, envelope.protocol_version);
    assert_eq!(decoded.source_shard_id, envelope.source_shard_id);
    assert_eq!(decoded.tick, envelope.tick);
    assert_eq!(decoded.payload, envelope.payload);
}

#[test]
fn backward_compatible_decode_missing_new_payload_field_uses_default() {
    let legacy_json = r#"{
        "protocol_version":1,
        "channel":"Input",
        "source_shard_id":2,
        "lease_epoch":3,
        "seq":4,
        "tick":5,
        "payload":{
            "player_id":"player:legacy",
            "thrust_forward":true
        }
    }"#;

    let decoded: NetEnvelope<PayloadV1> =
        decode_envelope_json(legacy_json.as_bytes()).expect("legacy payload should decode");

    assert_eq!(decoded.payload.player_id, "player:legacy");
    assert!(decoded.payload.thrust_forward);
    assert!(!decoded.payload.stop_requested);
}
