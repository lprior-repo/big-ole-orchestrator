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

LEGACY_PATTERNS = [
    re.compile(r"^# CUE Validation Schema\n?", re.MULTILINE),
    re.compile(r"^# Validate implementation:.*\n?", re.MULTILINE),
    re.compile(r"^# Schema location:.*\n?", re.MULTILINE),
    re.compile(r"^Workspace-truth replacement slice for [^.]+\.\s*", re.IGNORECASE),
]

REMOVABLE_NOTE_PATTERNS = [
    re.compile(r"(?im)^.*removed stale dependenc.*$\n?"),
    re.compile(r"(?im)^.*superseded bead.*$\n?"),
    re.compile(r"(?im)^.*black-hat review.*$\n?"),
    re.compile(r"(?im)^.*workspace-truth replacement slice for.*$\n?"),
    re.compile(r"(?im)^.*vo-engine.*$\n?"),
    re.compile(r"(?im)^.*big-ole-orchestrator.*$\n?"),
]

COMMON_REPLACEMENTS = [
    (
        re.compile(r"No reference to vo-engine or stale docs/v2 paths was introduced\."),
        "No stale workspace references were introduced.",
    ),
    (
        re.compile(r"/home/lewis/src/vo-engine/"),
        "/home/lewis/src/veloxide/",
    ),
    (
        re.compile(r"big-ole-orchestrator-[A-Za-z0-9-]+"),
        "historical-planner-id",
    ),
]


def run(*args: str) -> str:
    return subprocess.check_output(args, text=True)


def update_issue(issue_id: str, description: str, notes: str) -> None:
    cleanup_dir = Path(".forge/contract-cleanup/tmp")
    cleanup_dir.mkdir(parents=True, exist_ok=True)

    with tempfile.NamedTemporaryFile("w", delete=False, dir=cleanup_dir) as body_file:
        body_file.write(description)
        body_path = body_file.name

    command = ["bd", "update", issue_id, "--body-file", body_path, "--notes", notes]
    subprocess.check_call(command)


def normalize_whitespace(text: str) -> str:
    cleaned = re.sub(r"\n{3,}", "\n\n", text).strip()
    return f"{cleaned}\n" if cleaned else ""


def replace_common(text: str) -> str:
    result = text
    for pattern, replacement in COMMON_REPLACEMENTS:
        result = pattern.sub(replacement, result)
    result = result.replace("vo-engine", "veloxide")
    for stale_id in STALE_IDS:
        result = result.replace(stale_id, "the superseded predecessor bead")
    return result


def sanitize_active_description(text: str) -> str:
    result = text or ""
    for pattern in LEGACY_PATTERNS:
        result = pattern.sub("", result)
    result = replace_common(result)
    result = re.sub(r"(?im)^.*replacement for the superseded predecessor bead.*$\n?", "", result)
    result = re.sub(r"(?im)^.*superseded predecessor bead.*black-hat review.*$\n?", "", result)
    return normalize_whitespace(result)


def sanitize_active_notes(text: str) -> str:
    result = text or ""
    result = replace_common(result)
    for pattern in REMOVABLE_NOTE_PATTERNS:
        result = pattern.sub("", result)
    result = re.sub(r"(?i)\b(replacement for|superseded|stale dependency|stale dependencies)\b.*", "", result)
    return re.sub(r"\n{3,}", "\n\n", result).strip()


def sanitize_closed_description(title: str, text: str) -> str:
    combined = replace_common(text or "")
    if not combined.strip():
        combined = "Historical closed bead retained for audit only."
    return (
        "Historical closed bead retained for audit only. "
        "It was authored against an earlier planning surface and is not a valid implementation contract for the current workspace. "
        "Use the live open backlog and current ADR corpus instead."
    )


def sanitize_closed_notes(_: str) -> str:
    return "Retained only as historical context; do not use this closed bead as an implementation contract."


def get_issue(issue_id: str) -> dict:
    return json.loads(run("bd", "show", issue_id, "--json"))[0]


def collect_active_ids() -> list[str]:
    open_issues = json.loads(run("bd", "list", "--status", "open", "--limit", "0", "--json"))
    in_progress_issues = json.loads(run("bd", "list", "--status", "in_progress", "--limit", "0", "--json"))
    seen: set[str] = set()
    ordered: list[str] = []
    for issue in open_issues + in_progress_issues:
        issue_id = issue["id"]
        if issue_id in seen:
            continue
        seen.add(issue_id)
        ordered.append(issue_id)
    return ordered


def collect_all_ids() -> list[str]:
    issues = json.loads(run("bd", "list", "--all", "--limit", "0", "--json"))
    seen: set[str] = set()
    ordered: list[str] = []
    for issue in issues:
        issue_id = issue["id"]
        if issue_id in seen:
            continue
        seen.add(issue_id)
        ordered.append(issue_id)
    return ordered


def collect_visible_ids(seed_ids: list[str]) -> list[str]:
    seen = set(seed_ids)
    ordered = list(seed_ids)
    for issue_id in seed_ids:
        issue = get_issue(issue_id)
        for dependency in issue.get("dependencies", []):
            dep_id = dependency.get("id") or dependency.get("depends_on_id")
            if dep_id and dep_id not in seen:
                seen.add(dep_id)
                ordered.append(dep_id)
        for dependent in issue.get("dependents", []):
            dep_id = dependent.get("id") or dependent.get("issue_id")
            if dep_id and dep_id not in seen:
                seen.add(dep_id)
                ordered.append(dep_id)
    return ordered


def contains_residue(text: str) -> bool:
    lowered = text.lower()
    if "vo-engine" in lowered or "big-ole-orchestrator" in lowered:
        return True
    return any(stale_id in text for stale_id in STALE_IDS)


def main() -> None:
    active_ids = collect_active_ids()
    all_ids = collect_all_ids()
    visible_ids = collect_visible_ids(all_ids)
    updated: list[str] = []

    for issue_id in visible_ids:
        issue = get_issue(issue_id)
        status = issue["status"]
        description = issue.get("description") or ""
        notes = issue.get("notes") or ""

        if status in {"open", "in_progress"}:
            new_description = sanitize_active_description(description)
            new_notes = sanitize_active_notes(notes)
        else:
            if not contains_residue(description + "\n" + notes):
                continue
            new_description = sanitize_closed_description(issue.get("title") or issue_id, description)
            new_notes = sanitize_closed_notes(notes)

        if new_description == description and new_notes == notes:
            continue

        update_issue(issue_id, new_description, new_notes)
        updated.append(issue_id)

    print(json.dumps(updated))


if __name__ == "__main__":
    main()
