use shared_types::ArchiveReviewItem;
use uuid::Uuid;
use yew::Reducible;

/// Monotonic bundle keys. Generated at dispatch sites (not in the reducer)
/// so the failure path can schedule its re-enable timer against a known key.
static NEXT_BUNDLE_KEY: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

pub(super) fn next_bundle_key() -> usize {
    NEXT_BUNDLE_KEY.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

const REVIEW_BUNDLE_SIZE: usize = 10;
/// How long a returned bundle's buttons stay disabled after a failed accept
pub(super) const RETURNED_BUNDLE_COOLDOWN_MS: u32 = 4_000;

#[derive(Clone, PartialEq)]
pub(super) struct ReviewBundle {
    pub(super) key: usize,
    pub(super) items: Vec<ArchiveReviewItem>,
    /// Sender shared by every item, when the bundle is a per-sender group —
    /// scanning one name clears the whole category
    pub(super) sender: Option<String>,
    /// Came back after a failed accept
    pub(super) returned: bool,
    /// Server error from the failed accept, shown on the card
    pub(super) error: Option<String>,
}

/// Group proposals by sender: senders with multiple proposals get their own
/// bundle (largest first — one click clears a whole newsletter), and
/// one-off senders pool into assorted bundles of [`REVIEW_BUNDLE_SIZE`].
pub(super) fn bundle_by_sender(items: Vec<ArchiveReviewItem>) -> Vec<ReviewBundle> {
    let mut by_sender: std::collections::HashMap<String, Vec<ArchiveReviewItem>> =
        std::collections::HashMap::new();
    for item in items {
        by_sender
            .entry(item.from_address.clone())
            .or_default()
            .push(item);
    }

    let mut groups: Vec<(String, Vec<ArchiveReviewItem>)> = by_sender.into_iter().collect();
    groups.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then_with(|| a.0.cmp(&b.0)));

    let mut bundles = Vec::new();
    let mut singles: Vec<ArchiveReviewItem> = Vec::new();
    for (sender, group) in groups {
        if group.len() == 1 {
            singles.extend(group);
        } else {
            bundles.push(ReviewBundle {
                key: next_bundle_key(),
                items: group,
                sender: Some(sender),
                returned: false,
                error: None,
            });
        }
    }
    for chunk in singles.chunks(REVIEW_BUNDLE_SIZE) {
        bundles.push(ReviewBundle {
            key: next_bundle_key(),
            items: chunk.to_vec(),
            sender: None,
            returned: false,
            error: None,
        });
    }
    bundles
}

#[derive(PartialEq, Default)]
pub(super) struct ReviewQueue {
    pub(super) bundles: Vec<ReviewBundle>,
    /// Bundle keys whose buttons are in the post-failure cooldown
    pub(super) disabled: std::collections::HashSet<usize>,
    /// Unsubscribe progress/outcome per bundle key ("..." while in flight)
    pub(super) unsub: std::collections::HashMap<usize, String>,
    pub(super) archive_mode: String,
    pub(super) loaded: bool,
    pub(super) banner: Option<String>,
}

pub(super) enum ReviewMsg {
    Loaded {
        mode: String,
        bundles: Vec<ReviewBundle>,
    },
    LoadFailed(String),
    /// Optimistic accept: the front bundle disappears immediately
    PopFront,
    /// Failed accept: items rejoin at the back of the queue, disabled for a
    /// cooldown so the rapid-click flow isn't interrupted by a dead retry
    ReturnBundle {
        key: usize,
        items: Vec<ArchiveReviewItem>,
        error: String,
    },
    CooldownOver(usize),
    /// Per-item keep (optimistic)
    RemoveItem {
        bundle_key: usize,
        decision_id: Uuid,
    },
    /// Drop a permanently-failing returned bundle without a backend call
    DismissBundle(usize),
    Banner(Option<String>),
    /// Unsubscribe progress/outcome for a sender bundle
    UnsubStatus {
        key: usize,
        label: String,
    },
}

impl Reducible for ReviewQueue {
    type Action = ReviewMsg;

