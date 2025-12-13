#!/bin/bash
# Email Triage Script - Invokes Claude Code to process emails

set -e

EMAILS_DIR="${EMAILS_DIR:-./emails}"

# Ensure directories exist
mkdir -p "$EMAILS_DIR/inbox"
mkdir -p "$EMAILS_DIR/to_archive"
mkdir -p "$EMAILS_DIR/todos"
mkdir -p "$EMAILS_DIR/needs_review"
mkdir -p "$EMAILS_DIR/processed"

# Count emails to process
COUNT=$(find "$EMAILS_DIR/inbox" -name "*.json" 2>/dev/null | wc -l)

if [ "$COUNT" -eq 0 ]; then
    echo "No emails to process in $EMAILS_DIR/inbox"
    exit 0
fi

echo "Found $COUNT emails to triage"
echo ""

# Invoke Claude Code with the triage instructions
claude --print "
You are triaging emails. Read the instructions in EMAIL_TRIAGE_INSTRUCTIONS.md first.

Then process each .json file in $EMAILS_DIR/inbox/:

1. Read the file
2. Decide: archive, todo, or needs_review
3. Move the file to the appropriate folder:
   - Archive → $EMAILS_DIR/to_archive/
   - Todo → $EMAILS_DIR/todos/
   - Needs review → $EMAILS_DIR/needs_review/

For each email, briefly state your reasoning.

Start processing now.
"
