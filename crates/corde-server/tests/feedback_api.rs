//! Driver feedback on a recorded session: server in-process with
//! `--source sim` (so a session exists within a few seconds), then
//! `POST /api/sessions/{id}/feedback` (201 with the interpretation and a plan
//! iteration, 400 on empty / too long / bad plan, 404 on an unknown session),
//! `GET …/feedback` (oldest first, interpretation joined), `GET …/iterations`
//! (numbered per plan, delta on top of the previous one) and the cascade on
//! `DELETE /api/sessions/{id}` once the session is closed.

use corde_protocol::{
    ChangeDirection, CornerPhase, CornerSpeed, ExtractorKind, Feedback, FeedbackInterpretation,
    PlanIteration, SetupPhilosophy,
};
use corde_server::{Server, ServerConfig, SourceKind};
use serde_json::{json, Value};
use std::net::IpAddr;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tokio::sync::oneshot;

fn temp_data_dir(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "corde-feedback-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

async fn http(method: &'static str, url: String, body: Option<Value>) -> (u16, Value) {
    tokio::task::spawn_blocking(move || {
        let agent: ureq::Agent = ureq::config::Config::builder()
            .http_status_as_error(false)
            .proxy(None)
            .build()
            .new_agent();
        let mut resp = match (method, body) {
            ("GET", _) => agent.get(&url).call().expect("GET"),
            ("POST", Some(b)) => agent.post(&url).send_json(&b).expect("POST json"),
            ("POST", None) => agent.post(&url).send_empty().expect("POST"),
            ("DELETE", _) => agent.delete(&url).call().expect("DELETE"),
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
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stores_and_lists_driver_feedback() {
    let data_dir = temp_data_dir("crud");
    let cfg = ServerConfig {
        port: 0,
        bind: Some(IpAddr::from([127, 0, 0, 1])),
        source: SourceKind::Sim,
        data_dir: Some(data_dir.clone()),
        sim_seed: Some(3),
        sim_ai: Some(1),
        sim_lap_time: Some(30.0),
        sim_track_length: Some(1500.0),
        sim_speed: Some(3.0),
        sim_rate: Some(50.0),
        record_rate: 10.0,
        ..Default::default()
    };
    let server = Server::bind(cfg).await.expect("bind");
    let base = format!("http://{}", server.local_addr());
    let (stop, stop_rx) = oneshot::channel::<()>();
    let task = tokio::spawn(server.run(async move {
        let _ = stop_rx.await;
    }));

    let rec = wait_for("recording to start", Duration::from_secs(15), async || {
        let (code, v) = http("GET", format!("{base}/api/recording"), None).await;
        assert_eq!(code, 200);
        (v["recording"] == true && v["session_id"].is_string()).then_some(v)
    })
    .await;
    let id = rec["session_id"].as_str().unwrap().to_string();
    let url = format!("{base}/api/sessions/{id}/feedback");
    let iterations_url = format!("{base}/api/sessions/{id}/iterations");

    // The offline extractor is the default, and health says so.
    let (code, health) = http("GET", format!("{base}/api/health"), None).await;
    assert_eq!(code, 200);
    assert_eq!(health["llm"], "dictionary", "{health}");

    // Nothing yet.
    let (code, body) = http("GET", url.clone(), None).await;
    assert_eq!(code, 200, "{body}");
    assert_eq!(body, json!([]));
    let (code, body) = http("GET", iterations_url.clone(), None).await;
    assert_eq!(code, 200, "{body}");
    assert_eq!(body, json!([]));
    let (code, _) = http("GET", format!("{iterations_url}?plan=medium"), None).await;
    assert_eq!(code, 400);

    // Unknown session → 404 on both verbs.
    let unknown = format!("{base}/api/sessions/20200101-000000-nowhere-race/feedback");
    let (code, _) = http("GET", unknown.clone(), None).await;
    assert_eq!(code, 404);
    let (code, _) = http("POST", unknown, Some(json!({ "text": "x" }))).await;
    assert_eq!(code, 404);
    let (code, _) = http(
        "GET",
        format!("{base}/api/sessions/20200101-000000-nowhere-race/iterations"),
        None,
    )
    .await;
    assert_eq!(code, 404);

    // Validation: empty / blank / too long / unknown plan / wrong shape → 400 with a French message.
    for bad in [
        json!({ "text": "" }),
        json!({ "text": "   \n " }),
        json!({ "text": "é".repeat(2001) }),
        json!({ "plan": "medium", "text": "ok" }),
        json!({ "text": 12 }),
    ] {
        let (code, body) = http("POST", url.clone(), Some(bad.clone())).await;
        assert_eq!(code, 400, "{bad} → {body}");
        assert!(body["error"].is_string(), "{body}");
    }

    // 201 with the stored row, its interpretation and the first iteration of the safe plan;
    // text trimmed; the understeer-on-entry-slow symptom drives a coherent change.
    let (code, first) = http(
        "POST",
        url.clone(),
        Some(json!({
            "plan": "safe",
            "text": "  l'avant ne tourne pas en entrée des virages lents  "
        })),
    )
    .await;
    assert_eq!(code, 201, "{first}");
    let first_feedback: Feedback =
        serde_json::from_value(first["feedback"].clone()).expect("typed feedback");
    let first_interp: FeedbackInterpretation =
        serde_json::from_value(first["interpretation"].clone()).expect("typed interpretation");
    let first_iter: PlanIteration =
        serde_json::from_value(first["iteration"].clone()).expect("typed iteration");
    assert_eq!(first_feedback.session_id, id);
    assert_eq!(first_feedback.plan, Some(SetupPhilosophy::Safe));
    assert_eq!(
        first_feedback.text,
        "l'avant ne tourne pas en entrée des virages lents"
    );
    assert!(first_feedback.id > 0 && first_feedback.created_ms > 1_700_000_000_000);
    assert_eq!(first_feedback.interpretation, Some(first_interp.clone()));
    assert_eq!(first_interp.feedback_id, first_feedback.id);
    assert_eq!(first_interp.extractor, ExtractorKind::Dictionary);
    let us = first_interp
        .symptoms
        .iter()
        .find(|s| s.symptom_id == "US_ENTRY_SLOW")
        .expect("understeer entry slow");
    assert_eq!(us.phase, Some(CornerPhase::Entry));
    assert_eq!(us.corner_speed, Some(CornerSpeed::Slow));
    assert!(!us.negated);
    assert!(first_interp.confidence > 0.6, "{}", first_interp.confidence);
    assert_eq!(first_iter.iteration, 1);
    assert_eq!(first_iter.base_plan, SetupPhilosophy::Safe);
    assert_eq!(first_iter.feedback_id, Some(first_feedback.id));
    assert_eq!(first_iter.session_id, id);
    assert!(first_iter.id > 0);
    assert!(!first_iter.delta.is_empty(), "{:?}", first_iter.notes_fr);
    assert!(first_iter.delta.len() <= 3);
    // The lever comes from the US-ENTRY-SLOW-01 rule (front bias down, or coast / front bar…).
    assert!(
        first_iter
            .delta
            .iter()
            .all(|c| c.rule_ids.contains(&"US-ENTRY-SLOW-01".to_string())),
        "{:?}",
        first_iter.delta
    );
    let lever = &first_iter.delta[0];
    assert!(
        (lever.parameter_id == "brake.bias_front" && lever.direction == ChangeDirection::Decrease)
            || (lever.parameter_id == "susp.arb_front"
                && lever.direction == ChangeDirection::Decrease)
            || (lever.parameter_id == "diff.coast" && lever.direction == ChangeDirection::Decrease),
        "{lever:?}"
    );
    assert!(lever.rationale_fr.starts_with("Itération 1 —"));
    assert!(first_iter.changes.len() >= first_iter.delta.len());
    for d in &first_iter.delta {
        assert!(first_iter
            .changes
            .iter()
            .any(|c| c.parameter_id == d.parameter_id));
    }

    // A text without any symptom still stores and iterates (empty delta, a note), plan optional.
    let long = "é".repeat(2000);
    let (code, second) = http("POST", url.clone(), Some(json!({ "text": long }))).await;
    assert_eq!(code, 201, "{second}");
    let second_feedback: Feedback = serde_json::from_value(second["feedback"].clone()).unwrap();
    assert_eq!(second_feedback.plan, None);
    assert_eq!(second_feedback.text.chars().count(), 2000);
    let second_iter: PlanIteration = serde_json::from_value(second["iteration"].clone()).unwrap();
    assert_eq!(
        second_iter.base_plan,
        SetupPhilosophy::Safe,
        "last iterated plan"
    );
    assert_eq!(second_iter.iteration, 2);
    assert!(second_iter.delta.is_empty());
    assert!(second_iter
        .notes_fr
        .iter()
        .any(|n| n.contains("Aucun symptôme")));
    assert!(second["interpretation"]["symptoms"]
        .as_array()
        .unwrap()
        .is_empty());

    // Second real feedback on the same plan → iteration 3 with a delta on another parameter.
    let (code, third) = http(
        "POST",
        url.clone(),
        Some(json!({ "plan": "safe", "text": "toujours du sous-virage en entrée des virages lents, un peu moins" })),
    )
    .await;
    assert_eq!(code, 201, "{third}");
    let third_iter: PlanIteration = serde_json::from_value(third["iteration"].clone()).unwrap();
    assert_eq!(third_iter.iteration, 3);
    assert!(!third_iter.delta.is_empty(), "{:?}", third_iter.notes_fr);
    for d in &third_iter.delta {
        assert!(
            !first_iter
                .changes
                .iter()
                .any(|c| c.parameter_id == d.parameter_id),
            "new parameter: {}",
            d.parameter_id
        );
        assert!(d.rationale_fr.starts_with("Itération 3 —"));
    }
    assert!(third_iter.changes.len() > first_iter.changes.len());
    for c in &first_iter.changes {
        assert!(third_iter
            .changes
            .iter()
            .any(|x| x.parameter_id == c.parameter_id));
    }

    // Another plan starts its own numbering.
    let (code, wet) = http(
        "POST",
        url.clone(),
        Some(json!({ "plan": "wet", "text": "ça glisse de l'arrière à la remise des gaz" })),
    )
    .await;
    assert_eq!(code, 201, "{wet}");
    let wet_iter: PlanIteration = serde_json::from_value(wet["iteration"].clone()).unwrap();
    assert_eq!(wet_iter.base_plan, SetupPhilosophy::Wet);
    assert_eq!(wet_iter.iteration, 1);
    assert!(wet["interpretation"]["symptoms"]
        .as_array()
        .unwrap()
        .iter()
        .any(|s| s["symptom_id"] == "OS_EXIT"));

    // Listed oldest first, typed, interpretation joined.
    let (code, list) = http("GET", url.clone(), None).await;
    assert_eq!(code, 200);
    let list: Vec<Feedback> = serde_json::from_value(list).unwrap();
    assert_eq!(list.len(), 4);
    assert_eq!(list[0], first_feedback);
    assert_eq!(list[1], second_feedback);
    assert_eq!(list[3].plan, Some(SetupPhilosophy::Wet));
    assert!(list.iter().all(|f| f.interpretation.is_some()));
    assert!(list.windows(2).all(|w| w[0].id < w[1].id));
    let (code, its) = http("GET", iterations_url.clone(), None).await;
    assert_eq!(code, 200);
    let its: Vec<PlanIteration> = serde_json::from_value(its).unwrap();
    assert_eq!(its.len(), 4);
    assert_eq!(its[0], first_iter);
    assert_eq!(its[3], wet_iter);
    let (code, safe_only) = http("GET", format!("{iterations_url}?plan=safe"), None).await;
    assert_eq!(code, 200);
    let safe_only: Vec<PlanIteration> = serde_json::from_value(safe_only).unwrap();
    assert_eq!(safe_only.len(), 3);
    assert!(safe_only
        .iter()
        .all(|it| it.base_plan == SetupPhilosophy::Safe));

    // Still there in the session detail's neighbourhood: the session itself is untouched.
    let (code, detail) = http("GET", format!("{base}/api/sessions/{id}"), None).await;
    assert_eq!(code, 200);
    assert_eq!(detail["id"], id);

    // Stop recording, delete the session → feedback gone with it (cascade), 404 afterwards.
    let (code, _) = http("POST", format!("{base}/api/recording/stop"), None).await;
    assert_eq!(code, 202);
    wait_for("session to close", Duration::from_secs(10), async || {
        let (_, v) = http("GET", format!("{base}/api/sessions/{id}"), None).await;
        v["ended_wall_ms"].is_number().then_some(v)
    })
    .await;
    let (code, _) = http("DELETE", format!("{base}/api/sessions/{id}"), None).await;
    assert_eq!(code, 204);
    let (code, _) = http("GET", url.clone(), None).await;
    assert_eq!(code, 404);
    let (code, _) = http("GET", iterations_url.clone(), None).await;
    assert_eq!(code, 404);

    let _ = stop.send(());
    tokio::time::timeout(Duration::from_secs(10), task)
        .await
        .expect("server stops")
        .expect("join")
        .expect("run ok");

    let store = corde_store::Store::open(&data_dir).expect("reopen store");
    let n: i64 = store
        .db()
        .conn()
        .query_row("SELECT COUNT(*) FROM feedback", [], |r| r.get(0))
        .unwrap();
    assert_eq!(n, 0, "cascade on disk");
    let n: i64 = store
        .db()
        .conn()
        .query_row("SELECT COUNT(*) FROM plan_iterations", [], |r| r.get(0))
        .unwrap();
    assert_eq!(n, 0, "cascade on disk (iterations)");
    assert_eq!(
        store.db().schema_version().unwrap(),
        corde_store::db::SCHEMA_VERSION
    );
    drop(store);
    let _ = std::fs::remove_dir_all(&data_dir);
}
