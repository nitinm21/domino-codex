//! Drop mic-channel segments that are acoustic bleed from the system-audio
//! channel (e.g. laptop speakers playing YouTube audio that the built-in
//! microphone then picks up).
//!
//! The system-audio channel is canonical — we never touch it. We only filter
//! `You` segments whose text is largely covered by the concatenated text of
//! all `Meeting` segments in an overlapping time window.
//!
//! The dedup is gated by `DOMINO_DEDUP` — set to `off`/`0`/`false`/`no` to
//! skip and preserve every `You` segment verbatim (useful for debugging or
//! when a user is legitimately echoing the remote side).

use super::whisper::Segment;
use std::collections::HashSet;

/// How far BEFORE a Meeting segment a You segment can start and still be
/// considered potential bleed. Small — clock skew only.
const WINDOW_LEAD_SEC: f64 = 0.5;

/// How far AFTER a Meeting segment a You segment can start and still be
/// considered potential bleed. Wide — the acoustic path plus encoder buffer
/// plus whisper segmentation typically delays the mic-side echo by 1–2 s.
const WINDOW_LAG_SEC: f64 = 3.0;

/// Fraction of You tokens that must appear somewhere in the overlapping
/// Meeting segments to count as bleed (for You segments with ≥ 2 tokens).
const COVERAGE_THRESHOLD: f32 = 0.8;

/// Below this token count we require an exact (full-coverage) match — lowers
/// false positives for short utterances like "okay" or "yeah".
const MIN_TOKENS_FOR_PARTIAL: usize = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DedupStats {
    pub input_count: usize,
    pub dropped_count: usize,
}

impl DedupStats {
    pub fn noop(input_count: usize) -> Self {
        Self {
            input_count,
            dropped_count: 0,
        }
    }
}

pub fn is_enabled() -> bool {
    match std::env::var("DOMINO_DEDUP") {
        Ok(v) => !matches!(
            v.trim().to_ascii_lowercase().as_str(),
            "0" | "false" | "no" | "off"
        ),
        Err(_) => true,
    }
}

/// Filter `you` segments that are acoustic bleed from `meeting`. Preserves
/// the input order of kept segments. Meeting segments are never mutated.
pub fn dedup_mic_bleed(you: Vec<Segment>, meeting: &[Segment]) -> (Vec<Segment>, DedupStats) {
    let input_count = you.len();

    let meeting_tokens: Vec<Vec<String>> = meeting.iter().map(|m| tokenize(&m.text)).collect();

    let mut kept = Vec::with_capacity(input_count);
    let mut dropped_count = 0usize;

    for y in you {
        let y_tokens = tokenize(&y.text);
        if y_tokens.is_empty() {
            kept.push(y);
            continue;
        }

        let mut meeting_set: HashSet<&str> = HashSet::new();
        for (m, m_toks) in meeting.iter().zip(meeting_tokens.iter()) {
            if overlaps(&y, m) {
                for t in m_toks {
                    meeting_set.insert(t.as_str());
                }
            }
        }

        if meeting_set.is_empty() {
            kept.push(y);
            continue;
        }

        let matched = y_tokens
            .iter()
            .filter(|t| meeting_set.contains(t.as_str()))
            .count();
        let cov = matched as f32 / y_tokens.len() as f32;

        let is_bleed = if y_tokens.len() >= MIN_TOKENS_FOR_PARTIAL {
            cov >= COVERAGE_THRESHOLD
        } else {
            cov >= 1.0
        };

        if is_bleed {
            dropped_count += 1;
        } else {
            kept.push(y);
        }
    }

    (
        kept,
        DedupStats {
            input_count,
            dropped_count,
        },
    )
}

/// Does `you`'s time range intersect the acoustic-bleed window around
/// `meeting`? The window is asymmetric: bleed arrives *after* the meeting
/// audio (encoder + whisper latency), not before.
fn overlaps(you: &Segment, meeting: &Segment) -> bool {
    you.end_sec >= meeting.start_sec - WINDOW_LEAD_SEC
        && you.start_sec <= meeting.end_sec + WINDOW_LAG_SEC
}

