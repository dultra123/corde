//! End-to-end analysis: server in-process with `--source sim` (1.5 km track,
//! 30 s laps played ×3 → 10 s per lap, 20 Hz on disk), wait for four laps,
//! then check `GET /api/sessions/{id}/analysis` (422 before any clean lap,
//! 200 with a typed report, cache after the session closes, recompute), and
//! call `corde_engineer::analyse_session` directly on the store afterwards.

use corde_protocol::AnalysisReport;
use corde_server::{Server, ServerConfig, SourceKind};
use serde_json::Value;
use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tokio::sync::oneshot;

fn temp_data_dir(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "corde-analysis-{tag}-{}-{}",
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

fn check_report(r: &AnalysisReport, id: &str) {
    assert_eq!(r.session_id, id);
    assert_eq!(r.track_name, "Paul Ricard - 1A");
    assert!(!r.vehicle.is_empty());
    let q = &r.data_quality;
    assert!(q.valid_laps >= 1 && q.valid_laps <= q.total_laps, "{q:?}");
    assert!(
        (15.0..=25.0).contains(&q.sample_rate_hz),
        "rate {}",
        q.sample_rate_hz
    );
    assert!(
        q.has_slip,
        "the simulator fills slip_ratio / slip_angle: {q:?}"
    );
    assert!(
        q.has_ride_height,
        "the simulator fills ride_height_m: {q:?}"
    );
    assert!(
        q.has_optimal_temp,
        "optimal_temp_c_* is recorded and the simulator fills it: {q:?}"
    );
    assert_eq!(r.engine_version, corde_engineer::ENGINE_VERSION);
    for w in &r.tyres.per_wheel {
        assert_eq!(w.optimal_c, Some(90.0), "{w:?}");
        let d = w.delta_to_optimal_c.expect("delta to the optimum");
        assert!((d - (w.avg_carcass_c - 90.0)).abs() < 1e-3, "{w:?}");
    }
    assert!(!q.wet_session);
    assert_eq!(r.laps_analysed.len() as u32, q.valid_laps);
    assert!(r.lap_stats.best_s.unwrap() > 25.0, "{:?}", r.lap_stats);
    assert!(r.lap_stats.top_speed_kmh > 200.0);
    assert!(r.lap_stats.fuel_per_lap_l.unwrap() > 0.0);
    assert!(!r.balance.corners.is_empty(), "the sim track has corners");
    assert!(r
        .balance
        .corners
        .iter()
        .all(|c| c.passes >= 1 && c.max_g_lat > 0.5));
    assert!(!r.balance.overall_fr.is_empty());
    // The matrix is relative to the session reference, the absolute index is kept aside.
    let reference = r
        .balance
        .reference
        .as_ref()
        .expect("balance reference on the sim session");
    assert!(reference.passes >= 5, "{reference:?}");
    assert!(reference.slip_balance.is_some(), "{reference:?}");
    assert!(r.balance.overall_fr.contains("ta référence de la session"));
    for f in r.findings.iter().filter(|f| f.diagnostic_id == "D-BAL-01") {
        assert!(
            f.title_fr.contains("par rapport à ta référence"),
            "{}",
            f.title_fr
        );
    }
    for w in &r.tyres.per_wheel {
        assert!(w.avg_surface_c > 50.0 && w.avg_carcass_c > 50.0, "{w:?}");
        assert!(w.avg_pressure_kpa > 100.0, "{w:?}");
        assert!(w.inner_c > 0.0 && w.outer_c > 0.0);
    }
    for b in &r.brakes.per_wheel {
        assert!(b.max_c >= b.avg_c && b.avg_c > 100.0, "{b:?}");
    }
    assert!(r.brakes.bias_rear_avg.is_some());
    assert!(
        r.suspension.min_ride_height_mm[0] > 10.0,
        "{:?}",
        r.suspension
    );
    let h = &r.suspension.damper_velocity_hist[0];
    let total = h.slow_bump_pct + h.slow_rebound_pct + h.fast_bump_pct + h.fast_rebound_pct;
    assert!((total - 100.0).abs() < 1.0, "{h:?}");
    assert!(r.driving_style.brake_aggression >= 0.0 && r.driving_style.brake_aggression <= 100.0);
    assert!(r.driving_style.trail_braking >= 0.0 && r.driving_style.trail_braking <= 100.0);
    assert!(!r.plans.aggressive.changes.is_empty());
    assert!(!r.plans.safe.changes.is_empty());
    assert!(!r.plans.wet.changes.is_empty());
    for p in [&r.plans.aggressive, &r.plans.safe, &r.plans.wet] {
        assert!(p.changes.len() <= 8);
        for c in &p.changes {
            assert!(
                !c.parameter_id.is_empty() && !c.parameter_key.is_empty() && !c.label_fr.is_empty()
            );
            assert!((1..=5).contains(&c.priority));
            assert!(!c.rule_ids.is_empty());
            assert!(!c.rationale_fr.is_empty() && !c.rationale_en.is_empty());
        }
    }
    assert!(r
        .plans
        .wet
        .warnings_fr
        .iter()
        .any(|w| w.contains("Session sèche")));
    assert!(r.summary_fr.contains("Orientation conseillée"));
    assert!(r.summary_en.contains("Advised orientation"));
    for f in &r.findings {
        assert!(!f.id.is_empty() && !f.diagnostic_id.is_empty());
        assert!((0.0..=1.0).contains(&f.confidence));
        assert!(!f.title_fr.is_empty() && !f.title_en.is_empty());
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn analyses_a_recorded_sim_session() {
    let data_dir = temp_data_dir("laps");
    let cfg = ServerConfig {
        port: 0,
        bind: Some(IpAddr::from([127, 0, 0, 1])),
        source: SourceKind::Sim,
        data_dir: Some(data_dir.clone()),
        sim_seed: Some(11),
        sim_ai: Some(2),
        sim_lap_time: Some(30.0),
        sim_track_length: Some(1500.0),
        sim_speed: Some(3.0),
        sim_rate: Some(100.0),
        record_rate: 20.0,
        ..Default::default()
    };
    let server = Server::bind(cfg).await.expect("bind");
    let addr: SocketAddr = server.local_addr();
    let base = format!("http://{addr}");
    let (stop, stop_rx) = oneshot::channel::<()>();
    let task = tokio::spawn(server.run(async move {
        let _ = stop_rx.await;
    }));

    let rec = wait_for("recording to start", Duration::from_secs(15), async || {
        let (code, v) = http("GET", format!("{base}/api/recording")).await;
        assert_eq!(code, 200);
        (v["recording"] == true && v["session_id"].is_string()).then_some(v)
    })
    .await;
    let id = rec["session_id"].as_str().unwrap().to_string();

    // Unknown session → 404 ; malformed id → 400 ; no clean lap yet → 422 with a French message.
    let (code, _) = http(
        "GET",
        format!("{base}/api/sessions/20200101-000000-nowhere-race/analysis"),
    )
    .await;
    assert_eq!(code, 404);
    let (code, _) = http("GET", format!("{base}/api/sessions/..%2Fx/analysis")).await;
    assert!(code == 400 || code == 404, "{code}");
    let (code, body) = http("GET", format!("{base}/api/sessions/{id}/analysis")).await;
    assert_eq!(code, 422, "{body}");
    assert!(
        body["error"]
            .as_str()
            .unwrap()
            .contains("aucun tour exploitable"),
        "{body}"
    );

    // Four laps (10 s each): the first one is partial, three clean ones follow.
    wait_for("4 laps", Duration::from_secs(90), async || {
        let (code, v) = http("GET", format!("{base}/api/sessions/{id}")).await;
        assert_eq!(code, 200, "{v}");
        (v["laps"].as_array().map(|l| l.len()).unwrap_or(0) >= 4).then_some(v)
    })
    .await;

    // Live session: computed, typed, not cached.
    let (code, body) = http("GET", format!("{base}/api/sessions/{id}/analysis")).await;
    assert_eq!(code, 200, "{body}");
    let live: AnalysisReport = serde_json::from_value(body.clone()).expect("typed report");
    check_report(&live, &id);
    assert!(live.data_quality.valid_laps >= 2, "{:?}", live.data_quality);

    // Stop → closed session → cached report (same generated_ms twice), recompute refreshes it.
    let (code, _) = http("POST", format!("{base}/api/recording/stop")).await;
    assert_eq!(code, 202);
    wait_for("session to close", Duration::from_secs(10), async || {
        let (_, v) = http("GET", format!("{base}/api/sessions/{id}")).await;
        v["ended_wall_ms"].is_number().then_some(v)
    })
    .await;
    let (code, first) = http("GET", format!("{base}/api/sessions/{id}/analysis")).await;
    assert_eq!(code, 200, "{first}");
    tokio::time::sleep(Duration::from_millis(20)).await;
    let (code, second) = http("GET", format!("{base}/api/sessions/{id}/analysis")).await;
    assert_eq!(code, 200);
    assert_eq!(
        first["generated_ms"], second["generated_ms"],
        "served from the cache"
    );
    assert_eq!(first["findings"], second["findings"]);
    let cached: AnalysisReport = serde_json::from_value(first.clone()).unwrap();
    check_report(&cached, &id);
    assert!(
        cached.data_quality.valid_laps >= 3,
        "{:?}",
        cached.data_quality
    );
    tokio::time::sleep(Duration::from_millis(20)).await;
    let (code, third) = http(
        "POST",
        format!("{base}/api/sessions/{id}/analysis/recompute"),
    )
    .await;
    assert_eq!(code, 200, "{third}");
    assert!(third["generated_ms"].as_u64().unwrap() > first["generated_ms"].as_u64().unwrap());
    assert_eq!(third["findings"], first["findings"], "deterministic");
    let (_, fourth) = http("GET", format!("{base}/api/sessions/{id}/analysis")).await;
    assert_eq!(fourth["generated_ms"], third["generated_ms"]);

    // A report cached by an older engine (absolute balance, no version) is recomputed.
    {
        let mut old = fourth.clone();
        let obj = old.as_object_mut().unwrap();
        obj.remove("engine_version");
        obj.insert("generated_ms".into(), Value::from(1u64));
        let other = corde_store::Store::open(&data_dir).expect("second connection");
        other
            .db()
            .put_analysis(&id, 1, &serde_json::to_string(&old).unwrap())
            .unwrap();
    }
    let (code, fifth) = http("GET", format!("{base}/api/sessions/{id}/analysis")).await;
    assert_eq!(code, 200, "{fifth}");
    assert_ne!(fifth["generated_ms"], Value::from(1u64), "recomputed");
    assert_eq!(fifth["engine_version"], corde_engineer::ENGINE_VERSION);
    assert_eq!(fifth["findings"], first["findings"]);

    let _ = stop.send(());
    tokio::time::timeout(Duration::from_secs(10), task)
        .await
        .expect("server stops")
        .expect("join")
        .expect("run ok");

    // Library path on the closed store.
    let store = corde_store::Store::open(&data_dir).expect("reopen store");
    assert!(
        store.db().get_analysis(&id).unwrap().is_some(),
        "cached row"
    );
    let report =
        corde_engineer::analyse_session(&store, &id, &corde_engineer::AnalyseOptions::default())
            .expect("analyse_session");
    check_report(&report, &id);
    assert_eq!(report.findings, cached.findings);
    let json = serde_json::to_string(&report).unwrap();
    let back: AnalysisReport = serde_json::from_str(&json).unwrap();
    assert_eq!(back.plans, report.plans);
    let err = corde_engineer::analyse_session(
        &store,
        "20200101-000000-nowhere-race",
        &Default::default(),
    )
    .unwrap_err();
    assert!(err.is_not_found());
    drop(store);
    let _ = std::fs::remove_dir_all(&data_dir);
}
