//! Artifact-location policy: pure function over an `ArtifactDirectorySnapshot`.
//!
//! This module implements the business decision "which observed entry is the
//! download's output artifact" — extension whitelist, save_name exact /
//! prefix match, freshness window, mtime ordering, file-kind filter. It does
//! **not** touch the filesystem; it consumes the port's snapshot.
//!
//! Policy inputs are explicit (`ArtifactLocatePolicy`), not hidden constants.
//! The default policy (`default_for_n_m3u8dl_cli`) preserves the behavior of
//! the legacy `find_output_file`, with two deliberate changes documented in
//! ADR-0005 decision 9: (1) extension/name matching is ASCII-lowercase
//! case-insensitive (widens the `read_dir` branch, which was byte-level
//! case-sensitive); (2) directories are excluded even if their name ends in
//! a whitelisted extension.

use crate::application::artifact_inventory::{
    ArtifactDirectoryPresence, ArtifactDirectorySnapshot, ArtifactEntryKind, ArtifactPath,
    InventoryMoment, ObservedArtifactEntry,
};
use chrono::Duration;

/// Request facts specific to a single locate invocation (the task's save_name).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ArtifactLocateRequest {
    /// The `--saveName` the subprocess was invoked with. `None` or empty
    /// triggers the freshness-window fallback path.
    pub(crate) save_name: Option<String>,
}

impl ArtifactLocateRequest {
    pub(crate) fn new(save_name: Option<String>) -> Self {
        Self { save_name }
    }

    fn effective_save_name(&self) -> Option<&str> {
        self.save_name.as_deref().filter(|s| !s.is_empty())
    }
}

/// A normalized, dot-less, ASCII-lowercase extension. Construction lowercases
/// so that `ArtifactExtension::new("MP4") == ArtifactExtension::new("mp4")`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct ArtifactExtension(String);

impl ArtifactExtension {
    pub(crate) fn new(ext: &str) -> Self {
        // Strip a leading dot if present, ASCII-lowercase the rest.
        let trimmed = ext.strip_prefix('.').unwrap_or(ext);
        Self(to_ascii_lowercase(trimmed))
    }
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

/// Freshness window for the no-save_name fallback. A `Duration` newtype so
/// callers cannot accidentally pass a raw `Duration` that might carry an
/// invalid value (zero / negative-equivalent). Construction clamps to at
/// least 1 second.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ArtifactFreshnessWindow(Duration);

impl ArtifactFreshnessWindow {
    pub(crate) fn seconds(secs: i64) -> Self {
        Self(Duration::seconds(secs.max(1)))
    }
    pub(crate) fn as_chrono(&self) -> Duration {
        self.0
    }
}

/// Which filesystem entry kinds may be returned as an artifact. Default keeps
/// current behavior (symlinks not excluded); the policy is the single switch
/// if a future, evidence-backed decision tightens to files-only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AcceptedArtifactKinds {
    pub(crate) file: bool,
    pub(crate) symlink: bool,
}

impl Default for AcceptedArtifactKinds {
    fn default() -> Self {
        Self {
            file: true,
            symlink: true,
        }
    }
}

/// Explicit policy for artifact location. Not user-configurable today — the
/// production caller uses `default_for_n_m3u8dl_cli`. Making it a value (not
/// globals) keeps the policy function pure and lets tests parameterize it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ArtifactLocatePolicy {
    pub(crate) allowed_extensions: Vec<ArtifactExtension>,
    pub(crate) freshness_window: ArtifactFreshnessWindow,
    pub(crate) accepted_kinds: AcceptedArtifactKinds,
}

impl ArtifactLocatePolicy {
    /// Default policy for the N_m3u8DL-CLI backend. Preserves the legacy
    /// `find_output_file` extension set and 60-second freshness window.
    pub(crate) fn default_for_n_m3u8dl_cli() -> Self {
        Self {
            allowed_extensions: ["mp4", "mkv", "ts", "flv", "mpg", "mpeg"]
                .into_iter()
                .map(ArtifactExtension::new)
                .collect(),
            freshness_window: ArtifactFreshnessWindow::seconds(60),
            accepted_kinds: AcceptedArtifactKinds::default(),
        }
    }

    fn kind_acceptable(&self, kind: ArtifactEntryKind) -> bool {
        match kind {
            ArtifactEntryKind::File => self.accepted_kinds.file,
            ArtifactEntryKind::Symlink => self.accepted_kinds.symlink,
            ArtifactEntryKind::Directory | ArtifactEntryKind::Other => false,
        }
    }

