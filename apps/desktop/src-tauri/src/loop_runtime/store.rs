use crate::loop_runtime::{LoopEventJournal, LoopTaskProjectionStore};

#[derive(Debug, Clone)]
pub struct LoopRuntimeStore {
    pub journal: LoopEventJournal,
    pub projection_store: LoopTaskProjectionStore,
}

impl LoopRuntimeStore {
    pub fn persistent_default() -> Self {
        Self {
            journal: LoopEventJournal::persistent_default(),
            projection_store: LoopTaskProjectionStore::persistent_default(),
        }
    }
}
