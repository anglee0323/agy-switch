use super::{bytes_field, string_field, varint_field};
use serde::{Deserialize, Serialize};

const SAMPLE_LIMIT: usize = 10;
const MAX_AGE_SECONDS: i64 = 7 * 24 * 60 * 60;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentPerformance {
    pub model_count: usize,
    pub source_count: usize,
    pub sample_count: usize,
    pub first_text_seconds: f64,
    pub body_tokens_per_second: f64,
    pub last_activity: i64,
}

pub(super) struct Sample {
    model: String,
    source: String,
    timestamp: i64,
    first_text_seconds: f64,
    body_tokens_per_second: f64,
}

pub(super) fn is_completed_text(status: Option<i64>, payload: Option<&[u8]>) -> bool {
    let Some(response) = payload.and_then(|payload| bytes_field(20, payload)) else {
        return false;
    };
    // CortexStepPlannerResponse: response=1, tool_calls=7, stop_reason=12.
    // Only DONE (3), STOP_PATTERN (2) text responses qualify. Tool calls can
    // arrive in a single buffered chunk and would produce misleading TPS.
    status == Some(3)
        && varint_field(12, response) == Some(2)
        && string_field(1, response).is_some()
        && bytes_field(7, response).is_none()
}

fn duration(message: &[u8]) -> Option<f64> {
    let seconds = varint_field(1, message).unwrap_or(0);
    let nanos = varint_field(2, message).unwrap_or(0);
    // Negative protobuf int64 durations decode as very large unsigned values.
    if seconds > i64::MAX as u64 || nanos >= 1_000_000_000 {
        return None;
    }
    let value = seconds as f64 + nanos as f64 / 1_000_000_000.0;
    (value > 0.0 && value.is_finite()).then_some(value)
}

pub(super) fn sample(blob: &[u8], timestamp: Option<i64>, source: &str) -> Option<Sample> {
    let wrapped = bytes_field(1, blob)?;
    let usage = bytes_field(4, wrapped)?;
    let body_tokens = varint_field(10, usage)?; // response_output_tokens, excludes thinking (9).
    let first_text_seconds = duration(bytes_field(11, wrapped)?)?;
    let streaming_seconds = duration(bytes_field(12, wrapped)?)?;
    // Avoid tiny/buffered replies where chunk boundaries dominate the estimate.
    if body_tokens < 100 || streaming_seconds < 1.0 {
        return None;
    }
    Some(Sample {
        model: string_field(21, wrapped).or_else(|| string_field(19, wrapped))?,
        source: source.to_string(),
        timestamp: timestamp?,
        first_text_seconds,
        body_tokens_per_second: body_tokens as f64 / streaming_seconds,
    })
}

