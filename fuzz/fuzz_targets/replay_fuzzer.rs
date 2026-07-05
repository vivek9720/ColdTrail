#![no_main]

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    let _ = coldtrail::replay(data);
    if data.len() > 4 {
        let _ = coldtrail::parse(&data[1..]);
    }
});
