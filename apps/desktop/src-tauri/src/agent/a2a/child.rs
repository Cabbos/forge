use std::path::Path;
use std::sync::Arc;
use tokio::sync::Notify;

use crate::adapters::base::AiAdapter;
use crate::agent::event_sink::EventEmitter;
use crate::harness::Harness;

pub(crate) struct ChildAgentRuntime;

impl ChildAgentRuntime {
    pub(crate) async fn run_read_only(
        task: &str,
        adapter: Arc<dyn AiAdapter>,
        harness: Arc<Harness>,
        emitter: &dyn EventEmitter,
        cancel: Arc<Notify>,
        working_dir: &Path,
    ) -> String {
        crate::agent::sub::SubAgent::run_with_emitter(
            task,
            adapter,
            harness,
            emitter,
            cancel,
            working_dir,
        )
        .await
    }
}
