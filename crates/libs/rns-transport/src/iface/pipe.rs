use alloc::string::String;
use alloc::vec::Vec;
use std::sync::Arc;
use std::time::Duration;

use tokio::process::Command;
use tokio_util::sync::CancellationToken;

use super::{Interface, InterfaceContext, InterfaceTxReceiver};
use crate::hash::AddressHash;
use crate::iface::{IfacState, IfaceRole, InterfaceManager, RxMessage};

#[path = "pipe_parts/process_cleanup.rs"]
mod process_cleanup;
use process_cleanup::terminate_pipe_child;

#[path = "pipe_parts/status.rs"]
mod status;
use status::update_pipe_status;

#[path = "pipe_parts/stream.rs"]
mod stream;
use stream::run_pipe_stream;

pub struct PipeInterface {
    command: String,
    respawn_delay: Duration,
    mtu: usize,
    runtime_status: Arc<std::sync::Mutex<PipeRuntimeStatus>>,
}

impl PipeInterface {
    pub const DEFAULT_MTU: usize = 1_064;
    pub const DEFAULT_RESPAWN_DELAY: Duration = Duration::from_secs(5);

    pub fn new<T: Into<String>>(command: T) -> Self {
        let command = command.into();
        Self {
            command: command.clone(),
            respawn_delay: Self::DEFAULT_RESPAWN_DELAY,
            mtu: Self::DEFAULT_MTU,
            runtime_status: Arc::new(std::sync::Mutex::new(PipeRuntimeStatus::new(command))),
        }
    }

    #[must_use]
    pub fn with_respawn_delay(mut self, respawn_delay: Duration) -> Self {
        self.respawn_delay = respawn_delay;
        self
    }

    #[must_use]
    pub fn with_mtu(mut self, mtu: usize) -> Self {
        self.mtu = mtu.max(256);
        self
    }

    #[must_use]
    pub fn command(&self) -> &str {
        &self.command
    }

    #[must_use]
    pub fn runtime_status_json(&self) -> serde_json::Value {
        self.runtime_status.lock().expect("pipe runtime status mutex poisoned").to_json()
    }

    #[must_use]
    pub fn runtime_status_handle(&self) -> PipeRuntimeStatusHandle {
        PipeRuntimeStatusHandle { inner: self.runtime_status.clone() }
    }

    #[must_use]
    pub fn mtu_value(&self) -> usize {
        self.mtu
    }

    pub fn parse_command(command: &str) -> Result<Vec<String>, String> {
        let argv = shlex::split(command)
            .ok_or_else(|| "pipe.command contains unterminated shell quoting".to_string())?;
        if argv.is_empty() {
            return Err("pipe.command is required".to_string());
        }
        Ok(argv)
    }

