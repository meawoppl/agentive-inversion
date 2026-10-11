//! Hosted account and connection console. Review happens in the Portal plugin.
use gloo::storage::{SessionStorage, Storage};
use gloo_net::http::Request;
use shared_types::{
    connector::{ApprovePair, ConnectionInfo, PairInfo},
    GoogleAccountResponse,
};
use uuid::Uuid;
use yew::prelude::*;

#[derive(Clone, Default, PartialEq)]
struct Data {
    accounts: Vec<GoogleAccountResponse>,
    connections: Vec<ConnectionInfo>,
    pair: Option<PairInfo>,
}
async fn fetch<T: serde::de::DeserializeOwned>(path: &str) -> Result<T, String> {
    let r = Request::get(path).send().await.map_err(|e| e.to_string())?;
    if !r.ok() {
        return Err(format!(
            "{}: {}",
            r.status(),
            r.text().await.unwrap_or_default()
        ));
    }
    r.json().await.map_err(|e| e.to_string())
}
#[derive(Properties, PartialEq)]
pub struct ConsoleProps {
    pub email: String,
    pub on_logout: Callback<()>,
}
#[function_component(Console)]
pub fn console(props: &ConsoleProps) -> Html {
    let data = use_state(Data::default);
    let error = use_state(|| None::<String>);
    let selected = use_state(Vec::<Uuid>::new);
    let revision = use_state(|| 0u32);
    let busy = use_state(|| false);
    {
        let data = data.clone();
        let error = error.clone();
        use_effect_with(*revision, move |_| {
            wasm_bindgen_futures::spawn_local(async move {
                let result = async {
                    let accounts = fetch("/api/v1/accounts").await?;
                    let connections = fetch("/api/v1/connections").await?;
                    let pair = if let Ok(id) = SessionStorage::get::<String>("agentive-pairing") {
                        Some(fetch(&format!("/api/v1/pairings/{id}")).await?)
                    } else {
                        None
                    };
                    Ok::<_, String>(Data {
                        accounts,
                        connections,
                        pair,
                    })
                }
                .await;
                match result {
                    Ok(d) => {
                        data.set(d);
                        error.set(None);
                    }
                    Err(e) => error.set(Some(e)),
                }
            });
            || ()
        });
    }
    let approve = {
        let data = data.clone();
        let selected = selected.clone();
        let error = error.clone();
        let revision = revision.clone();
        let busy = busy.clone();
        Callback::from(move |_| {
            let Some(pair) = data.pair.as_ref() else {
                return;
            };
            let url = format!("/api/v1/pairings/{}/approve", pair.id);
            let input = ApprovePair {
                account_ids: (*selected).clone(),
            };
            let error = error.clone();
            let revision = revision.clone();
            let busy = busy.clone();
            busy.set(true);
            wasm_bindgen_futures::spawn_local(async move {
                let result = async {
                    let r = Request::post(&url)
                        .header("x-agentive-request", "1")
                        .json(&input)
                        .map_err(|e| e.to_string())?
                        .send()
                        .await
                        .map_err(|e| e.to_string())?;
                    if !r.ok() {
                        return Err(r.text().await.unwrap_or_default());
                    }
                    Ok::<_, String>(())
                }
                .await;
                match result {
                    Ok(()) => {
                        SessionStorage::delete("agentive-pairing");
                        revision.set(*revision + 1);
                    }
                    Err(e) => error.set(Some(e)),
                }
                busy.set(false);
            });
        })
    };
    let logout = props.on_logout.clone();
    let refresh = {
        let r = revision.clone();
        Callback::from(move |_| r.set(*r + 1))
    };
    html! {<div class="connector-console">
     <header class="connector-header"><div><p class="eyebrow">{"AGENTIVE CONNECTOR"}</p><h1>{"Your accounts. Your agents."}</h1><p>{"Google connections stay here. Project work and approvals live in Portal."}</p></div><button onclick={Callback::from(move |_|logout.emit(()))}>{"Sign out"}</button></header>
     <div class="connector-toolbar"><span>{&props.email}</span><button onclick={refresh}>{"Refresh status"}</button></div>
     if let Some(e)=&*error {<div role="alert" class="connector-error">{e}<button onclick={Callback::from(move |_|{SessionStorage::delete("agentive-pairing");if let Some(w)=web_sys::window(){let _=w.location().set_href("/");}})}>{"Dismiss pairing"}</button></div>}
     if let Some(pair)=&data.pair {<section class="connector-card"><p class="eyebrow">{"CONNECTION REQUEST"}</p><h2>{&pair.name}</h2><p>{if pair.review{"This is a Portal REVIEW connection. It can approve or reject proposals for the selected accounts. Keep this credential out of scheduled-agent environments."}else{"This is an AGENT connection. It can read mail, calendars and Google Docs, claim work, and propose actions. It cannot approve actions. It can also export the shared legacy project records for migration."}}</p><p>{"Only approve a request you started. Select the accounts this connection may access:"}</p>
      {for data.accounts.iter().map(|a|{let id=a.id;let s=selected.clone();let checked=s.contains(&id);html!{<label class="account-choice"><input type="checkbox" checked={checked} onchange={Callback::from(move |_|{let mut ids=(*s).clone();if ids.contains(&id){ids.retain(|x|*x!=id);}else{ids.push(id);}s.set(ids);})}/>{&a.email}</label>}})}
      <p class="muted">{"Expires after 90 days; revoke below at any time. Google tokens are never shared."}</p><button disabled={selected.is_empty()||*busy} onclick={approve}>{"Authorize connection"}</button>
     </section>}
     <section class="connector-card"><h2>{"Google accounts"}</h2><p>{"Mail sync continues independently of Portal sessions. To enable read-only Google Docs access for an existing account, use Connect or refresh and grant the added permission."}</p>{for data.accounts.iter().map(|a|html!{<article class="connector-row"><div><strong>{&a.email}</strong><p class="muted">{a.last_synced_at.map(|t|format!("Last synced {}",t.format("%Y-%m-%d %H:%M UTC"))).unwrap_or_else(||"Waiting for first sync".into())}</p></div><span>{if a.last_sync_error.is_some(){"Needs attention"}else{"Connected"}}</span></article>})}<button onclick={Callback::from(|_|{wasm_bindgen_futures::spawn_local(async{if let Ok(r)=Request::get("/api/auth/login").send().await{if let Ok(v)=r.json::<shared_types::LoginInitResponse>().await{if let Some(w)=web_sys::window(){let _=w.location().set_href(&v.auth_url);}}}});})}>{"Connect or refresh a Google account"}</button></section>
     <section class="connector-card"><h2>{"Portal connections"}</h2><p>{"Create connections using the workspace CLI. Use a separate review connection for approvals."}</p>
      {for data.connections.iter().map(|connection|{let id=connection.id;let r=revision.clone();let e=error.clone();let revoked=connection.revoked_at.is_some();let revoke=Callback::from(move |_|{let r=r.clone();let e=e.clone();wasm_bindgen_futures::spawn_local(async move{match Request::post(&format!("/api/v1/connections/{id}/revoke")).header("x-agentive-request","1").send().await{Ok(resp) if resp.ok()=>r.set(*r+1),_=>e.set(Some("Could not revoke connection".into()))}});});html!{<article class="connector-row"><div><strong>{&connection.name}</strong><p class="muted">{connection.scopes.join(" · ")}</p><p class="muted">{format!("Expires {}",connection.expires_at.format("%Y-%m-%d"))}</p></div><button onclick={revoke} disabled={revoked}>{if revoked{"Revoked"}else{"Revoke"}}</button></article>}})}
      if data.connections.is_empty(){<p class="muted">{"No Portal connections yet."}</p>}
     </section>
     <footer>{"Project notes, tasks, and review happen in your Git workspace through Agent Portal. Existing records remain available through the authenticated migration export."}</footer>
    </div>}
}
