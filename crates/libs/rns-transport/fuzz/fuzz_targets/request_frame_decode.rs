#![no_main]

use libfuzzer_sys::fuzz_target;
use rns_transport::destination::request::RequestFrame;

fuzz_target!(|data: &[u8]| {
    if let Ok(frame) = RequestFrame::decode(data) {
        let encoded = frame.encode();
        let again = RequestFrame::decode(&encoded).expect("re-encoded frame decodes");
        assert_eq!(again.encode(), encoded);
    }
});
