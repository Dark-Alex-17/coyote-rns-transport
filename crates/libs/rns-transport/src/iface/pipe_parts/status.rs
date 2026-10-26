use std::sync::Arc;

use super::PipeRuntimeStatus;

pub(super) fn update_pipe_status(
    runtime_status: &Arc<std::sync::Mutex<PipeRuntimeStatus>>,
    update: impl FnOnce(&mut PipeRuntimeStatus),
) {
    let mut guard = runtime_status.lock().expect("pipe runtime status mutex poisoned");
    update(&mut guard);
}

/// Records one side of the pump failing: the pipe is no longer open and the
/// state names the side that failed.
pub(super) fn mark_task_failed(
    runtime_status: &Arc<std::sync::Mutex<PipeRuntimeStatus>>,
    state: &str,
    error: String,
) {
    update_pipe_status(runtime_status, |status| {
        status.process_state = state.to_string();
        status.pipe_is_open = false;
        status.last_error = Some(error);
    });
}
