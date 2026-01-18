use crate::db::{self, DbPool, Email};
use shared_types::{
    build_todo_action_from_rule, AgentRule, EmailMatchInput, ProposedTodoAction, ReasoningDetails,
    RuleEngine, RuleMatchResult,
};
use uuid::Uuid;

/// Convert a database Email to EmailMatchInput for rule matching
pub fn email_to_match_input(email: &Email) -> EmailMatchInput {
    EmailMatchInput {
        from_address: email.from_address.clone(),
        from_name: email.from_name.clone(),
        subject: email.subject.clone(),
        body_text: email.body_text.clone(),
        snippet: email.snippet.clone(),
        labels: email
            .labels
            .as_ref()
            .map(|l| l.iter().filter_map(|s| s.clone()).collect())
            .unwrap_or_default(),
        to_addresses: email
            .to_addresses
            .iter()
            .filter_map(|s| s.clone())
            .collect(),
        cc_addresses: email
            .cc_addresses
            .as_ref()
            .map(|l| l.iter().filter_map(|s| s.clone()).collect())
            .unwrap_or_default(),
    }
}

/// Result of processing an email through the rule engine
#[derive(Debug)]
pub struct EmailProcessingResult {
    pub email_id: Uuid,
    pub gmail_id: String,
    pub decision_type: String,
    pub proposed_action: ProposedTodoAction,
    pub reasoning: String,
    pub reasoning_details: ReasoningDetails,
    pub confidence: f32,
    pub matched_rule: Option<RuleMatchResult>,
    pub auto_execute: bool,
}

/// Process an email through the rule engine
///
/// Returns a processing result with:
/// - The matched rule (if any)
/// - The proposed action
/// - Whether to auto-execute or propose for user review
pub fn process_email_with_rules(
    email: &Email,
    rules: &[AgentRule],
) -> Option<EmailProcessingResult> {
    let input = email_to_match_input(email);

    // Try to match against rules first
    if let Some(rule_match) = RuleEngine::get_best_match(rules, &input) {
        // Rule matched - determine action based on rule
        let proposed_action = build_todo_action_from_rule(&rule_match.action_params, &input);

        let reasoning = format!(
            "Matched rule '{}': {}",
            rule_match.rule_name,
            rule_match.matched_clauses.join(", ")
        );

        let reasoning_details = ReasoningDetails {
            matched_keywords: Some(rule_match.matched_clauses.clone()),
            detected_deadline: None,
            sender_frequency: None,
            thread_length: None,
            heuristic_score: None,
            llm_analysis: None,
        };

        return Some(EmailProcessingResult {
            email_id: email.id,
            gmail_id: email.gmail_id.clone(),
            decision_type: rule_match.action.clone(),
            proposed_action,
            reasoning,
            reasoning_details,
            confidence: 1.0, // Rules are high confidence
            matched_rule: Some(rule_match),
            auto_execute: true, // Rules auto-execute
        });
    }

    // No rule matched - fall back to heuristic detection
    process_email_with_heuristics(email, &input)
}

/// Actionable keywords to detect in emails
const ACTIONABLE_KEYWORDS: &[&str] = &[
    "todo",
    "action required",
    "action needed",
    "please review",
    "urgent",
    "asap",
    "deadline",
    "reminder",
    "follow up",
    "followup",
    "respond",
    "reply needed",
    "awaiting your",
    "your input",
    "by end of day",
    "eod",
    "by friday",
    "by monday",
];

/// Process an email using keyword heuristics (fallback when no rules match)
fn process_email_with_heuristics(
    email: &Email,
    input: &EmailMatchInput,
) -> Option<EmailProcessingResult> {
    let subject_lower = input.subject.to_lowercase();
    let body_lower = input
        .body_text
        .as_ref()
        .map(|b| b.to_lowercase())
        .unwrap_or_default();
    let content = format!("{} {}", subject_lower, body_lower);

    // Find matching keywords
    let matched_keywords: Vec<String> = ACTIONABLE_KEYWORDS
        .iter()
        .filter(|kw| content.contains(*kw))
        .map(|s| s.to_string())
        .collect();

    if matched_keywords.is_empty() {
        return None;
    }

    // Calculate confidence based on number of keywords and their location
    let subject_matches = ACTIONABLE_KEYWORDS
        .iter()
        .filter(|kw| subject_lower.contains(*kw))
        .count();
    let confidence = match (subject_matches, matched_keywords.len()) {
        (s, _) if s >= 2 => 0.9,
        (1, t) if t >= 2 => 0.8,
        (1, _) => 0.7,
        (0, t) if t >= 3 => 0.6,
        (0, t) if t >= 2 => 0.5,
        _ => 0.4,
    };

    let proposed_action = build_todo_action_from_rule(&None, input);

    let reasoning = format!(
        "Detected actionable keywords in email: {}",
        matched_keywords.join(", ")
    );

    let reasoning_details = ReasoningDetails {
        matched_keywords: Some(matched_keywords),
        detected_deadline: None,
        sender_frequency: None,
        thread_length: None,
        heuristic_score: Some(confidence),
        llm_analysis: None,
    };

    Some(EmailProcessingResult {
        email_id: email.id,
        gmail_id: email.gmail_id.clone(),
        decision_type: "create_todo".to_string(),
        proposed_action,
        reasoning,
        reasoning_details,
        confidence,
        matched_rule: None,
        auto_execute: false, // Heuristics require user review
    })
}

