use forge::continuity::{
    form_continuity_experience_context, form_experiences_from_reflection, ContinuityEvent,
    ContinuityService, ContinuityStore, Episode, ExperienceKind, ExperienceMemory,
    ExperienceStatus, FileChangeRecord, ReflectionEvent, ReflectionOutcome,
};
use rusqlite::{params, Connection};
use std::path::{Path, PathBuf};

fn reflection_with_lessons(lessons: Vec<&str>) -> ReflectionEvent {
    ReflectionEvent {
        session_id: "session-1".to_string(),
        user_goal: "Add continuity shadow mode".to_string(),
        execution_summary: "Edited Rust backend and ran cargo test".to_string(),
        outcome: ReflectionOutcome::Completed,
        verification_summary: Some("cargo test passed".to_string()),
        lessons: lessons.into_iter().map(str::to_string).collect(),
        episode: None,
        timestamp_ms: 1_778_688_000_000,
    }
}

fn experience_with_status(
    id: &str,
    status: ExperienceStatus,
    body: &str,
    updated_at_ms: u64,
) -> ExperienceMemory {
    ExperienceMemory {
        id: id.to_string(),
        kind: ExperienceKind::Lesson,
        status,
        title: body.split('.').next().unwrap_or(body).to_string(),
        body: body.to_string(),
        project_path: Some("/repo/forge".to_string()),
        source_session_id: Some("session-1".to_string()),
        confidence: 0.74,
        created_at_ms: 10,
        updated_at_ms,
        tags: Vec::new(),
    }
}

fn temp_project_path(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "forge-continuity-test-{label}-{}-{}",
        std::process::id(),
        uuid::Uuid::now_v7()
    ))
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
fn formation_rejects_prompt_echo_question_lessons() {
    let reflection = reflection_with_lessons(vec![
        "项目已定方案：接下来这个项目有什么可以继续的方向呢: 接下来这个项目有什么可以继续的方向呢",
        "TaskNotes 当前只使用 useState，刷新后任务会丢失，下一步应加 localStorage 持久化验证。",
    ]);

    let experiences =
        form_experiences_from_reflection(&reflection, Some("/repo/forge"), 1_778_688_001_000);

    assert_eq!(experiences.len(), 1);
    assert_eq!(
        experiences[0].body,
        "TaskNotes 当前只使用 useState，刷新后任务会丢失，下一步应加 localStorage 持久化验证。"
    );
}

#[test]
fn formation_rejects_raw_user_prompt_as_lesson() {
    let reflection = reflection_with_lessons(vec![
        "我们现在在 /Users/cabbos/project/test-app 做一次 Forge Continuity 的人工验证。目标不是大改项目，而是验证本地经验系统的完整闭环。",
        "When adding IPC-adjacent behavior, keep the first slice backend-only.",
    ]);

    let experiences = form_experiences_from_reflection(&reflection, Some("/repo/forge"), 42);

    assert_eq!(experiences.len(), 1);
    assert_eq!(
        experiences[0].body,
        "When adding IPC-adjacent behavior, keep the first slice backend-only."
    );
}

