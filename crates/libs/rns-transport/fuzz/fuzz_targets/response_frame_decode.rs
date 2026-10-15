#![no_main]

use libfuzzer_sys::fuzz_target;
use rns_transport::destination::request::ResponseFrame;

fuzz_target!(|data: &[u8]| {
    if let Ok(frame) = ResponseFrame::decode(data) {
        let encoded = frame.encode();
        let again = ResponseFrame::decode(&encoded).expect("re-encoded frame decodes");
        assert_eq!(again.encode(), encoded);
    }
});
