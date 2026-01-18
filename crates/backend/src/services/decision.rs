//! Decision approval and execution service.
//!
//! Extracts business logic from HTTP handlers for better testability and reuse.

use crate::db::{decisions, todos, DbPool};
use anyhow::{Context, Result};
use diesel_async::AsyncPgConnection;
use shared_types::{AgentDecision, ProposedTodoAction};
use uuid::Uuid;

/// Result of approving a decision
pub struct ApprovalResult {
    pub decision: AgentDecision,
    #[allow(dead_code)]
    pub created_todo_id: Option<Uuid>,
}

/// Result of a batch operation
pub struct BatchResult {
    pub successful: Vec<Uuid>,
    pub failed: Vec<BatchFailure>,
}

pub struct BatchFailure {
    pub id: Uuid,
    pub error: String,
}

/// Service for decision-related business logic
pub struct DecisionService;

impl DecisionService {
    /// Approve a decision, optionally with modifications to the proposed action
    pub async fn approve(
        conn: &mut AsyncPgConnection,
        decision_id: Uuid,
        modifications: Option<ProposedTodoAction>,
    ) -> Result<ApprovalResult> {
        // Get the decision
        let decision = decisions::get_by_id(conn, decision_id)
            .await
            .context("Decision not found")?;

        // Create todo if decision type is create_todo
        let created_todo_id = if decision.decision_type == "create_todo" {
            let action = Self::get_action(&decision, modifications)?;
            let todo = todos::create(
                conn,
                &action.todo_title,
                action.todo_description.as_deref(),
                action.due_date,
                None,
                action.category_id,
            )
            .await
            .context("Failed to create todo")?;
            Some(todo.id)
        } else {
            None
        };

        // Update decision status
        decisions::approve(conn, decision_id, created_todo_id)
            .await
            .context("Failed to approve decision")?;

        // Mark as executed if todo was created
        let final_decision = if created_todo_id.is_some() {
            decisions::mark_executed(conn, decision_id)
                .await
                .context("Failed to mark as executed")?
        } else {
            decisions::get_by_id(conn, decision_id).await?
        };

        Ok(ApprovalResult {
            decision: final_decision,
            created_todo_id,
        })
    }

    /// Reject a decision with optional feedback
    pub async fn reject(
        conn: &mut AsyncPgConnection,
        decision_id: Uuid,
        feedback: Option<&str>,
    ) -> Result<AgentDecision> {
        decisions::reject(conn, decision_id, feedback)
            .await
            .context("Failed to reject decision")
    }

    /// Approve multiple decisions in batch
    pub async fn batch_approve(pool: &DbPool, decision_ids: Vec<Uuid>) -> Result<BatchResult> {
        let mut conn = pool.get().await.context("Failed to get connection")?;
        let mut successful = Vec::new();
        let mut failed = Vec::new();

        for decision_id in decision_ids {
            match Self::approve(&mut conn, decision_id, None).await {
                Ok(result) => successful.push(result.decision.id),
                Err(e) => failed.push(BatchFailure {
                    id: decision_id,
                    error: e.to_string(),
                }),
            }
        }

        Ok(BatchResult { successful, failed })
    }

    /// Reject multiple decisions in batch
    pub async fn batch_reject(
        pool: &DbPool,
        decision_ids: Vec<Uuid>,
        feedback: Option<&str>,
    ) -> Result<BatchResult> {
        let mut conn = pool.get().await.context("Failed to get connection")?;
        let mut successful = Vec::new();
        let mut failed = Vec::new();

        for decision_id in decision_ids {
            match Self::reject(&mut conn, decision_id, feedback).await {
                Ok(decision) => successful.push(decision.id),
                Err(e) => failed.push(BatchFailure {
                    id: decision_id,
                    error: e.to_string(),
                }),
            }
        }

        Ok(BatchResult { successful, failed })
    }

    /// Extract action from decision, using modifications if provided
    fn get_action(
        decision: &AgentDecision,
        modifications: Option<ProposedTodoAction>,
    ) -> Result<ProposedTodoAction> {
        match modifications {
            Some(action) => Ok(action),
            None => serde_json::from_str(&decision.proposed_action)
                .context("Failed to parse proposed_action"),
        }
    }
}