#[test]
fn formation_rejects_short_low_value_continuation() {
    let reflection =
        reflection_with_lessons(vec!["继续", "就行", "下一步应加 localStorage 持久化验证"]);

    let experiences = form_experiences_from_reflection(&reflection, Some("/repo/forge"), 42);

    assert_eq!(experiences.len(), 1);
    assert_eq!(experiences[0].body, "下一步应加 localStorage 持久化验证");
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
    let project = temp_project_path("roundtrip");
    let db_path = project.join(".forge").join("continuity.db");
    {
        let store = ContinuityStore::open(&db_path).expect("open store");
        store
            .record_event(
                &project.to_string_lossy(),
                &ContinuityEvent::AssistantResponse {
                    session_id: "session-1".to_string(),
                    content_summary: "Answered with implementation summary".to_string(),
                    timestamp_ms: 30,
                },
            )
            .expect("record assistant");
        store
            .record_event(
                &project.to_string_lossy(),
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
        .list_events_for_session(&project.to_string_lossy(), "session-1")
        .expect("list events");

    assert_eq!(events.len(), 2);
    assert!(matches!(events[0], ContinuityEvent::UserMessage { .. }));
    assert!(matches!(
        events[1],
        ContinuityEvent::AssistantResponse { .. }
    ));

    let _ = std::fs::remove_dir_all(&project);
}

#[test]
fn store_isolates_events_by_project_path() {
    let project_a = temp_project_path("project-a");
    let project_b = temp_project_path("project-b");
    let db_path_a = project_a.join(".forge").join("continuity.db");
    let store = ContinuityStore::open(&db_path_a).expect("open store");

    store
        .record_event(
            &project_a.to_string_lossy(),
            &ContinuityEvent::UserMessage {
                session_id: "session-1".to_string(),
                content: "repo one".to_string(),
                timestamp_ms: 10,
            },
        )
        .expect("record forge");
    store
        .record_event(
            &project_b.to_string_lossy(),
            &ContinuityEvent::UserMessage {
                session_id: "session-1".to_string(),
                content: "repo two".to_string(),
                timestamp_ms: 20,
            },
        )
        .expect("record other");

    let events = store
        .list_events_for_session(&project_a.to_string_lossy(), "session-1")
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

    let _ = std::fs::remove_dir_all(&project_a);
    let _ = std::fs::remove_dir_all(&project_b);
}

#[test]
fn store_roundtrips_reflection_events() {
    let project = temp_project_path("reflection");
    let db_path = project.join(".forge").join("continuity.db");
    let store = ContinuityStore::open(&db_path).expect("open store");
    let reflection = reflection_with_lessons(vec![
        "Keep graph deferred until experience quality is proven.",
    ]);

    store
        .record_event(
            &project.to_string_lossy(),
            &ContinuityEvent::Reflection(reflection.clone()),
        )
        .expect("record reflection");

    let events = store
        .list_events_for_session(&project.to_string_lossy(), "session-1")
        .expect("list events");

    assert_eq!(events, vec![ContinuityEvent::Reflection(reflection)]);

    let _ = std::fs::remove_dir_all(&project);
}

#[test]
fn store_upserts_candidate_experiences_idempotently() {
    let project = temp_project_path("experience-upsert");
    let db_path = project.join(".forge").join("continuity.db");
    let store = ContinuityStore::open(&db_path).expect("open store");
    let reflection =
        reflection_with_lessons(vec!["Keep candidate lessons out of automatic injection."]);
    let experiences =
        form_experiences_from_reflection(&reflection, Some(&project.to_string_lossy()), 42);

    let first_inserted = store
        .upsert_experiences(&experiences)
        .expect("first upsert");
    let second_inserted = store
        .upsert_experiences(&experiences)
        .expect("second upsert");
    let stored = store
        .list_experiences_for_project(&project.to_string_lossy())
        .expect("list experiences");

    assert_eq!(first_inserted.len(), 1);
    assert!(second_inserted.is_empty());
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0], experiences[0]);

    let _ = std::fs::remove_dir_all(&project);
}

#[test]
fn store_updates_experience_status_and_json_snapshot() {
    let project = temp_project_path("experience-status-update");
    let db_path = project.join(".forge").join("continuity.db");
    let store = ContinuityStore::open(&db_path).expect("open store");
    let mut experience = experience_with_status(
        "experience-status",
        ExperienceStatus::Candidate,
        "Run npm test after changing package scripts.",
        10,
    );
    experience.project_path = Some(project.to_string_lossy().to_string());
    store
        .upsert_experiences(std::slice::from_ref(&experience))
        .expect("insert experience");

    let updated = store
        .update_experience_status(
            &project.to_string_lossy(),
            "experience-status",
            ExperienceStatus::Pinned,
            99,
        )
        .expect("update experience");
    let listed = store
        .list_experiences_for_project(&project.to_string_lossy())
        .expect("list experiences");

    assert_eq!(updated.status, ExperienceStatus::Pinned);
    assert_eq!(updated.updated_at_ms, 99);
    assert_eq!(listed[0].status, ExperienceStatus::Pinned);
    assert_eq!(listed[0].updated_at_ms, 99);

    let _ = std::fs::remove_dir_all(&project);
}

