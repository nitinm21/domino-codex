use std::process::Command;
use std::sync::{Arc, Barrier};
use std::thread;

#[test]
fn two_concurrent_starts_exactly_one_succeeds() {
    let binary = env!("CARGO_BIN_EXE_domino-codex-recorder");
    let tmp = std::env::temp_dir().join("domino-race-test");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();

    let domino_dir = dirs::home_dir().unwrap().join(".domino");
    let pid_path = domino_dir.join("current.pid");
    let lock_path = domino_dir.join("session.lock");
    let _ = std::fs::remove_file(&pid_path);
    let _ = std::fs::remove_file(&lock_path);
    let _ = Command::new(binary).arg("stop").output();

    let barrier = Arc::new(Barrier::new(2));

    let handles: Vec<_> = (0..2)
        .map(|_| {
            let barrier = Arc::clone(&barrier);
            let bin = binary.to_string();
            let dir = tmp.clone();
            thread::spawn(move || {
                barrier.wait();
                // start now forks and the parent exits, so output() returns quickly.
                let output = Command::new(&bin)
                    .args(["start", "--out-dir"])
                    .arg(&dir)
                    .output()
                    .expect("failed to spawn");
                output.status.success()
            })
        })
        .collect();

    let results: Vec<bool> = handles.into_iter().map(|h| h.join().unwrap()).collect();

    // Wait for the daemon to settle and write its PID file.
    std::thread::sleep(std::time::Duration::from_secs(1));

    let successes = results.iter().filter(|&&r| r).count();
    let failures = results.iter().filter(|&&r| !r).count();

    // Verify exactly one daemon is running via the PID file.
    let daemon_alive = if pid_path.exists() {
        let content = std::fs::read_to_string(&pid_path).unwrap();
        let info: serde_json::Value = serde_json::from_str(&content).unwrap();
        let pid = info["pid"].as_u64().unwrap() as i32;
        unsafe { libc::kill(pid, 0) == 0 }
    } else {
        false
    };

    // Clean up: stop the running daemon.
    let _ = Command::new(binary).arg("stop").output();
    std::thread::sleep(std::time::Duration::from_millis(500));

    assert_eq!(
        successes, 1,
        "expected exactly 1 successful start, got {successes}"
    );
    assert_eq!(
        failures, 1,
        "expected exactly 1 failed start, got {failures}"
    );
    assert!(daemon_alive, "expected the daemon process to be alive");

    let _ = std::fs::remove_dir_all(&tmp);
}
