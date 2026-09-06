//! Grouping for the decision inbox.
//!
//! The pending backlog runs to four figures, so the inbox neither ships nor
//! renders it whole: decisions that share a proposed action and a sender are
//! collapsed into a group the reviewer can accept or reject in one judgement,
//! and only a page of groups is hydrated. The grouping itself is pure so it
//! can be tested without a database.

use chrono::{DateTime, Utc};
use uuid::Uuid;

/// One pending decision reduced to what grouping needs
#[derive(Debug, Clone)]
pub struct GroupInput {
    pub id: Uuid,
    pub decision_type: String,
    /// From-address of the source email; absent for non-email decisions
    pub sender_address: Option<String>,
    pub sender_name: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// A group of pending decisions, before their full rows are loaded
#[derive(Debug, Clone, PartialEq)]
pub struct GroupIndex {
    pub key: String,
    pub decision_type: String,
    pub sender_address: Option<String>,
    pub sender_name: Option<String>,
    pub latest_at: DateTime<Utc>,
    /// Members, newest first
    pub decision_ids: Vec<Uuid>,
}

/// Collapse pending decisions into (decision_type, sender) groups.
///
/// Groups come back largest first — the biggest group is the most backlog one
/// judgement can clear — then newest first, then by key so paging is stable
/// across requests that see the same backlog.
pub fn group_pending(inputs: Vec<GroupInput>) -> Vec<GroupIndex> {
    let mut groups: Vec<GroupIndex> = Vec::new();
    let mut index: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

    for input in inputs {
        // An empty sender segment means "not email-sourced"; a real address
        // can never be empty, so the two can't collide.
        let key = format!(
            "{}|{}",
            input.decision_type,
            input.sender_address.as_deref().unwrap_or("")
        );

        match index.get(&key) {
            Some(&pos) => {
                let group: &mut GroupIndex = &mut groups[pos];
                group.decision_ids.push(input.id);
                if input.created_at > group.latest_at {
                    group.latest_at = input.created_at;
                }
                // Senders send under a display name inconsistently; keep the
                // first one we see rather than blanking a labelled group
                if group.sender_name.is_none() {
                    group.sender_name = input.sender_name;
                }
            }
            None => {
                index.insert(key.clone(), groups.len());
                groups.push(GroupIndex {
                    key,
                    decision_type: input.decision_type,
                    sender_address: input.sender_address,
                    sender_name: input.sender_name,
                    latest_at: input.created_at,
                    decision_ids: vec![input.id],
                });
            }
        }
    }

    groups.sort_by(|a, b| {
        b.decision_ids
            .len()
            .cmp(&a.decision_ids.len())
            .then_with(|| b.latest_at.cmp(&a.latest_at))
            .then_with(|| a.key.cmp(&b.key))
    });

    groups
}

/// Clamp a caller-supplied page size into a range the server will serve.
/// Zero or negative falls back to the default; anything past the cap is
/// truncated so one request can't ask for the whole backlog again.
pub fn clamp_page_size(requested: Option<i64>, default: i64, max: i64) -> i64 {
    match requested {
        Some(n) if n > 0 => n.min(max),
        _ => default,
    }
}

/// Clamp a caller-supplied 0-based page index
pub fn clamp_page(requested: Option<i64>) -> i64 {
    requested.filter(|n| *n > 0).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(secs: i64) -> DateTime<Utc> {
        DateTime::from_timestamp(1_700_000_000 + secs, 0).unwrap()
    }

    fn input(decision_type: &str, sender: Option<&str>, secs: i64) -> GroupInput {
        GroupInput {
            id: Uuid::new_v4(),
            decision_type: decision_type.to_string(),
            sender_address: sender.map(str::to_string),
            sender_name: None,
            created_at: at(secs),
        }
    }

    #[test]
    fn same_sender_and_type_collapse_into_one_group() {
        let groups = group_pending(vec![
            input("create_todo", Some("news@example.com"), 30),
            input("create_todo", Some("news@example.com"), 20),
            input("create_todo", Some("news@example.com"), 10),
        ]);

        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].decision_ids.len(), 3);
        assert_eq!(groups[0].key, "create_todo|news@example.com");
        assert_eq!(groups[0].latest_at, at(30));
    }

    #[test]
    fn one_sender_with_two_action_types_stays_two_groups() {
        // Approving a forward is not the same judgement as approving a todo,
        // so a shared sender must not merge them
        let groups = group_pending(vec![
            input("create_todo", Some("a@example.com"), 10),
            input("forward_email", Some("a@example.com"), 20),
        ]);

        assert_eq!(groups.len(), 2);
    }

    #[test]
    fn largest_group_sorts_first_then_newest() {
        let groups = group_pending(vec![
            input("create_todo", Some("small@example.com"), 90),
            input("create_todo", Some("big@example.com"), 10),
            input("create_todo", Some("big@example.com"), 11),
            input("create_todo", Some("mid@example.com"), 50),
            input("create_todo", Some("mid@example.com"), 51),
        ]);

        assert_eq!(groups[0].decision_ids.len(), 2);
        // Two groups of two: the one with the newer member wins the tie
        assert_eq!(groups[0].sender_address.as_deref(), Some("mid@example.com"));
        assert_eq!(groups[1].sender_address.as_deref(), Some("big@example.com"));
        assert_eq!(
            groups[2].sender_address.as_deref(),
            Some("small@example.com")
        );
    }

    #[test]
    fn decisions_without_a_sender_group_by_type_alone() {
        let groups = group_pending(vec![
            input("create_todo", None, 10),
            input("create_todo", None, 20),
        ]);

        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].sender_address, None);
        assert_eq!(groups[0].key, "create_todo|");
    }

    #[test]
    fn a_sendered_group_never_merges_with_the_senderless_one() {
        let groups = group_pending(vec![
            input("create_todo", None, 10),
            input("create_todo", Some("a@example.com"), 20),
        ]);

        assert_eq!(groups.len(), 2);
    }

    #[test]
    fn first_seen_display_name_labels_the_group() {
        let mut unnamed = input("create_todo", Some("a@example.com"), 30);
        unnamed.sender_name = None;
        let mut named = input("create_todo", Some("a@example.com"), 20);
        named.sender_name = Some("Alice".to_string());

        let groups = group_pending(vec![unnamed, named]);
        assert_eq!(groups[0].sender_name.as_deref(), Some("Alice"));
    }

    #[test]
    fn page_size_is_clamped_to_the_server_range() {
        assert_eq!(clamp_page_size(None, 20, 100), 20);
        assert_eq!(clamp_page_size(Some(0), 20, 100), 20);
        assert_eq!(clamp_page_size(Some(-5), 20, 100), 20);
        assert_eq!(clamp_page_size(Some(50), 20, 100), 50);
        assert_eq!(clamp_page_size(Some(10_000), 20, 100), 100);
    }

    #[test]
    fn negative_pages_clamp_to_the_first_page() {
        assert_eq!(clamp_page(None), 0);
        assert_eq!(clamp_page(Some(-1)), 0);
        assert_eq!(clamp_page(Some(3)), 3);
    }
}
