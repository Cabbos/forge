#[derive(Debug, Clone, serde::Serialize)]
pub struct ManualCompactResult {
    pub compacted: bool,
    pub skipped_reason: Option<String>,
    pub retained_messages: usize,
    pub compacted_messages: usize,
    pub estimated_tokens_before: u32,
    pub estimated_tokens_after: u32,
}
