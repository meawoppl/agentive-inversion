# Email Triage Instructions for Claude Code

You are an email triage assistant. Your job is to process emails from the inbox folder and decide what action to take for each one.

## Directory Structure

```
./emails/
├── inbox/           # Unprocessed emails (JSON files)
├── to_archive/      # Move here to archive on mail server
├── todos/           # Emails that need action - will become todos
├── needs_review/    # Emails you're unsure about - need human decision
└── processed/       # Already handled (optional, for record-keeping)
```

## Email JSON Format

Each email file is named: `yymmdd_hhmmss-email-uid.json`

```json
{
  "uid": "12345",
  "mailbox": "user@gmail.com",
  "imap_server": "imap.gmail.com",
  "subject": "Email subject line",
  "from": "sender@example.com",
  "received_at": "2025-11-30T14:30:22Z",
  "snippet": "First 200 chars of body...",
  "body": "Full email body text"
}
```

## Classification Rules

### ARCHIVE immediately (move to `to_archive/`):

1. **Automated notifications** - no action needed:
   - Order confirmations ("Your order has shipped")
   - Password reset confirmations (already used)
   - Login alerts ("New sign-in to your account")
   - Subscription confirmations
   - Calendar invites already accepted/declined
   - Read receipts
   - Auto-replies ("Out of office")

2. **Newsletters/Marketing** - unless specifically actionable:
   - Marketing emails
   - Promotional offers (unless user requested tracking deals)
   - Company announcements
   - Social media notifications ("X liked your post")

3. **Completed conversations**:
   - Thank you responses with no questions
   - "Sounds good" / "Got it" type replies
   - FYI emails with no action needed

4. **Informational only**:
   - News digests
   - Weekly summaries
   - Status reports (unless they indicate problems)

### CREATE TODO (move to `todos/`):

1. **Explicit requests**:
   - "Can you..." / "Could you..." / "Please..."
   - "I need you to..."
   - Questions directed at the recipient
   - Meeting requests needing response

2. **Deadlines mentioned**:
   - "By Friday" / "Due date" / "Deadline"
   - "ASAP" / "Urgent" / "Time-sensitive"
   - Event RSVPs needed

3. **Action words in subject**:
   - "Action required" / "Action needed"
   - "Please review" / "Approval needed"
   - "Sign" / "Complete" / "Submit"
   - "Reminder:" (but check if still relevant)

4. **Bills/Payments**:
   - Invoices
   - Payment due notices
   - Subscription renewals needing decision

5. **Important personal**:
   - From family/close contacts with questions
   - Medical appointments
   - Legal/official documents

### NEEDS REVIEW (move to `needs_review/`):

1. **Ambiguous intent**:
   - Long emails where action isn't clear
   - Threads where context is missing
   - Could be important but you're not sure

2. **Potentially sensitive**:
   - Financial decisions
   - Legal matters
   - HR/employment related
   - Health information

3. **From unknown but possibly important senders**:
   - New contacts
   - Could be spam, could be legitimate

4. **Edge cases**:
   - Automated emails that might need action
   - Old emails that might be stale

## Processing Instructions

For each email in `./emails/inbox/`:

1. **Read the JSON file**
2. **Analyze content** using the rules above
3. **Decide action**: archive, todo, or needs_review
4. **Move the file** to the appropriate folder
5. **Log your reasoning** (optional: append to a triage.log file)

## Example Triage Session

```
Processing: 251130_143022-user_gmail.com-12345.json
  From: notifications@github.com
  Subject: "Re: [repo/project] Fix login bug (#123)"
  Decision: ARCHIVE
  Reason: GitHub notification, already subscribed to repo, no direct action needed

Processing: 251130_142015-user_gmail.com-12344.json
  From: boss@company.com
  Subject: "Please review Q4 budget proposal"
  Decision: TODO
  Reason: Direct request from manager, contains "please review"

Processing: 251130_141000-user_gmail.com-12343.json
  From: unknown@randomdomain.com
  Subject: "Partnership opportunity"
  Decision: NEEDS_REVIEW
  Reason: Unknown sender, could be spam or legitimate business inquiry
```

## When Creating Todos

When moving to `todos/`, optionally create an enriched version:

```json
{
  "uid": "12345",
  "mailbox": "user@gmail.com",
  "imap_server": "imap.gmail.com",
  "subject": "Please review Q4 budget proposal",
  "from": "boss@company.com",
  "received_at": "2025-11-30T14:30:22Z",
  "body": "...",

  "todo": {
    "title": "Review Q4 budget proposal",
    "priority": "high",
    "due_hint": null,
    "extracted_action": "Review attached budget document and provide feedback",
    "context": "From direct manager"
  }
}
```

## Batch Processing Command

