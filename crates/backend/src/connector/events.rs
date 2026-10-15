//! Scoped invalidation hints; clients refetch authorized REST snapshots.
use super::auth::{authenticate, Principal};
use crate::{error::ApiResult, AppState};
use axum::{
    extract::{
        ws::{Message, WebSocketUpgrade},
        State,
    },
    http::{HeaderMap, Method},
    response::Response,
    Extension,
};
use tokio::sync::broadcast;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct Change {
    accounts: Vec<Uuid>,
    connection: Option<Uuid>,
}
impl Change {
    fn visible_to(&self, p: &Principal) -> bool {
        self.accounts.iter().any(|id| p.account_ids.contains(id))
            && (self.connection.is_none()
                || p.scopes.iter().any(|s| s == "actions:approve")
                || self.connection == p.connection_id)
    }
}
#[derive(Clone)]
pub struct ReviewEvents(broadcast::Sender<Change>);
impl Default for ReviewEvents {
    fn default() -> Self {
        Self(broadcast::channel(256).0)
    }
}
impl ReviewEvents {
    pub fn publish(&self, accounts: Vec<Uuid>, connection: Option<Uuid>) {
        let _ = self.0.send(Change {
            accounts,
            connection,
        });
    }
    pub fn subscribe(&self) -> broadcast::Receiver<Change> {
        self.0.subscribe()
    }
}

pub async fn subscribe(
    State(state): State<AppState>,
    Extension(p): Extension<Principal>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> ApiResult<Response> {
    p.connection()?;
    p.scope("actions:propose")?;
    let mut changes = state.review_events.subscribe();
    Ok(ws
        .max_message_size(4096)
        .max_frame_size(4096)
        .on_upgrade(move |mut socket| async move {
            // Reconnect always invalidates snapshots, including changes missed while offline.
            if socket
                .send(Message::Text("{\"type\":\"invalidate\"}".into()))
                .await
                .is_err()
            {
                return;
            }
            let mut heartbeat = tokio::time::interval(std::time::Duration::from_secs(20));
            loop {
                let notify = tokio::select! {
                    message = socket.recv() => {
                        match message {
                            Some(Ok(Message::Ping(bytes))) => {
                                if socket.send(Message::Pong(bytes)).await.is_err() { break; }
                                continue;
                            }
                            Some(Ok(Message::Pong(_))) => continue,
                            _ => break,
                        }
                    }
                    change = changes.recv() => match change {
                        Ok(change) => change.visible_to(&p),
                        Err(broadcast::error::RecvError::Lagged(_)) => true,
                        Err(_) => break,
                    },
                    _ = heartbeat.tick() => false,
                };
                // A socket must not outlive revocation, expiry, or owner allowlisting.
                if authenticate(&state, headers.clone(), Method::GET)
                    .await
                    .is_err()
                {
                    break;
                }
                let message = if notify {
                    Message::Text("{\"type\":\"invalidate\"}".into())
                } else {
                    Message::Ping(Vec::new())
                };
                if socket.send(message).await.is_err() {
                    break;
                }
            }
            let _ = socket.close().await;
        }))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hints_follow_account_and_connection_scope() {
        let account = Uuid::new_v4();
        let connection = Uuid::new_v4();
        let mut p = Principal {
            email: "a@example.com".into(),
            account_ids: vec![account],
            connection_id: Some(connection),
            scopes: vec!["actions:propose".into()],
        };
        assert!(Change {
            accounts: vec![account],
            connection: Some(connection)
        }
        .visible_to(&p));
        assert!(!Change {
            accounts: vec![account],
            connection: Some(Uuid::new_v4())
        }
        .visible_to(&p));
        p.scopes.push("actions:approve".into());
        assert!(Change {
            accounts: vec![account],
            connection: Some(Uuid::new_v4())
        }
        .visible_to(&p));
        assert!(!Change {
            accounts: vec![Uuid::new_v4()],
            connection: Some(connection)
        }
        .visible_to(&p));
    }
}
