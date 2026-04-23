mod decode;
mod dedup;
mod merge;
pub mod model;
mod output;
mod progress;
mod resample;
mod whisper;

use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::time::Instant;

#[derive(Debug)]
pub struct RunOutcome {
    pub transcript_path: PathBuf,
    pub segment_count: usize,
    pub duration_sec: f64,
    pub wall_sec: f64,
    pub accelerator: &'static str,
}

/// Run the full offline transcription pipeline on a finalized session dir.
pub fn run_on_session(session_dir: &Path) -> Result<RunOutcome> {
    let _log_guard = progress::init_log_file(&session_dir.join("transcription.log"))?;

    let opus_path = session_dir.join("meeting.opus");
    if !opus_path.exists() {
        bail!("meeting.opus not found at {}", opus_path.display());
    }

    tracing::info!(session_dir = %session_dir.display(), "starting transcription run");
    println!("Preparing offline transcription...");
    let total_start = Instant::now();

    println!("Checking transcription model...");
    let model_path =
        model::ensure_model_available().context("could not prepare ggml-small.en model")?;

    println!("Decoding audio...");
    let decode_start = Instant::now();
    let (left_48k, right_48k, duration_sec) = decode::decode_stereo_opus(&opus_path)?;
    tracing::info!(
        decode_ms = decode_start.elapsed().as_millis() as u64,
        samples_per_channel = left_48k.len(),
        duration_sec,
        "decoded meeting.opus"
    );

    println!("Resampling channels to 16 kHz...");
    let resample_start = Instant::now();
    let left_16k = resample::resample_mono(&left_48k, 48_000, 16_000)?;
    let right_16k = resample::resample_mono(&right_48k, 48_000, 16_000)?;
    drop((left_48k, right_48k));
    tracing::info!(
        resample_ms = resample_start.elapsed().as_millis() as u64,
        left_samples = left_16k.len(),
        right_samples = right_16k.len(),
        "resampled channels"
    );

    let transcriber = whisper::Transcriber::load(&model_path)?;
    let accelerator = transcriber.accelerator;
    tracing::info!(accelerator, "whisper context ready");

    let total_channel_ms = (duration_sec * 1000.0).round() as u64;
    let progress = progress::overall_bar(duration_sec);
    progress.set_message(format!("transcribing mic channel ({accelerator})"));
    progress.set_position(0);

    let whisper_start = Instant::now();
    let you_segments = transcriber.transcribe(&left_16k, whisper::Speaker::You)?;
    progress.set_position(total_channel_ms);

    progress.set_message(format!("transcribing meeting channel ({accelerator})"));
    let meeting_segments = transcriber.transcribe(&right_16k, whisper::Speaker::Meeting)?;
    progress.set_position(total_channel_ms.saturating_mul(2));
    progress.finish_with_message("transcription complete");
    tracing::info!(
        whisper_ms = whisper_start.elapsed().as_millis() as u64,
        you_segments = you_segments.len(),
        meeting_segments = meeting_segments.len(),
        "transcription finished"
    );

    let (you_segments, dedup_stats) = if dedup::is_enabled() {
        dedup::dedup_mic_bleed(you_segments, &meeting_segments)
    } else {
        let count = you_segments.len();
        (you_segments, dedup::DedupStats::noop(count))
    };
    if dedup_stats.input_count > 0 && (dedup_stats.dropped_count * 2) > dedup_stats.input_count {
        tracing::warn!(
            you_in = dedup_stats.input_count,
            you_dropped = dedup_stats.dropped_count,
            "mic-bleed dedup dropped more than half of You segments — inspect \
             transcript.json, or run with DOMINO_DEDUP=off to bypass"
        );
    } else {
        tracing::info!(
            you_in = dedup_stats.input_count,
            you_dropped = dedup_stats.dropped_count,
            meeting_segments = meeting_segments.len(),
            "mic-bleed dedup complete"
        );
    }

    let segments = merge::merge_segments(you_segments, meeting_segments);
    let segment_count = segments.len();
    let wall_sec = total_start.elapsed().as_secs_f64();
    let transcript_path = session_dir.join("transcript.json");
    output::write_transcript_json(
        &transcript_path,
        "meeting.opus",
        duration_sec,
        model::MODEL_SHA256_HEX,
        wall_sec,
        accelerator,
        &segments,
    )?;
    tracing::info!(
        transcript_path = %transcript_path.display(),
        segment_count,
        wall_sec,
        "wrote transcript output"
    );

    Ok(RunOutcome {
        transcript_path,
        segment_count,
        duration_sec,
        wall_sec,
        accelerator,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn run_on_session_rejects_missing_audio_file() {
        let session_dir = std::env::temp_dir().join(format!(
            "domino-test-run-on-session-missing-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&session_dir);
        fs::create_dir_all(&session_dir).unwrap();

        let err = run_on_session(&session_dir).unwrap_err();
        let message = format!("{err:#}");
        assert!(message.contains("meeting.opus not found"), "{message}");
        assert!(session_dir.join("transcription.log").exists());

        fs::remove_dir_all(&session_dir).ok();
    }
}
