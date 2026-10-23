//! Panics reach the log file, with where they happened and a backtrace: once
//! the Windows tray has detached from its console, standard error leads
//! nowhere. A binary of its own: logging is set up once per process.

#[test]
fn panics_are_logged_with_their_place_and_a_backtrace() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("logs").join("pley.log");
    pley::logging::init("info", Some(&log));

    let worker = std::thread::Builder::new().name("worker".into()).spawn(|| panic!("boom number {}", 42)).unwrap();
    assert!(worker.join().is_err());

    let text = std::fs::read_to_string(&log).unwrap();
    assert!(text.contains("ERROR thread 'worker' panicked at "), "{text}");
    assert!(text.contains("logging.rs:"), "where it happened: {text}");
    assert!(text.contains(": boom number 42\nbacktrace:\n"), "{text}");
}
