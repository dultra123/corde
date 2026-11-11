//! End-to-end import of the two real results files of `reference/samples`
//! (`--source sim --lmu-results-dir reference/samples --no-record`): the
//! start-up scan, `GET /api/profile`, `GET /api/results`, `GET /api/results/{id}`
//! and `POST /api/profile/import`.

use corde_server::{Server, ServerConfig, SourceKind};
use serde_json::Value;
use std::net::IpAddr;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tokio::sync::oneshot;

fn temp_data_dir(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "corde-profile-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

async fn http(method: &'static str, url: String) -> (u16, Value) {
    tokio::task::spawn_blocking(move || {
        let agent: ureq::Agent = ureq::config::Config::builder()
            .http_status_as_error(false)
            .proxy(None)
            .build()
            .new_agent();
        let mut resp = match method {
            "GET" => agent.get(&url).call().expect("GET"),
            "POST" => agent.post(&url).send_empty().expect("POST"),
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn imports_sample_results_and_serves_the_profile() {
    let data_dir = temp_data_dir("samples");
    let samples = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../reference/samples");
    let cfg = ServerConfig {
        port: 0,
        bind: Some(IpAddr::from([127, 0, 0, 1])),
        source: SourceKind::Sim,
        data_dir: Some(data_dir.clone()),
        no_record: true,
        lmu_results_dir: Some(samples.clone()),
        ..Default::default()
    };
    let server = Server::bind(cfg).await.expect("bind");
    let addr = server.local_addr();
    let (stop, stop_rx) = oneshot::channel::<()>();
    let task = tokio::spawn(server.run(async move {
        let _ = stop_rx.await;
    }));
    let base = format!("http://{addr}");

    // The start-up scan runs on the blocking pool: wait for it.
    let deadline = Instant::now() + Duration::from_secs(20);
    let profile = loop {
        let (status, body) = http("GET", format!("{base}/api/profile")).await;
        assert_eq!(status, 200, "{body}");
        if body["import"]["last_scan_ms"].is_number() && body["totals"]["sessions"] == 2 {
            break body;
        }
        assert!(Instant::now() < deadline, "import did not finish: {body}");
        tokio::time::sleep(Duration::from_millis(200)).await;
    };

    // --- profile
    assert_eq!(profile["player_name"], "alex martin");
    let totals = &profile["totals"];
    assert_eq!(totals["sessions"], 2);
    assert_eq!(totals["races"], 1);
    assert_eq!(totals["practices"], 1);
    assert_eq!(totals["qualifyings"], 0);
    assert_eq!(totals["laps"], 15);
    assert!((totals["km"].as_f64().unwrap() - 85.479).abs() < 0.01);
    assert_eq!(totals["wins"], 0);
    assert_eq!(totals["class_podiums"], 0);
    let tracks = profile["tracks"].as_array().unwrap();
    assert_eq!(tracks.len(), 2);
    assert_eq!(tracks[0]["track_course"], "Paul Ricard - 1A");
    assert_eq!(tracks[0]["track_venue"], "Paul Ricard Circuit");
    assert_eq!(tracks[0]["sessions"], 1);
    assert_eq!(tracks[0]["races"], 1);
    assert!((tracks[0]["best_lap_s"].as_f64().unwrap() - 118.3745).abs() < 1e-3);
    assert_eq!(tracks[0]["best_lap_vehicle"], "Mercedes-AMG LMGT3");
    assert_eq!(tracks[0]["best_class_position"], 12);
    assert_eq!(tracks[0]["best_lap_date"], 1_793_563_502_000_i64);
    assert_eq!(
        tracks[1]["track_course"],
        "Silverstone International Circuit"
    );
    assert!(tracks[1]["best_lap_s"].is_null());
    let vehicles = profile["vehicles"].as_array().unwrap();
    assert_eq!(vehicles.len(), 1);
    assert_eq!(vehicles[0]["vehicle_name"], "Mercedes-AMG LMGT3");
    assert_eq!(vehicles[0]["car_class"], "GT3");
    assert_eq!(vehicles[0]["sessions"], 2);
    assert_eq!(vehicles[0]["best_track"], "Paul Ricard - 1A");
    let recent = profile["recent"].as_array().unwrap();
    assert_eq!(recent.len(), 2);
    assert_eq!(recent[0]["session_kind"], "race");
    assert_eq!(recent[0]["player_position"], 23);
    assert_eq!(recent[0]["player_class_position"], 12);
    assert_eq!(recent[0]["num_class_drivers"], 19);
    assert_eq!(recent[0]["num_drivers"], 38);
    let import = &profile["import"];
    assert_eq!(import["enabled"], true);
    assert_eq!(import["dir_source"], "cli");
    assert_eq!(import["files_seen"], 2);
    assert_eq!(import["total_imported"], 2);
    assert_eq!(import["files_skipped"], 0);
    assert!(import["error"].is_null());
    assert!(import["results_dir"].as_str().unwrap().ends_with("samples"));

    // --- list + filters
    let (status, all) = http("GET", format!("{base}/api/results")).await;
    assert_eq!(status, 200);
    assert_eq!(all.as_array().unwrap().len(), 2);
    let race_id = all[0]["id"].as_i64().unwrap();
    let (_, races) = http("GET", format!("{base}/api/results?kind=race")).await;
    assert_eq!(races.as_array().unwrap().len(), 1);
    let (_, silver) = http("GET", format!("{base}/api/results?track=silverstone")).await;
    assert_eq!(silver.as_array().unwrap().len(), 1);
    assert_eq!(silver[0]["session_kind"], "practice");
    let (_, none) = http("GET", format!("{base}/api/results?type=qualify&limit=5")).await;
    assert_eq!(none.as_array().unwrap().len(), 0);
    let (status, _) = http("GET", format!("{base}/api/results?kind=bogus")).await;
    assert_eq!(status, 400);

    // --- detail: player laps, full classification, no recorded session to link
    let (status, detail) = http("GET", format!("{base}/api/results/{race_id}")).await;
    assert_eq!(status, 200);
    assert_eq!(detail["file_name"], "results-R1-paulricard.xml");
    assert_eq!(detail["player_name"], "alex martin");
    assert!(detail["linked_session_id"].is_null());
    let laps = detail["laps"].as_array().unwrap();
    assert_eq!(laps.len(), 15);
    assert!(laps[0]["lap_time_s"].is_null());
    assert!((laps[8]["lap_time_s"].as_f64().unwrap() - 118.3745).abs() < 1e-3);
    assert_eq!(laps[8]["lap_number"], 9);
    assert_eq!(laps[8]["front_compound"], "Medium");
    let drivers = detail["file"]["drivers"].as_array().unwrap();
    assert_eq!(drivers.len(), 38);
    let winner = drivers
        .iter()
        .find(|d| d["car_class"] == "GT3" && d["class_position"] == 1)
        .unwrap();
    assert_eq!(winner["name"], "Hugo Lambrin");
    assert_eq!(winner["laps_completed"], 16);
    let (status, _) = http("GET", format!("{base}/api/results/999999")).await;
    assert_eq!(status, 404);

    // --- forced rescan: nothing new, status is returned
    let (status, st) = http("POST", format!("{base}/api/profile/import")).await;
    assert_eq!(status, 200, "{st}");
    assert_eq!(st["files_seen"], 2);
    assert_eq!(st["files_imported"], 0);
    assert_eq!(st["total_imported"], 2);

    let _ = stop.send(());
    tokio::time::timeout(Duration::from_secs(5), task)
        .await
        .expect("server stops")
        .expect("join")
        .expect("run ok");
    let _ = std::fs::remove_dir_all(&data_dir);
}
