use anyhow::{Context, Result};
use audiopus::coder::Encoder;
use audiopus::{Application, Bitrate, Channels, SampleRate};
use ogg::writing::{PacketWriteEndInfo, PacketWriter};
use ringbuf::traits::{Consumer, Observer};
use ringbuf::HeapCons;
use std::fs::File;
use std::io::BufWriter;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const FRAME_SIZE: usize = 960; // 20ms per channel at 48kHz
const SAMPLE_RATE: u32 = 48_000;
const STEREO_BITRATE_BPS: i32 = 64_000;
const ENCODE_BUF_SIZE: usize = 4000;

/// If we've been waiting this long for one stream to catch up, pad it
/// with silence so the encoder keeps producing frames at wall-clock rate.
const STALL_TIMEOUT: Duration = Duration::from_millis(500);

/// How often to log drift / drop metrics.
const METRICS_INTERVAL: Duration = Duration::from_secs(30);

/// Maximum drift between mic and system buffer occupancy that we
/// consider acceptable before logging a warning. ~100ms at 48kHz.
const DRIFT_WARN_SAMPLES: i64 = 4_800;

fn build_opus_head(channels: u8, pre_skip: u16, input_sample_rate: u32) -> Vec<u8> {
    let mut head = Vec::with_capacity(19);
    head.extend_from_slice(b"OpusHead");
    head.push(1); // version
    head.push(channels);
    head.extend_from_slice(&pre_skip.to_le_bytes());
    head.extend_from_slice(&input_sample_rate.to_le_bytes());
    head.extend_from_slice(&0u16.to_le_bytes()); // output gain
    head.push(0); // channel mapping family
    head
}

fn build_opus_tags() -> Vec<u8> {
    let vendor = b"domino-codex-recorder";
    let mut tags = Vec::with_capacity(8 + 4 + vendor.len() + 4);
    tags.extend_from_slice(b"OpusTags");
    tags.extend_from_slice(&(vendor.len() as u32).to_le_bytes());
    tags.extend_from_slice(vendor);
    tags.extend_from_slice(&0u32.to_le_bytes()); // no comments
    tags
}

/// Interleave a mono left channel and mono right channel into a stereo
/// buffer with the layout `[L0, R0, L1, R1, ...]`.
fn interleave_stereo(left: &[f32], right: &[f32], out: &mut [f32]) {
    debug_assert_eq!(left.len(), right.len());
    debug_assert_eq!(out.len(), left.len() * 2);
    for i in 0..left.len() {
        out[i * 2] = left[i];
        out[i * 2 + 1] = right[i];
    }
}

pub fn spawn_encoder(
    mic_consumer: HeapCons<f32>,
    system_consumer: Option<HeapCons<f32>>,
    output_path: PathBuf,
    shutdown: Arc<AtomicBool>,
    mic_dropped: Arc<AtomicU64>,
    system_dropped: Arc<AtomicU64>,
) -> Result<JoinHandle<Result<()>>> {
    let handle = thread::Builder::new()
        .name("opus-encoder".into())
        .spawn(move || {
            encoder_loop(
                mic_consumer,
                system_consumer,
                output_path,
                shutdown,
                mic_dropped,
                system_dropped,
            )
        })?;
    Ok(handle)
}

