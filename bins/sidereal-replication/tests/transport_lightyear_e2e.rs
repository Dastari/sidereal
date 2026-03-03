use jsonwebtoken::{EncodingKey, Header, encode};
use postgres::{Client, NoTls};
use serde::Serialize;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use std::time::{SystemTime, UNIX_EPOCH};

fn test_database_url() -> String {
    std::env::var("SIDEREAL_TEST_DATABASE_URL")
        .or_else(|_| std::env::var("REPLICATION_DATABASE_URL"))
        .unwrap_or_else(|_| "postgres://sidereal:sidereal@127.0.0.1:5432/sidereal".to_string())
}

fn ensure_test_db_available() -> bool {
    Client::connect(&test_database_url(), NoTls).is_ok()
}

fn available_player_entity_ids(limit: i64) -> Option<Vec<String>> {
    let mut client = Client::connect(&test_database_url(), NoTls).ok()?;
    let rows = client
        .query(
            "SELECT player_entity_id FROM auth_characters ORDER BY created_at_epoch_s ASC LIMIT $1",
            &[&limit],
        )
        .ok()?;
    Some(rows.into_iter().map(|row| row.get(0)).collect())
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root should resolve")
}

fn target_debug_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("CARGO_TARGET_DIR") {
        PathBuf::from(dir).join("debug")
    } else {
        workspace_root().join("target/debug")
    }
}

fn free_udp_port() -> u16 {
    std::net::UdpSocket::bind("127.0.0.1:0")
        .expect("bind ephemeral UDP port")
        .local_addr()
        .expect("ephemeral addr")
        .port()
}

fn spawn_logged(mut cmd: Command) -> (Child, Arc<Mutex<String>>) {
    let mut child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("process should spawn");
    let stdout = child.stdout.take().expect("stdout pipe should exist");
    let stderr = child.stderr.take().expect("stderr pipe should exist");
    let buffer = Arc::new(Mutex::new(String::new()));
    let out_buffer = Arc::clone(&buffer);
    thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines().map_while(Result::ok) {
            let mut guard = out_buffer.lock().expect("stdout buffer lock");
            guard.push_str(&line);
            guard.push('\n');
        }
    });
    let err_buffer = Arc::clone(&buffer);
    thread::spawn(move || {
        let reader = BufReader::new(stderr);
        for line in reader.lines().map_while(Result::ok) {
            let mut guard = err_buffer.lock().expect("stderr buffer lock");
            guard.push_str(&line);
            guard.push('\n');
        }
    });
    (child, buffer)
}

fn wait_for_log(buffer: &Arc<Mutex<String>>, pattern: &str, timeout: Duration) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if buffer.lock().expect("log buffer lock").contains(pattern) {
            return true;
        }
        thread::sleep(Duration::from_millis(100));
    }
    false
}

fn stop_child(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

fn spawn_headless_client(
    client_bin: &Path,
    replication_udp_addr: &str,
    client_udp_addr: &str,
    player_entity_id: &str,
    access_token: &str,
    switch_plan: Option<(&str, &str, f64)>,
) -> (Child, Arc<Mutex<String>>) {
    let mut client_cmd = Command::new(client_bin);
    client_cmd
        .env("SIDEREAL_CLIENT_HEADLESS", "1")
        .env(
            "SIDEREAL_CLIENT_HEADLESS_PLAYER_ENTITY_ID",
            player_entity_id,
        )
        .env("SIDEREAL_CLIENT_HEADLESS_ACCESS_TOKEN", access_token)
        .env("REPLICATION_UDP_ADDR", replication_udp_addr)
        .env("CLIENT_UDP_BIND", client_udp_addr)
        .env("RUST_LOG", "info");
    if let Some((next_player, next_token, after_s)) = switch_plan {
        client_cmd
            .env(
                "SIDEREAL_CLIENT_HEADLESS_SWITCH_PLAYER_ENTITY_ID",
                next_player,
            )
            .env("SIDEREAL_CLIENT_HEADLESS_SWITCH_ACCESS_TOKEN", next_token)
            .env(
                "SIDEREAL_CLIENT_HEADLESS_SWITCH_AFTER_S",
                after_s.to_string(),
            );
    }
    spawn_logged(client_cmd)
}

#[derive(Serialize)]
struct TestAccessTokenClaims {
    player_entity_id: String,
    exp: usize,
}

fn test_jwt_secret() -> String {
    // Keep this >=32 bytes to satisfy replication auth checks.
    "sidereal-test-jwt-secret-0123456789".to_string()
}

fn test_access_token(player_entity_id: &str, jwt_secret: &str) -> String {
    let exp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as usize + 3600)
        .unwrap_or(4_102_444_800);
    encode(
        &Header::default(),
        &TestAccessTokenClaims {
            player_entity_id: player_entity_id.to_string(),
            exp,
        },
        &EncodingKey::from_secret(jwt_secret.as_bytes()),
    )
    .expect("test token should encode")
}

