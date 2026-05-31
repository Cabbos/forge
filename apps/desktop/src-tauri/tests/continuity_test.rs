use forge::continuity::{
    form_experiences_from_reflection, ContinuityEvent, ContinuityService, ContinuityStore,
    ExperienceKind, ExperienceStatus, ReflectionEvent, ReflectionOutcome,
};
use std::path::PathBuf;

fn reflection_with_lessons(lessons: Vec<&str>) -> ReflectionEvent {
    ReflectionEvent {
        session_id: "session-1".to_string(),
        user_goal: "Add continuity shadow mode".to_string(),
        execution_summary: "Edited Rust backend and ran cargo test".to_string(),
        outcome: ReflectionOutcome::Completed,
        verification_summary: Some("cargo test passed".to_string()),
        lessons: lessons.into_iter().map(str::to_string).collect(),
        timestamp_ms: 1_778_688_000_000,
    }
}

#[test]
fn reflection_lessons_form_candidate_experiences() {
    let reflection = reflection_with_lessons(vec![
        "When adding IPC-adjacent behavior, keep the first slice backend-only.",
        "When adding IPC-adjacent behavior, keep the first slice backend-only.",
        "  Use Reflection before memory injection.  ",
    ]);

    let experiences =
        form_experiences_from_reflection(&reflection, Some("/repo/forge"), 1_778_688_001_000);

    assert_eq!(experiences.len(), 2);
    assert_eq!(experiences[0].kind, ExperienceKind::Lesson);
    assert_eq!(experiences[0].status, ExperienceStatus::Candidate);
    assert_eq!(
        experiences[0].source_session_id.as_deref(),
        Some("session-1")
    );
    assert_eq!(experiences[0].project_path.as_deref(), Some("/repo/forge"));
    assert_eq!(
        experiences[0].body,
        "When adding IPC-adjacent behavior, keep the first slice backend-only."
    );
    assert!(experiences[0].title.contains("IPC-adjacent"));
    assert!(experiences[0].confidence >= 0.7);
    assert_eq!(experiences[0].created_at_ms, 1_778_688_001_000);
}

#[test]
fn formation_rejects_empty_and_sensitive_lessons() {
    let reflection = reflection_with_lessons(vec![
        "   ",
        "Remember the API key is sk-1234567890abcdefghijkl",
        "Prefer shadow mode until candidate quality is measured.",
    ]);

    let experiences = form_experiences_from_reflection(&reflection, None, 1_778_688_001_000);

    assert_eq!(experiences.len(), 1);
    assert_eq!(
        experiences[0].body,
        "Prefer shadow mode until candidate quality is measured."
    );
}

#[test]
fn failed_reflection_forms_lower_confidence_candidates() {
    let mut reflection = reflection_with_lessons(vec!["Do not auto-inject unverified lessons."]);
    reflection.outcome = ReflectionOutcome::Failed;

    let experiences = form_experiences_from_reflection(&reflection, Some("/repo/forge"), 42);

    assert_eq!(experiences.len(), 1);
    assert!(experiences[0].confidence < 0.7);
    assert_eq!(experiences[0].status, ExperienceStatus::Candidate);
}

#[test]
fn store_records_and_reloads_session_events_in_timestamp_order() {
    let db_path = temp_db_path("roundtrip");
    {
        let store = ContinuityStore::open(&db_path).expect("open store");
        store
            .record_event(
                "/repo/forge",
                &ContinuityEvent::AssistantResponse {
                    session_id: "session-1".to_string(),
                    content_summary: "Answered with implementation summary".to_string(),
                    timestamp_ms: 30,
                },
            )
            .expect("record assistant");
        store
            .record_event(
                "/repo/forge",
                &ContinuityEvent::UserMessage {
                    session_id: "session-1".to_string(),
                    content: "继续".to_string(),
                    timestamp_ms: 10,
                },
            )
            .expect("record user");
    }

    let reopened = ContinuityStore::open(&db_path).expect("reopen store");
    let events = reopened
        .list_events_for_session("/repo/forge", "session-1")
        .expect("list events");

    assert_eq!(events.len(), 2);
    assert!(matches!(events[0], ContinuityEvent::UserMessage { .. }));
    assert!(matches!(
        events[1],
        ContinuityEvent::AssistantResponse { .. }
    ));

    let _ = std::fs::remove_file(db_path);
}

#[test]
fn store_isolates_events_by_project_path() {
    let db_path = temp_db_path("project-isolation");
    let store = ContinuityStore::open(&db_path).expect("open store");

    store
        .record_event(
            "/repo/forge",
            &ContinuityEvent::UserMessage {
                session_id: "session-1".to_string(),
                content: "repo one".to_string(),
                timestamp_ms: 10,
            },
        )
        .expect("record forge");
    store
        .record_event(
            "/repo/other",
            &ContinuityEvent::UserMessage {
                session_id: "session-1".to_string(),
                content: "repo two".to_string(),
                timestamp_ms: 20,
            },
        )
        .expect("record other");

    let events = store
        .list_events_for_session("/repo/forge", "session-1")
        .expect("list events");

    assert_eq!(events.len(), 1);
    assert_eq!(
        events[0],
        ContinuityEvent::UserMessage {
            session_id: "session-1".to_string(),
            content: "repo one".to_string(),
            timestamp_ms: 10,
        }
    );

    let _ = std::fs::remove_file(db_path);
}