#[test]
fn store_recall_uses_only_accepted_and_pinned_with_pinned_first() {
    let project = temp_project_path("experience-recall");
    let db_path = project.join(".forge").join("continuity.db");
    let store = ContinuityStore::open(&db_path).expect("open store");
    let project_path_str = project.to_string_lossy().to_string();
    let mut candidate = experience_with_status(
        "experience-candidate",
        ExperienceStatus::Candidate,
        "Package script changes should run npm test.",
        40,
    );
    candidate.project_path = Some(project_path_str.clone());
    let mut accepted = experience_with_status(
        "experience-accepted",
        ExperienceStatus::Accepted,
        "Package script changes should run npm test after editing package.json.",
        30,
    );
    accepted.project_path = Some(project_path_str.clone());
    let mut pinned = experience_with_status(
        "experience-pinned",
        ExperienceStatus::Pinned,
        "Pinned package script lesson for npm test verification.",
        20,
    );
    pinned.project_path = Some(project_path_str.clone());
    let mut forgotten = experience_with_status(
        "experience-forgotten",
        ExperienceStatus::Forgotten,
        "Forgotten package script lesson should not appear.",
        50,
    );
    forgotten.project_path = Some(project_path_str.clone());
    store
        .upsert_experiences(&[candidate, accepted.clone(), pinned.clone(), forgotten])
        .expect("insert experiences");

    let recalled = store
        .recall_experiences_for_project(&project_path_str, "package script npm test", 5)
        .expect("recall experiences");
    let context = form_continuity_experience_context(&recalled).expect("context");

    assert_eq!(
        recalled
            .iter()
            .map(|experience| experience.id.as_str())
            .collect::<Vec<_>>(),
        vec!["experience-pinned", "experience-accepted"]
    );
    assert!(context.contains("[pinned] Pinned package script lesson"));
    assert!(context.contains("[accepted] Package script changes"));
    assert!(!context.contains("candidate"));
    assert!(!context.contains("Forgotten"));

    let _ = std::fs::remove_dir_all(&project);
}

#[test]
fn store_migration_indexes_existing_experiences_for_search() {
    let project = temp_project_path("legacy-experience-search");
    let db_path = project.join(".forge").join("continuity.db");
    std::fs::create_dir_all(db_path.parent().unwrap()).expect("create .forge dir");
    let reflection = reflection_with_lessons(vec!["Use Reflection before memory injection."]);
    let experience =
        form_experiences_from_reflection(&reflection, Some(&project.to_string_lossy()), 42)
            .remove(0);
    seed_legacy_experience_db(&db_path, &experience);

    let store = ContinuityStore::open(&db_path).expect("open store");
    let results = store
        .search_experiences_for_project(&project.to_string_lossy(), "reflection memory", 5)
        .expect("search experiences");

    assert_eq!(results.len(), 1);
    assert_eq!(results[0], experience);

    let _ = std::fs::remove_dir_all(&project);
}

#[test]
fn service_forms_and_persists_experiences_from_recorded_reflections() {
    let project = temp_project_path("service-formation");
    let service = ContinuityService::new();
    let reflection = reflection_with_lessons(vec![
        "Keep graph deferred until recall quality is measured.",
    ]);

    let project_path = project.to_string_lossy().to_string();
    service
        .record_event(
            &project_path,
            &ContinuityEvent::Reflection(reflection.clone()),
        )
        .expect("record reflection");

    let formed = service
        .form_experiences_for_session(&project_path, "session-1", 100)
        .expect("form experiences");
    let formed_again = service
        .form_experiences_for_session(&project_path, "session-1", 101)
        .expect("form experiences again");
    let stored = service
        .list_experiences_for_project(&project_path)
        .expect("list experiences");

    assert_eq!(formed.len(), 1);
    assert!(formed_again.is_empty());
    assert_eq!(stored.len(), 1);
    assert_eq!(
        stored[0].body,
        "Keep graph deferred until recall quality is measured."
    );

    let _ = std::fs::remove_dir_all(&project);
}

