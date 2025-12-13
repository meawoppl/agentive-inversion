#!/usr/bin/env python3
import json
from pathlib import Path
from collections import Counter
import sys

def main():
    path = sys.argv[1] if len(sys.argv) > 1 else "emails/inbox"
    limit = int(sys.argv[2]) if len(sys.argv) > 2 else 50
    emails_dir = Path(path)
    senders = Counter()

    for json_file in emails_dir.glob("*.json"):
        try:
            data = json.loads(json_file.read_text())
            sender = data.get("from", "")
            if sender:
                senders[sender] += 1
        except (json.JSONDecodeError, KeyError):
            continue
    for sender, count in senders.most_common(limit):
        print(f"{count:5d}  {sender}")

if __name__ == "__main__":
    main()
