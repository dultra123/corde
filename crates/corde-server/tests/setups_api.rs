//! Real setups end to end: a simulator session, a fake `UserData` folder
//! (`--game-user-data`) holding the driver's real `.svm` files
//! (`crates/corde-lmu/tests/fixtures/svm/`) plus two files made for the
//! simulator's car, then `GET /api/setups`, the analysis and the
//! iterations resolved against the base, `GET …/setup-preview`, and
//! `POST …/setup-files` (files written next to the base, never overwritten,
//! paths outside `Settings` refused, only from this PC).

use corde_protocol::{
    AnalysisReport, PlanIteration, ResolveStatus, SetupFilesCreated, SetupLibrary, SetupPreview,
};
use corde_server::{Server, ServerConfig, SourceKind};
use serde_json::{json, Value};
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};
use tokio::sync::oneshot;

fn temp_dir(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "corde-setups-{tag}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

fn fixture(name: &str) -> String {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../corde-lmu/tests/fixtures/svm")
        .join(name);
    String::from_utf8(std::fs::read(p).unwrap()).unwrap()
}

/// `UserData/player/Settings` with the fixtures, and two setups of the
/// simulator's car (« Mercedes-AMG GT3 ») at its track (« Paul Ricard »),
/// made from the BMW file (same keys and labels; the older one has the rear
/// bar on P1 so that its label is known).
fn fake_user_data(root: &Path) -> PathBuf {
    let user_data = root.join("UserData");
    let settings = user_data.join("player").join("Settings");
    for (folder, name) in [
        ("Longbeach", "BMW_LongBeach.svm"),
        ("Longbeach", "BMW_LongBeach_v2.svm"),
        ("Road Atlanta", "Ligier_LMP3_RoadAtlanta_v2.svm"),
    ] {
        std::fs::create_dir_all(settings.join(folder)).unwrap();
        std::fs::write(settings.join(folder).join(name), fixture(name)).unwrap();
    }
    let pr = settings.join("Paulricard");
    std::fs::create_dir_all(&pr).unwrap();
    let merc = fixture("BMW_LongBeach.svm").replace(
        "VehicleClassSetting=\"BMW_M4_LMGT3 GT3 WEC2026\"",
        "VehicleClassSetting=\"Mercedes_AMG_LMGT3 GT3 WEC2026\"",
    );
    let old = merc.replace("RearAntiSwaySetting=2//P2", "RearAntiSwaySetting=1//P1");
    std::fs::write(pr.join("Mercedes_PaulRicard_v1.svm"), old).unwrap();
    std::fs::write(pr.join("Mercedes_PaulRicard_v2.svm"), merc).unwrap();
    // v1 older than v2.
    let v1 = std::fs::File::options()
        .write(true)
        .open(pr.join("Mercedes_PaulRicard_v1.svm"))
        .unwrap();
    v1.set_modified(SystemTime::now() - Duration::from_secs(3600))
        .unwrap();
    std::fs::write(pr.join("broken.svm"), "garbage").unwrap();
    user_data
}

async fn http(
    method: &'static str,
    url: String,
    body: Option<Value>,
    origin: Option<&'static str>,
) -> (u16, Value) {
    tokio::task::spawn_blocking(move || {
        let agent: ureq::Agent = ureq::config::Config::builder()
            .http_status_as_error(false)
            .proxy(None)
            .build()
            .new_agent();
        let mut resp = match (method, body) {
            ("GET", _) => agent.get(&url).call().expect("GET"),
            ("POST", Some(b)) => {
                let mut req = agent.post(&url);
                if let Some(o) = origin {
                    req = req.header("Origin", o);
                }
                req.send_json(&b).expect("POST json")
            }
            _ => unreachable!(),
        };
        let status = resp.status().as_u16();
        let text = resp.body_mut().read_to_string().unwrap_or_default();
        let value = if text.is_empty() {
            Value::Null
        } else {
            serde_json::from_str(&text).unwrap_or(Value::String(text))
        };
        (status, value)
    })
    .await
    .unwrap()
}

async fn wait_for(
    what: &str,
    timeout: Duration,
    mut check: impl AsyncFnMut() -> Option<Value>,
) -> Value {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(v) = check().await {
            return v;
        }
        assert!(Instant::now() < deadline, "timeout waiting for {what}");
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

fn config(data_dir: &Path, user_data: Option<PathBuf>) -> ServerConfig {
    ServerConfig {
        port: 0,
        bind: Some(IpAddr::from([127, 0, 0, 1])),
        source: SourceKind::Sim,
        data_dir: Some(data_dir.to_path_buf()),
        sim_seed: Some(5),
        sim_ai: Some(1),
        sim_lap_time: Some(30.0),
        sim_track_length: Some(1500.0),
        sim_speed: Some(3.0),
        sim_rate: Some(100.0),
        record_rate: 20.0,
        no_import: true,
        game_user_data: user_data,
        ..Default::default()
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn library_preview_and_files() {
    let root = temp_dir("files");
    let user_data = fake_user_data(&root);
    let settings = user_data.join("player").join("Settings");
    let base_bytes = std::fs::read(settings.join("Paulricard/Mercedes_PaulRicard_v2.svm")).unwrap();
    let server = Server::bind(config(&root.join("data"), Some(user_data.clone())))
        .await
        .expect("bind");
    let base = format!("http://{}", server.local_addr());
    let (stop, stop_rx) = oneshot::channel::<()>();
    let task = tokio::spawn(server.run(async move {
        let _ = stop_rx.await;
    }));

    let rec = wait_for("recording", Duration::from_secs(15), async || {
        let (_, v) = http("GET", format!("{base}/api/recording"), None, None).await;
        (v["recording"] == true && v["session_id"].is_string()).then_some(v)
    })
    .await;
    let id = rec["session_id"].as_str().unwrap().to_string();

    // The library, seen from the session: its car, its track folder, the latest file.
    let (code, body) = http("GET", format!("{base}/api/setups?session={id}"), None, None).await;
    assert_eq!(code, 200, "{body}");
    let lib: SetupLibrary = serde_json::from_value(body).unwrap();
    assert!(lib.user_data_known);
    assert_eq!(lib.vehicle, "Mercedes-AMG GT3");
    assert_eq!(lib.track_folder.as_deref(), Some("Paulricard"));
    assert_eq!(lib.total_files, 5);
    assert_eq!(lib.unreadable, ["Paulricard/broken.svm"]);
    let paths: Vec<&str> = lib.candidates.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(
        paths,
        [
            "Paulricard/Mercedes_PaulRicard_v2.svm",
            "Paulricard/Mercedes_PaulRicard_v1.svm"
        ]
    );
    assert_eq!(
        lib.proposed.as_ref().unwrap().path,
        "Paulricard/Mercedes_PaulRicard_v2.svm"
    );
    assert!(lib.proposed_reason_fr.unwrap().contains("Paulricard"));
    assert!(!lib.bounds_known);
    let (code, all) = http("GET", format!("{base}/api/setups"), None, None).await;
    assert_eq!(code, 200);
    assert_eq!(all["candidates"].as_array().unwrap().len(), 5);
    let (code, _) = http(
        "GET",
        format!("{base}/api/setups?session=20200101-000000-nowhere-race"),
        None,
        None,
    )
    .await;
    assert_eq!(code, 404);

    // Three clean laps for an analysis.
    wait_for("4 laps", Duration::from_secs(90), async || {
        let (_, v) = http("GET", format!("{base}/api/sessions/{id}"), None, None).await;
        (v["laps"].as_array().map(|l| l.len()).unwrap_or(0) >= 4).then_some(v)
    })
    .await;
    let (code, _) = http(
        "POST",
        format!("{base}/api/recording/stop"),
        Some(json!({})),
        None,
    )
    .await;
    assert_eq!(code, 202);
    wait_for("session to close", Duration::from_secs(10), async || {
        let (_, v) = http("GET", format!("{base}/api/sessions/{id}"), None, None).await;
        v["ended_wall_ms"].is_number().then_some(v)
    })
    .await;

    // The analysis is resolved against the proposed base.
    let (code, body) = http(
        "GET",
        format!("{base}/api/sessions/{id}/analysis"),
        None,
        None,
    )
    .await;
    assert_eq!(code, 200, "{body}");
    let report: AnalysisReport = serde_json::from_value(body).unwrap();
    assert!(report.plans.current_setup_known);
    assert_eq!(
        report.plans.base.as_ref().unwrap().path,
        "Paulricard/Mercedes_PaulRicard_v2.svm"
    );
    for plan in [
        &report.plans.aggressive,
        &report.plans.safe,
        &report.plans.wet,
    ] {
        assert!(!plan.changes.is_empty());
        for c in &plan.changes {
            let r = c.resolved.as_ref().expect("resolved");
            assert!(
                !matches!(
                    r.status,
                    ResolveStatus::NotAdjustable | ResolveStatus::UnknownKey
                ),
                "{c:?}"
            );
            assert!(
                !["diff.power", "diff.coast", "geo.caster"].contains(&c.parameter_id.as_str()),
                "{} is not adjustable on this car",
                c.parameter_id
            );
        }
        for d in &plan.dropped {
            assert!(d.resolved.is_some());
        }
    }
    // Another base: the older file.
    let (code, body) = http(
        "GET",
        format!("{base}/api/sessions/{id}/analysis?base=Paulricard/Mercedes_PaulRicard_v1.svm"),
        None,
        None,
    )
    .await;
    assert_eq!(code, 200, "{body}");
    assert_eq!(
        body["plans"]["base"]["path"],
        "Paulricard/Mercedes_PaulRicard_v1.svm"
    );
    for bad in ["../../x.svm", "Paulricard/../../x.svm", "Paulricard/x.txt"] {
        let (code, body) = http(
            "GET",
            format!("{base}/api/sessions/{id}/analysis?base={bad}"),
            None,
            None,
        )
        .await;
        assert_eq!(code, 400, "{bad}: {body}");
    }
    let (code, _) = http(
        "GET",
        format!("{base}/api/sessions/{id}/analysis?base=Paulricard/missing.svm"),
        None,
        None,
    )
    .await;
    assert_eq!(code, 404);

    // Preview: nothing written.
    let (code, body) = http(
        "GET",
        format!("{base}/api/sessions/{id}/setup-preview?plan=safe"),
        None,
        None,
    )
    .await;
    assert_eq!(code, 200, "{body}");
    let preview: SetupPreview = serde_json::from_value(body).unwrap();
    assert_eq!(preview.file_name, "Corde_Sur_Mercedes_PaulRicard_v2.svm");
    assert_eq!(preview.plan_label_fr, "Soft & sûr");
    assert!(preview
        .notes
        .starts_with("Corde - Soft et sur - base : Mercedes_PaulRicard_v2.svm - "));
    assert!(preview.notes.is_ascii() && preview.notes.len() <= 200);
    assert!(preview.changed_lines >= 1);
    let (code, _) = http(
        "GET",
        format!("{base}/api/sessions/{id}/setup-preview?plan=fast"),
        None,
        None,
    )
    .await;
    assert_eq!(code, 400);
    let (code, _) = http(
        "GET",
        format!("{base}/api/sessions/{id}/setup-preview?plan=iteration:4"),
        None,
        None,
    )
    .await;
    assert_eq!(code, 404);
    assert!(!settings
        .join("Paulricard/Corde_Sur_Mercedes_PaulRicard_v2.svm")
        .exists());

    // A feedback: the iteration comes back resolved, and can be written.
    let (code, fb) = http(
        "POST",
        format!("{base}/api/sessions/{id}/feedback"),
        Some(json!({"plan": "safe", "text": "l'arrière décroche à la remise des gaz dans les lents"})),
        None,
    )
    .await;
    assert_eq!(code, 201, "{fb}");
    let it: PlanIteration = serde_json::from_value(fb["iteration"].clone()).unwrap();
    assert!(it.current_setup_known);
    assert!(it.changes.iter().all(|c| c.resolved.is_some()));
    let (code, its) = http(
        "GET",
        format!("{base}/api/sessions/{id}/iterations"),
        None,
        None,
    )
    .await;
    assert_eq!(code, 200);
    assert_eq!(its[0]["current_setup_known"], true);

    // Writing: refused from another machine, outside Settings, without plans.
    let url = format!("{base}/api/sessions/{id}/setup-files");
    let good = json!({"base": "Paulricard/Mercedes_PaulRicard_v2.svm", "plans": ["safe", "aggressive", "iteration:1"]});
    let (code, body) = http(
        "POST",
        url.clone(),
        Some(good.clone()),
        Some("http://192.168.1.20:8471"),
    )
    .await;
    assert_eq!(code, 403, "{body}");
    for bad in [
        json!({"base": "../../../etc/passwd.svm", "plans": ["safe"]}),
        json!({"base": "Paulricard/../Longbeach/../../x.svm", "plans": ["safe"]}),
        json!({"base": "/tmp/x.svm", "plans": ["safe"]}),
        json!({"base": "Paulricard/Mercedes_PaulRicard_v2.svm", "plans": []}),
        json!({"base": "Paulricard/Mercedes_PaulRicard_v2.svm", "plans": ["nope"]}),
    ] {
        let (code, body) = http("POST", url.clone(), Some(bad.clone()), None).await;
        assert_eq!(code, 400, "{bad}: {body}");
    }

    // Written next to the base, from this PC.
    let (code, body) = http(
        "POST",
        url.clone(),
        Some(good.clone()),
        Some("http://127.0.0.1:8471"),
    )
    .await;
    assert_eq!(code, 201, "{body}");
    let created: SetupFilesCreated = serde_json::from_value(body).unwrap();
    assert_eq!(created.folder, "Paulricard");
    let names: Vec<&str> = created.files.iter().map(|f| f.file.as_str()).collect();
    assert_eq!(
        names,
        [
            "Corde_Sur_Mercedes_PaulRicard_v2.svm",
            "Corde_Agressif_Mercedes_PaulRicard_v2.svm",
            "Corde_Iteration1_Sur_Mercedes_PaulRicard_v2.svm"
        ]
    );
    assert!(created.instructions_fr.contains("rafraîchir"));
    assert!(created
        .instructions_fr
        .contains("Corde_Sur_Mercedes_PaulRicard_v2.svm"));
    for f in &created.files {
        let text = std::fs::read_to_string(settings.join(&f.path)).unwrap();
        assert!(text.contains("\r\n"), "CRLF kept");
        assert!(text.starts_with("VehicleClassSetting=\"Mercedes_AMG_LMGT3 GT3 WEC2026\"\r\n"));
        assert!(text.contains("Notes=\"Corde - "));
        assert!(text.contains("Front3rdTenderSpringSetting=0//Détaché\r\n"));
        assert!(text.contains("DiffPowerSetting=0//Non-adjustable\r\n"));
    }
    // The base is untouched; a second request never overwrites.
    assert_eq!(
        std::fs::read(settings.join("Paulricard/Mercedes_PaulRicard_v2.svm")).unwrap(),
        base_bytes
    );
    let (code, body) = http(
        "POST",
        url.clone(),
        Some(json!({"base": "Paulricard/Mercedes_PaulRicard_v2.svm", "plans": ["safe"]})),
        None,
    )
    .await;
    assert_eq!(code, 201, "{body}");
    assert_eq!(
        body["files"][0]["file"],
        "Corde_Sur_Mercedes_PaulRicard_v2_2.svm"
    );
    // The new files are in the library (bases to choose from), but the base proposed for this
    // session is still the driver's own file: Corde's files were written after it started.
    let (_, body) = http("GET", format!("{base}/api/setups?session={id}"), None, None).await;
    assert_eq!(body["candidates"].as_array().unwrap().len(), 6);
    assert_eq!(
        body["proposed"]["path"],
        "Paulricard/Mercedes_PaulRicard_v2.svm"
    );

    let _ = stop.send(());
    let _ = tokio::time::timeout(Duration::from_secs(10), task).await;
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn without_the_game_folder_nothing_is_resolved_or_written() {
    let root = temp_dir("nogame");
    let mut cfg = config(&root.join("data"), None);
    cfg.no_record = false;
    let server = Server::bind(cfg).await.expect("bind");
    let base = format!("http://{}", server.local_addr());
    let (stop, stop_rx) = oneshot::channel::<()>();
    let task = tokio::spawn(server.run(async move {
        let _ = stop_rx.await;
    }));
    let rec = wait_for("recording", Duration::from_secs(15), async || {
        let (_, v) = http("GET", format!("{base}/api/recording"), None, None).await;
        (v["recording"] == true && v["session_id"].is_string()).then_some(v)
    })
    .await;
    let id = rec["session_id"].as_str().unwrap().to_string();
    let (code, body) = http("GET", format!("{base}/api/setups?session={id}"), None, None).await;
    assert_eq!(code, 200);
    assert_eq!(body["user_data_known"], false);
    assert_eq!(body["candidates"], json!([]));
    let (code, body) = http(
        "POST",
        format!("{base}/api/sessions/{id}/setup-files"),
        Some(json!({"base": "Paulricard/x.svm", "plans": ["safe"]})),
        None,
    )
    .await;
    assert_eq!(code, 409, "{body}");
    let (code, _) = http(
        "GET",
        format!("{base}/api/sessions/{id}/setup-preview?plan=safe"),
        None,
        None,
    )
    .await;
    assert_eq!(code, 409);
    let _ = stop.send(());
    let _ = tokio::time::timeout(Duration::from_secs(10), task).await;
    let _ = std::fs::remove_dir_all(&root);
}
