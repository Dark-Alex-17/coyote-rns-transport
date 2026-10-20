#![no_main]

use libfuzzer_sys::fuzz_target;
use rns_transport::resource::ResourceAdvertisement;

fuzz_target!(|data: &[u8]| {
    if let Ok(advertisement) = ResourceAdvertisement::unpack(data) {
        let packed = advertisement.pack().expect("decoded advertisement packs");
        let again = ResourceAdvertisement::unpack(&packed).expect("re-packed advertisement unpacks");
        assert_eq!(again, advertisement);
    }
});
