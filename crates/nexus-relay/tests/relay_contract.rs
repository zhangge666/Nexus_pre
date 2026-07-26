//! 本文件验证零知识中继的密文增量、配对、设备撤销、墓碑与持久化契约。

use std::{fs, path::Path};

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode, header},
};
use http_body_util::BodyExt;
use nexus_relay::{
    ApprovePairingRequest, BootstrapDeviceRequest, CreatePairingRequest,
    FetchPairingPackageRequest, PairingDeviceRequest, RecoverDeviceRequest, RelayState,
    RevokeDeviceRequest, approve_pairing_action, bootstrap_message, create_device_proof,
    pairing_request_message, recover_device_message, recovery_registration_message,
};
use nexus_sync::{
    DeviceIdentity, EncryptedSyncEnvelope, OperationKind, PairingOffer, PlainSyncOperation,
    SyncKey, VersionVector, VersionedRecord,
};
use serde::Serialize;
use serde_json::Value;
use tower::ServiceExt;
use uuid::Uuid;

const TOKEN: &str = "relay-contract-token-with-at-least-32-characters";

/// 对中继路由发送带账户 Bearer token 的 JSON 请求。
async fn json_request<T: Serialize + ?Sized>(
    app: &Router,
    method: &str,
    uri: &str,
    body: Option<&T>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header(header::AUTHORIZATION, format!("Bearer {TOKEN}"));
    let body = match body {
        Some(body) => {
            builder = builder.header(header::CONTENT_TYPE, "application/json");
            Body::from(serde_json::to_vec(body).unwrap())
        }
        None => Body::empty(),
    };
    let response = app
        .clone()
        .oneshot(builder.body(body).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or_else(|error| {
            panic!(
                "响应不是 JSON（状态 {status}）：{error}；正文：{}",
                String::from_utf8_lossy(&bytes)
            )
        })
    };
    (status, value)
}

/// 登记空工作区的首台签名设备。
async fn bootstrap(app: &Router, key: &SyncKey, identity: &DeviceIdentity, name: &str) {
    let mut request = BootstrapDeviceRequest {
        workspace_id: key.workspace_id(),
        device_id: identity.device_id().into(),
        name: name.into(),
        public_key: identity.public_key().into(),
        recovery_public_key: key.recovery_public_key().unwrap(),
        signature: String::new(),
        recovery_signature: String::new(),
    };
    request.signature = identity.sign(bootstrap_message(&request).as_bytes());
    request.recovery_signature = key
        .sign_recovery_claim(recovery_registration_message(&request).as_bytes())
        .unwrap();
    let (status, _) = json_request(app, "POST", "/v1/sync/devices/bootstrap", Some(&request)).await;
    assert_eq!(status, StatusCode::CREATED);
}

/// 创建指定序号的写入或墓碑同步操作。
fn operation(
    identity: &DeviceIdentity,
    version: &mut VersionVector,
    entity_id: &str,
    kind: OperationKind,
) -> PlainSyncOperation {
    let sequence = version.increment(identity.device_id()).unwrap();
    PlainSyncOperation {
        operation_id: Uuid::now_v7(),
        entity_id: entity_id.into(),
        device_id: identity.device_id().into(),
        device_sequence: sequence,
        version: version.clone(),
        kind,
        payload: (kind == OperationKind::Upsert)
            .then(|| serde_json::json!({"content": "very secret relay payload"})),
        created_at: 1_700_000_000_000 + sequence as i64,
    }
}

/// 构造带签名证明的增量拉取 URI。
fn pull_uri(
    key: &SyncKey,
    identity: &DeviceIdentity,
    after: u64,
    limit: usize,
    nonce: &str,
) -> String {
    let action = format!("changes:pull:{}:{after}:{limit}", key.workspace_id());
    let proof = create_device_proof(identity, &action, nonce);
    format!(
        "/v1/sync/changes?workspaceId={}&after={after}&limit={limit}&deviceId={}&timestamp={}&nonce={}&signature={}",
        key.workspace_id(),
        proof.device_id,
        proof.timestamp,
        proof.nonce,
        proof.signature
    )
}

