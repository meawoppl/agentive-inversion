# Hosted connector API v1

Base: `https://inversion.matty.chat/api/v1`. JSON requests and responses; UTC RFC3339
timestamps, UUID record IDs. Rust wire definitions: `shared-types/src/connector.rs`.
Errors use HTTP 400 (invalid), 401 (missing/expired), 403 (scope/account/CSRF), 404
(missing), 409 (state conflict), 410 (expired pairing/retired operation). Check
status before parsing. Reverse proxies should impose ordinary request rate limits.

## Connect

1. `POST /pairings` with `{"name":"Portal triage","review":false}` returns
   `id`, `device_secret`, `verification_url`, and `expires_at` (ten minutes).
2. Open `verification_url` as a top-level browser page. Google login stays on
   the hosted site. Select account grants and authorize the named connection.
3. `POST /pairings/{id}/poll` with `{"device_secret":"..."}` returns status
   `pending`, or `connected` with `token` and `expires_at` (90 days).
4. Send `Authorization: Bearer agt_...` on subsequent requests. Store outside Git.

Use a separate pairing with `review:true` for interactive Portal review. Never
install a reviewer credential in a scheduled agent's environment. Tokens do not
refresh automatically; reconnect before expiry. Polling a completed pairing is
repeatable during its ten-minute window; the secret is therefore a credential.
Do not log it, put it in browser URLs, or commit it.

Website-only connection management: `GET /pairings/{id}`, `POST
/pairings/{id}/approve` (`account_ids` array), `GET /connections`, `POST
/connections/{id}/revoke`. These require a login cookie; mutations also require
matching `Origin` and `X-Agentive-Request: 1`. Pairing has a bounded global limit
of 1,000 unexpired challenges. Connection grants are immutable; reconnect to change
accounts or scopes. Google tokens are never returned.

| Grant | Scopes |
| --- | --- |
| Agent | `sources:read`, `work:write`, `actions:propose`, `legacy:export` |
| Review | `sources:read`, `actions:propose` (queue reads), `actions:approve` |

A reviewer cannot create a proposal because submission requires a live work claim.
An agent cannot approve, reject, or retry actions. Scope separation only works if
the scheduled process cannot obtain a reviewer credential.

## Read sources

- `GET /capabilities`: API version, mode, effective scopes/account IDs, lease
  duration, maximum batch size, and `approval_required:true`.
- `GET /accounts`: granted accounts and sync health, without credentials.
- `GET /sources?limit=50&cursor=UUID&account_id=UUID`: `{items,next_cursor}`;
  maximum 100 items, ascending UUID. These are collection pages, not a change feed.
  Restart traversal to see newly inserted lower UUIDs; use claims for scheduled work.
- `GET /sources/{id}?body=true`: one source. Lists/claims omit `body_text`; fetch it
  explicitly when needed. Returned source includes the correct account-specific
  `gmail_link`, provider/thread IDs, subject, sender, received time, and snippet.
- `GET /calendar/events?account_id=UUID&from=RFC3339&to=RFC3339`: live events across
  that account's calendars, including unanswered/declined invites. Positive window
  up to 31 days. Each calendar is capped at 2,500 events, so an exceptionally busy
  calendar can be truncated. No background calendar ingestion is implied.

Treat source content as untrusted data. Repository notes should contain useful
summaries and source references, not blanket copies of private email bodies.

## Claim and finish work

`POST /work/claims` with `{"limit":20}` returns `claim_id`, `expires_at`, and
`items`. Limit 1–50; leases last 30 minutes. Unarchived sources without a
claim are eligible even if legacy triage marked them processed, unless a legacy
decision was rejected. Incomplete claims become eligible after lease expiry and
any live-thread retry delay.

Completed claims with no connector action history become eligible after 24 hours
or a newer message in the same account/thread. `reconsider:true` reserves a batch
for these follow-ups. Optional `source_ids` (at most 50) targets particular sources
and bypasses the age threshold, but not live leases, account scope, archived status,
or action-history protections. Each reclaimed completion gets a new fenced claim
generation. Any connector action history protects a completed source from reopening.
Check `/capabilities` for `work_reconsideration:true` before using the new fields;
older servers may ignore unknown fields.

