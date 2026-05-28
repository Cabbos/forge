use parking_lot::{Mutex, MutexGuard};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tokio::sync::Notify;

pub(crate) fn lock_unpoisoned<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock()
}

#[derive(Debug)]
pub(crate) struct TurnInflightGuard {
    active: Arc<AtomicBool>,
}

impl Drop for TurnInflightGuard {
    fn drop(&mut self) {
        self.active.store(false, Ordering::SeqCst);
    }
}

pub(crate) fn try_begin_turn(active: Arc<AtomicBool>) -> Result<TurnInflightGuard, String> {
    active
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .map(|_| TurnInflightGuard { active })
        .map_err(|_| "当前会话仍在处理上一条请求，请等待完成，或先停止后再继续。".to_string())
}

pub(crate) fn sub_agent_join_error_message(error: &tokio::task::JoinError) -> String {
    if error.is_cancelled() {
        "子任务已取消，主任务会继续收集已有结果。".to_string()
    } else if error.is_panic() {
        format!("子任务异常中断：{}", error)
    } else {
        format!("子任务执行失败：{}", error)
    }
}

#[derive(Debug)]
pub(crate) struct ActiveCancelGuard<'a> {
    slot: &'a Mutex<Option<Arc<Notify>>>,
    token: Arc<Notify>,
}

impl<'a> ActiveCancelGuard<'a> {
    pub(crate) fn new(slot: &'a Mutex<Option<Arc<Notify>>>, token: Arc<Notify>) -> Self {
        Self { slot, token }
    }
}

impl Drop for ActiveCancelGuard<'_> {
    fn drop(&mut self) {
        let mut current = lock_unpoisoned(self.slot);
        if current
            .as_ref()
            .is_some_and(|token| Arc::ptr_eq(token, &self.token))
        {
            *current = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{lock_unpoisoned, sub_agent_join_error_message, try_begin_turn, ActiveCancelGuard};
    use parking_lot::Mutex;
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };
    use tokio::sync::Notify;

    #[test]
    fn turn_inflight_guard_rejects_concurrent_turn_and_releases_on_drop() {
        let flag = Arc::new(AtomicBool::new(false));

        let first_turn = try_begin_turn(flag.clone()).expect("first turn should start");
        assert!(flag.load(Ordering::SeqCst));

        let error = try_begin_turn(flag.clone()).expect_err("second turn should be rejected");
        assert!(error.contains("上一条请求"));

        drop(first_turn);
        assert!(!flag.load(Ordering::SeqCst));

        let second_turn = try_begin_turn(flag.clone()).expect("guard should release on drop");
        drop(second_turn);
        assert!(!flag.load(Ordering::SeqCst));
    }

    #[test]
    fn active_cancel_guard_clears_only_current_token() {
        let slot = Mutex::new(None);
        let first = Arc::new(Notify::new());
        let second = Arc::new(Notify::new());

        *lock_unpoisoned(&slot) = Some(first.clone());
        let first_guard = ActiveCancelGuard::new(&slot, first);
        *lock_unpoisoned(&slot) = Some(second.clone());
        drop(first_guard);

        assert!(lock_unpoisoned(&slot)
            .as_ref()
            .is_some_and(|current| Arc::ptr_eq(current, &second)));

        let second_guard = ActiveCancelGuard::new(&slot, second);
        drop(second_guard);
        assert!(lock_unpoisoned(&slot).is_none());
    }

    #[tokio::test]
    async fn sub_agent_join_error_message_reports_panics() {
        let previous_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let handle = tokio::spawn(async {
            panic!("sub-agent fixture panic");
        });
        let err = handle.await.expect_err("fixture task should panic");
        std::panic::set_hook(previous_hook);

        let message = sub_agent_join_error_message(&err);

        assert!(message.contains("子任务"));
        assert!(message.contains("异常中断"));
    }
}