fn encoder_loop(
    mut mic_consumer: HeapCons<f32>,
    mut system_consumer: Option<HeapCons<f32>>,
    output_path: PathBuf,
    shutdown: Arc<AtomicBool>,
    mic_dropped: Arc<AtomicU64>,
    system_dropped: Arc<AtomicU64>,
) -> Result<()> {
    let mut encoder = Encoder::new(SampleRate::Hz48000, Channels::Stereo, Application::Voip)
        .context("failed to create Opus encoder")?;
    encoder
        .set_bitrate(Bitrate::BitsPerSecond(STEREO_BITRATE_BPS))
        .context("failed to set bitrate")?;

    let pre_skip = encoder.lookahead().unwrap_or(312) as u16;

    let file = File::create(&output_path)
        .with_context(|| format!("failed to create output file: {}", output_path.display()))?;
    let mut ogg = PacketWriter::new(BufWriter::new(file));
    let serial = std::process::id();

    ogg.write_packet(
        build_opus_head(2, pre_skip, SAMPLE_RATE),
        serial,
        PacketWriteEndInfo::EndPage,
        0,
    )
    .context("failed to write OpusHead")?;

    ogg.write_packet(build_opus_tags(), serial, PacketWriteEndInfo::EndPage, 0)
        .context("failed to write OpusTags")?;

    let mut mic_buf = vec![0.0f32; FRAME_SIZE];
    let mut sys_buf = vec![0.0f32; FRAME_SIZE];
    let mut stereo_buf = vec![0.0f32; FRAME_SIZE * 2];
    let mut encode_buf = vec![0u8; ENCODE_BUF_SIZE];

    let mut mic_pos = 0usize;
    let mut sys_pos = 0usize;
    let mut granule_pos: u64 = pre_skip as u64;
    let mut last_progress = Instant::now();
    let mut last_metrics = Instant::now();

    tracing::info!(
        pre_skip,
        system_audio = system_consumer.is_some(),
        "stereo encoder started"
    );

    loop {
        let shutting_down = shutdown.load(Ordering::Relaxed);

        // Drain ring buffers into the per-channel frame buffers.
        let mic_before = mic_pos;
        if mic_pos < FRAME_SIZE {
            mic_pos += mic_consumer.pop_slice(&mut mic_buf[mic_pos..FRAME_SIZE]);
        }
        let sys_before = sys_pos;
        match system_consumer.as_mut() {
            Some(c) => {
                if sys_pos < FRAME_SIZE {
                    sys_pos += c.pop_slice(&mut sys_buf[sys_pos..FRAME_SIZE]);
                }
            }
            None => {
                // No system stream — right channel is permanently silent.
                if sys_pos < FRAME_SIZE {
                    for s in sys_buf[sys_pos..].iter_mut() {
                        *s = 0.0;
                    }
                    sys_pos = FRAME_SIZE;
                }
            }
        }
        if mic_pos > mic_before || sys_pos > sys_before {
            last_progress = Instant::now();
        }

        if last_metrics.elapsed() > METRICS_INTERVAL {
            log_metrics(
                &mic_consumer,
                system_consumer.as_ref(),
                &mic_dropped,
                &system_dropped,
            );
            last_metrics = Instant::now();
        }

        let both_ready = mic_pos == FRAME_SIZE && sys_pos == FRAME_SIZE;
        let stalled = last_progress.elapsed() > STALL_TIMEOUT;
        let has_partial = mic_pos > 0 || sys_pos > 0;

        if both_ready || (shutting_down && has_partial) || (stalled && has_partial) {
            if !both_ready {
                let inserted_mic = FRAME_SIZE - mic_pos;
                let inserted_sys = FRAME_SIZE - sys_pos;
                if stalled && !shutting_down {
                    tracing::warn!(
                        inserted_mic,
                        inserted_sys,
                        "stream stalled — inserting silence to maintain sync"
                    );
                }
                for s in mic_buf[mic_pos..].iter_mut() {
                    *s = 0.0;
                }
                for s in sys_buf[sys_pos..].iter_mut() {
                    *s = 0.0;
                }
            }

            interleave_stereo(&mic_buf, &sys_buf, &mut stereo_buf);

            let len = encoder
                .encode_float(&stereo_buf, &mut encode_buf)
                .context("Opus encode failed")?;

            granule_pos += FRAME_SIZE as u64;

            let drained = mic_consumer.occupied_len() == 0
                && system_consumer
                    .as_ref()
                    .map_or(true, |c| c.occupied_len() == 0);
            let end_info = if shutting_down && drained {
                PacketWriteEndInfo::EndStream
            } else {
                PacketWriteEndInfo::EndPage
            };

            ogg.write_packet(encode_buf[..len].to_vec(), serial, end_info, granule_pos)
                .context("failed to write Ogg packet")?;

            mic_pos = 0;
            sys_pos = 0;
            last_progress = Instant::now();

            if matches!(end_info, PacketWriteEndInfo::EndStream) {
                break;
            }
        } else if shutting_down {
            // Nothing left to flush.
            break;
        } else {
            thread::sleep(Duration::from_millis(5));
        }
    }

    log_metrics(
        &mic_consumer,
        system_consumer.as_ref(),
        &mic_dropped,
        &system_dropped,
    );

    tracing::info!(
        granule_pos,
        path = %output_path.display(),
        "encoder finished"
    );

    Ok(())
}