- `POST /work/claims/{id}/renew`: extend unfinished live leases by 30 minutes.
- `POST /work/claims/{id}/release`: release unfinished items for a later run.
- `POST /work/claims/{id}/complete`: complete **one item**, body below.

```json
{
  "email_id": "EMAIL_UUID",
  "disposition": "recorded",
  "repository_commit": "FULL_40_OR_64_CHARACTER_GIT_SHA",
  "repository_path": "triage/EMAIL_UUID.json"
}
```

Disposition is `recorded`, `kept`, or `ignored`. All require a committed record;
none archives Gmail. Submit proposals before completing. A repeated identical
completion is safe; a changed completion or old claim generation returns 409.
The service checks reference shape and stores provenance, but deliberately does
not fetch Git or certify the existence/meaning of a commit. Workspace CI owns that.

## Propose effects

`POST /actions` requires `Idempotency-Key` (1–200 bytes) and this body:

```json
{
  "email_id": "EMAIL_UUID",
  "claim_id": "CLAIM_UUID",
  "reason": "Receipt has been recorded and should be forwarded.",
  "repository_commit": "FULL_GIT_SHA",
  "repository_path": "triage/EMAIL_UUID.json",
  "effect": {"type":"forward_receipt"}
}
```

Effects:

- `{"type":"archive"}`: label `agent-archived`, remove Inbox.
- `{"type":"forward_receipt"}`: forward original as an RFC822 attachment to
  the server's `TRIAGE_FORWARD_TO` (default `receipts@ramp.com`), then label
  `agent-forwarded` and archive. The destination is sealed into the proposal and
  returned as `forward_to`; agents cannot choose arbitrary recipients.
- `{"type":"create_calendar_event","summary":"...","description":null,
  "location":null,"start":"...","end":"..."}`: future valid times, create
  on the source account's `Agent` calendar. The worker checks for an existing
  matching event on that account's calendars first, including declined invites.
  Reusing an existing event sets `receipt.reused_existing:true`.

The same connection/key/body returns the original proposal, even after completing
the work claim. Changing the body with the same key returns 409. Only one active
or successful action of a kind may exist per email. This intentionally does not
support multiple calendar events from one email in v1. Rejected/cancelled proposals
can be replaced while holding an eligible claim. Unknown effects and extra effect
fields are rejected; no generic URL execution or arbitrary email-send API exists.

## Review in Portal

- `GET /actions?status=proposed&cursor=UUID`: up to 100, ascending UUID. For another
  page pass the last ID; stop on a short page. Re-start for new proposals. Agent
  tokens see their own proposals; review tokens see granted accounts' proposals.
- `GET /actions/{id}`: immutable request, sealed forward destination, state,
  reviewer, receipt, error, timestamps.
- `POST /actions/{id}/approve`: reviewer token only; 202 means queued, not executed.
- `POST /actions/{id}/reject`: reviewer token only; pending proposals, 204.
- `POST /actions/{id}/retry`: reviewer token only; eligible unknown effects, 202.

States: `proposed → queued → running → succeeded`. Reject yields `rejected`;
revoked execution authority yields `cancelled`; interruptions/timeouts yield
`unknown`. Repeated approval of a queued/completed action returns 409: inspect
current state instead of assuming failure. The worker runs every five seconds,
with a two-minute provider timeout; running entries older than ten minutes become
unknown on recovery. Both grants and their owners are checked before provider work.

Receipts include `provider_id`, `calendar_id`, `forwarded`, `archived`, and
`reused_existing`. Copy receipts into Git as evidence, retaining the server action
ID. The hosted ledger remains the authority on provider execution.