#[test]
fn replication_client_lightyear_transport_flow() {
    if !ensure_test_db_available() {
        eprintln!("skipping transport e2e test; postgres unavailable");
        return;
    }

    let root = workspace_root();
    let status = Command::new("cargo")
        .current_dir(&root)
        .args([
            "build",
            "-p",
            "sidereal-replication",
            "-p",
            "sidereal-client",
        ])
        .status()
        .expect("cargo build should run");
    assert!(status.success(), "cargo build failed for transport e2e");

    let bin_dir = target_debug_dir();
    let replication_bin = bin_dir.join("sidereal-replication");
    let client_bin = bin_dir.join("sidereal-client");
    assert!(
        replication_bin.exists(),
        "missing binary: {replication_bin:?}"
    );
    assert!(client_bin.exists(), "missing binary: {client_bin:?}");

    let replication_udp_port = free_udp_port();
    let control_udp_port = free_udp_port();
    let client_udp_port = free_udp_port();
    let replication_udp_addr = format!("127.0.0.1:{replication_udp_port}");
    let control_udp_addr = format!("127.0.0.1:{control_udp_port}");
    let client_udp_addr = format!("127.0.0.1:{client_udp_port}");
    let Some(player_entity_id) = available_player_entity_ids(1).and_then(|mut ids| ids.pop())
    else {
        eprintln!("skipping transport e2e test; no auth_characters player_entity_id available");
        return;
    };
    let jwt_secret = test_jwt_secret();
    let access_token = test_access_token(&player_entity_id, &jwt_secret);

    let mut rep_cmd = Command::new(&replication_bin);
    rep_cmd
        .env("REPLICATION_UDP_BIND", &replication_udp_addr)
        .env("REPLICATION_CONTROL_UDP_BIND", &control_udp_addr)
        .env("REPLICATION_DATABASE_URL", test_database_url())
        .env("GATEWAY_JWT_SECRET", &jwt_secret)
        .env("RUST_LOG", "debug");
    let (mut rep_child, rep_log) = spawn_logged(rep_cmd);

    assert!(
        wait_for_log(
            &rep_log,
            "replication lightyear UDP server starting",
            Duration::from_secs(15),
        ),
        "replication did not start:\n{}",
        rep_log.lock().expect("rep log lock"),
    );

    let (mut client_child, client_log) = spawn_headless_client(
        &client_bin,
        &replication_udp_addr,
        &client_udp_addr,
        &player_entity_id,
        &access_token,
        None,
    );

    let client_connected_ok = wait_for_log(
        &client_log,
        "native client lightyear transport connected",
        Duration::from_secs(20),
    );
    let client_input_ok = wait_for_log(
        &rep_log,
        "replication received client input:",
        Duration::from_secs(20),
    );

    stop_child(&mut client_child);
    stop_child(&mut rep_child);
    assert!(
        client_connected_ok,
        "native client did not connect.\nclient log:\n{}",
        client_log.lock().expect("client log lock"),
    );
    assert!(
        client_input_ok,
        "replication did not ingest client input.\nreplication log:\n{}\nclient log:\n{}",
        rep_log.lock().expect("rep log lock"),
        client_log.lock().expect("client log lock"),
    );
}