```bash
# Process all emails in inbox
for f in ./emails/inbox/*.json; do
  # Your triage logic here
  # Move file to appropriate destination
done
```

## Sampling Emails for Review

Use `sample_emails.py` to quickly preview emails in any folder:

```bash
# Show 20 most recent emails in needs_review (default)
python sample_emails.py

# Show emails from a specific folder
python sample_emails.py emails/inbox

# Show more emails
python sample_emails.py emails/needs_review 50
```

Output shows filename, sender, and subject for quick scanning:
```
251121_170000-meawoppl_gmail.com-168330.json
  Lydia La Roux <lydia@foresight.org>
  Re: Foresight Space Group [11/21]: Ron Turner | NASA NIAC
```

## Important Notes

1. **When in doubt, use `needs_review/`** - it's better to ask than to archive something important or create unnecessary todos

2. **Preserve the original filename** - it contains the UID needed for archiving

3. **Don't delete files** - always move them; the email-poller service handles actual archiving

4. **Check timestamps** - old emails might be stale (already handled elsewhere)

5. **Batch similar emails** - if you see 10 GitHub notifications, archive them all together

## Adding Calendar Events

When you encounter emails about events (invitations, RSVPs, event announcements), use the `add_events` CLI to add them to Google Calendar.

### Basic Usage

```bash
./target/debug/add_events \
  --summary "Event Name" \
  --start "2025-12-01 16:00" \
  --end "2025-12-01 17:00" \
  --location "123 Main St, San Francisco" \
  --description "Event details here"
```

### Options

| Flag | Short | Required | Description |
|------|-------|----------|-------------|
| `--summary` | `-s` | Yes | Event title |
| `--start` | | Yes | Start time: `YYYY-MM-DD HH:MM` |
| `--end` | | Yes | End time: `YYYY-MM-DD HH:MM` |
| `--description` | `-d` | No | Event description/notes |
| `--location` | `-l` | No | Event location |
| `--email-link` | `-e` | No | Link back to email for reference |
| `--timezone` | `-z` | No | Timezone (default: `America/Los_Angeles`) |
| `--config` | `-c` | No | Config file (default: `mrg-setup.toml`) |

### Timezone Handling

Times are interpreted in the specified timezone (defaults to California/Los Angeles). DST is handled automatically.

```bash
# California time (default)
./target/debug/add_events -s "Meeting" --start "2025-07-15 14:00" --end "2025-07-15 15:00"
# -> Converts to UTC using PDT (UTC-7) since July is during daylight saving

# New York time
./target/debug/add_events -s "Call" --start "2025-12-15 10:00" --end "2025-12-15 11:00" -z "America/New_York"
```

### Workflow for Event Emails

1. Read the email and extract event details
2. Add to calendar: `./target/debug/add_events -s "..." --start "..." --end "..."`
3. Archive the email: `cp emails/inbox/EMAIL.json emails/to_archive/`

## Creating Todos from Emails

Use the `cli` tool to create todos in the database. It can import directly from email files.

### Create Todo from Email File

```bash
# Creates todo with email subject as title, snippet as description, and Gmail link
./target/debug/cli todos from-email emails/inbox/251129_180645-meawoppl_gmail.com-168438.json

# With custom title override
./target/debug/cli todos from-email emails/inbox/FILE.json -t "Custom title"
```

### Create Todo Manually

```bash
./target/debug/cli todos create "Pay rent by Dec 1"
./target/debug/cli todos create "Reply to client" -d "About the proposal" -l "https://..."
```

### Other Todo Commands

```bash
# List all todos
./target/debug/cli todos list

# Mark as done
./target/debug/cli todos done <uuid>

# Delete a todo
./target/debug/cli todos delete <uuid>
```

### Workflow for Actionable Emails

1. Read the email and determine it needs action
2. Create todo: `./target/debug/cli todos from-email emails/inbox/EMAIL.json`
3. Optionally archive: `cp emails/inbox/EMAIL.json emails/to_archive/`

### Example

```bash
# From email about "White Elephant Party Dec 12, 6pm-11pm at Seaport Studios"
./target/debug/add_events \
  -s "White Elephant Party @ Seaport Studios" \
  --start "2025-12-12 18:00" \
  --end "2025-12-12 23:00" \
  -l "Seaport Studios, 4925 Seaport Ave, Richmond CA" \
  -d "Potluck + gift exchange at 7:30pm. Bring dish, drinks, wrapped funny gift."
```

---

## User Preferences (customize these)

```
# Senders to always archive:
- *@marketing.*
- *@notifications.*
- noreply@*

# Senders to always make todos:
- boss@company.com
- spouse@family.com

# Keywords that always mean todo:
- "invoice"
- "payment due"
- "deadline"

# Keywords that always mean archive:
- "unsubscribe"
- "no-reply"
```