    fn reduce(self: std::rc::Rc<Self>, msg: ReviewMsg) -> std::rc::Rc<Self> {
        let mut next = ReviewQueue {
            bundles: self.bundles.clone(),
            disabled: self.disabled.clone(),
            unsub: self.unsub.clone(),
            archive_mode: self.archive_mode.clone(),
            loaded: self.loaded,
            banner: self.banner.clone(),
        };
        match msg {
            ReviewMsg::Loaded { mode, bundles } => {
                next.archive_mode = mode;
                next.bundles = bundles;
                next.disabled.clear();
                next.loaded = true;
                next.banner = None;
            }
            ReviewMsg::LoadFailed(e) => {
                next.loaded = true;
                next.banner = Some(e);
            }
            ReviewMsg::PopFront => {
                if !next.bundles.is_empty() {
                    next.bundles.remove(0);
                }
            }
            ReviewMsg::ReturnBundle { key, items, error } => {
                next.disabled.insert(key);
                next.banner = Some(format!(
                    "{} email(s) couldn't be archived - returned to the back of the queue",
                    items.len()
                ));
                let sender = items
                    .first()
                    .map(|i| i.from_address.clone())
                    .filter(|s| items.iter().all(|i| &i.from_address == s));
                next.bundles.push(ReviewBundle {
                    key,
                    items,
                    sender,
                    returned: true,
                    error: Some(error),
                });
            }
            ReviewMsg::CooldownOver(key) => {
                next.disabled.remove(&key);
            }
            ReviewMsg::RemoveItem {
                bundle_key,
                decision_id,
            } => {
                for b in &mut next.bundles {
                    if b.key == bundle_key {
                        b.items.retain(|i| i.decision_id != decision_id);
                    }
                }
                next.bundles.retain(|b| !b.items.is_empty());
            }
            ReviewMsg::DismissBundle(key) => {
                next.bundles.retain(|b| b.key != key);
                next.disabled.remove(&key);
            }
            ReviewMsg::Banner(b) => next.banner = b,
            ReviewMsg::UnsubStatus { key, label } => {
                next.unsub.insert(key, label);
            }
        }
        next.into()
    }
}

#[cfg(test)]
mod tests {
    use super::{bundle_by_sender, next_bundle_key, ReviewMsg, ReviewQueue};
    use chrono::DateTime;
    use shared_types::ArchiveReviewItem;
    use std::rc::Rc;
    use uuid::Uuid;
    use yew::Reducible;

    fn item(id: u128, sender: &str) -> ArchiveReviewItem {
        let received_at = DateTime::from_timestamp(1_700_000_000, 0).unwrap();
        ArchiveReviewItem {
            decision_id: Uuid::from_u128(id),
            email_id: Uuid::from_u128(id + 100),
            status: "proposed".into(),
            confidence: 0.9,
            reasoning: "Newsletter".into(),
            decided_at: received_at,
            account_email: "reader@example.com".into(),
            gmail_link: format!("https://mail.google.com/mail/u/reader@example.com/#all/{id}"),
            subject: format!("Message {id}"),
            from_address: sender.into(),
            from_name: None,
            received_at,
            snippet: None,
            body_preview: None,
        }
    }

    #[test]
    fn sender_bundles_and_singletons_preserve_every_proposal() {
        let mut items = vec![item(1, "news@example.com"), item(2, "news@example.com")];
        items.extend((3..=14).map(|id| item(id, &format!("sender{id}@example.com"))));
        let bundles = bundle_by_sender(items);

        assert_eq!(bundles.len(), 3);
        assert_eq!(bundles[0].sender.as_deref(), Some("news@example.com"));
        assert_eq!(bundles[0].items.len(), 2);
        assert_eq!(bundles[1].items.len(), 10);
        assert_eq!(bundles[2].items.len(), 2);
        assert!(bundles[1..].iter().all(|bundle| bundle.sender.is_none()));
        let mut ids: Vec<_> = bundles
            .iter()
            .flat_map(|bundle| bundle.items.iter().map(|item| item.decision_id))
            .collect();
        ids.sort();
        assert_eq!(ids, (1..=14).map(Uuid::from_u128).collect::<Vec<_>>());
    }

    #[test]
    fn failed_accept_returns_behind_remaining_work_until_cooldown_ends() {
        let bundles = bundle_by_sender(vec![
            item(1, "news@example.com"),
            item(2, "news@example.com"),
            item(3, "other@example.com"),
        ]);
        let failed_items = bundles[0].items.clone();
        let remaining_key = bundles[1].key;
        let queue = Rc::new(ReviewQueue::default()).reduce(ReviewMsg::Loaded {
            mode: "review".into(),
            bundles,
        });
        let queue = queue.reduce(ReviewMsg::PopFront);
        assert_eq!(queue.bundles.len(), 1);

        let returned_key = next_bundle_key();
        let queue = queue.reduce(ReviewMsg::ReturnBundle {
            key: returned_key,
            items: failed_items.clone(),
            error: "Gmail unavailable".into(),
        });
        assert_eq!(queue.bundles[0].key, remaining_key);
        let returned = &queue.bundles[1];
        assert_eq!(returned.items, failed_items);
        assert!(returned.returned);
        assert_eq!(returned.error.as_deref(), Some("Gmail unavailable"));
        assert!(queue.disabled.contains(&returned_key));

        let queue = queue.reduce(ReviewMsg::CooldownOver(returned_key));
        assert!(!queue.disabled.contains(&returned_key));
        assert_eq!(queue.bundles.len(), 2);
        assert_eq!(queue.bundles[0].key, remaining_key);
    }
}
