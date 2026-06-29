//! Storage round-trip and cascade tests against an in-memory SQLite database.

use flighttrace_core::{
    AssertionOutcome, AssertionResult, AssertionResultId, FailureReason, FailureReport,
    FailureReportId, MissionPhase, MissionRun, MissionRunId, MissionSource, MissionStatus,
    NormalizedEvent, NormalizedEventKind, PhaseTransition, TelemetrySample, Timestamp,
};
use flighttrace_demo::generate_failed_takeoff_run;
use flighttrace_storage::{
    AssertionResultRepo, FailureReportRepo, MissionRunRepo, NormalizedEventRepo, RunBundle,
    TelemetrySampleRepo, connect_in_memory, load_bundle, persist_bundle,
};
use time::OffsetDateTime;
use time::macros::datetime;

fn demo_bundle_as_storage_bundle() -> RunBundle {
    let demo = generate_failed_takeoff_run();
    RunBundle {
        run: demo.run,
        events: demo.events,
        telemetry: demo.telemetry,
        assertions: demo.assertions,
        failure_report: demo.failure_report,
    }
}

#[tokio::test]
async fn full_bundle_round_trip() {
    let pool = connect_in_memory().await.expect("pool");
    let bundle = demo_bundle_as_storage_bundle();

    let id = persist_bundle(&pool, &bundle).await.expect("persist");
    assert_eq!(id, bundle.run.id);

    let loaded = load_bundle(&pool, id)
        .await
        .expect("load")
        .expect("present");
    assert_eq!(loaded.run, bundle.run);
    assert_eq!(loaded.events, bundle.events);
    assert_eq!(loaded.telemetry, bundle.telemetry);
    assert_eq!(loaded.assertions, bundle.assertions);
    assert_eq!(loaded.failure_report, bundle.failure_report);
}

#[tokio::test]
async fn run_repo_lists_in_started_at_desc_order() {
    let pool = connect_in_memory().await.expect("pool");
    let repo = MissionRunRepo::new(&pool);

    let earlier = stub_run(0x10, datetime!(2025-01-01 00:00:00 UTC));
    let middle = stub_run(0x11, datetime!(2025-01-02 00:00:00 UTC));
    let latest = stub_run(0x12, datetime!(2025-01-03 00:00:00 UTC));

    // Insert out of order on purpose.
    repo.insert(&middle).await.unwrap();
    repo.insert(&latest).await.unwrap();
    repo.insert(&earlier).await.unwrap();

    let listed = repo.list(10, 0).await.unwrap();
    let ids: Vec<MissionRunId> = listed.iter().map(|r| r.id).collect();
    assert_eq!(ids, vec![latest.id, middle.id, earlier.id]);
}

#[tokio::test]
async fn run_repo_limit_and_offset() {
    let pool = connect_in_memory().await.expect("pool");
    let repo = MissionRunRepo::new(&pool);

    for i in 0..5u128 {
        let run = stub_run(
            0x20 + i,
            datetime!(2025-01-01 00:00:00 UTC) + time::Duration::seconds(i as i64),
        );
        repo.insert(&run).await.unwrap();
    }

    let page = repo.list(2, 0).await.unwrap();
    assert_eq!(page.len(), 2);
    let next = repo.list(2, 2).await.unwrap();
    assert_eq!(next.len(), 2);
    let tail = repo.list(2, 4).await.unwrap();
    assert_eq!(tail.len(), 1);
}

#[tokio::test]
async fn cascade_delete_removes_all_children() {
    let pool = connect_in_memory().await.expect("pool");
    let bundle = demo_bundle_as_storage_bundle();
    persist_bundle(&pool, &bundle).await.unwrap();

    let id = bundle.run.id;
    assert!(
        !NormalizedEventRepo::new(&pool)
            .list_for_run(id)
            .await
            .unwrap()
            .is_empty()
    );

    let removed = MissionRunRepo::new(&pool).delete(id).await.unwrap();
    assert!(removed);

    assert!(MissionRunRepo::new(&pool).get(id).await.unwrap().is_none());
    assert!(
        NormalizedEventRepo::new(&pool)
            .list_for_run(id)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        TelemetrySampleRepo::new(&pool)
            .list_for_run(id)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        AssertionResultRepo::new(&pool)
            .list_for_run(id)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        FailureReportRepo::new(&pool)
            .get_for_run(id)
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn individual_repos_round_trip_single_rows() {
    let pool = connect_in_memory().await.expect("pool");
    let run_id = MissionRunId::from_u128(0x30);
    let run = stub_run(0x30, datetime!(2025-01-01 00:00:00 UTC));
    MissionRunRepo::new(&pool).insert(&run).await.unwrap();

    let event = NormalizedEvent {
        id: flighttrace_core::EventId::from_u128(0x31),
        run_id,
        at: Timestamp(datetime!(2025-01-01 00:00:01 UTC)),
        kind: NormalizedEventKind::ArmedChanged { armed: true },
    };
    NormalizedEventRepo::new(&pool)
        .insert(&event)
        .await
        .unwrap();
    assert_eq!(
        NormalizedEventRepo::new(&pool)
            .list_for_run(run_id)
            .await
            .unwrap(),
        vec![event]
    );

    let sample = TelemetrySample {
        run_id,
        at: Timestamp(datetime!(2025-01-01 00:00:02 UTC)),
        altitude_m: 1.5,
        vertical_speed_mps: 0.6,
        mode: "GUIDED".into(),
        armed: true,
        battery_remaining_pct: 80.0,
    };
    TelemetrySampleRepo::new(&pool)
        .insert(&sample)
        .await
        .unwrap();
    assert_eq!(
        TelemetrySampleRepo::new(&pool)
            .list_for_run(run_id)
            .await
            .unwrap(),
        vec![sample]
    );

    let assertion = AssertionResult {
        id: AssertionResultId::from_u128(0x32),
        run_id,
        name: "demo_assertion".into(),
        outcome: AssertionOutcome::Pass,
        detail: "ok".into(),
        evaluated_at: Timestamp(datetime!(2025-01-01 00:00:03 UTC)),
    };
    AssertionResultRepo::new(&pool)
        .insert(&assertion)
        .await
        .unwrap();
    assert_eq!(
        AssertionResultRepo::new(&pool)
            .list_for_run(run_id)
            .await
            .unwrap(),
        vec![assertion]
    );

    let report = FailureReport {
        id: FailureReportId::from_u128(0x33),
        run_id,
        reason: FailureReason::TakeoffTimeout,
        summary: "Mission failed: TAKEOFF_TIMEOUT".into(),
        evidence: vec![],
        likely_cause: "Vehicle never reached target altitude.".into(),
        generated_at: Timestamp(datetime!(2025-01-01 00:00:04 UTC)),
    };
    FailureReportRepo::new(&pool).insert(&report).await.unwrap();
    assert_eq!(
        FailureReportRepo::new(&pool)
            .get_for_run(run_id)
            .await
            .unwrap(),
        Some(report)
    );
}

fn stub_run(id_seed: u128, started_at: OffsetDateTime) -> MissionRun {
    MissionRun {
        id: MissionRunId::from_u128(id_seed),
        vehicle: "demo".into(),
        mission: "simple_takeoff".into(),
        source: MissionSource::Demo,
        status: MissionStatus::Failed,
        failure_reason: Some(FailureReason::TakeoffTimeout),
        started_at: Timestamp(started_at),
        ended_at: Some(Timestamp(started_at + time::Duration::seconds(12))),
        phases: vec![PhaseTransition {
            at: Timestamp(started_at),
            phase: MissionPhase::Connected,
        }],
    }
}