/// Process unprocessed emails, creating decisions and optionally executing them
pub async fn process_pending_emails(pool: &DbPool, limit: i64) -> anyhow::Result<ProcessingStats> {
    let mut conn = pool.get().await?;

    // Get active rules once for all emails
    let rules = db::get_active_email_rules(&mut conn).await?;
    tracing::debug!("Loaded {} active email rules", rules.len());

    // Get unprocessed emails
    let emails = db::get_unprocessed_emails(&mut conn, limit).await?;
    tracing::info!("Processing {} unprocessed emails", emails.len());

    let mut stats = ProcessingStats::default();

    for email in emails {
        match process_single_email(&mut conn, &email, &rules).await {
            Ok(result) => {
                stats.processed += 1;
                match result {
                    ProcessedEmailOutcome::RuleMatched => stats.rule_matched += 1,
                    ProcessedEmailOutcome::HeuristicProposed => stats.heuristic_proposed += 1,
                    ProcessedEmailOutcome::Ignored => stats.ignored += 1,
                }
            }
            Err(e) => {
                stats.errors += 1;
                tracing::error!("Failed to process email {}: {}", email.id, e);
            }
        }
    }

    Ok(stats)
}

#[derive(Debug, Default)]
pub struct ProcessingStats {
    pub processed: usize,
    pub rule_matched: usize,
    pub heuristic_proposed: usize,
    pub ignored: usize,
    pub errors: usize,
}

#[derive(Debug)]
enum ProcessedEmailOutcome {
    RuleMatched,
    HeuristicProposed,
    Ignored,
}

/// Process a single email
async fn process_single_email(
    conn: &mut diesel_async::AsyncPgConnection,
    email: &Email,
    rules: &[AgentRule],
) -> anyhow::Result<ProcessedEmailOutcome> {
    let result = process_email_with_rules(email, rules);

    let outcome = match result {
        Some(processing_result) => {
            // Serialize proposed action and reasoning details
            let proposed_action_json = serde_json::to_string(&processing_result.proposed_action)?;
            let reasoning_details_json =
                serde_json::to_string(&processing_result.reasoning_details)?;

            // Determine status based on whether to auto-execute
            let (status, applied_rule_id) = if processing_result.auto_execute {
                if let Some(ref rule_match) = processing_result.matched_rule {
                    ("auto_approved", Some(rule_match.rule_id))
                } else {
                    ("proposed", None)
                }
            } else {
                ("proposed", None)
            };

            // Create the decision
            let decision_id = db::create_decision(
                conn,
                "email",
                Some(email.id),
                Some(&email.gmail_id),
                &processing_result.decision_type,
                &proposed_action_json,
                &processing_result.reasoning,
                Some(&reasoning_details_json),
                processing_result.confidence,
                status,
                applied_rule_id,
            )
            .await?;

            tracing::info!(
                "Created decision {} for email {} (status: {}, action: {})",
                decision_id,
                email.gmail_id,
                status,
                processing_result.decision_type
            );

            // If rule matched, increment match count
            if let Some(ref rule_match) = processing_result.matched_rule {
                db::increment_rule_match_count(conn, rule_match.rule_id).await?;
            }

            // If auto-approved and action is create_todo, create the todo immediately
            if status == "auto_approved" && processing_result.decision_type == "create_todo" {
                let todo_id = db::create_todo_from_decision(
                    conn,
                    decision_id,
                    &processing_result.proposed_action.todo_title,
                    processing_result
                        .proposed_action
                        .todo_description
                        .as_deref(),
                    "email",
                    Some(&email.gmail_id),
                    processing_result.proposed_action.due_date,
                    processing_result.proposed_action.category_id,
                )
                .await?;

                db::update_decision_result_todo(conn, decision_id, todo_id).await?;

                tracing::info!(
                    "Auto-created todo {} from decision {} for email {}",
                    todo_id,
                    decision_id,
                    email.gmail_id
                );
            }

            if processing_result.matched_rule.is_some() {
                ProcessedEmailOutcome::RuleMatched
            } else {
                ProcessedEmailOutcome::HeuristicProposed
            }
        }
        None => {
            tracing::debug!(
                "Email {} not actionable, marking as processed",
                email.gmail_id
            );
            ProcessedEmailOutcome::Ignored
        }
    };

    // Mark email as processed
    db::mark_email_processed(conn, email.id).await?;

    Ok(outcome)
}