/// 验证中继磁盘只出现密文，墓碑删除旧密文并在全部设备确认后自删除。
#[tokio::test]
async fn relays_only_ciphertext_and_compacts_tombstones() {
    let directory = tempfile::tempdir().unwrap();
    let state_path = directory.path().join("relay-state.json");
    let app = nexus_relay::router(RelayState::open(&state_path, TOKEN).unwrap());
    let key = SyncKey::generate();
    let identity = DeviceIdentity::generate("desktop-primary").unwrap();
    bootstrap(&app, &key, &identity, "桌面主设备").await;

    let mut version = VersionVector::default();
    let upsert = operation(
        &identity,
        &mut version,
        "memory-private-id",
        OperationKind::Upsert,
    );
    let envelope = key.encrypt_operation(&upsert, &identity).unwrap();
    let (status, pushed) = json_request(&app, "POST", "/v1/sync/changes", Some(&envelope)).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(pushed["cursor"], 1);

    let persisted = fs::read_to_string(&state_path).unwrap();
    assert!(!persisted.contains("very secret relay payload"));
    assert!(!persisted.contains("memory-private-id"));

    let (status, pulled) = json_request::<Value>(
        &app,
        "GET",
        &pull_uri(&key, &identity, 0, 200, "pull-nonce-00000001"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let pulled_envelope = serde_json::from_value(pulled["changes"][0]["envelope"].clone()).unwrap();
    assert_eq!(
        key.decrypt_operation(&pulled_envelope, identity.public_key())
            .unwrap(),
        upsert
    );

    let tombstone = operation(
        &identity,
        &mut version,
        "memory-private-id",
        OperationKind::Tombstone,
    );
    let tombstone_envelope = key.encrypt_operation(&tombstone, &identity).unwrap();
    let (status, deleted) =
        json_request(&app, "POST", "/v1/sync/changes", Some(&tombstone_envelope)).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(deleted["removedCiphertexts"], 1);
    assert!(deleted["deletionReceipt"].as_str().unwrap().len() >= 64);

    let action = format!("changes:ack:{}:2", key.workspace_id());
    let ack = serde_json::json!({
        "workspaceId": key.workspace_id(),
        "cursor": 2,
        "proof": create_device_proof(&identity, &action, "ack-nonce-000000001")
    });
    let (status, acknowledged) = json_request(&app, "POST", "/v1/sync/ack", Some(&ack)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(acknowledged["removedTombstones"], 1);

    let (status, empty) = json_request::<Value>(
        &app,
        "GET",
        &pull_uri(&key, &identity, 0, 200, "pull-nonce-00000002"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(empty["changes"].as_array().unwrap().len(), 0);
    assert_snapshot_has_no_ciphertext_for_entity(&state_path);
}

/// 验证二维码秘密不上传中继，新设备能取回根密钥且撤销后无法再上传。
#[tokio::test]
async fn pairs_and_revokes_signed_devices() {
    let app = nexus_relay::router(RelayState::in_memory(TOKEN).unwrap());
    let key = SyncKey::generate();
    let primary = DeviceIdentity::generate("desktop-primary").unwrap();
    let mobile = DeviceIdentity::generate("android-secondary").unwrap();
    bootstrap(&app, &key, &primary, "桌面主设备").await;

    let offer = PairingOffer::create(&key);
    let create_action = format!("pairing:create:{}:{}", key.workspace_id(), offer.session_id);
    let create = CreatePairingRequest {
        session_id: offer.session_id,
        workspace_id: key.workspace_id(),
        proof: create_device_proof(&primary, &create_action, "pair-create-nonce01"),
    };
    let (status, _) = json_request(&app, "POST", "/v1/sync/pairings", Some(&create)).await;
    assert_eq!(status, StatusCode::CREATED);

    let mut join = PairingDeviceRequest {
        device_id: mobile.device_id().into(),
        name: "Android 手机".into(),
        public_key: mobile.public_key().into(),
        signature: String::new(),
    };
    join.signature = mobile.sign(pairing_request_message(offer.session_id, &join).as_bytes());
    let (status, _) = json_request(
        &app,
        "POST",
        &format!("/v1/sync/pairings/{}/request", offer.session_id),
        Some(&join),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED);

    let sealed = offer
        .secret
        .seal_sync_key(&key, &offer, mobile.device_id())
        .unwrap();
    let approve_action = approve_pairing_action(offer.session_id, mobile.device_id(), &sealed);
    let approval = ApprovePairingRequest {
        sealed_key: sealed,
        proof: create_device_proof(&primary, &approve_action, "pair-approve-nonce1"),
    };
    let (status, _) = json_request(
        &app,
        "POST",
        &format!("/v1/sync/pairings/{}/approve", offer.session_id),
        Some(&approval),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let fetch = FetchPairingPackageRequest {
        device_id: mobile.device_id().into(),
        signature: mobile
            .sign(format!("pairing:fetch:{}:{}", offer.session_id, mobile.device_id()).as_bytes()),
    };
    let (status, package) = json_request(
        &app,
        "POST",
        &format!("/v1/sync/pairings/{}/package", offer.session_id),
        Some(&fetch),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let package = serde_json::from_value(package).unwrap();
    let recovered = offer
        .secret
        .open_sync_key(&package, mobile.device_id())
        .unwrap();
    assert_eq!(recovered.to_bytes(), key.to_bytes());

    let recovered_device = DeviceIdentity::generate("android-recovered").unwrap();
    let mut recovery = RecoverDeviceRequest {
        workspace_id: key.workspace_id(),
        device_id: recovered_device.device_id().into(),
        name: "恢复设备".into(),
        public_key: recovered_device.public_key().into(),
        device_signature: String::new(),
        recovery_signature: String::new(),
    };
    let recovery_message = recover_device_message(&recovery);
    recovery.device_signature = recovered_device.sign(recovery_message.as_bytes());
    recovery.recovery_signature = key
        .sign_recovery_claim(recovery_message.as_bytes())
        .unwrap();
    let (status, restored) =
        json_request(&app, "POST", "/v1/sync/devices/recover", Some(&recovery)).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(restored["deviceId"], recovered_device.device_id());

    let revoke_action = format!(
        "device:revoke:{}:{}",
        key.workspace_id(),
        mobile.device_id()
    );
    let revoke = RevokeDeviceRequest {
        workspace_id: key.workspace_id(),
        proof: create_device_proof(&primary, &revoke_action, "revoke-device-nonce1"),
    };
    let (status, revoked) = json_request(
        &app,
        "DELETE",
        &format!("/v1/sync/devices/{}", mobile.device_id()),
        Some(&revoke),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(revoked["revokedAt"].as_i64().is_some());

    let mut mobile_version = VersionVector::default();
    let unauthorized = operation(
        &mobile,
        &mut mobile_version,
        "memory-after-revoke",
        OperationKind::Upsert,
    );
    let envelope = key.encrypt_operation(&unauthorized, &mobile).unwrap();
    let (status, _) = json_request(&app, "POST", "/v1/sync/changes", Some(&envelope)).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

/// 验证双设备并发写入能在客户端确定性收敛，且墓碑必须等待全部有效设备确认。
#[tokio::test]
async fn converges_concurrent_devices_and_waits_for_all_tombstone_acks() {
    let app = nexus_relay::router(RelayState::in_memory(TOKEN).unwrap());
    let key = SyncKey::generate();
    let primary = DeviceIdentity::generate("desktop-primary").unwrap();
    let mobile = DeviceIdentity::generate("android-secondary").unwrap();
    bootstrap(&app, &key, &primary, "桌面主设备").await;

    let mut recovery = RecoverDeviceRequest {
        workspace_id: key.workspace_id(),
        device_id: mobile.device_id().into(),
        name: "Android 手机".into(),
        public_key: mobile.public_key().into(),
        device_signature: String::new(),
        recovery_signature: String::new(),
    };
    let recovery_message = recover_device_message(&recovery);
    recovery.device_signature = mobile.sign(recovery_message.as_bytes());
    recovery.recovery_signature = key
        .sign_recovery_claim(recovery_message.as_bytes())
        .unwrap();
    let (status, _) = json_request(&app, "POST", "/v1/sync/devices/recover", Some(&recovery)).await;
    assert_eq!(status, StatusCode::CREATED);

    let mut primary_version = VersionVector::default();
    let mut mobile_version = VersionVector::default();
    let primary_write = operation(
        &primary,
        &mut primary_version,
        "memory-concurrent",
        OperationKind::Upsert,
    );
    let mut mobile_write = operation(
        &mobile,
        &mut mobile_version,
        "memory-concurrent",
        OperationKind::Upsert,
    );
    mobile_write.payload = Some(serde_json::json!({"content": "android concurrent payload"}));
    for (identity, operation) in [(&primary, &primary_write), (&mobile, &mobile_write)] {
        let envelope = key.encrypt_operation(operation, identity).unwrap();
        let (status, _) = json_request(&app, "POST", "/v1/sync/changes", Some(&envelope)).await;
        assert_eq!(status, StatusCode::CREATED);
    }

    let (status, pulled) = json_request::<Value>(
        &app,
        "GET",
        &pull_uri(&key, &mobile, 0, 200, "concurrent-pull-nonce1"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let operations = pulled["changes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|change| {
            let envelope: EncryptedSyncEnvelope =
                serde_json::from_value(change["envelope"].clone()).unwrap();
            let public_key = if envelope.device_id == primary.device_id() {
                primary.public_key()
            } else {
                mobile.public_key()
            };
            key.decrypt_operation(&envelope, public_key).unwrap()
        })
        .collect::<Vec<_>>();
    let to_record = |operation: &PlainSyncOperation| VersionedRecord {
        value: operation.payload.clone(),
        version: operation.version.clone(),
        device_id: operation.device_id.clone(),
        modified_at: operation.created_at,
        conflicts: Vec::new(),
    };
    let forward = to_record(&operations[0])
        .merge(to_record(&operations[1]))
        .record;
    let reverse = to_record(&operations[1])
        .merge(to_record(&operations[0]))
        .record;
    assert_eq!(forward, reverse);
    assert_eq!(forward.conflicts.len(), 1);

    primary_version.merge(&mobile_version);
    let primary_sequence = primary_version.observe(primary.device_id(), 2).unwrap();
    let tombstone = PlainSyncOperation {
        operation_id: Uuid::now_v7(),
        entity_id: "memory-concurrent".into(),
        device_id: primary.device_id().into(),
        device_sequence: primary_sequence,
        version: primary_version,
        kind: OperationKind::Tombstone,
        payload: None,
        created_at: 1_700_000_000_100,
    };
    let envelope = key.encrypt_operation(&tombstone, &primary).unwrap();
    let (status, deleted) = json_request(&app, "POST", "/v1/sync/changes", Some(&envelope)).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(deleted["removedCiphertexts"], 2);
    assert_eq!(deleted["cursor"], 3);

    let primary_action = format!("changes:ack:{}:3", key.workspace_id());
    let primary_ack = serde_json::json!({
        "workspaceId": key.workspace_id(),
        "cursor": 3,
        "proof": create_device_proof(&primary, &primary_action, "primary-ack-nonce01")
    });
    let (_, first_ack) = json_request(&app, "POST", "/v1/sync/ack", Some(&primary_ack)).await;
    assert_eq!(first_ack["removedTombstones"], 0);

    let (status, still_visible) = json_request::<Value>(
        &app,
        "GET",
        &pull_uri(&key, &mobile, 0, 200, "tombstone-pull-nonce1"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(still_visible["changes"].as_array().unwrap().len(), 1);
    assert_eq!(still_visible["changes"][0]["envelope"]["kind"], "tombstone");

    let mobile_action = format!("changes:ack:{}:3", key.workspace_id());
    let mobile_ack = serde_json::json!({
        "workspaceId": key.workspace_id(),
        "cursor": 3,
        "proof": create_device_proof(&mobile, &mobile_action, "mobile-ack-nonce0001")
    });
    let (_, final_ack) = json_request(&app, "POST", "/v1/sync/ack", Some(&mobile_ack)).await;
    assert_eq!(final_ack["removedTombstones"], 1);
}

/// 确认墓碑被全部设备确认后，持久化快照不再保留任何操作密文。
fn assert_snapshot_has_no_ciphertext_for_entity(path: &Path) {
    let snapshot: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    assert_eq!(snapshot["changes"].as_object().unwrap().len(), 0);
}