#[test]
fn service_searches_project_experiences_by_query_terms() {
    let project = temp_project_path("service-search");
    let service = ContinuityService::new();
    let reflection = reflection_with_lessons(vec![
        "Use Reflection before memory injection.",
        "Keep graph deferred until recall quality is measured.",
    ]);

    let project_path = project.to_string_lossy().to_string();
    service
        .record_event(&project_path, &ContinuityEvent::Reflection(reflection))
        .expect("record reflection");
    service
        .form_experiences_for_session(&project_path, "session-1", 100)
        .expect("form experiences");

    let results = service
        .search_experiences_for_project(&project_path, "reflection memory", 5)
        .expect("search experiences");

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].body, "Use Reflection before memory injection.");

    let _ = std::fs::remove_dir_all(&project);
}

#[test]
fn service_search_ranks_denser_term_matches_first() {
    let project = temp_project_path("service-search-ranking");
    let service = ContinuityService::new();
    let reflection = reflection_with_lessons(vec![
        "Reflection belongs in continuity recall.",
        "Reflection reflection reflection recall should rank above a single mention.",
    ]);

    let project_path = project.to_string_lossy().to_string();
    service
        .record_event(&project_path, &ContinuityEvent::Reflection(reflection))
        .expect("record reflection");
    service
        .form_experiences_for_session(&project_path, "session-1", 100)
        .expect("form experiences");

    let results = service
        .search_experiences_for_project(&project_path, "reflection", 5)
        .expect("search experiences");

    assert_eq!(results.len(), 2);
    assert_eq!(
        results[0].body,
        "Reflection reflection reflection recall should rank above a single mention."
    );

    let _ = std::fs::remove_dir_all(&project);
}

#[test]
fn service_search_keeps_project_boundaries() {
    let project_a = temp_project_path("service-search-boundaries-a");
    let project_b = temp_project_path("service-search-boundaries-b");
    let service = ContinuityService::new();
    let reflection = reflection_with_lessons(vec!["Use Reflection before memory injection."]);

    let path_a = project_a.to_string_lossy().to_string();
    let path_b = project_b.to_string_lossy().to_string();
    service
        .record_event(&path_a, &ContinuityEvent::Reflection(reflection.clone()))
        .expect("record forge reflection");
    service
        .record_event(&path_b, &ContinuityEvent::Reflection(reflection))
        .expect("record other reflection");
    service
        .form_experiences_for_session(&path_a, "session-1", 100)
        .expect("form forge experiences");
    service
        .form_experiences_for_session(&path_b, "session-1", 100)
        .expect("form other experiences");

    let results = service
        .search_experiences_for_project(&path_a, "reflection", 5)
        .expect("search experiences");

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].project_path.as_deref(), Some(path_a.as_str()));

    let _ = std::fs::remove_dir_all(&project_a);
    let _ = std::fs::remove_dir_all(&project_b);
}

#[test]
fn service_uses_per_project_db() {
    let project_a = temp_project_path("service-db-a");
    let project_b = temp_project_path("service-db-b");
    let service = ContinuityService::new();
    let path_a = project_a.to_string_lossy().to_string();
    let path_b = project_b.to_string_lossy().to_string();

    service
        .record_event(
            &path_a,
            &ContinuityEvent::UserMessage {
                session_id: "session-1".to_string(),
                content: "project a message".to_string(),
                timestamp_ms: 10,
            },
        )
        .expect("record a");
    service
        .record_event(
            &path_b,
            &ContinuityEvent::UserMessage {
                session_id: "session-1".to_string(),
                content: "project b message".to_string(),
                timestamp_ms: 20,
            },
        )
        .expect("record b");

    // Each project should have its own physical DB file
    assert!(project_a.join(".forge").join("continuity.db").exists());
    assert!(project_b.join(".forge").join("continuity.db").exists());

    let events_a = service
        .list_experiences_for_project(&path_a)
        .expect("list a");
    let events_b = service
        .list_experiences_for_project(&path_b)
        .expect("list b");

    // Both should be empty (no experiences formed yet, just events)
    assert_eq!(events_a.len(), 0);
    assert_eq!(events_b.len(), 0);

    let _ = std::fs::remove_dir_all(&project_a);
    let _ = std::fs::remove_dir_all(&project_b);
}

