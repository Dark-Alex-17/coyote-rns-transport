//! Captures the announce limiter's `tp(<name>): holding announce for <dst> before queue for
//! <delay>` records and reads them back by transport name; shared by the integration tests
//! that pin which interfaces hold an announce. `log::set_logger` is process-wide, so the two
//! functions share one logger per test binary and the records are attributed to a test by
//! the transport name in the message.

use std::sync::Once;

struct HoldLogger {
    records: std::sync::Mutex<Vec<String>>,
}

static HOLDS: HoldLogger = HoldLogger { records: std::sync::Mutex::new(Vec::new()) };

impl log::Log for HoldLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.target().starts_with("rns_transport::transport")
    }

    fn log(&self, record: &log::Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let message = record.args().to_string();
        if message.contains("holding announce") {
            self.records.lock().expect("hold record store").push(message);
        }
    }

    fn flush(&self) {}
}

/// Installs the hold logger once per process at `Debug`; later calls are no-ops.
pub fn install_hold_logger() {
    static INSTALL: Once = Once::new();
    INSTALL.call_once(|| {
        log::set_logger(&HOLDS).expect("no other logger in this test binary");
        log::set_max_level(log::LevelFilter::Debug);
    });
}

/// The hold records logged by the transports named in `names`.
pub fn holds_by(names: &[&str]) -> Vec<String> {
    HOLDS
        .records
        .lock()
        .expect("hold record store")
        .iter()
        .filter(|message| names.iter().any(|name| message.contains(&format!("tp({name})"))))
        .cloned()
        .collect()
}
