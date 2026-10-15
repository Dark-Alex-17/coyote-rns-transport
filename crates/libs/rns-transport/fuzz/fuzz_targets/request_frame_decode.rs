#![no_main]

use libfuzzer_sys::fuzz_target;
use rns_transport::destination::request::{FrameDecodeError, RequestFrame};

fuzz_target!(|data: &[u8]| {
    match RequestFrame::decode(data) {
        Ok(frame) => {
            let encoded = frame.encode();
            let again = RequestFrame::decode(&encoded).expect("re-encoded frame decodes");
            assert_eq!(again.encode(), encoded);
        }
        Err(FrameDecodeError::Malformed) => {
            panic!("the walk accepted an input the value reader refused")
        }
        Err(_) => {}
    }
});
