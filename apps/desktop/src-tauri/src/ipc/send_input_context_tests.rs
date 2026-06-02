use crate::harness::capability::CapabilityKind;
use crate::harness::Harness;
use crate::ipc::send_input_context::capability_names_by_kind;

#[test]
fn turn_capability_names_omit_internal_infrastructure() {
    let nonce = uuid::Uuid::now_v7();
    let workspace = std::env::temp_dir().join(format!("forge-turn-capabilities-{nonce}"));
    std::fs::create_dir_all(&workspace).expect("workspace");
    let harness = Harness::new(workspace.clone());

    let skills = capability_names_by_kind(&harness, CapabilityKind::Skill);
    let hooks = capability_names_by_kind(&harness, CapabilityKind::Hook);

    assert!(!skills.iter().any(|name| name == "Skill Loader"));
    assert!(!hooks.iter().any(|name| name == "Logging Hook"));
    assert!(!hooks.iter().any(|name| name == "File System Audit Hook"));
    assert!(hooks.iter().any(|name| name == "Sensitive Content Guard"));
    assert!(hooks.iter().any(|name| name == "Workspace Boundary Guard"));

    let _ = std::fs::remove_dir_all(&workspace);
}
