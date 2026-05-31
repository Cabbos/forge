use forge::continuity::{
    form_experiences_from_reflection, ExperienceKind, ExperienceStatus, ReflectionEvent,
    ReflectionOutcome,
};

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