/// Lowercase, strip whisper meta tokens (`[BLANK_AUDIO]`, `[ Pause ]`,
/// `[Music]` …), and split on non-alphanumeric characters. Returns the bag
/// of tokens in order; empty strings are discarded.
fn tokenize(text: &str) -> Vec<String> {
    let stripped = strip_whisper_meta(text);
    let mut out = Vec::new();
    let mut cur = String::new();
    for c in stripped.chars() {
        if c.is_ascii_alphanumeric() {
            for lc in c.to_lowercase() {
                cur.push(lc);
            }
        } else if !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// Remove the content of `[...]` regions (including the brackets). Nested
/// brackets are handled by depth counting; unmatched closing brackets are
/// silently dropped. Whisper uses these for non-speech markers we don't want
/// to count as content words.
fn strip_whisper_meta(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut depth: i32 = 0;
    for c in s.chars() {
        match c {
            '[' => depth += 1,
            ']' if depth > 0 => depth -= 1,
            _ if depth > 0 => {}
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transcription::whisper::Speaker;

    fn you(start: f64, end: f64, text: &str) -> Segment {
        Segment {
            start_sec: start,
            end_sec: end,
            speaker: Speaker::You,
            text: text.to_string(),
        }
    }

    fn meeting(start: f64, end: f64, text: &str) -> Segment {
        Segment {
            start_sec: start,
            end_sec: end,
            speaker: Speaker::Meeting,
            text: text.to_string(),
        }
    }

    #[test]
    fn test_tokenize_basic() {
        assert_eq!(
            tokenize("Hello, world!"),
            vec!["hello".to_string(), "world".to_string()]
        );
    }

    #[test]
    fn test_tokenize_strips_whisper_meta() {
        assert_eq!(
            tokenize("hello [BLANK_AUDIO] world"),
            vec!["hello".to_string(), "world".to_string()]
        );
        assert_eq!(tokenize("[ Pause ]"), Vec::<String>::new());
        assert_eq!(tokenize("[Music]"), Vec::<String>::new());
    }

    #[test]
    fn test_tokenize_apostrophes_stripped() {
        assert_eq!(
            tokenize("it's going"),
            vec!["it".to_string(), "s".to_string(), "going".to_string()]
        );
    }

    #[test]
    fn test_overlaps_within_lag_window() {
        let m = meeting(10.0, 12.0, "x");
        let y = you(13.5, 14.5, "x");
        assert!(overlaps(&y, &m));
    }

    #[test]
    fn test_overlaps_outside_window_rejected() {
        let m = meeting(10.0, 12.0, "x");
        let y = you(16.0, 17.0, "x");
        assert!(!overlaps(&y, &m));
    }

    #[test]
    fn test_overlaps_lead_window_small() {
        let m = meeting(10.0, 12.0, "x");
        let y = you(9.7, 10.5, "x");
        assert!(overlaps(&y, &m));
        let y_too_early = you(9.0, 9.4, "x");
        assert!(!overlaps(&y_too_early, &m));
    }

    #[test]
    fn test_drops_exact_duplicate_in_window() {
        let you_segs = vec![you(10.5, 12.5, "hello world.")];
        let meet_segs = vec![meeting(10.0, 12.0, "Hello, world!")];
        let (out, stats) = dedup_mic_bleed(you_segs, &meet_segs);
        assert!(out.is_empty());
        assert_eq!(stats.input_count, 1);
        assert_eq!(stats.dropped_count, 1);
    }

    #[test]
    fn test_keeps_non_overlapping_duplicate() {
        let you_segs = vec![you(40.0, 42.0, "hello world.")];
        let meet_segs = vec![meeting(5.0, 7.0, "Hello, world!")];
        let (out, stats) = dedup_mic_bleed(you_segs, &meet_segs);
        assert_eq!(out.len(), 1);
        assert_eq!(stats.dropped_count, 0);
    }

    #[test]
    fn test_keeps_distinct_text_in_window() {
        let you_segs = vec![you(10.5, 11.0, "sounds good.")];
        let meet_segs = vec![meeting(10.0, 12.0, "quarterly numbers.")];
        let (out, stats) = dedup_mic_bleed(you_segs, &meet_segs);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].text, "sounds good.");
        assert_eq!(stats.dropped_count, 0);
    }

    #[test]
    fn test_drops_against_concatenated_meeting_segments() {
        let you_segs = vec![you(
            25.32,
            29.64,
            "Different people were in any given instance. Most of it's just not, it's not going to be",
        )];
        let meet_segs = vec![
            meeting(24.0, 27.04, "Different people were in any given instance,"),
            meeting(
                27.04,
                30.30,
                "most of it's just not, it's not going to be an input.",
            ),
        ];
        let (out, stats) = dedup_mic_bleed(you_segs, &meet_segs);
        assert!(out.is_empty());
        assert_eq!(stats.dropped_count, 1);
    }

    #[test]
    fn test_drops_short_you_covered_by_meeting() {
        let you_segs = vec![you(29.64, 30.64, "an input.")];
        let meet_segs = vec![meeting(
            27.04,
            30.30,
            "most of it's just not, it's not going to be an input.",
        )];
        let (out, stats) = dedup_mic_bleed(you_segs, &meet_segs);
        assert!(out.is_empty());
        assert_eq!(stats.dropped_count, 1);
    }

    #[test]
    fn test_keeps_single_token_you_not_in_meeting() {
        let you_segs = vec![you(10.5, 11.0, "yeah.")];
        let meet_segs = vec![meeting(10.0, 12.0, "quarterly.")];
        let (out, stats) = dedup_mic_bleed(you_segs, &meet_segs);
        assert_eq!(out.len(), 1);
        assert_eq!(stats.dropped_count, 0);
    }

    #[test]
    fn test_single_token_requires_full_coverage() {
        let you_segs = vec![you(10.5, 11.0, "okay.")];
        let meet_segs = vec![meeting(10.0, 12.0, "Okay, let's continue.")];
        let (out, stats) = dedup_mic_bleed(you_segs, &meet_segs);
        assert!(
            out.is_empty(),
            "single token 'okay' should drop when meeting contains it"
        );
        assert_eq!(stats.dropped_count, 1);
    }

    #[test]
    fn test_window_skew_forward() {
        let you_segs = vec![you(28.5, 29.5, "sales pipeline updates")];
        let meet_segs = vec![meeting(25.0, 27.0, "sales pipeline updates.")];
        let (out, stats) = dedup_mic_bleed(you_segs, &meet_segs);
        assert!(out.is_empty());
        assert_eq!(stats.dropped_count, 1);
    }

    #[test]
    fn test_window_skew_rejects_late() {
        let you_segs = vec![you(31.0, 32.0, "sales pipeline updates")];
        let meet_segs = vec![meeting(25.0, 27.0, "sales pipeline updates.")];
        let (out, stats) = dedup_mic_bleed(you_segs, &meet_segs);
        assert_eq!(out.len(), 1);
        assert_eq!(stats.dropped_count, 0);
    }

    #[test]
    fn test_empty_meeting_returns_unchanged() {
        let you_segs = vec![you(0.0, 1.0, "hello"), you(2.0, 3.0, "goodbye")];
        let (out, stats) = dedup_mic_bleed(you_segs, &[]);
        assert_eq!(out.len(), 2);
        assert_eq!(stats.input_count, 2);
        assert_eq!(stats.dropped_count, 0);
    }

    #[test]
    fn test_meeting_meta_only_does_not_dedup_you() {
        let you_segs = vec![you(
            0.0,
            25.32,
            "Testing this out. I really hope this works.",
        )];
        let meet_segs = vec![meeting(0.0, 24.0, "[ Pause ]")];
        let (out, stats) = dedup_mic_bleed(you_segs, &meet_segs);
        assert_eq!(out.len(), 1);
        assert_eq!(stats.dropped_count, 0);
    }

    #[test]
    fn test_mixed_you_segments_partial_drop() {
        let you_segs = vec![
            you(0.0, 5.0, "Testing one two three."),
            you(10.5, 12.0, "quarterly revenue numbers"),
        ];
        let meet_segs = vec![meeting(10.0, 12.0, "Quarterly revenue numbers.")];
        let (out, stats) = dedup_mic_bleed(you_segs, &meet_segs);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].text, "Testing one two three.");
        assert_eq!(stats.dropped_count, 1);
    }

    #[test]
    fn test_drops_user_transcript_overlap_case() {
        let you_segs = vec![you(
            12.0,
            30.0,
            "How do you think about prolificness versus depth?",
        )];
        let meet_segs = vec![
            meeting(0.0, 26.6, "[ Pause ]"),
            meeting(
                26.6,
                29.64,
                "How do you think about prolificness versus depth?",
            ),
            meeting(
                29.64,
                31.52,
                "Where -- I don't know, maybe Darwin's an example",
            ),
        ];
        let (out, stats) = dedup_mic_bleed(you_segs, &meet_segs);
        assert!(out.is_empty());
        assert_eq!(stats.input_count, 1);
        assert_eq!(stats.dropped_count, 1);
    }

    #[test]
    fn test_stats_counts_match() {
        let you_segs = vec![
            you(0.0, 1.0, "kept one"),
            you(10.5, 12.0, "the bleed"),
            you(20.0, 21.0, "kept two"),
        ];
        let meet_segs = vec![meeting(10.0, 12.0, "the bleed.")];
        let (out, stats) = dedup_mic_bleed(you_segs, &meet_segs);
        assert_eq!(stats.input_count, 3);
        assert_eq!(stats.dropped_count, 1);
        assert_eq!(out.len(), stats.input_count - stats.dropped_count);
    }

    #[test]
    fn test_is_enabled_default_on() {
        let orig = std::env::var("DOMINO_DEDUP").ok();
        std::env::remove_var("DOMINO_DEDUP");
        assert!(is_enabled());
        std::env::set_var("DOMINO_DEDUP", "off");
        assert!(!is_enabled());
        std::env::set_var("DOMINO_DEDUP", "0");
        assert!(!is_enabled());
        std::env::set_var("DOMINO_DEDUP", "true");
        assert!(is_enabled());
        match orig {
            Some(v) => std::env::set_var("DOMINO_DEDUP", v),
            None => std::env::remove_var("DOMINO_DEDUP"),
        }
    }
}