pub(super) fn summarize(mut samples: Vec<Sample>, now: i64) -> Option<RecentPerformance> {
    samples.retain(|sample| sample.timestamp >= now - MAX_AGE_SECONDS && sample.timestamp <= now);
    samples.sort_by(|left, right| {
        right
            .timestamp
            .cmp(&left.timestamp)
            .then_with(|| left.source.cmp(&right.source))
            .then_with(|| left.model.cmp(&right.model))
    });
    let latest = samples.first()?;
    let last_activity = latest.timestamp;
    // Summarize recent user experience across all models and local stores.
    // Each eligible generation has equal weight; no model gets its own window.
    let recent: Vec<_> = samples.iter().take(SAMPLE_LIMIT).collect();
    let median = |mut values: Vec<f64>| {
        values.sort_by(f64::total_cmp);
        let middle = values.len() / 2;
        if values.len() % 2 == 0 {
            (values[middle - 1] + values[middle]) / 2.0
        } else {
            values[middle]
        }
    };
    Some(RecentPerformance {
        model_count: recent
            .iter()
            .map(|sample| &sample.model)
            .collect::<std::collections::HashSet<_>>()
            .len(),
        source_count: recent
            .iter()
            .map(|sample| &sample.source)
            .collect::<std::collections::HashSet<_>>()
            .len(),
        last_activity,
        sample_count: recent.len(),
        first_text_seconds: median(
            recent
                .iter()
                .map(|sample| sample.first_text_seconds)
                .collect(),
        ),
        body_tokens_per_second: median(
            recent
                .iter()
                .map(|sample| sample.body_tokens_per_second)
                .collect(),
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vi(mut value: u64) -> Vec<u8> {
        let mut bytes = Vec::new();
        loop {
            let next = (value & 127) as u8;
            value >>= 7;
            bytes.push(if value == 0 { next } else { next | 128 });
            if value == 0 {
                return bytes;
            }
        }
    }
    fn scalar(number: u32, value: u64) -> Vec<u8> {
        [vi(u64::from(number << 3)), vi(value)].concat()
    }
    fn message(number: u32, value: &[u8]) -> Vec<u8> {
        [
            vi(u64::from(number << 3 | 2)),
            vi(value.len() as u64),
            value.to_vec(),
        ]
        .concat()
    }
    fn blob(tokens: u64, seconds: u64, nanos: u64) -> Vec<u8> {
        let usage = [scalar(3, tokens + 600), scalar(9, 600), scalar(10, tokens)].concat();
        message(
            1,
            &[
                message(4, &usage),
                message(11, &[scalar(1, 4), scalar(2, 250_000_000)].concat()),
                message(12, &[scalar(1, seconds), scalar(2, nanos)].concat()),
                message(19, b"gemini-test"),
            ]
            .concat(),
        )
    }
    fn text_payload(stop: u64, tools: bool) -> Vec<u8> {
        let mut response = [message(1, b"synthetic text"), scalar(12, stop)].concat();
        if tools {
            response.extend(message(7, b""));
        }
        message(20, &response)
    }

    #[test]
    fn decodes_nanosecond_timings_and_excludes_thinking_tokens() {
        let decoded = sample(&blob(1000, 6, 250_000_000), Some(100), "antigravity").unwrap();
        assert_eq!(decoded.first_text_seconds, 4.25);
        assert_eq!(decoded.body_tokens_per_second, 160.0);
        assert_eq!(decoded.model, "gemini-test");
    }

    #[test]
    fn rejects_missing_negative_invalid_and_buffered_timings() {
        for (tokens, secs, nanos) in [
            (99, 2, 0),
            (100, 0, 999_999_999),
            (100, 0, 0),
            (100, u64::MAX, 0),
            (100, 2, 1_000_000_000),
        ] {
            assert!(sample(&blob(tokens, secs, nanos), Some(100), "antigravity").is_none());
        }
        assert!(sample(&blob(1000, 6, 0), None, "antigravity").is_none());
        assert!(sample(
            &message(1, &message(4, &scalar(3, 1000))),
            Some(100),
            "antigravity"
        )
        .is_none());
    }

    #[test]
    fn excludes_tool_calls_canceled_and_incomplete_generations() {
        assert!(is_completed_text(Some(3), Some(&text_payload(2, false))));
        for stop in [0, 1, 3, 4, 6, 9] {
            assert!(!is_completed_text(
                Some(3),
                Some(&text_payload(stop, false))
            ));
        }
        assert!(!is_completed_text(Some(3), Some(&text_payload(2, true))));
        assert!(!is_completed_text(Some(1), Some(&text_payload(2, false))));
        assert!(!is_completed_text(None, Some(&text_payload(2, false))));
        assert!(!is_completed_text(Some(3), None));
        assert!(!is_completed_text(Some(3), Some(&[0xff])));
    }

    #[test]
    fn recent_median_limits_age_and_sample_count_across_models_and_sources() {
        let now = 1_800_000_000;
        let mut samples: Vec<_> = (0..12)
            .map(|i| Sample {
                model: "gemini-test".into(),
                source: "antigravity".into(),
                timestamp: now - i,
                first_text_seconds: if i == 0 { 112.0 } else { i as f64 },
                body_tokens_per_second: 100.0 + i as f64,
            })
            .collect();
        for (model, source, timestamp) in [
            ("other", "antigravity", now - 1),
            ("gemini-test", "antigravity-cli", now - 1),
            ("other", "antigravity", now + 1),
            ("gemini-test", "antigravity", now - MAX_AGE_SECONDS - 1),
        ] {
            samples.push(Sample {
                model: model.into(),
                source: source.into(),
                timestamp,
                first_text_seconds: 999.0,
                body_tokens_per_second: 999.0,
            });
        }
        let result = summarize(samples, now).unwrap();
        assert_eq!(result.sample_count, 10);
        assert_eq!(result.model_count, 2);
        assert_eq!(result.source_count, 2);
        assert_eq!(result.first_text_seconds, 5.5);
        assert_eq!(result.body_tokens_per_second, 104.5);
        assert_eq!(result.last_activity, now);
        assert!(summarize(Vec::new(), now).is_none());
    }

    #[test]
    fn five_gemini_and_five_claude_responses_contribute_to_one_summary() {
        let now = 1_800_000_000;
        let samples = (0..10)
            .map(|i| Sample {
                model: if i % 2 == 0 {
                    "gemini-test"
                } else {
                    "claude-test"
                }
                .into(),
                source: if i % 2 == 0 {
                    "antigravity"
                } else {
                    "antigravity-cli"
                }
                .into(),
                timestamp: now - i,
                first_text_seconds: if i % 2 == 0 { 2.0 } else { 8.0 },
                body_tokens_per_second: if i % 2 == 0 { 100.0 } else { 40.0 },
            })
            .collect();
        let summary = summarize(samples, now).unwrap();
        assert_eq!(summary.sample_count, 10);
        assert_eq!(summary.model_count, 2);
        assert_eq!(summary.source_count, 2);
        assert_eq!(summary.first_text_seconds, 5.0);
        assert_eq!(summary.body_tokens_per_second, 70.0);
    }

    #[test]
    fn reads_live_timing_even_when_token_usage_was_archived() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("antigravity/conversations/test.db");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let connection = rusqlite::Connection::open(&path).unwrap();
        connection.execute_batch("CREATE TABLE gen_metadata (idx INTEGER PRIMARY KEY, data BLOB);
            CREATE TABLE steps (idx INTEGER PRIMARY KEY, step_type INTEGER, metadata BLOB, status INTEGER, step_payload BLOB);").unwrap();
        let timestamp = 1_800_000_000;
        let meta = message(1, &scalar(1, timestamp));
        connection
            .execute(
                "INSERT INTO steps VALUES (41, 15, ?1, 3, ?2)",
                rusqlite::params![meta, text_payload(2, false)],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO gen_metadata VALUES (7, ?1)",
                rusqlite::params![blob(1000, 6, 250_000_000)],
            )
            .unwrap();
        drop(connection);
        let result = super::super::scan_database(
            &path,
            chrono::NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
            &std::collections::HashSet::from([7]),
        )
        .unwrap();
        assert_eq!(result.generations_scanned, 0);
        assert!(result.events.is_empty());
        let summary = summarize(result.performance_samples, timestamp as i64).unwrap();
        assert_eq!(summary.sample_count, 1);
        assert_eq!(summary.model_count, 1);
        assert_eq!(summary.source_count, 1);
        assert_eq!(summary.body_tokens_per_second, 160.0);
    }
}
