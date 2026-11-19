//! `tracing` setup with secret redaction (spec §9): Authorization headers, OAuth/API query
//! parameters and token-shaped JSON fields are masked before anything reaches a log sink.

use std::borrow::Cow;
use std::io::{self, Write};
use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{fmt, EnvFilter};

/// Patterns and replacements. Compiled once; a pattern that fails to compile disables logging
/// of that line entirely rather than risk leaking (see `redact`).
static RULES: LazyLock<Option<Vec<(Regex, &'static str)>>> = LazyLock::new(|| {
    let specs: [(&str, &'static str); 3] = [
        // Authorization: Bearer xyz  /  authorization=xyz
        (r"(?i)(authorization\s*[:=]\s*)(bearer\s+|basic\s+)?[^\s,;&]+", "${1}${2}[REDACTED]"),
        // ?access_token=…&code=…&key=… in URLs or form bodies
        (
            r"(?i)([?&\s]|^)(access_token|refresh_token|id_token|code|code_verifier|key|client_id|client_secret)=[^&\s]+",
            "${1}${2}=[REDACTED]",
        ),
        // "access_token": "…" in JSON
        (
            r#"(?i)"(access_token|refresh_token|id_token|client_secret|client_id|code_verifier)"\s*:\s*"[^"]*""#,
            r#""${1}":"[REDACTED]""#,
        ),
    ];
    specs
        .into_iter()
        .map(|(pattern, replacement)| Regex::new(pattern).ok().map(|re| (re, replacement)))
        .collect()
});

/// Masks secrets in one log line.
pub fn redact(input: &str) -> Cow<'_, str> {
    let Some(rules) = RULES.as_ref() else {
        return Cow::Borrowed("[log line withheld: redaction unavailable]");
    };
    let mut out = Cow::Borrowed(input);
    for (re, replacement) in rules {
        if re.is_match(&out) {
            out = Cow::Owned(re.replace_all(&out, *replacement).into_owned());
        }
    }
    out
}

/// Writer adapter that redacts everything written through it.
pub struct Redacting<W>(W);

impl<W: Write> Write for Redacting<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let text = String::from_utf8_lossy(buf);
        self.0.write_all(redact(&text).as_bytes())?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}

/// `MakeWriter` that wraps another one in [`Redacting`].
#[derive(Clone)]
pub struct RedactingMakeWriter<M>(M);

impl<'a, M: MakeWriter<'a>> MakeWriter<'a> for RedactingMakeWriter<M> {
    type Writer = Redacting<M::Writer>;

    fn make_writer(&'a self) -> Self::Writer {
        Redacting(self.0.make_writer())
    }
}

/// Starts logging to a daily-rolling file in `log_dir` (and stdout in debug builds).
/// Keep the returned guard alive for the app's lifetime so buffered lines are flushed.
pub fn init(log_dir: &Path) -> io::Result<WorkerGuard> {
    std::fs::create_dir_all(log_dir)?;
    let appender = tracing_appender::rolling::daily(log_dir, "unison.log");
    let (file_writer, guard) = tracing_appender::non_blocking(appender);

    let filter = EnvFilter::try_from_env("UNISON_LOG")
        .unwrap_or_else(|_| EnvFilter::new("info,unison_lib=debug,sqlx=warn"));

    let file_layer = fmt::layer()
        .with_ansi(false)
        .with_target(true)
        .with_writer(RedactingMakeWriter(file_writer));

    let stdout_layer = cfg!(debug_assertions)
        .then(|| fmt::layer().with_writer(RedactingMakeWriter(io::stdout)));

    // try_init: a second init (tests, hot restart) is a no-op instead of a panic.
    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(file_layer)
        .with(stdout_layer)
        .try_init();

    Ok(guard)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rules_compile() {
        assert!(RULES.is_some());
    }

    #[test]
    fn redacts_authorization_headers() {
        let line = r#"request headers={"authorization": "Bearer abc.def"} Authorization: Bearer xyz123"#;
        let out = redact(line);
        assert!(!out.contains("xyz123"), "{out}");
        assert!(out.contains("Authorization: Bearer [REDACTED]"), "{out}");
    }

    #[test]
    fn redacts_sensitive_query_params_only() {
        let url = "GET https://x.test/cb?code=AUTHCODE&state=abc&key=APIKEY&limit=50";
        let out = redact(url);
        assert!(!out.contains("AUTHCODE") && !out.contains("APIKEY"), "{out}");
        assert!(out.contains("state=abc") && out.contains("limit=50"), "{out}");
        let body = "grant_type=refresh_token&refresh_token=RT123&client_id=CID";
        let out = redact(body);
        assert!(!out.contains("RT123") && !out.contains("CID"), "{out}");
    }

    #[test]
    fn redacts_token_json() {
        let json = r#"{"access_token":"AT1","token_type":"Bearer","refresh_token": "RT1","expires_in":3600}"#;
        let out = redact(json);
        assert!(!out.contains("AT1") && !out.contains("RT1"), "{out}");
        assert!(out.contains(r#""expires_in":3600"#), "{out}");
    }

    #[test]
    fn leaves_ordinary_lines_untouched() {
        let line = "synced 50 liked tracks in 1.2s";
        assert!(matches!(redact(line), Cow::Borrowed(_)));
    }

    #[test]
    fn writer_redacts_through_io() {
        let mut buf = Vec::new();
        {
            let mut w = Redacting(&mut buf);
            w.write_all(b"access_token=SECRET&x=1").unwrap();
        }
        let s = String::from_utf8(buf).unwrap();
        assert!(!s.contains("SECRET"), "{s}");
    }
}
