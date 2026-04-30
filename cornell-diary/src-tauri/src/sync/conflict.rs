//! Conflict resolution between a local and a Cloud entry sharing the same
//! `entry_date`.
//!
//! This is the FAZ 2 strategy: integer version compare, with an
//! `updated_at` tie-break only when both local and cloud are dirty. FAZ 3
//! adds char-level CRDT merging on top, but for the REST sync surface
//! version compare + last-write-wins is the right level.

use chrono::{DateTime, Utc};

use crate::db::DiaryEntry;
use crate::sync::models::CloudEntry;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictDecision {
    /// No local row for this date — insert the cloud copy.
    InsertCloud,
    /// Local exists but is stale and clean — overwrite with cloud.
    OverwriteWithCloud,
    /// Both sides have edits; cloud's are newer. Keep a backup of the
    /// local copy in the audit trail before overwriting.
    CloudWonOverDirtyLocal,
    /// Both sides have edits; local's are newer. Don't pull, push will
    /// carry it on the next leg.
    LocalWon,
    /// Local is at least as fresh as cloud — leave it alone.
    LocalAlreadyFresher,
}

pub fn decide(
    local: Option<&DiaryEntry>,
    local_is_dirty: bool,
    local_updated_at: Option<DateTime<Utc>>,
    cloud: &CloudEntry,
) -> ConflictDecision {
    let Some(local) = local else {
        return ConflictDecision::InsertCloud;
    };

    if local.version < cloud.version {
        if !local_is_dirty {
            return ConflictDecision::OverwriteWithCloud;
        }
        // Both sides moved forward — last-write-wins on updated_at.
        match local_updated_at {
            Some(local_at) if cloud.last_modified_at > local_at => {
                ConflictDecision::CloudWonOverDirtyLocal
            }
            _ => ConflictDecision::LocalWon,
        }
    } else {
        ConflictDecision::LocalAlreadyFresher
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::models::CueItem;

    fn local_entry(version: i64) -> DiaryEntry {
        DiaryEntry {
            date: "2026-04-29".into(),
            diary: "local".into(),
            cue_items: vec![CueItem {
                position: 1,
                title: "t".into(),
                content: "c".into(),
            }],
            summary: String::new(),
            quote: String::new(),
            created_at: "2026-04-29T10:00:00Z".into(),
            updated_at: "2026-04-29T10:00:00Z".into(),
            device_id: None,
            version,
        }
    }

    fn cloud_entry(version: i64, modified: &str) -> CloudEntry {
        CloudEntry {
            id: uuid::Uuid::nil(),
            journal_id: uuid::Uuid::nil(),
            entry_date: "2026-04-29".into(),
            diary: "cloud".into(),
            title_1: None,
            content_1: None,
            title_2: None,
            content_2: None,
            title_3: None,
            content_3: None,
            title_4: None,
            content_4: None,
            title_5: None,
            content_5: None,
            title_6: None,
            content_6: None,
            title_7: None,
            content_7: None,
            summary: String::new(),
            quote: String::new(),
            version,
            last_modified_at: chrono::DateTime::parse_from_rfc3339(modified)
                .unwrap()
                .with_timezone(&Utc),
            created_at: chrono::Utc::now(),
        }
    }

    #[test]
    fn no_local_means_insert() {
        let cloud = cloud_entry(1, "2026-04-29T11:00:00Z");
        assert_eq!(
            decide(None, false, None, &cloud),
            ConflictDecision::InsertCloud
        );
    }

    #[test]
    fn stale_clean_local_is_overwritten() {
        let local = local_entry(1);
        let cloud = cloud_entry(2, "2026-04-29T11:00:00Z");
        assert_eq!(
            decide(Some(&local), false, None, &cloud),
            ConflictDecision::OverwriteWithCloud
        );
    }

    #[test]
    fn dirty_local_with_older_updated_loses_to_newer_cloud() {
        let local = local_entry(1);
        let local_at = chrono::DateTime::parse_from_rfc3339("2026-04-29T10:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let cloud = cloud_entry(2, "2026-04-29T12:00:00Z");
        assert_eq!(
            decide(Some(&local), true, Some(local_at), &cloud),
            ConflictDecision::CloudWonOverDirtyLocal
        );
    }

    #[test]
    fn dirty_local_with_newer_updated_wins_against_lower_cloud_version() {
        let local = local_entry(1);
        let local_at = chrono::DateTime::parse_from_rfc3339("2026-04-29T13:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let cloud = cloud_entry(2, "2026-04-29T12:00:00Z");
        assert_eq!(
            decide(Some(&local), true, Some(local_at), &cloud),
            ConflictDecision::LocalWon
        );
    }

    #[test]
    fn local_already_higher_version_is_not_touched() {
        let local = local_entry(5);
        let cloud = cloud_entry(3, "2026-04-29T12:00:00Z");
        assert_eq!(
            decide(Some(&local), false, None, &cloud),
            ConflictDecision::LocalAlreadyFresher
        );
    }
}
