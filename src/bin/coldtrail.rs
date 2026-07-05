use std::env;
use std::fs;
use std::process;

fn main() {
    let mut args = env::args().skip(1);
    let Some(path) = args.next() else {
        eprintln!("usage: coldtrail <evidence-bundle>");
        process::exit(2);
    };
    let data = match fs::read(&path) {
        Ok(data) => data,
        Err(err) => {
            eprintln!("{path}: {err}");
            process::exit(2);
        }
    };
    let bundle = match coldtrail::parse_evidence_bundle(&data) {
        Ok(bundle) => bundle,
        Err(err) => {
            eprintln!("{path}: parse failed: {err}");
            process::exit(1);
        }
    };
    let report = coldtrail::validate_evidence(&bundle);
    print!("{}", coldtrail::report::render_text_report(&report));
    if !report.is_acceptable() {
        process::exit(1);
    }
}
