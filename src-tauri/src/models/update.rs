//! In-app update check (iTop Wrap addition, not in the Swift app). Notify only: the app asks
//! GitHub for the latest release's version, says when a newer one exists, and opens its release
//! page. It never downloads or installs anything itself.
//!
//! What GitHub sends back is trusted for one thing only: a plain `x.y.z` version. The link the
//! app opens is built here from that version and the fixed repository, never taken from the
//! response. Automatic checks run at most once a day, never in debug builds, and can be turned
//! off; a failed automatic check stays silent.

use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::prefs::Prefs;
use super::workspace::{State, Workspace};
use crate::error::AppError;

/// The repository whose releases are checked.
pub const REPO: &str = "alfi-rajasyah/itop-wrap";
/// This build's version (the Cargo workspace version, as in `tauri.conf.json`).
pub const CURRENT: &str = env!("CARGO_PKG_VERSION");
/// Automatic checks: at most once in this long.
pub const CHECK_EVERY_SECS: i64 = 24 * 60 * 60;
/// How often the automatic check wakes up to see whether a day has passed.
const WAKE_EVERY: Duration = Duration::from_secs(60 * 60);

pub mod keys {
    pub const AUTO_CHECK: &str = "update.autoCheck";
    pub const LAST_CHECK: &str = "update.lastCheck";
    pub const DISMISSED: &str = "update.dismissed";
}

/// A release version: exactly `major.minor.patch`, optionally written with a leading `v`.
/// Anything else (a pre-release suffix, extra parts, huge numbers) isn't a version here.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version(u64, u64, u64);

impl Version {
    pub fn parse(text: &str) -> Option<Version> {
        let text = text.trim();
        let text = text.strip_prefix('v').unwrap_or(text);
        let mut parts = text.split('.');
        let mut next = || -> Option<u64> {
            let p = parts.next()?;
            if p.is_empty() || p.len() > 6 || !p.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            p.parse().ok()
        };
        let v = Version(next()?, next()?, next()?);
        if parts.next().is_some() {
            return None;
        }
        Some(v)
    }

    pub fn current() -> Version {
        Version::parse(CURRENT).expect("the Cargo version is x.y.z")
    }

    /// The release page for this version, built from the fixed repository.
    pub fn release_page(self) -> String {
        format!("https://github.com/{REPO}/releases/tag/v{self}")
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.0, self.1, self.2)
    }
}

/// Whether an automatic check is due: switched on, a release build, and a day since the last.
pub fn auto_check_due(enabled: bool, debug_build: bool, last: Option<i64>, now: i64) -> bool {
    enabled && !debug_build && last.is_none_or(|l| now - l >= CHECK_EVERY_SECS || now < l)
}

/// Where the latest release's tag comes from (GitHub; a fake in tests).
#[async_trait::async_trait]
pub trait ReleaseFeed: Send + Sync {
    /// The latest published release's tag name (drafts and pre-releases aren't "latest").
    async fn latest_tag(&self) -> Result<String, String>;
}

/// `GET https://api.github.com/repos/<REPO>/releases/latest`, unauthenticated.
pub struct GitHubFeed;

#[async_trait::async_trait]
impl ReleaseFeed for GitHubFeed {
    async fn latest_tag(&self) -> Result<String, String> {
        const HOST: &str = "api.github.com";
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .user_agent(format!("iTop Wrap/{CURRENT}"))
            // Only ever talk to the API host, however it redirects.
            .redirect(reqwest::redirect::Policy::custom(|attempt| {
                if attempt.url().host_str() == Some(HOST) && attempt.previous().len() < 3 {
                    attempt.follow()
                } else {
                    attempt.stop()
                }
            }))
            .build()
            .map_err(|e| e.to_string())?;
        let response = client
            .get(format!("https://{HOST}/repos/{REPO}/releases/latest"))
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    "GitHub didn't answer in time".to_owned()
                } else {
                    "GitHub couldn't be reached".to_owned()
                }
            })?;
        let status = response.status();
        if !status.is_success() {
            return Err(format!("GitHub answered {}", status.as_u16()));
        }
        let body = response
            .bytes()
            .await
            .map_err(|_| "GitHub's answer was cut off".to_owned())?;
        if body.len() > 1_000_000 {
            return Err("GitHub's answer was unexpectedly large".to_owned());
        }
        tag_name(&body).ok_or_else(|| "GitHub's answer had no release in it".to_owned())
    }
}

