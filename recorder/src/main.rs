mod audio;
mod cli;
mod session;
mod signals;
mod transcription;

use anyhow::{bail, Result};
use cli::{Cli, Command};
use ringbuf::traits::Split;
use ringbuf::HeapRb;
use std::os::unix::io::AsRawFd;
use std::path::Path;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;

const RING_BUFFER_SAMPLES: usize = 96_000; // 2 seconds at 48kHz

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("domino_recorder=info".parse()?),
        )
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse_with_runtime_bin_name();

    match cli.command {
        Command::Start { out_dir } => cmd_start(out_dir.as_deref()),
        Command::Stop => cmd_stop(),
        Command::Status => cmd_status(),
        Command::Doctor => cmd_doctor(),
    }
}

fn cmd_start(out_dir: Option<&Path>) -> Result<()> {
    let (session_dir, started_at) = session::prepare_session(out_dir)?;
    let opus_path = session_dir.join("meeting.opus");
    let log_path = session_dir.join("recorder.log");

    match unsafe { libc::fork() } {
        -1 => bail!("fork failed: {}", std::io::Error::last_os_error()),
        0 => {
            // === Child (daemon) ===
            unsafe {
                libc::setsid();
            }

            // Redirect stdout/stderr to log file, close stdin
            let log_file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&log_path)?;
            let log_fd = log_file.as_raw_fd();
            unsafe {
                libc::dup2(log_fd, libc::STDOUT_FILENO);
                libc::dup2(log_fd, libc::STDERR_FILENO);
                libc::close(libc::STDIN_FILENO);
            }
            drop(log_file);

            // Write PID file with daemon's actual PID
            session::write_pid_file(std::process::id(), &session_dir, &started_at)?;

            tracing::info!(
                session_dir = %session_dir.display(),
                pid = std::process::id(),
                "daemon started"
            );

            let shutdown = signals::shutdown_flag()?;

            // Mic capture
            let mic_rb = HeapRb::<f32>::new(RING_BUFFER_SAMPLES);
            let (mic_prod, mic_cons) = mic_rb.split();
            let mic = audio::mic::start_mic_capture(mic_prod)?;

            // System capture (graceful degradation if it fails)
            let sys_rb = HeapRb::<f32>::new(RING_BUFFER_SAMPLES);
            let (sys_prod, sys_cons) = sys_rb.split();
            let (system, system_cons, system_dropped) = match start_system(sys_prod) {
                Ok(cap) => {
                    let dropped = cap.dropped_samples.clone();
                    (Some(cap), Some(sys_cons), dropped)
                }
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        "system audio capture unavailable — recording mic only (right channel will be silent)"
                    );
                    (None, None, Arc::new(AtomicU64::new(0)))
                }
            };

            let encoder_handle = audio::encoder::spawn_encoder(
                mic_cons,
                system_cons,
                opus_path,
                shutdown.clone(),
                mic.dropped_samples.clone(),
                system_dropped,
            )?;

            // Wait for shutdown signal
            while !signals::is_shutdown(&shutdown) {
                std::thread::sleep(std::time::Duration::from_millis(100));
            }

            tracing::info!("shutdown signal received, stopping capture");
            drop(mic.stream);
            if let Some(sys) = system {
                sys.stop();
            }

            // Wait for encoder to flush and finalize the file
            match encoder_handle.join() {
                Ok(Ok(())) => tracing::info!("encoder finished cleanly"),
                Ok(Err(e)) => tracing::error!("encoder error: {e:#}"),
                Err(_) => tracing::error!("encoder thread panicked"),
            }

            session::remove_pid_file()?;
            tracing::info!("daemon exiting");

            std::process::exit(0);
        }
        child_pid => {
            // === Parent ===
            // Give daemon a moment to initialize and write PID file
            std::thread::sleep(std::time::Duration::from_millis(300));

            let info = session::write_pid_file(child_pid as u32, &session_dir, &started_at)?;
            let json = serde_json::to_string(&info)?;
            println!("{json}");

            std::process::exit(0);
        }
    }
}

fn cmd_stop() -> Result<()> {
    let info = session::stop_session()?;

    let opus_path = info.session_dir.join("meeting.opus");
    if !opus_path.exists() {
        println!(
            "Session stopped: {} (no audio file produced)",
            info.session_dir.display()
        );
        return Ok(());
    }

    let size_mb = std::fs::metadata(&opus_path)?.len() as f64 / (1024.0 * 1024.0);

    match transcription::run_on_session(&info.session_dir) {
        Ok(outcome) => {
            println!("Saved:");
            println!("  {} ({:.1} MB)", opus_path.display(), size_mb);
            println!(
                "  {} ({} segments, {:.0}s audio, {:.0}s wall, {})",
                outcome.transcript_path.display(),
                outcome.segment_count,
                outcome.duration_sec,
                outcome.wall_sec,
                outcome.accelerator,
            );
            Ok(())
        }
        Err(error) => {
            eprintln!("Transcription failed: {error:#}");
            eprintln!("Audio is preserved at: {}", opus_path.display());
            eprintln!(
                "Logs: {}",
                info.session_dir.join("transcription.log").display()
            );
            std::process::exit(2);
        }
    }
}

fn cmd_status() -> Result<()> {
    match session::read_active_session()? {
        Some(info) => {
            let json = serde_json::to_string(&info)?;
            println!("{json}");
        }
        None => {
            println!("{{}}");
        }
    }
    Ok(())
}

fn cmd_doctor() -> Result<()> {
    println!("Domino Recorder — Health Check");
    println!("  (doctor checks will be implemented in Phase 4)");
    Ok(())
}

#[cfg(target_os = "macos")]
fn start_system(producer: ringbuf::HeapProd<f32>) -> Result<audio::system::SystemCapture> {
    audio::system::start_system_capture(producer)
}

#[cfg(not(target_os = "macos"))]
fn start_system(_producer: ringbuf::HeapProd<f32>) -> Result<NoSystemCapture> {
    bail!("system audio capture is only supported on macOS")
}

#[cfg(not(target_os = "macos"))]
struct NoSystemCapture {
    pub dropped_samples: Arc<AtomicU64>,
}

#[cfg(not(target_os = "macos"))]
impl NoSystemCapture {
    fn stop(self) {}
}