#[test]
fn service_records_review_event_on_status_change() {
    let project = temp_project_path("service-review-event");
    let service = ContinuityService::new();
    let reflection =
        reflection_with_lessons(vec!["Keep candidate lessons out of automatic injection."]);

    let project_path = project.to_string_lossy().to_string();
    service
        .record_event(&project_path, &ContinuityEvent::Reflection(reflection))
        .expect("record reflection");
    let formed = service
        .form_experiences_for_session(&project_path, "session-1", 100)
        .expect("form experiences");
    assert_eq!(formed.len(), 1);
    let experience_id = &formed[0].id;

    // Accept the candidate
    let updated = service
        .update_experience_status(
            &project_path,
            experience_id,
            ExperienceStatus::Accepted,
            Some("review-session"),
            200,
        )
        .expect("update status");
    assert_eq!(updated.status, ExperienceStatus::Accepted);

    // A review event should be attributed to the session that performed the review,
    // not the session that originally produced the candidate.
    let events = ContinuityStore::open(project.join(".forge").join("continuity.db"))
        .expect("open store")
        .list_events_for_session(&project_path, "review-session")
        .expect("list events");

    let review_events: Vec<_> = events
        .iter()
        .filter(|e| matches!(e, ContinuityEvent::ExperienceStatusChanged { .. }))
        .collect();
    assert_eq!(review_events.len(), 1);
    if let ContinuityEvent::ExperienceStatusChanged {
        experience_id: eid,
        old_status,
        new_status,
        project_path: pp,
        ..
    } = &review_events[0]
    {
        assert_eq!(eid, experience_id);
        assert_eq!(*old_status, ExperienceStatus::Candidate);
        assert_eq!(*new_status, ExperienceStatus::Accepted);
        assert_eq!(pp.as_deref(), Some(project_path.as_str()));
    }

    let _ = std::fs::remove_dir_all(&project);
}

#[test]
fn store_migration_prunes_prompt_echo_candidate_experiences() {
    let project = temp_project_path("migration-prunes-prompt-echo");
    let db_path = project.join(".forge").join("continuity.db");
    std::fs::create_dir_all(db_path.parent().unwrap()).expect("create .forge dir");
    let prompt_echo = ExperienceMemory {
        id: "experience-old-prompt-echo".to_string(),
        kind: ExperienceKind::Lesson,
        status: ExperienceStatus::Candidate,
        title: "项目已定方案：接下来这个项目有什么可以继续的方向呢: 接下来这个项目有什么可以继续的方向呢".to_string(),
        body: "项目已定方案：接下来这个项目有什么可以继续的方向呢: 接下来这个项目有什么可以继续的方向呢".to_string(),
        project_path: Some(project.to_string_lossy().to_string()),
        source_session_id: Some("session-1".to_string()),
        confidence: 0.74,
        created_at_ms: 10,
        updated_at_ms: 10,
        tags: Vec::new(),
    };
    seed_legacy_experience_db(&db_path, &prompt_echo);

    let service = ContinuityService::open(&db_path).expect("open service");
    let listed = service
        .list_experiences_for_project(&project.to_string_lossy())
        .expect("list experiences");
    let searched = service
        .search_experiences_for_project(&project.to_string_lossy(), "接下来", 5)
        .expect("search experiences");

    assert!(listed.is_empty());
    assert!(searched.is_empty());

    let _ = std::fs::remove_dir_all(&project);
}

#[test]
fn candidate_does_not_auto_inject() {
    let project = temp_project_path("candidate-no-inject");
    let service = ContinuityService::new();
    let reflection = reflection_with_lessons(vec![
        "Keep graph deferred until recall quality is measured.",
    ]);

    let project_path = project.to_string_lossy().to_string();
    service
        .record_event(&project_path, &ContinuityEvent::Reflection(reflection))
        .expect("record reflection");
    service
        .form_experiences_for_session(&project_path, "session-1", 100)
        .expect("form experiences");

    let recalled = service
        .recall_experiences_for_project(&project_path, "graph deferred", 5)
        .expect("recall experiences");

    // Candidate experiences should NOT appear in recall
    assert!(recalled.is_empty());

    let _ = std::fs::remove_dir_all(&project);
}

