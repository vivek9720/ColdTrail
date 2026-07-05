#![no_main]

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    if let Ok(bundle) = coldtrail::parse_evidence_bundle(data) {
        let report = coldtrail::validate_evidence(&bundle);
        let _ = coldtrail::report::render_text_report(&report);
    }
    let _ = coldtrail::parse_logger_csv(data);
    let _ = coldtrail::parse_edi214(data);
});