#[test]
fn store_roundtrips_reflection_events() {
    let db_path = temp_db_path("reflection");
    let store = ContinuityStore::open(&db_path).expect("open store");
    let reflection = reflection_with_lessons(vec![
        "Keep graph deferred until experience quality is proven.",
    ]);

    store
        .record_event(
            "/repo/forge",
            &ContinuityEvent::Reflection(reflection.clone()),
        )
        .expect("record reflection");

    let events = store
        .list_events_for_session("/repo/forge", "session-1")
        .expect("list events");

    assert_eq!(events, vec![ContinuityEvent::Reflection(reflection)]);

    let _ = std::fs::remove_file(db_path);
}

#[test]
fn store_upserts_candidate_experiences_idempotently() {
    let db_path = temp_db_path("experience-upsert");
    let store = ContinuityStore::open(&db_path).expect("open store");
    let reflection =
        reflection_with_lessons(vec!["Keep candidate lessons out of automatic injection."]);
    let experiences = form_experiences_from_reflection(&reflection, Some("/repo/forge"), 42);

    let first_inserted = store
        .upsert_experiences(&experiences)
        .expect("first upsert");
    let second_inserted = store
        .upsert_experiences(&experiences)
        .expect("second upsert");
    let stored = store
        .list_experiences_for_project("/repo/forge")
        .expect("list experiences");

    assert_eq!(first_inserted.len(), 1);
    assert!(second_inserted.is_empty());
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0], experiences[0]);

    let _ = std::fs::remove_file(db_path);
}

#[test]
fn service_forms_and_persists_experiences_from_recorded_reflections() {
    let db_path = temp_db_path("service-formation");
    let service = ContinuityService::open(&db_path).expect("open service");
    let reflection = reflection_with_lessons(vec![
        "Keep graph deferred until recall quality is measured.",
    ]);

    service
        .record_event(
            "/repo/forge",
            &ContinuityEvent::Reflection(reflection.clone()),
        )
        .expect("record reflection");

    let formed = service
        .form_experiences_for_session("/repo/forge", "session-1", 100)
        .expect("form experiences");
    let formed_again = service
        .form_experiences_for_session("/repo/forge", "session-1", 101)
        .expect("form experiences again");
    let stored = service
        .list_experiences_for_project("/repo/forge")
        .expect("list experiences");

    assert_eq!(formed.len(), 1);
    assert!(formed_again.is_empty());
    assert_eq!(stored.len(), 1);
    assert_eq!(
        stored[0].body,
        "Keep graph deferred until recall quality is measured."
    );

    let _ = std::fs::remove_file(db_path);
}

#[test]
fn service_searches_project_experiences_by_query_terms() {
    let db_path = temp_db_path("service-search");
    let service = ContinuityService::open(&db_path).expect("open service");
    let reflection = reflection_with_lessons(vec![
        "Use Reflection before memory injection.",
        "Keep graph deferred until recall quality is measured.",
    ]);

    service
        .record_event("/repo/forge", &ContinuityEvent::Reflection(reflection))
        .expect("record reflection");
    service
        .form_experiences_for_session("/repo/forge", "session-1", 100)
        .expect("form experiences");

    let results = service
        .search_experiences_for_project("/repo/forge", "reflection memory", 5)
        .expect("search experiences");

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].body, "Use Reflection before memory injection.");

    let _ = std::fs::remove_file(db_path);
}

#[test]
fn service_search_keeps_project_boundaries() {
    let db_path = temp_db_path("service-search-boundaries");
    let service = ContinuityService::open(&db_path).expect("open service");
    let reflection = reflection_with_lessons(vec!["Use Reflection before memory injection."]);

    service
        .record_event(
            "/repo/forge",
            &ContinuityEvent::Reflection(reflection.clone()),
        )
        .expect("record forge reflection");
    service
        .record_event("/repo/other", &ContinuityEvent::Reflection(reflection))
        .expect("record other reflection");
    service
        .form_experiences_for_session("/repo/forge", "session-1", 100)
        .expect("form forge experiences");
    service
        .form_experiences_for_session("/repo/other", "session-1", 100)
        .expect("form other experiences");

    let results = service
        .search_experiences_for_project("/repo/forge", "reflection", 5)
        .expect("search experiences");

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].project_path.as_deref(), Some("/repo/forge"));

    let _ = std::fs::remove_file(db_path);
}

fn temp_db_path(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "forge-continuity-{label}-{}-{}.db",
        std::process::id(),
        uuid::Uuid::now_v7()
    ))
}