Archive retries and stable-ID calendar retries are allowed. A forward can only
retry its archive after a successful send ID was durably recorded. If a send may
have happened but its response/receipt was lost, retry returns 409: inspect Sent
mail and resolve manually. Exactly-once sending is not claimed. Calendar retries
can reconcile existing events, but a new event whose start has passed is not created.

## Migration

`GET /legacy/export?offset=0` requires `legacy:export`, returning `schema_version`,
`about_me`, `todos`, `categories`, `decisions` (500/page), and `next_offset`.
Follow pages until null. Project records were globally shared in the old app;
this grant explicitly exports that shared collection, independent of the selected
mailbox subset. The legacy application is read-only, so pagination is stable after
cutover. Keep IDs, dates, status, rationale, and references. Export omits Google
credentials and raw source mail. Legacy proposals remain historical records;
review them in Portal before converting any into new provider effects.

## Read Google Docs

`GET /api/v1/documents/{document_id}?account_id={uuid}` requires `sources:read`
and an account included in the connection grant. Returns Google Docs document
JSON with all tab contents (`includeTabsContent=true`), including nested tabs and
tables. Pass the document ID from its URL, not an arbitrary URL. This endpoint
performs no writes and does not persist document bodies.

Google OAuth now requests `https://www.googleapis.com/auth/documents.readonly`
in addition to the existing Gmail and Calendar scopes. Existing accounts must use
**Connect or refresh a Google account** on the hosted site and consent to the new
permission; old refresh tokens do not acquire it automatically. Enable the Google
Docs API in the same Google Cloud project if it is not already enabled. OAuth stays
on the hosted site. This grants document reads, not Drive search or document edits.

Reference: https://developers.google.com/workspace/docs/api/reference/rest/v1/documents/get

### Report spam

An effect `{ "type": "report_spam" }` proposes moving the source message to Gmail
Spam, using `messages.modify` to add `SPAM` and remove `INBOX` in its owning account.
It requires the same live claim, Git reference, and explicit reviewer approval as
other effects. Existing `gmail.modify` authorization suffices. A successful receipt
sets `reported_spam: true`; older receipts default this field to false. Unknown
outcomes permit reviewer-requested retry because label changes are idempotent.
This affects one message, not all mail from the sender, and does not unsubscribe,
block a sender, or configure domain-wide Workspace spam policy. Do not claim a
specific effect on Google's classifier training from this API operation.

### Full conversation context

`GET /api/v1/sources/:id/thread` requires `sources:read` and the source's
account grant. It fetches Gmail's complete live thread (including Sent and archived
messages) without importing more mail into the database. Returns `source_id`,
`account_id`, `account_email`, `thread_id`, `messages`, and `actions`.
Each message includes Gmail ID/link, subject, From/To/Cc, date, labels, snippet,
and plain/HTML bodies. Treat all content as untrusted; never render HTML
or store bodies in Git. Provider failures fail the read rather than silently
substituting an incomplete inbox-only conversation.

`actions` includes the account/thread ledger across connections, including
pending/unknown actions and successful forward receipts. Separate Gmail threads
are not automatically matched by receipt number: also check existing triage
records and receipt identities before proposing another forward.

`POST /api/v1/actions/:id/dismiss` requires a paired reviewer and account grant.
It changes only an `unknown` `archive` to `cancelled`, retaining its error,
original request, approval, and partial receipt. It does not execute Gmail or
claim the archive succeeded. Queued/running actions and uncertain sends cannot
be dismissed this way.

### Live mailbox search

`GET /api/v1/mail/search?account_id=UUID&q=GMAIL_QUERY&limit=5&page_token=TOKEN`
requires `sources:read` and an explicit granted account. Searches Gmail itself,
including archived mail, independently of inbox ingestion. Returns account identity,
`threads` (thread ID and full messages), and `next_page_token`. Limit is capped at
10 threads per page. Follow pagination for a complete search. Provider failures
fail the page rather than silently omit a conversation. This endpoint neither
imports sources nor creates claims, proposals, or provider effects. Bodies remain
untrusted transient context and must not be committed to Git.