/// The `tag_name` of a GitHub release object, and nothing else from it.
pub fn tag_name(body: &[u8]) -> Option<String> {
    let value: serde_json::Value = serde_json::from_slice(body).ok()?;
    value.get("tag_name")?.as_str().map(str::to_owned)
}

// MARK: - Model

#[derive(Debug, Default)]
pub(crate) struct UpdateState {
    pub checking: bool,
    /// The latest release seen this session.
    pub latest: Option<Version>,
    pub dialog: Option<UpdateDialog>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub enum UpdateDialogKind {
    UpToDate,
    Available,
    Failed,
}

/// The answer to Check for Updates… (manual checks only).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub struct UpdateDialog {
    pub kind: UpdateDialogKind,
    pub title: String,
    pub message: String,
}

/// The quiet line in the sidebar while a newer release exists and isn't dismissed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub struct UpdateNotice {
    pub version: String,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub struct UpdateSnapshot {
    pub current: String,
    pub checking: bool,
    pub auto_check: bool,
    pub notice: Option<UpdateNotice>,
    pub dialog: Option<UpdateDialog>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export, export_to = "../../ui/src/bindings/app/")]
pub enum UpdateIntent {
    /// Check for Updates…: always ends with a dialog.
    Check,
    CloseDialog,
    /// Opens the newer release's page on GitHub (from the notice or the dialog).
    Download,
    /// Hides the notice until a release newer than this one comes out.
    Dismiss,
    SetAutoCheck {
        on: bool,
    },
}

fn auto_check_on(prefs: &dyn Prefs) -> bool {
    prefs.bool(keys::AUTO_CHECK).unwrap_or(true)
}

/// The newer release, if the latest one seen is newer than this build.
fn newer(st: &State) -> Option<Version> {
    st.update.latest.filter(|v| *v > Version::current())
}

impl Workspace {
    pub(crate) fn release_feed(&self) -> Arc<dyn ReleaseFeed> {
        self.0.releases.get_or_init(|| Arc::new(GitHubFeed)).clone()
    }

    /// Tests: answer from this feed instead of GitHub (before the first check).
    pub fn set_release_feed(&self, feed: Arc<dyn ReleaseFeed>) {
        let _ = self.0.releases.set(feed);
    }

    pub(crate) fn update_snapshot(&self, st: &State) -> UpdateSnapshot {
        let prefs = self.0.prefs.as_ref();
        let dismissed = prefs.string(keys::DISMISSED);
        let notice = newer(st)
            .filter(|v| dismissed.as_deref() != Some(v.to_string().as_str()))
            .map(|v| UpdateNotice {
                version: v.to_string(),
                text: format!("iTop Wrap {v} is available"),
            });
        UpdateSnapshot {
            current: CURRENT.to_owned(),
            checking: st.update.checking,
            auto_check: auto_check_on(prefs),
            notice,
            dialog: st.update.dialog.clone(),
        }
    }

    pub(crate) async fn dispatch_update(
        &self,
        intent: UpdateIntent,
    ) -> Result<serde_json::Value, AppError> {
        match intent {
            UpdateIntent::Check => self.update_check(true).await,
            UpdateIntent::CloseDialog => {
                self.lock().update.dialog = None;
                self.publish();
            }
            UpdateIntent::Download => {
                let target = {
                    let mut st = self.lock();
                    st.update.dialog = None;
                    newer(&st)
                };
                if let Some(v) = target {
                    self.0.ctx.native.open_url(&v.release_page());
                }
                self.publish();
            }
            UpdateIntent::Dismiss => {
                let target = newer(&self.lock());
                if let Some(v) = target {
                    self.0
                        .prefs
                        .set_string(keys::DISMISSED, Some(&v.to_string()));
                }
                self.publish();
            }
            UpdateIntent::SetAutoCheck { on } => {
                self.0.prefs.set_bool(keys::AUTO_CHECK, on);
                self.publish();
            }
        }
        Ok(serde_json::Value::Null)
    }

