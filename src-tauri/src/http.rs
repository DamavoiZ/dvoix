//! Downloads for the plugin store. Only `https` to GitHub, with a size limit;
//! which URLs are built from what is decided in `dvoix_core::store`.

use std::sync::OnceLock;
use std::time::Duration;

use dvoix_core::store::MAX_FILE_BYTES;

/// Hosts a download may start at. GitHub redirects release files to its own
/// storage hosts, which ureq follows.
const ALLOWED_HOSTS: &[&str] = &["github.com", "raw.githubusercontent.com"];

/// One agent for all downloads, so connections to GitHub stay open between
/// requests: a manifest check, an install or an update check reuses the
/// TCP+TLS connection instead of paying a handshake for every file.
fn agent() -> &'static ureq::Agent {
    static AGENT: OnceLock<ureq::Agent> = OnceLock::new();
    AGENT.get_or_init(|| {
        ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(30)))
            .user_agent(concat!("dvoix/", env!("CARGO_PKG_VERSION")))
            .https_only(true)
            .build()
            .into()
    })
}

fn allowed(url: &str) -> bool {
    url.strip_prefix("https://")
        .and_then(|rest| rest.split('/').next())
        .is_some_and(|host| ALLOWED_HOSTS.contains(&host))
}

/// Fetches a text file. `Ok(None)` if it does not exist (404), so optional
/// files like `styles.css` can be told apart from network errors.
pub fn get_text(url: &str) -> Result<Option<String>, String> {
    if !allowed(url) {
        return Err(format!("Download not allowed: {url}"));
    }
    match agent().get(url).call() {
        Ok(mut res) => res
            .body_mut()
            .with_config()
            .limit(MAX_FILE_BYTES)
            .read_to_string()
            .map(Some)
            .map_err(|e| format!("Download from {url} failed: {e}")),
        Err(ureq::Error::StatusCode(404)) => Ok(None),
        Err(e) => Err(format!("Download from {url} failed: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_github_over_https() {
        assert!(allowed(
            "https://github.com/a/b/releases/download/1.0.0/main.js"
        ));
        assert!(allowed(
            "https://raw.githubusercontent.com/a/b/main/registry.json"
        ));
        assert!(!allowed("http://github.com/a/b"));
        assert!(!allowed("https://github.com.evil.example/a"));
        assert!(!allowed("https://evil.example/github.com/"));
    }
}
