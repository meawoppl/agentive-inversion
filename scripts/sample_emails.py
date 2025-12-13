#!/usr/bin/env python3
"""
Sample emails from a folder for review
Usage:
  python sample_emails.py [folder] [count]
  python sample_emails.py -s boxshop          # search in subject/from
  python sample_emails.py -s boxshop --files  # just print filenames for piping
"""
import argparse
import json
import sys
from pathlib import Path

def sample_emails(folder="emails/needs_review", count=100, search=None, files_only=False):
    folder_path = Path(folder)
    if not folder_path.exists():
        print(f"Folder not found: {folder}", file=sys.stderr)
        return []

    files = sorted(folder_path.glob("*.json"))
    total = len(files)

    matches = []
    for f in files:
        try:
            with open(f) as fp:
                email = json.load(fp)
            sender = email.get("from", "unknown")
            subject = email.get("subject", "no subject")

            if search:
                search_lower = search.lower()
                if search_lower not in sender.lower() and search_lower not in subject.lower():
                    continue

            matches.append((f, sender, subject))
        except Exception as e:
            if not search:
                matches.append((f, "error", str(e)))

    if not files_only:
        if search:
            print(f"Found {len(matches)} matching '{search}' in {folder} ({total} total)\n")
        else:
            print(f"Total: {total} emails in {folder}\n")

    # Get the most recent N
    recent = matches[-count:] if len(matches) > count else matches

    for f, sender, subject in recent:
        if files_only:
            print(f)
        else:
            print(f"{f.name}")
            print(f"  {sender}")
            print(f"  {subject}")
            print()

    return [str(f) for f, _, _ in recent]

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Sample emails from a folder")
    parser.add_argument("folder", nargs="?", default="emails/needs_review", help="Folder to search")
    parser.add_argument("-n", "--count", type=int, default=100, help="Number of emails to show")
    parser.add_argument("-s", "--search", help="Filter by sender or subject (case-insensitive)")
    parser.add_argument("--files", action="store_true", help="Only print full file paths (for piping)")
    args = parser.parse_args()

    sample_emails(args.folder, args.count, args.search, args.files)