    pub async fn spawn(context: InterfaceContext<Self>) {
        let iface_stop = context.channel.stop.clone();
        let iface_address = context.channel.address;
        let ifac_state = context.channel.ifac_state.clone();
        let ifac_violations = context.channel.ifac_violations.clone();
        let online = context.channel.online.clone();
        online.store(false, std::sync::atomic::Ordering::Release);
        let (rx_channel, tx_channel) = context.channel.split();
        let mut tx_slot = Some(tx_channel);

        loop {
            if context.cancel.is_cancelled() || iface_stop.is_cancelled() {
                break;
            }

            let (command, respawn_delay, mtu) = {
                let guard = context.inner.lock().expect("pipe interface mutex poisoned");
                (guard.command.clone(), guard.respawn_delay, guard.mtu)
            };
            let runtime_status = {
                let guard = context.inner.lock().expect("pipe interface mutex poisoned");
                guard.runtime_status.clone()
            };
            update_pipe_status(&runtime_status, |status| {
                status.command.clone_from(&command);
                status.process_state = "spawning".to_string();
                status.pipe_is_open = false;
                status.last_error = None;
            });

            if let Err(err) = run_pipe_process(
                command.as_str(),
                iface_address,
                mtu,
                context.cancel.clone(),
                iface_stop.clone(),
                rx_channel.clone(),
                &mut tx_slot,
                runtime_status.clone(),
                online.clone(),
                ifac_state.clone(),
                ifac_violations.clone(),
            )
            .await
            {
                update_pipe_status(&runtime_status, |status| {
                    status.process_state = "respawning".to_string();
                    status.pipe_is_open = false;
                    status.respawn_attempts = status.respawn_attempts.saturating_add(1);
                    status.last_error = Some(err.clone());
                });
                log::warn!("pipe_interface command failed iface={} err={}", iface_address, err);
            } else if tx_slot.is_some()
                && !context.cancel.is_cancelled()
                && !iface_stop.is_cancelled()
            {
                update_pipe_status(&runtime_status, |status| {
                    status.process_state = "respawning".to_string();
                    status.pipe_is_open = false;
                    status.respawn_attempts = status.respawn_attempts.saturating_add(1);
                });
            }

            if tx_slot.is_none() {
                log::error!(
                    "pipe interface closed iface={iface_address}: transmit task lost its receiver"
                );
                update_pipe_status(&runtime_status, |status| {
                    status.process_state = "closed".to_string();
                    status.pipe_is_open = false;
                });
                break;
            }
            if context.cancel.is_cancelled() || iface_stop.is_cancelled() {
                break;
            }

            tokio::select! {
                _ = context.cancel.cancelled() => break,
                _ = iface_stop.cancelled() => break,
                _ = tokio::time::sleep(respawn_delay) => {}
            }
        }

        let runtime_status = {
            let guard = context.inner.lock().expect("pipe interface mutex poisoned");
            guard.runtime_status.clone()
        };
        update_pipe_status(&runtime_status, |status| {
            status.process_state = "stopped".to_string();
            status.pipe_is_open = false;
        });
        online.store(false, std::sync::atomic::Ordering::Release);
        iface_stop.cancel();
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PipeRuntimeStatus {
    pub command: String,
    pub process_state: String,
    pub pipe_is_open: bool,
    pub respawn_attempts: u64,
    pub last_error: Option<String>,
}

#[derive(Clone)]
pub struct PipeRuntimeStatusHandle {
    inner: Arc<std::sync::Mutex<PipeRuntimeStatus>>,
}

impl PipeRuntimeStatusHandle {
    #[must_use]
    pub fn to_json(&self) -> serde_json::Value {
        self.inner.lock().expect("pipe runtime status mutex poisoned").to_json()
    }

    pub fn record_error_for_test(&self, state: &str, error: impl Into<String>) {
        update_pipe_status(&self.inner, |status| {
            status.process_state = state.to_string();
            status.pipe_is_open = false;
            status.respawn_attempts = status.respawn_attempts.saturating_add(1);
            status.last_error = Some(error.into());
        });
    }
}

impl PipeRuntimeStatus {
    #[must_use]
    pub fn new(command: impl Into<String>) -> Self {
        Self {
            command: command.into(),
            process_state: "configured".to_string(),
            pipe_is_open: false,
            respawn_attempts: 0,
            last_error: None,
        }
    }

    #[must_use]
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "command": self.command,
            "process_state": self.process_state,
            "pipe_is_open": self.pipe_is_open,
            "respawn_attempts": self.respawn_attempts,
            "last_error": self.last_error,
        })
    }
}

impl Interface for PipeInterface {
    fn ifac_default_size_bytes() -> usize {
        8
    }

    fn mtu() -> usize {
        Self::DEFAULT_MTU
    }

    fn configured_mtu(&self) -> usize {
        self.mtu
    }
}

#[allow(clippy::too_many_arguments)]
async fn run_pipe_process(
    command: &str,
    iface_address: AddressHash,
    mtu: usize,
    cancel: CancellationToken,
    iface_stop: CancellationToken,
    rx_channel: tokio::sync::mpsc::Sender<RxMessage>,
    tx_slot: &mut Option<InterfaceTxReceiver>,
    runtime_status: Arc<std::sync::Mutex<PipeRuntimeStatus>>,
    online: Arc<std::sync::atomic::AtomicBool>,
    ifac_state: IfacState,
    ifac_violations: Arc<std::sync::atomic::AtomicU64>,
) -> Result<(), String> {
    let argv = PipeInterface::parse_command(command)?;
    let mut child = Command::new(&argv[0])
        .args(&argv[1..])
        .kill_on_drop(true)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .map_err(|err| format!("spawn {} failed: {}", argv[0], err))?;

    let stdout = child.stdout.take().ok_or_else(|| "pipe stdout unavailable".to_string())?;
    let stdin = child.stdin.take().ok_or_else(|| "pipe stdin unavailable".to_string())?;
    let tx_channel =
        tx_slot.take().ok_or_else(|| "pipe transmit receiver unavailable".to_string())?;
    log::info!("pipe_interface spawned iface={} command={}", iface_address, command);
    update_pipe_status(&runtime_status, |status| {
        status.process_state = "running".to_string();
        status.pipe_is_open = true;
        status.last_error = None;
    });
    online.store(true, std::sync::atomic::Ordering::Release);

    *tx_slot = run_pipe_stream(
        stdout,
        stdin,
        iface_address,
        mtu,
        cancel,
        iface_stop,
        rx_channel,
        tx_channel,
        runtime_status,
        ifac_state,
        ifac_violations,
    )
    .await;
    online.store(false, std::sync::atomic::Ordering::Release);

    terminate_pipe_child(&mut child, command).await?;
    Ok(())
}

pub fn spawn_pipe(
    mgr: &mut InterfaceManager,
    command: String,
    respawn_delay: Duration,
    mtu: usize,
) -> AddressHash {
    let iface = PipeInterface::new(command).with_respawn_delay(respawn_delay).with_mtu(mtu);
    mgr.spawn_as(iface, PipeInterface::spawn, IfaceRole::Unicast)
}

#[cfg(test)]
#[path = "pipe_tests.rs"]
mod tests;