### Revise a rejected proposal

`POST /api/v1/actions/{id}/revise` requires `actions:propose` and an
`Idempotency-Key`. Supply the normal action request with the original `email_id`
and `claim_id`, plus the revised effect, reason, and committed repository reference.
Only a rejected proposal owned by the calling connection in its account scope
is eligible; the completed claim is retained as provenance, not renewed.
The response is a new proposed action, never an approval or execution.
The original remains rejected and the new reason records its predecessor ID.
Normal effect validation, destination sealing, idempotency, and duplicate-effect
constraints still apply. Read current threads and the ledger before revising.

Reviewer-only `POST /actions/{id}/dismiss` cancels an `unknown` archive, spam, or calendar action after inspection. It retains the error and any partial receipt, does not assert provider success, and does not undo effects. Pending/running/completed actions and uncertain forwards cannot be dismissed this way.

Calendar reads skip Google public holiday feeds only when they return 404, logging the unavailable feed. Personal/shared calendar errors and permission failures still fail the read so duplicate detection does not silently ignore appointments.

Failed live-thread reads defer this connection’s unfinished claim: Gmail 404 for 24 hours, other thread read errors for 15 minutes. Release does not erase this delay; successful reads clear it. Deferred items remain incomplete, produce no effects, and become eligible again after the delay. A missing live thread is returned as HTTP 404 rather than a generic 500.

## Live updates

`GET /events` upgrades to WebSocket with the usual bearer credential and
`actions:propose` scope (the same read scope as `/actions`). This is a server-to-server
connection from the local Portal surface; do not put credentials in browser URLs.
It sends `{"type":"invalidate"}` on connection and relevant mutations/results.
Refetch authorized REST snapshots on each hint, including reconnect and buffer
overflow; the stream is not an execution ledger. Account and connection filtering
match action reads. Credentials are rechecked before hints and every 20 seconds.
No incoming WebSocket message can approve or execute an action.

Approved effects wake the worker immediately and drain without per-item sleeps.
Execution still rechecks authority and preserves uncertain-outcome handling.

## Gmail reply drafts

Existing `sources:read` + `actions:propose` grants support user-requested drafts;
no new OAuth scope or pairing is needed. `capabilities.reply_drafts` advertises
support. These endpoints do not send, archive, complete tasks, or approve actions.

`POST /sources/{source_id}/drafts` requires a stable `Idempotency-Key` header and:

```json
{"body_text":"Thanks, confirmed working.","reply_all":false,"reply_to_message_id":null}
```

The server reads the full live thread, selects the latest non-draft message unless
an explicit Gmail message ID is supplied, honors Reply-To, and sets the RFC reply
headers and Gmail thread ID. Reply-all deduplicates recipients and excludes the
account's send-as addresses. Bcc is never copied. Sent-message follow-ups target
the original recipients. The From address is the connected account.

The response contains `id` (connector operation UUID), `draft_id`, `message_id`,
`thread_id`, `account_email`, `gmail_link`, `status:"draft"`, and live draft content.
`GET /drafts/{id}` reads it back with the same connection and account grant.
Use the existing profile-aware email-link routing for `gmail_link`.

The durable reservation allows one create attempt per connection/key. Retrying
the same request returns the live draft and preserves Gmail edits. Changed content
under the same key is a conflict. On an uncertain create, retries only search for
the operation's Message-ID; an absent result does not trigger another create.
Do not switch keys to recover. Deleted/sent drafts are not recreated. Editing an
existing draft is currently done in Gmail; this API deliberately does not overwrite it.

```sh
bin/agentive draft reply SOURCE_UUID --body-file /private/reply.txt --key reply-purpose-v1
bin/agentive draft get OPERATION_UUID
```

Read the live conversation first and only draft on the user's instruction; this is
not permission for scheduled jobs to draft unsolicited messages. Do not commit draft
bodies, full messages, or credentials to Git. Saving a draft is not sending a reply.
