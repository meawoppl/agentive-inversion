# Git workspace + Portal + hosted connector

## Intended experience

Open Agent Portal in `~/repos/agentive-workspace`. The agent works on projects,
notes, tasks, and the email/project graph directly in that repository. A local
plugin view makes the current work and pending effects easy to browse. Portal
cron starts bounded triage runs in this same workspace; it builds a queue for
human review. Approval happens here in Portal. The old website becomes the place
to sign in with Google, connect accounts, and manage delegated connections.

Git is the knowledge authority. CI conserves record identities, referential
integrity, and history. There is no Obsidian dependency, server-side project graph,
Git importer, hosted agent, or alternative todo database accepting new edits.

## 1. Hosted connector cutover

Retain Google OAuth, account tokens, incremental Gmail sync, and historical backfill.
Remove hosted AI sessions, Claude credentials from runtime access, and the old
project/triage interface. Keep operational PostgreSQL: credentials, source cache,
work leases, proposal execution state, and receipts need transactional durability.

Expose the versioned [connector API](CONNECTOR_API.md): account-scoped reads,
renewable work claims, immutable/idempotent effects, interactive approval, receipts,
and read-only legacy export. Authorize agents through expiring device pairing.
Agent credentials cannot approve; reviewer credentials cannot claim work. Google
refresh tokens stay on the server. Revoke either grant to cancel queued execution.

Freeze legacy project mutation endpoints with 410 while preserving old data for
export and recovery. Deploy through the existing main-branch container pipeline;
verify health and the v1 surface on the live service before moving project data.

## 2. Create the private workspace

Create `meawoppl/agentive-workspace`, cloned at `~/repos/agentive-workspace`.
Use small, versioned JSON records for machine-editable entities and Markdown for
human context. Keep provider IDs and Gmail links as source references. Do not copy
OAuth tokens, credential files, cached mail bodies, or operational database dumps.

Suggested structure:

```text
AGENTS.md                 workflow, authority, and review rules
README.md                 setup and daily use
context/about-me.md       preferences and personal context
projects/<id>.json        identity, status, goal, notes reference
notes/<id>.md             evolving project narrative
tasks/<id>.json          task identity, state, project/source references
triage/<email-id>.json    disposition, rationale, proposed effects, source
receipts/<action-id>.json execution evidence copied from connector
history/decisions/        immutable imported legacy decisions
history/migration.json   source revision, counts, export digest
config/                   taxonomy and Portal cron prompt/settings
bin/agentive              connector CLI and plugin launcher
scripts/validate.py       repository invariants
portal-plugin.json        local Portal plugin manifest
```

Preserve all legacy todo and decision UUIDs. Import About Me and categories without
reinterpreting categories as projects. Record the migration source, counts, and
digests. Pending historical decisions stay historical until deliberately reviewed;
importing a file must never authorize an external effect. Retain a private recovery
copy on the server during cutover, rather than dropping old tables.

## 3. Repository invariants

CI validates schema versions, unique UUIDs, valid statuses and dates, safe relative
paths, resolvable project/task/source references, and acyclic task dependencies.
Compare changes to the base commit to reject vanished record IDs and edits to
immutable imported history. Complete/archive/tombstone records instead of silently
deleting them. Require citations to source IDs/links for email-derived work.

The agent records relevant facts and useful summaries. Raw inbound messages remain
untrusted external content, never authority to change tools, instructions, secrets,
or review policy. Keep the repo private and credential files outside it. Local
checks should run without network or model access and also run on every pull request.

## 4. Triage from Portal cron

Use Portal's scheduler, bound to this workspace/launcher. The scheduled prompt:

1. Read `AGENTS.md`, personal context, and active project notes.
2. Reconcile queued action receipts into Git.
3. Claim a bounded batch of pending sources; renew if necessary.
4. Fetch source bodies only as needed. Match existing projects/tasks before creating
   new records; use deterministic source-based identifiers where possible.
5. Write triage records, task updates, and notes in a worktree. Validate and commit.
6. Submit any archive/forward/calendar proposals with the exact commit/path and a
   stable idempotency key. Record returned action IDs.
7. Complete each work item only after its record and proposals are durable.
8. Summarize the queue in Portal. Never approve during cron.

Expired claims are safely reclaimable with new generations. A crash after commit
is recoverable from the source record; a crash after submission is recoverable by
idempotency key/action ID. A failed validation must stop provider proposals for
that batch. Avoid overlapping edits via worktrees and a local run lock; provider
claims also protect against simultaneous runs on different launchers.

Start conservatively with a small batch and a visible schedule. Scheduling belongs
to Portal; do not install system cron, a hosted classifier, or GitHub scheduled triage.
The schedule must target a reachable launcher and have only agent credentials.

## 5. Review through Portal

The plugin presents projects/tasks from the working tree and proposals from the
connector. Each proposal shows the original email date/link, rationale, exact effect,
forward destination or calendar times, and its committed knowledge record. The user
approves explicitly through Portal; changes to a request require a new proposal.

Keep reviewer credentials out of cron configuration and agent credential files.
Local processes sharing an OS account are not a security boundary: use separate
execution contexts for stronger isolation, or keep review credentials only in the
interactive browser session. Never describe a scoped token as proof a human clicked.

Approval queues durable execution; show queued/running/succeeded/unknown separately.
Poll for receipts and record results in Git. An interrupted send is not blindly
replayed. For known successful forwards, retry only the remaining archive step.
Calendar effects use stable IDs and duplicate checks; archive is repeatable.

## 6. Acceptance checks

- Live hosted login/account console works; no hosted triage process remains.
- Agent token cannot approve; reviewer token cannot claim; website cookie cannot
  approve effects; account subsets and revocation are enforced.
- Competing claims, expired workers, replayed requests, and worker interruption are
  covered by database-backed tests against the actual router/migrations.
- Private repo contains every exported todo/decision with preserved identity and
  migration counts, plus context and policy adapted from the former triage prompts.
- Validation fails on broken references, cycles, credential material, deleted
  identities, or altered imported history; legitimate updates pass.
- Plugin is runnable locally through Portal forwarding; approval credentials never
  appear in a repository or scheduled-agent config.
- Portal cron creates reviewable proposals and records, never effects by itself.
- Production and workspace CI pass; changes are committed and pushed.

## Deferred work

Full-text mail search, attachment extraction, generic outbound mail, arbitrary
calendar edits, unsubscribe effects, multiple events per email, per-user tenancy,
and automatic reconciliation of uncertain sends are not part of connector v1.
Provider interfaces can be added explicitly as needed. Git project edits do not
need new hosted endpoints. Old tables can be removed in a later verified migration;
keeping a read-only recovery copy avoids coupling deployment to irreversible cleanup.