    fn name_has_allowed_extension(&self, name_lower: &str) -> bool {
        self.allowed_extensions
            .iter()
            .any(|ext| has_extension(name_lower, ext.as_str()))
    }
}

/// Locate the artifact entry that best represents the subprocess output.
///
/// Returns `None` when:
/// - the directory was missing or empty,
/// - no entry satisfied kind / extension / prefix / freshness constraints.
///
/// The function is total and side-effect-free: same `(snapshot, request,
/// policy, now)` always yields the same result.
pub(crate) fn locate_artifact(
    snapshot: &ArtifactDirectorySnapshot,
    request: &ArtifactLocateRequest,
    policy: &ArtifactLocatePolicy,
    now: InventoryMoment,
) -> Option<ArtifactPath> {
    // Missing directory ⇒ no artifact here. Distinct from error (which the
    // port surfaces via Result::Err before we ever get a snapshot).
    if snapshot.presence == ArtifactDirectoryPresence::Missing {
        return None;
    }

    // Candidates: keep only acceptable-kind entries. Pre-compute the
    // ASCII-lowercase name once per entry — used for both extension and
    // prefix checks.
    let candidates: Vec<(&ObservedArtifactEntry, String)> = snapshot
        .entries
        .iter()
        .filter(|e| policy.kind_acceptable(e.kind))
        .map(|e| (e, to_ascii_lowercase(&e.name)))
        .collect();

    if candidates.is_empty() {
        return None;
    }

    match request.effective_save_name() {
        Some(save_name) => {
            let save_name_lower = to_ascii_lowercase(save_name);
            // 1. Exact match: "{save_name}.{ext}" — first extension in
            //    policy order that names an existing entry wins.
            for ext in &policy.allowed_extensions {
                let target = format!("{}.{}", save_name_lower, ext.as_str());
                if let Some((entry, _)) = candidates.iter().find(|(_, name_lower)| *name_lower == target)
                {
                    return Some(entry.path.clone());
                }
            }
            // 2. save_name prefix match among allowed-extension entries,
            //    newest mtime first, name asc as tie-breaker.
            let mut prefixed: Vec<&(&ObservedArtifactEntry, String)> = candidates
                .iter()
                .filter(|(_, name_lower)| {
                    name_lower.starts_with(&save_name_lower)
                        && policy.name_has_allowed_extension(name_lower)
                })
                .collect();
            sort_newest_first(&mut prefixed);
            prefixed.first().map(|(entry, _)| entry.path.clone())
        }
        None => {
            // 3. No save_name: freshness window + allowed extension, newest
            //    first, name asc as tie-breaker.
            let threshold = now.as_chrono() - policy.freshness_window.as_chrono();
            let mut fresh: Vec<&(&ObservedArtifactEntry, String)> = candidates
                .iter()
                .filter(|(entry, name_lower)| {
                    policy.name_has_allowed_extension(name_lower)
                        && entry.modified_at.as_chrono() >= threshold
                })
                .collect();
            sort_newest_first(&mut fresh);
            fresh.first().map(|(entry, _)| entry.path.clone())
        }
    }
}

/// Sort by mtime desc, name asc as deterministic tie-breaker.
fn sort_newest_first<'a>(items: &mut Vec<&'a (&'a ObservedArtifactEntry, String)>) {
    items.sort_by(|a, b| {
        b.0.modified_at
            .cmp(&a.0.modified_at)
            .then_with(|| a.1.cmp(&b.1))
    });
}

/// ASCII-lowercase: ADR-0005 decision 9 deliberately scopes case-folding to
/// ASCII (the whitelist is ASCII-only; Unicode case folding is out of scope
/// and would risk platform drift).
fn to_ascii_lowercase(s: &str) -> String {
    s.bytes()
        .map(|b| {
            if b.is_ascii_uppercase() {
                b.to_ascii_lowercase() as char
            } else {
                b as char
            }
        })
        .collect()
}