    /// One check. A manual one always ends with a dialog; an automatic one only updates the
    /// notice, and says nothing when it fails.
    pub(crate) async fn update_check(&self, manual: bool) {
        {
            let mut st = self.lock();
            if st.update.checking {
                return;
            }
            st.update.checking = true;
            if manual {
                st.update.dialog = None;
            }
        }
        self.publish();
        let answer = self.release_feed().latest_tag().await;
        let current = Version::current();
        let dialog = match answer.as_deref().map(Version::parse) {
            Ok(Some(latest)) => {
                self.0
                    .prefs
                    .set_int(keys::LAST_CHECK, self.now().timestamp());
                self.lock().update.latest = Some(latest);
                if latest > current {
                    UpdateDialog {
                        kind: UpdateDialogKind::Available,
                        title: format!("iTop Wrap {latest} is available"),
                        message: format!(
                            "You have {current}. Download it from GitHub, then drag it to Applications to replace this version."
                        ),
                    }
                } else {
                    UpdateDialog {
                        kind: UpdateDialogKind::UpToDate,
                        title: "You're up to date".to_owned(),
                        message: format!("iTop Wrap {current} is the latest version."),
                    }
                }
            }
            Ok(None) => UpdateDialog {
                kind: UpdateDialogKind::Failed,
                title: "Couldn't check for updates".to_owned(),
                message: "GitHub's latest release doesn't have a version number.".to_owned(),
            },
            Err(e) => UpdateDialog {
                kind: UpdateDialogKind::Failed,
                title: "Couldn't check for updates".to_owned(),
                message: format!("{e}. Try again later."),
            },
        };
        {
            let mut st = self.lock();
            st.update.checking = false;
            if manual {
                st.update.dialog = Some(dialog);
            }
        }
        self.publish();
    }

    /// The automatic check: wakes every hour, checks when a day has passed.
    pub(crate) async fn update_loop(&self) {
        loop {
            let due = auto_check_due(
                auto_check_on(self.0.prefs.as_ref()),
                cfg!(debug_assertions),
                self.0.prefs.int(keys::LAST_CHECK),
                self.now().timestamp(),
            );
            if due {
                self.update_check(false).await;
            }
            tokio::time::sleep(WAKE_EVERY).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_parse_only_plain_x_y_z() {
        assert_eq!(Version::parse("v2.1.0"), Some(Version(2, 1, 0)));
        assert_eq!(Version::parse("2.0.10"), Some(Version(2, 0, 10)));
        assert_eq!(Version::parse(" v1.0.2 "), Some(Version(1, 0, 2)));
        for bad in [
            "",
            "v",
            "2.1",
            "2.1.0.1",
            "2.1.0-beta",
            "v2.x.0",
            "1..2",
            "-1.0.0",
            "1.0.0;rm",
            "9999999.0.0",
        ] {
            assert_eq!(Version::parse(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn versions_compare_numerically() {
        let v = |s| Version::parse(s).unwrap();
        assert!(v("2.0.10") > v("2.0.9"));
        assert!(v("10.0.0") > v("9.9.9"));
        assert!(v("2.0.0") == v("v2.0.0"));
        assert!(v("1.0.2") < v("2.0.0"));
    }

    #[test]
    fn the_release_page_is_built_from_the_version_only() {
        assert_eq!(
            Version(2, 1, 0).release_page(),
            "https://github.com/alfi-rajasyah/itop-wrap/releases/tag/v2.1.0"
        );
    }

    #[test]
    fn current_is_the_cargo_version() {
        assert_eq!(Version::current().to_string(), CURRENT);
    }

    #[test]
    fn automatic_checks_are_daily_release_only_and_can_be_off() {
        let day = CHECK_EVERY_SECS;
        assert!(auto_check_due(true, false, None, 1_000));
        assert!(!auto_check_due(false, false, None, 1_000), "switched off");
        assert!(!auto_check_due(true, true, None, 1_000), "debug build");
        assert!(!auto_check_due(true, false, Some(1_000), 1_000 + day - 1));
        assert!(auto_check_due(true, false, Some(1_000), 1_000 + day));
        assert!(
            auto_check_due(true, false, Some(1_000 + day), 1_000),
            "clock went back"
        );
    }

    #[test]
    fn only_tag_name_is_read_from_githubs_answer() {
        let body = br#"{"tag_name":"v2.1.0","html_url":"https://evil.example/","assets":[]}"#;
        assert_eq!(tag_name(body).as_deref(), Some("v2.1.0"));
        assert_eq!(tag_name(b"{}"), None);
        assert_eq!(tag_name(b"not json"), None);
        assert_eq!(tag_name(br#"{"tag_name":7}"#), None);
    }
}