#[test]
fn replication_rebinds_same_remote_after_player_switch() {
    if !ensure_test_db_available() {
        eprintln!("skipping transport e2e test; postgres unavailable");
        return;
    }

    let root = workspace_root();
    let status = Command::new("cargo")
        .current_dir(&root)
        .args([
            "build",
            "-p",
            "sidereal-replication",
            "-p",
            "sidereal-client",
        ])
        .status()
        .expect("cargo build should run");
    assert!(status.success(), "cargo build failed for transport e2e");

    let bin_dir = target_debug_dir();
    let replication_bin = bin_dir.join("sidereal-replication");
    let client_bin = bin_dir.join("sidereal-client");
    assert!(
        replication_bin.exists(),
        "missing binary: {replication_bin:?}"
    );
    assert!(client_bin.exists(), "missing binary: {client_bin:?}");

    let replication_udp_port = free_udp_port();
    let control_udp_port = free_udp_port();
    let client_udp_port = free_udp_port();
    let replication_udp_addr = format!("127.0.0.1:{replication_udp_port}");
    let control_udp_addr = format!("127.0.0.1:{control_udp_port}");
    let client_udp_addr = format!("127.0.0.1:{client_udp_port}");
    let Some(player_ids) = available_player_entity_ids(2) else {
        eprintln!("skipping transport e2e test; failed loading auth_characters player ids");
        return;
    };
    if player_ids.len() < 2 {
        eprintln!("skipping transport e2e test; need at least 2 player_entity_id values");
        return;
    }
    let player_a = player_ids[0].clone();
    let player_b = player_ids[1].clone();
    let jwt_secret = test_jwt_secret();
    let token_a = test_access_token(&player_a, &jwt_secret);
    let token_b = test_access_token(&player_b, &jwt_secret);

    let mut rep_cmd = Command::new(&replication_bin);
    rep_cmd
        .env("REPLICATION_UDP_BIND", &replication_udp_addr)
        .env("REPLICATION_CONTROL_UDP_BIND", &control_udp_addr)
        .env("REPLICATION_DATABASE_URL", test_database_url())
        .env("GATEWAY_JWT_SECRET", &jwt_secret)
        .env("RUST_LOG", "debug");
    let (mut rep_child, rep_log) = spawn_logged(rep_cmd);

    assert!(
        wait_for_log(
            &rep_log,
            "replication lightyear UDP server starting",
            Duration::from_secs(15),
        ),
        "replication did not start:\n{}",
        rep_log.lock().expect("rep log lock"),
    );

    let (mut client, client_log) = spawn_headless_client(
        &client_bin,
        &replication_udp_addr,
        &client_udp_addr,
        &player_a,
        &token_a,
        Some((&player_b, &token_b, 1.0)),
    );
    let a_bound = wait_for_log(
        &rep_log,
        &format!("player_entity_id={player_a}"),
        Duration::from_secs(20),
    );
    let b_bound = wait_for_log(
        &rep_log,
        &format!("player_entity_id={player_b}"),
        Duration::from_secs(25),
    );
    let spoofed_b_vs_a = wait_for_log(
        &rep_log,
        &format!("claimed={player_b}, bound={player_a}"),
        Duration::from_secs(5),
    );

    stop_child(&mut client);
    stop_child(&mut rep_child);

    assert!(
        a_bound,
        "replication did not bind/input player A.\nreplication log:\n{}\nclient log:\n{}",
        rep_log.lock().expect("rep log lock"),
        client_log.lock().expect("client log lock"),
    );
    assert!(
        b_bound,
        "replication did not bind/input player B after switch.\nreplication log:\n{}\nclient log:\n{}",
        rep_log.lock().expect("rep log lock"),
        client_log.lock().expect("client log lock"),
    );
    assert!(
        !spoofed_b_vs_a,
        "replication kept stale player binding after switch.\nreplication log:\n{}",
        rep_log.lock().expect("rep log lock"),
    );
}