/// `name_lower` ends with `.{ext}` and the stem is non-empty.
fn has_extension(name_lower: &str, ext: &str) -> bool {
    let needle = format!(".{}", ext);
    name_lower.ends_with(&needle) && name_lower.len() > needle.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::artifact_inventory::{
        ArtifactDir, ArtifactModifiedAt, InventoryMoment,
    };
    use chrono::Utc;

    fn present_snapshot(entries: Vec<ObservedArtifactEntry>) -> ArtifactDirectorySnapshot {
        ArtifactDirectorySnapshot {
            dir: ArtifactDir::new("D:/Downloads".to_string()),
            presence: ArtifactDirectoryPresence::Present,
            entries,
            skipped_entry_count: 0,
        }
    }

    fn missing_snapshot() -> ArtifactDirectorySnapshot {
        ArtifactDirectorySnapshot {
            dir: ArtifactDir::new("D:/Missing".to_string()),
            presence: ArtifactDirectoryPresence::Missing,
            entries: vec![],
            skipped_entry_count: 0,
        }
    }

    fn entry(name: &str, path: &str, minutes_ago: i64, kind: ArtifactEntryKind) -> ObservedArtifactEntry {
        let at = Utc::now() - Duration::minutes(minutes_ago);
        ObservedArtifactEntry {
            name: name.to_string(),
            path: ArtifactPath::new(path.to_string()),
            modified_at: ArtifactModifiedAt::new(at),
            kind,
        }
    }

    fn now() -> InventoryMoment {
        InventoryMoment::new(Utc::now())
    }

    fn policy() -> ArtifactLocatePolicy {
        ArtifactLocatePolicy::default_for_n_m3u8dl_cli()
    }

    // ---- presence ----

    #[test]
    fn missing_directory_returns_none() {
        let snap = missing_snapshot();
        assert_eq!(
            locate_artifact(&snap, &ArtifactLocateRequest::new(None), &policy(), now()),
            None
        );
    }

    #[test]
    fn empty_present_directory_returns_none() {
        let snap = present_snapshot(vec![]);
        assert_eq!(
            locate_artifact(&snap, &ArtifactLocateRequest::new(None), &policy(), now()),
            None
        );
    }

    // ---- exact match ----

    #[test]
    fn exact_match_returns_save_name_path() {
        let snap = present_snapshot(vec![
            entry("video.mp4", "D:/Downloads/video.mp4", 0, ArtifactEntryKind::File),
            entry("other.mp4", "D:/Downloads/other.mp4", 0, ArtifactEntryKind::File),
        ]);
        let req = ArtifactLocateRequest::new(Some("video".to_string()));
        assert_eq!(
            locate_artifact(&snap, &req, &policy(), now()),
            Some(ArtifactPath::new("D:/Downloads/video.mp4".to_string()))
        );
    }

    #[test]
    fn exact_match_first_extension_in_policy_order_wins() {
        // Two extensions would match the same stem; mp4 comes before mkv in
        // policy order, so the .mp4 entry wins even if both exist.
        let snap = present_snapshot(vec![
            entry("video.mkv", "D:/Downloads/video.mkv", 0, ArtifactEntryKind::File),
            entry("video.mp4", "D:/Downloads/video.mp4", 0, ArtifactEntryKind::File),
        ]);
        let req = ArtifactLocateRequest::new(Some("video".to_string()));
        assert_eq!(
            locate_artifact(&snap, &req, &policy(), now()),
            Some(ArtifactPath::new("D:/Downloads/video.mp4".to_string()))
        );
    }

    #[test]
    fn exact_match_is_case_insensitive() {
        // ADR-0005 decision 9: ASCII lowercase normalization. Windows
        // behavior preservation (the legacy `exists()` was case-insensitive).
        let snap = present_snapshot(vec![entry(
            "Video.MP4",
            "D:/Downloads/Video.MP4",
            0,
            ArtifactEntryKind::File,
        )]);
        let req = ArtifactLocateRequest::new(Some("video".to_string()));
        assert_eq!(
            locate_artifact(&snap, &req, &policy(), now()),
            Some(ArtifactPath::new("D:/Downloads/Video.MP4".to_string()))
        );
    }

    // ---- prefix fallback ----

    #[test]
    fn prefix_match_picks_newest() {
        let snap = present_snapshot(vec![
            entry("video-part1.mp4", "D:/Downloads/video-part1.mp4", 5, ArtifactEntryKind::File),
            entry("video-part2.mp4", "D:/Downloads/video-part2.mp4", 1, ArtifactEntryKind::File),
        ]);
        let req = ArtifactLocateRequest::new(Some("video".to_string()));
        // exact match fails (no "video.mp4"), prefix match picks newest (part2)
        assert_eq!(
            locate_artifact(&snap, &req, &policy(), now()),
            Some(ArtifactPath::new("D:/Downloads/video-part2.mp4".to_string()))
        );
    }

    #[test]
    fn prefix_match_breaks_ties_by_name_asc() {
        // Same mtime, different names — deterministic name-asc tie-break.
        let snap = present_snapshot(vec![
            entry("video-b.mp4", "D:/Downloads/video-b.mp4", 1, ArtifactEntryKind::File),
            entry("video-a.mp4", "D:/Downloads/video-a.mp4", 1, ArtifactEntryKind::File),
        ]);
        let req = ArtifactLocateRequest::new(Some("video".to_string()));
        assert_eq!(
            locate_artifact(&snap, &req, &policy(), now()),
            Some(ArtifactPath::new("D:/Downloads/video-a.mp4".to_string()))
        );
    }

    #[test]
    fn prefix_match_ignores_wrong_extension() {
        let snap = present_snapshot(vec![entry(
            "video.txt",
            "D:/Downloads/video.txt",
            0,
            ArtifactEntryKind::File,
        )]);
        let req = ArtifactLocateRequest::new(Some("video".to_string()));
        assert_eq!(locate_artifact(&snap, &req, &policy(), now()), None);
    }

    // ---- freshness fallback (no save_name) ----

    #[test]
    fn freshness_fallback_picks_newest_within_window() {
        let snap = present_snapshot(vec![
            entry("a.mp4", "D:/Downloads/a.mp4", 10, ArtifactEntryKind::File),
            entry("b.mp4", "D:/Downloads/b.mp4", 0, ArtifactEntryKind::File),
        ]);
        // no save_name: freshness window (60s) admits both, newest wins
        let req = ArtifactLocateRequest::new(None);
        assert_eq!(
            locate_artifact(&snap, &req, &policy(), now()),
            Some(ArtifactPath::new("D:/Downloads/b.mp4".to_string()))
        );
    }

    #[test]
    fn freshness_fallback_excludes_stale_entries() {
        // 10 minutes ago — well outside the 60s window.
        let snap = present_snapshot(vec![entry(
            "stale.mp4",
            "D:/Downloads/stale.mp4",
            10,
            ArtifactEntryKind::File,
        )]);
        let req = ArtifactLocateRequest::new(None);
        assert_eq!(locate_artifact(&snap, &req, &policy(), now()), None);
    }

    // ---- kind filtering ----

    #[test]
    fn directory_named_like_video_is_excluded() {
        let snap = present_snapshot(vec![entry(
            "video.mp4",
            "D:/Downloads/video.mp4",
            0,
            ArtifactEntryKind::Directory,
        )]);
        let req = ArtifactLocateRequest::new(Some("video".to_string()));
        assert_eq!(locate_artifact(&snap, &req, &policy(), now()), None);
    }

    #[test]
    fn symlink_is_accepted_by_default_policy() {
        let snap = present_snapshot(vec![entry(
            "video.mp4",
            "D:/Downloads/video.mp4",
            0,
            ArtifactEntryKind::Symlink,
        )]);
        let req = ArtifactLocateRequest::new(Some("video".to_string()));
        assert_eq!(
            locate_artifact(&snap, &req, &policy(), now()),
            Some(ArtifactPath::new("D:/Downloads/video.mp4".to_string()))
        );
    }

    #[test]
    fn symlink_can_be_excluded_via_policy() {
        let snap = present_snapshot(vec![entry(
            "video.mp4",
            "D:/Downloads/video.mp4",
            0,
            ArtifactEntryKind::Symlink,
        )]);
        let req = ArtifactLocateRequest::new(Some("video".to_string()));
        let mut p = policy();
        p.accepted_kinds.symlink = false;
        assert_eq!(locate_artifact(&snap, &req, &p, now()), None);
    }

    // ---- extension normalization ----

    #[test]
    fn artifact_extension_strips_leading_dot_and_lowercases() {
        assert_eq!(ArtifactExtension::new(".MP4").as_str(), "mp4");
        assert_eq!(ArtifactExtension::new("MKV").as_str(), "mkv");
    }

    #[test]
    fn empty_save_name_treated_as_none() {
        // Mirrors the legacy behavior where empty save_name falls through to
        // the freshness path.
        let snap = present_snapshot(vec![entry(
            "fresh.mp4",
            "D:/Downloads/fresh.mp4",
            0,
            ArtifactEntryKind::File,
        )]);
        let req = ArtifactLocateRequest::new(Some(String::new()));
        assert_eq!(
            locate_artifact(&snap, &req, &policy(), now()),
            Some(ArtifactPath::new("D:/Downloads/fresh.mp4".to_string()))
        );
    }
}
