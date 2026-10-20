#![no_main]

use libfuzzer_sys::fuzz_target;
use rns_transport::destination::request::{FrameDecodeError, ResponseFrame};

fuzz_target!(|data: &[u8]| {
    match ResponseFrame::decode(data) {
        Ok(frame) => {
            let encoded = frame.encode();
            let again = ResponseFrame::decode(&encoded).expect("re-encoded frame decodes");
            assert_eq!(again.encode(), encoded);
        }
        Err(FrameDecodeError::Malformed) => {
            panic!("the walk accepted an input the value reader refused")
        }
        Err(_) => {}
    }
});
