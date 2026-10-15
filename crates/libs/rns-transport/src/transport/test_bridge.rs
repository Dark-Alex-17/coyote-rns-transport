use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::storage::messages::MessageRecord;

pub trait InboundTestBridge: Send + Sync {
    fn accept_inbound_for_test(&self, record: MessageRecord) -> std::io::Result<()>;
}

thread_local! {
    static BRIDGE: RefCell<HashMap<String, Rc<dyn InboundTestBridge>>> =
        RefCell::new(HashMap::new());
}

pub fn reset() {
    BRIDGE.with(|bridge| bridge.borrow_mut().clear());
}

pub fn register(identity: impl Into<String>, daemon: Rc<dyn InboundTestBridge>) {
    BRIDGE.with(|bridge| {
        bridge.borrow_mut().insert(identity.into(), daemon);
    });
}

pub fn deliver_outbound(record: &MessageRecord) -> bool {
    let daemon = BRIDGE.with(|bridge| bridge.borrow().get(&record.destination).cloned());
    let Some(daemon) = daemon else {
        return false;
    };

    let inbound = MessageRecord {
        id: record.id.clone(),
        source: record.source.clone(),
        destination: record.destination.clone(),
        title: record.title.clone(),
        content: record.content.clone(),
        timestamp: record.timestamp,
        direction: "in".into(),
        fields: record.fields.clone(),
        receipt_status: None,
    };
    daemon.accept_inbound_for_test(inbound).is_ok()
}