fn log_metrics(
    mic: &HeapCons<f32>,
    system: Option<&HeapCons<f32>>,
    mic_dropped: &AtomicU64,
    system_dropped: &AtomicU64,
) {
    let mic_level = mic.occupied_len();
    let sys_level = system.map(|c| c.occupied_len()).unwrap_or(0);
    let drift = mic_level as i64 - sys_level as i64;
    let drift_ms = drift * 1000 / SAMPLE_RATE as i64;

    if drift.abs() > DRIFT_WARN_SAMPLES {
        tracing::warn!(drift_ms, mic_level, sys_level, "stream drift exceeds 100ms");
    } else {
        tracing::debug!(drift_ms, mic_level, sys_level, "stream drift");
    }

    let md = mic_dropped.swap(0, Ordering::Relaxed);
    let sd = system_dropped.swap(0, Ordering::Relaxed);
    if md > 0 || sd > 0 {
        tracing::warn!(
            mic_dropped = md,
            system_dropped = sd,
            "samples dropped (ring buffer full)"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ringbuf::traits::{Producer, Split};
    use ringbuf::HeapRb;
    use std::sync::atomic::AtomicBool;

    #[test]
    fn test_opus_head_format() {
        let head = build_opus_head(2, 312, 48000);
        assert_eq!(&head[..8], b"OpusHead");
        assert_eq!(head[8], 1); // version
        assert_eq!(head[9], 2); // channels
        assert_eq!(u16::from_le_bytes([head[10], head[11]]), 312);
        assert_eq!(
            u32::from_le_bytes([head[12], head[13], head[14], head[15]]),
            48000
        );
        assert_eq!(head.len(), 19);
    }

    #[test]
    fn test_opus_tags_format() {
        let tags = build_opus_tags();
        assert_eq!(&tags[..8], b"OpusTags");
        let vendor_len = u32::from_le_bytes([tags[8], tags[9], tags[10], tags[11]]) as usize;
        let vendor = &tags[12..12 + vendor_len];
        assert_eq!(vendor, b"domino-codex-recorder");
    }

    #[test]
    fn test_interleave_stereo() {
        let left = [1.0, 2.0, 3.0];
        let right = [-1.0, -2.0, -3.0];
        let mut out = [0.0; 6];
        interleave_stereo(&left, &right, &mut out);
        assert_eq!(out, [1.0, -1.0, 2.0, -2.0, 3.0, -3.0]);
    }

    fn run_encoder_with(
        mic_samples: &[f32],
        sys_samples: Option<&[f32]>,
        path: PathBuf,
    ) -> Result<()> {
        let mic_rb = HeapRb::<f32>::new((mic_samples.len() + FRAME_SIZE).max(FRAME_SIZE * 2));
        let (mut mic_prod, mic_cons) = mic_rb.split();
        mic_prod.push_slice(mic_samples);

        let sys_pair = sys_samples.map(|s| {
            let rb = HeapRb::<f32>::new((s.len() + FRAME_SIZE).max(FRAME_SIZE * 2));
            let (mut prod, cons) = rb.split();
            prod.push_slice(s);
            (prod, cons)
        });
        let sys_cons = sys_pair.map(|(_p, c)| c);

        let shutdown = Arc::new(AtomicBool::new(true));
        let mic_dropped = Arc::new(AtomicU64::new(0));
        let sys_dropped = Arc::new(AtomicU64::new(0));

        let handle = spawn_encoder(mic_cons, sys_cons, path, shutdown, mic_dropped, sys_dropped)?;
        handle.join().expect("encoder thread panicked")
    }

    #[test]
    fn test_stereo_single_frame_produces_valid_ogg() {
        let tmp = std::env::temp_dir().join("domino-test-encoder-stereo");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        let path = tmp.join("test.opus");

        let mic = vec![0.1f32; FRAME_SIZE];
        let sys = vec![-0.1f32; FRAME_SIZE];
        run_encoder_with(&mic, Some(&sys), path.clone()).unwrap();

        assert!(path.exists());
        let bytes = std::fs::read(&path).unwrap();
        assert!(!bytes.is_empty());
        // Ogg magic
        assert_eq!(&bytes[..4], b"OggS");
        // OpusHead should appear in early bytes
        assert!(bytes.windows(8).any(|w| w == b"OpusHead"));

        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn test_stereo_partial_frame_zero_padded() {
        let tmp = std::env::temp_dir().join("domino-test-encoder-stereo-partial");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        let path = tmp.join("test.opus");

        let mic = vec![0.05f32; 500];
        let sys = vec![0.05f32; 500];
        run_encoder_with(&mic, Some(&sys), path.clone()).unwrap();

        assert!(path.exists());
        assert!(std::fs::metadata(&path).unwrap().len() > 0);

        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn test_mono_fallback_when_system_disabled() {
        let tmp = std::env::temp_dir().join("domino-test-encoder-mic-only");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        let path = tmp.join("test.opus");

        let mic = vec![0.1f32; FRAME_SIZE];
        run_encoder_with(&mic, None, path.clone()).unwrap();

        assert!(path.exists());
        let bytes = std::fs::read(&path).unwrap();
        // Even without system audio, output is still 2-channel Opus
        // (right channel filled with silence).
        assert_eq!(&bytes[..4], b"OggS");

        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn test_drift_compensation_pads_lagging_channel() {
        let tmp = std::env::temp_dir().join("domino-test-encoder-drift");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        let path = tmp.join("test.opus");

        // Mic has a full frame, system has only a partial one. Stall
        // timeout should kick in and we should still produce valid output
        // padded with silence on the system channel.
        let mic = vec![0.1f32; FRAME_SIZE * 2];
        let sys = vec![0.1f32; 200];
        run_encoder_with(&mic, Some(&sys), path.clone()).unwrap();

        assert!(path.exists());
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(&bytes[..4], b"OggS");

        std::fs::remove_dir_all(&tmp).ok();
    }
}
