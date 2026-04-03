from __future__ import annotations

import json
import re
import subprocess
import tempfile
from pathlib import Path


STALE_IDS = [
    "vel-3fs",
    "vel-7dg",
    "vel-60k",
    "vel-sd1",
    "vel-3hv",
    "vel-9i2",
    "vel-2gi",
    "vel-q8s",
    "vel-1rz",
    "vel-y7g",
    "vel-edo",
    "vel-50j",
    "vel-2a2",
]

SCHEMA_HEADER_PATTERNS = [
    re.compile(r"^# CUE Validation Schema\n?", re.MULTILINE),
    re.compile(r"^# Validate implementation:.*\n?", re.MULTILINE),
    re.compile(r"^# Schema location:.*\n?", re.MULTILINE),
]


def run(*args: str) -> str:
    return subprocess.check_output(args, text=True)


def sanitize_description(issue_id: str, text: str) -> str:
    result = text or ""

    for pattern in SCHEMA_HEADER_PATTERNS:
        result = pattern.sub("", result)

    result = re.sub(r"/home/lewis/src/vo-engine/", "/home/lewis/src/veloxide/", result)
    result = re.sub(r"big-ole-orchestrator-[A-Za-z0-9-]+", issue_id, result)
    result = result.replace("vo-engine", "veloxide")

    for stale_id in STALE_IDS:
        result = result.replace(stale_id, "the superseded predecessor bead")

    result = re.sub(
        r"(?im)^.*replacement for.*$\n?",
        "",
        result,
    )
    result = re.sub(
        r"(?im)^.*supersed(?:ed|es|ing).*$\n?",
        "",
        result,
    )
    result = re.sub(r"\n{3,}", "\n\n", result).strip() + "\n"
    return result


def sanitize_notes(text: str) -> str:
    if not text:
        return ""

    result = text
    for stale_id in STALE_IDS:
        result = result.replace(stale_id, "the superseded predecessor bead")

    result = re.sub(r"(?im)^.*replacement for.*$\n?", "", result)
    result = re.sub(r"(?im)^.*supersed(?:ed|es|ing).*$\n?", "", result)
    result = re.sub(r"(?im)^.*vo-engine.*$\n?", "", result)
    result = re.sub(r"(?im)^.*big-ole-orchestrator.*$\n?", "", result)
    return re.sub(r"\n{3,}", "\n\n", result).strip()


def main() -> None:
    open_issues = json.loads(run("bd", "list", "--status", "open", "--json"))
    in_progress_issues = json.loads(run("bd", "list", "--status", "in_progress", "--json"))
    seen: set[str] = set()
    issues = []
    for issue in open_issues + in_progress_issues:
        issue_id = issue["id"]
        if issue_id in seen:
            continue
        seen.add(issue_id)
        issues.append(issue)
    updated: list[str] = []

    for issue in issues:
        issue_id = issue["id"]
        old_description = issue.get("description") or ""
        old_notes = issue.get("notes") or ""

        new_description = sanitize_description(issue_id, old_description)
        new_notes = sanitize_notes(old_notes)

        if new_description == old_description and new_notes == old_notes:
            continue

        with tempfile.NamedTemporaryFile("w", delete=False, dir=str(Path(".forge/contract-cleanup"))) as body_file:
            body_file.write(new_description)
            body_path = body_file.name

        command = ["bd", "update", issue_id, "--body-file", body_path]
        if new_notes != old_notes:
            command.extend(["--notes", new_notes])

        subprocess.check_call(command)
        updated.append(issue_id)

    print(json.dumps(updated))


if __name__ == "__main__":
    main()