#[test]
fn accepted_experience_forms_hidden_context() {
    let project = temp_project_path("accepted-context");
    let service = ContinuityService::new();
    let reflection =
        reflection_with_lessons(vec!["Keep candidate lessons out of automatic injection."]);

    let project_path = project.to_string_lossy().to_string();
    service
        .record_event(&project_path, &ContinuityEvent::Reflection(reflection))
        .expect("record reflection");
    let formed = service
        .form_experiences_for_session(&project_path, "session-1", 100)
        .expect("form experiences");
    assert_eq!(formed.len(), 1);

    service
        .update_experience_status(
            &project_path,
            &formed[0].id,
            ExperienceStatus::Accepted,
            Some("review-session"),
            200,
        )
        .expect("accept");

    let recalled = service
        .recall_experiences_for_project(&project_path, "candidate injection", 5)
        .expect("recall");
    assert_eq!(recalled.len(), 1);

    let context = form_continuity_experience_context(&recalled);
    assert!(context.is_some());
    let context_text = context.unwrap();
    assert!(context_text.contains("[accepted]"));
    assert!(context_text.contains("Keep candidate lessons"));

    let _ = std::fs::remove_dir_all(&project);
}

#[test]
fn episode_based_formation_produces_structured_experience() {
    let reflection = ReflectionEvent {
        session_id: "session-1".to_string(),
        user_goal: "Add npm test script".to_string(),
        execution_summary: "Updated package.json and ran tests".to_string(),
        outcome: ReflectionOutcome::Completed,
        verification_summary: Some("npm test passed".to_string()),
        lessons: vec![], // lessons are ignored when episode is present
        episode: Some(Episode {
            project_path: Some("/repo".to_string()),
            session_id: "session-1".to_string(),
            user_goal_summary: "Add npm test script".to_string(),
            changed_files: vec!["package.json".to_string()],
            tool_count: 3,
            failed_tools: 0,
            file_changes: vec![FileChangeRecord {
                path: "package.json".to_string(),
                operation: "modified".to_string(),
                tool_name: "write_file".to_string(),
            }],
            verification_status: forge::continuity::AgentVerificationStatus::Passed,
            verification_command: Some("npm test".to_string()),
            verification_summary: Some("passed; cmd=npm test; exit=0".to_string()),
            outcome: ReflectionOutcome::Completed,
            evidence_event_ids: vec!["t1".to_string(), "t2".to_string(), "t3".to_string()],
            notable_failures: vec![],
            final_result_summary: Some(
                "1 write(s), 1 shell command(s), 1 file(s) changed".to_string(),
            ),
            timestamp_ms: 1_778_688_000_000,
        }),
        timestamp_ms: 1_778_688_000_000,
    };

    let experiences = form_experiences_from_reflection(&reflection, Some("/repo"), 42);

    assert_eq!(experiences.len(), 1);
    let exp = &experiences[0];
    assert_eq!(exp.kind, ExperienceKind::Lesson);
    assert!(exp.body.contains("Problem:"));
    assert!(exp.body.contains("Fix:"));
    assert!(exp.body.contains("Verified by:"));
    assert!(exp.body.contains("Applies when:"));
    assert!(exp.body.contains("Evidence:"));
    assert!(exp.body.contains("package.json"));
    assert!(exp.confidence >= 0.7);
}

#[test]
fn episode_formation_skips_no_file_changes() {
    let reflection = ReflectionEvent {
        session_id: "session-1".to_string(),
        user_goal: "Run tests".to_string(),
        execution_summary: "Ran npm test".to_string(),
        outcome: ReflectionOutcome::Completed,
        verification_summary: Some("tests passed".to_string()),
        lessons: vec!["Legacy lesson that should be ignored".to_string()],
        episode: Some(Episode {
            project_path: Some("/repo".to_string()),
            session_id: "session-1".to_string(),
            user_goal_summary: "Run tests".to_string(),
            changed_files: vec![],
            tool_count: 1,
            failed_tools: 0,
            file_changes: vec![],
            verification_status: forge::continuity::AgentVerificationStatus::Passed,
            verification_command: Some("npm test".to_string()),
            verification_summary: Some("passed".to_string()),
            outcome: ReflectionOutcome::Completed,
            evidence_event_ids: vec!["t1".to_string()],
            notable_failures: vec![],
            final_result_summary: Some("1 shell command(s)".to_string()),
            timestamp_ms: 1_778_688_000_000,
        }),
        timestamp_ms: 1_778_688_000_000,
    };

    let experiences = form_experiences_from_reflection(&reflection, Some("/repo"), 42);

    assert!(
        experiences.is_empty(),
        "episode with no file changes should produce no experiences"
    );
}

