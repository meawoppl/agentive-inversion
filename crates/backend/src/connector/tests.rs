use super::auth::{hash, Principal};
#[test]
fn connection_cannot_approve_its_own_effects() {
    let p = Principal {
        email: "test@example.com".into(),
        connection_id: Some(uuid::Uuid::new_v4()),
        account_ids: vec![],
        scopes: vec!["actions:propose".into()],
    };
    assert!(p.admin().is_err());
    assert!(p.scope("actions:propose").is_ok());
    assert!(p.scope("legacy:export").is_err());
    assert!(p.account(uuid::Uuid::new_v4()).is_err());
}
#[test]
fn stored_token_digest_is_deterministic_and_not_the_secret() {
    assert_eq!(hash("secret"), hash("secret"));
    assert_ne!(hash("secret"), hash("different"));
    assert_eq!(hash("secret").len(), 64);
}

/// Run only against an isolated disposable database. The normal unit suite
/// remains database-free; CI runs this test in a dedicated Postgres service.
#[tokio::test]
#[ignore = "requires isolated TEST_DATABASE_URL"]
async fn connector_database_contract() {
    use axum::{
        body::{to_bytes, Body},
        http::{Request, StatusCode},
    };
    use diesel::{
        sql_query,
        sql_types::{Text, Uuid as SqlUuid},
        Connection,
    };
    use diesel_async::{
        pooled_connection::{deadpool::Pool, AsyncDieselConnectionManager},
        RunQueryDsl,
    };
    use diesel_migrations::MigrationHarness;
    use serde_json::{json, Value};
    use std::sync::Arc;
    use tower::ServiceExt;
    use uuid::Uuid;
    let url = std::env::var("TEST_DATABASE_URL").expect("isolated TEST_DATABASE_URL required");
    let mut migration_conn = diesel::PgConnection::establish(&url).unwrap();
    migration_conn
        .run_pending_migrations(crate::db::MIGRATIONS)
        .unwrap();
    let pool = Pool::builder(AsyncDieselConnectionManager::new(url))
        .build()
        .unwrap();
    let cfg = crate::auth::types::AuthConfig {
        jwt_secret: "test-connector-secret-not-production".into(),
        allowed_emails: vec!["owner@example.com".into()],
        token_duration_days: 7,
        cookie_name: "auth_token".into(),
        google_client_id: "test".into(),
        google_client_secret: "test".into(),
        auth_redirect_uri: "http://localhost:3000/api/auth/callback".into(),
    };
    let admin = crate::auth::jwt::create_token(&cfg, "owner@example.com", None).unwrap();
    let state = crate::AppState {
        pool: pool.clone(),
        auth_config: Arc::new(cfg),
        triage_health: Arc::new(tokio::sync::RwLock::new(Default::default())),
    };
    let app = crate::build_app(
        state.clone(),
        &crate::config::Config {
            host: "127.0.0.1".into(),
            port: 3000,
            cors_allowed_origins: vec![],
        },
    );
    let mut c = pool.get().await.unwrap();
    sql_query("TRUNCATE connector_actions,connector_claims,connector_pairings,connector_connections,emails,google_accounts CASCADE").execute(&mut c).await.unwrap();
    let account = Uuid::new_v4();
    let other = Uuid::new_v4();
    for (id, email) in [(account, "owner@example.com"), (other, "other@example.com")] {
        sql_query("INSERT INTO google_accounts(id,email,refresh_token) VALUES($1,$2,'fake')")
            .bind::<SqlUuid, _>(id)
            .bind::<Text, _>(email)
            .execute(&mut c)
            .await
            .unwrap();
    }
    let email = Uuid::new_v4();
    let hidden = Uuid::new_v4();
    for (id, aid) in [(email, account), (hidden, other)] {
        sql_query("INSERT INTO emails(id,account_id,gmail_id,thread_id,subject,from_address,to_addresses,body_text,received_at) VALUES($1,$2,$3,'thread','Test','sender@example.com',ARRAY['owner@example.com'],'Private body',now())").bind::<SqlUuid,_>(id).bind::<SqlUuid,_>(aid).bind::<Text,_>(id.to_string()).execute(&mut c).await.unwrap();
    }
    drop(c);
    async fn call(
        app: &axum::Router,
        method: &str,
        path: &str,
        token: Option<&str>,
        admin: Option<&str>,
        body: Value,
        key: Option<&str>,
    ) -> (StatusCode, Value) {
        let mut b = Request::builder()
            .method(method)
            .uri(path)
            .header("content-type", "application/json");
        if let Some(t) = token {
            b = b.header("authorization", format!("Bearer {t}"));
        }
        if let Some(a) = admin {
            b = b
                .header("cookie", format!("auth_token={a}"))
                .header("origin", "http://localhost:3000")
                .header("x-agentive-request", "1");
        }
        if let Some(k) = key {
            b = b.header("idempotency-key", k);
        }
        let response = app
            .clone()
            .oneshot(b.body(Body::from(body.to_string())).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let bytes = to_bytes(response.into_body(), 2_000_000).await.unwrap();
        (
            status,
            serde_json::from_slice(&bytes)
                .unwrap_or_else(|_| json!({"raw":String::from_utf8_lossy(&bytes)})),
        )
    }
    let (status, _) = call(
        &app,
        "GET",
        "/api/v1/capabilities",
        None,
        None,
        json!(null),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    async fn pair(
        app: &axum::Router,
        admin: &str,
        account: Uuid,
        review: bool,
    ) -> (String, String) {
        let (status, start) = call(
            app,
            "POST",
            "/api/v1/pairings",
            None,
            None,
            json!({"name":"contract-test","review":review}),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{start}");
        let id = start["id"].as_str().unwrap();
        let secret = start["device_secret"].as_str().unwrap();
        let (_, pending) = call(
            app,
            "POST",
            &format!("/api/v1/pairings/{id}/poll"),
            None,
            None,
            json!({"device_secret":secret}),
            None,
        )
        .await;
        assert_eq!(pending["status"], "pending");
        let (s, grant) = call(
            app,
            "POST",
            &format!("/api/v1/pairings/{id}/approve"),
            None,
            Some(admin),
            json!({"account_ids":[account]}),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{grant}");
        let (s, ready) = call(
            app,
            "POST",
            &format!("/api/v1/pairings/{id}/poll"),
            None,
            None,
            json!({"device_secret":secret}),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{ready}");
        (
            ready["token"].as_str().unwrap().into(),
            grant["id"].as_str().unwrap().into(),
        )
    }
    let (agent, agent_id) = pair(&app, &admin, account, false).await;
    let (reviewer, _) = pair(&app, &admin, account, true).await;
    let (s, source) = call(
        &app,
        "GET",
        &format!("/api/v1/sources/{email}?body=true"),
        Some(&agent),
        None,
        json!(null),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{source}");
    assert_eq!(source["body_text"], "Private body");
    let (s, _) = call(
        &app,
        "GET",
        &format!("/api/v1/sources/{hidden}"),
        Some(&agent),
        None,
        json!(null),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let a = call(
        &app,
        "POST",
        "/api/v1/work/claims",
        Some(&agent),
        None,
        json!({"limit":1}),
        None,
    );
    let b = call(
        &app,
        "POST",
        "/api/v1/work/claims",
        Some(&agent),
        None,
        json!({"limit":1}),
        None,
    );
    let (a, b) = tokio::join!(a, b);
    assert_eq!(a.0, StatusCode::OK, "{}", a.1);
    assert_eq!(b.0, StatusCode::OK, "{}", b.1);
    assert_eq!(
        a.1["items"].as_array().unwrap().len() + b.1["items"].as_array().unwrap().len(),
        1
    );
    let claim = if a.1["items"].as_array().unwrap().is_empty() {
        b.1
    } else {
        a.1
    };
    let expired_cid = claim["claim_id"].as_str().unwrap();
    let mut c = pool.get().await.unwrap();
    sql_query("UPDATE connector_claims SET expires_at=now()-interval '1 second'")
        .execute(&mut c)
        .await
        .unwrap();
    drop(c);
    let (s, claim) = call(
        &app,
        "POST",
        "/api/v1/work/claims",
        Some(&agent),
        None,
        json!({"limit":1}),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let cid = claim["claim_id"].as_str().unwrap();
    assert_ne!(cid, expired_cid);
    let stale = json!({"email_id":email,"disposition":"kept","repository_commit":"a".repeat(40),"repository_path":"tasks/test.json"});
    let (s, _) = call(
        &app,
        "POST",
        &format!("/api/v1/work/claims/{expired_cid}/complete"),
        Some(&agent),
        None,
        stale,
        None,
    )
    .await;
    assert_eq!(s, StatusCode::CONFLICT);
    let (s, _) = call(
        &app,
        "POST",
        "/api/v1/work/claims",
        Some(&reviewer),
        None,
        json!({"limit":1}),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let request = json!({"email_id":email,"claim_id":cid,"reason":"Recorded in Git","repository_commit":"a".repeat(40),"repository_path":"tasks/test.json","effect":{"type":"archive"}});
    let (s, action) = call(
        &app,
        "POST",
        "/api/v1/actions",
        Some(&agent),
        None,
        request.clone(),
        Some("test-key"),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{action}");
    let id = action["id"].as_str().unwrap();
    let (s, replay) = call(
        &app,
        "POST",
        "/api/v1/actions",
        Some(&agent),
        None,
        request.clone(),
        Some("test-key"),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(replay["id"], action["id"]);
    let mut altered = request.clone();
    altered["reason"] = json!("Changed");
    let (s, _) = call(
        &app,
        "POST",
        "/api/v1/actions",
        Some(&agent),
        None,
        altered,
        Some("test-key"),
    )
    .await;
    assert_eq!(s, StatusCode::CONFLICT);
    let (s, _) = call(
        &app,
        "POST",
        &format!("/api/v1/actions/{id}/approve"),
        Some(&agent),
        None,
        json!(null),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (s, _) = call(
        &app,
        "POST",
        &format!("/api/v1/actions/{id}/approve"),
        None,
        Some(&admin),
        json!(null),
        None,
    )
    .await;
    assert_eq!(
        s,
        StatusCode::FORBIDDEN,
        "Website cookie must not approve provider actions"
    );
    let (s, list) = call(
        &app,
        "GET",
        "/api/v1/actions?status=proposed",
        Some(&reviewer),
        None,
        json!(null),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{list}");
    assert_eq!(list.as_array().unwrap().len(), 1);
    let (s, _) = call(
        &app,
        "POST",
        &format!("/api/v1/actions/{id}/approve"),
        Some(&reviewer),
        None,
        json!(null),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::ACCEPTED);
    let completion = json!({"email_id":email,"disposition":"recorded","repository_commit":"a".repeat(40),"repository_path":"tasks/test.json"});
    for _ in 0..2 {
        let (s, r) = call(
            &app,
            "POST",
            &format!("/api/v1/work/claims/{cid}/complete"),
            Some(&agent),
            None,
            completion.clone(),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::NO_CONTENT, "{r}");
    }
    let (s, _) = call(
        &app,
        "POST",
        "/api/v1/actions",
        Some(&agent),
        None,
        request,
        Some("different-key"),
    )
    .await;
    assert_eq!(s, StatusCode::CONFLICT);
    let (s, _) = call(
        &app,
        "POST",
        &format!("/api/v1/connections/{agent_id}/revoke"),
        None,
        Some(&admin),
        json!(null),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::NO_CONTENT);
    let (s, _) = call(
        &app,
        "GET",
        "/api/v1/capabilities",
        Some(&agent),
        None,
        json!(null),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    // Revoking a proposal's authority cancels queued effects before any provider call.
    super::actions::tick(&state).await.unwrap();
    let (s, action) = call(
        &app,
        "GET",
        &format!("/api/v1/actions/{id}"),
        Some(&reviewer),
        None,
        json!(null),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(action["status"], "cancelled");
    let mut c = pool.get().await.unwrap();
    sql_query(
        "UPDATE connector_actions SET status='running',updated_at=now()-interval '11 minutes'",
    )
    .execute(&mut c)
    .await
    .unwrap();
    drop(c);
    super::actions::tick(&state).await.unwrap();
    let (_, action) = call(
        &app,
        "GET",
        &format!("/api/v1/actions/{id}"),
        Some(&reviewer),
        None,
        json!(null),
        None,
    )
    .await;
    assert_eq!(action["status"], "unknown");
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/connections/{agent_id}/revoke"))
                .header("cookie", format!("auth_token={admin}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        StatusCode::FORBIDDEN,
        "Cookie mutations require same-origin CSRF protection"
    );
    let (s, _) = call(
        &app,
        "POST",
        "/api/todos",
        None,
        Some(&admin),
        json!({"title":"Must not create a second authority"}),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::GONE);
}