#[test]
fn episode_formation_produces_bug_pattern_for_failed_tools() {
    let reflection = ReflectionEvent {
        session_id: "session-1".to_string(),
        user_goal: "Fix build error".to_string(),
        execution_summary: "Fixed import and ran cargo test".to_string(),
        outcome: ReflectionOutcome::Failed,
        verification_summary: Some("cargo test failed".to_string()),
        lessons: vec![],
        episode: Some(Episode {
            project_path: Some("/repo".to_string()),
            session_id: "session-1".to_string(),
            user_goal_summary: "Fix build error".to_string(),
            changed_files: vec!["src/lib.rs".to_string()],
            tool_count: 2,
            failed_tools: 1,
            file_changes: vec![FileChangeRecord {
                path: "src/lib.rs".to_string(),
                operation: "modified".to_string(),
                tool_name: "write_file".to_string(),
            }],
            verification_status: forge::continuity::AgentVerificationStatus::Failed,
            verification_command: Some("cargo test".to_string()),
            verification_summary: Some("failed; cmd=cargo test; exit=101".to_string()),
            outcome: ReflectionOutcome::Failed,
            evidence_event_ids: vec!["t1".to_string(), "t2".to_string()],
            notable_failures: vec![forge::continuity::ToolFailureRecord {
                tool_name: "run_shell".to_string(),
                command: Some("cargo test".to_string()),
                summary: "error: unresolved import".to_string(),
            }],
            final_result_summary: Some("Build failed".to_string()),
            timestamp_ms: 1_778_688_000_000,
        }),
        timestamp_ms: 1_778_688_000_000,
    };

    let experiences = form_experiences_from_reflection(&reflection, Some("/repo"), 42);

    assert!(
        !experiences.is_empty(),
        "failed turn with file changes should produce experiences"
    );
    let primary = &experiences[0];
    assert_eq!(primary.kind, ExperienceKind::BugPattern);
    assert!(primary.body.contains("Cause:"));
    assert!(
        primary.confidence < 0.7,
        "failed verification should lower confidence"
    );
}

#[test]
fn legacy_reflection_without_episode_still_forms_lessons() {
    let reflection = reflection_with_lessons(vec![
        "When adding IPC-adjacent behavior, keep the first slice backend-only.",
    ]);

    let experiences = form_experiences_from_reflection(&reflection, Some("/repo"), 42);

    assert_eq!(experiences.len(), 1);
    assert_eq!(
        experiences[0].body,
        "When adding IPC-adjacent behavior, keep the first slice backend-only."
    );
}

fn seed_legacy_experience_db(db_path: &Path, experience: &ExperienceMemory) {
    let conn = Connection::open(db_path).expect("open legacy db");
    conn.execute_batch(
        "
        CREATE TABLE continuity_experiences (
            id TEXT PRIMARY KEY,
            project_path TEXT,
            source_session_id TEXT,
            kind TEXT NOT NULL,
            status TEXT NOT NULL,
            title TEXT NOT NULL,
            body TEXT NOT NULL,
            confidence REAL NOT NULL,
            created_at_ms INTEGER NOT NULL,
            updated_at_ms INTEGER NOT NULL,
            tags_json TEXT NOT NULL,
            experience_json TEXT NOT NULL,
            created_at TEXT DEFAULT (datetime('now'))
        );
        ",
    )
    .expect("create legacy experiences table");

    let tags_json = serde_json::to_string(&experience.tags).expect("serialize tags");
    let experience_json = serde_json::to_string(experience).expect("serialize experience");
    conn.execute(
        "INSERT INTO continuity_experiences
            (
                id,
                project_path,
                source_session_id,
                kind,
                status,
                title,
                body,
                confidence,
                created_at_ms,
                updated_at_ms,
                tags_json,
                experience_json
            )
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            experience.id,
            experience.project_path.as_deref(),
            experience.source_session_id.as_deref(),
            "lesson",
            "candidate",
            experience.title,
            experience.body,
            experience.confidence,
            experience.created_at_ms as i64,
            experience.updated_at_ms as i64,
            tags_json,
            experience_json,
        ],
    )
    .expect("insert legacy experience");
}
