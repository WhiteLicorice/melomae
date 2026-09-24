"""Check that tasks/README.md and the task records agree.

Usage: python tasks/validate_board.py [root]

Ported from tagpuan-app/tasks/validate-board.mjs. It adds one check: every
status must be a value from the board's status legend. Uses only the standard
library. Needs Python 3.10 or later.
"""

import re
import sys
from pathlib import Path

ROW_PATTERN = re.compile(
    r"^\|\s*([0-9]{2,3}|B-[0-9]{3})\s*\|\s*\[([^\]]+)\]\(([^)]+)\)\s*\|\s*([^|]+?)\s*\|\s*([^|]+?)\s*\|$",
    re.MULTILINE,
)
HEADING_PATTERN = re.compile(r"^#\s+([0-9]{2,3}|B-[0-9]{3})\s+—\s+(.+)$", re.MULTILINE)
STATUS_PATTERN = re.compile(r"^\*\*Status:\*\*\s+(.+)$", re.MULTILINE)
DEPENDS_PATTERN = re.compile(r"^\*\*Depends on:\*\*\s+(.+)$", re.MULTILINE)
TASK_FILENAME = re.compile(r"^(?:[0-9]{2,3}|B-[0-9]{3})-.*\.md$")
STATUSES = {"TODO", "IN PROGRESS", "BLOCKED", "CANCELLED", "DONE"}
NO_DEPENDENCIES = "—"


def first_group(pattern, text):
    match = pattern.search(text)
    return match.group(1).strip() if match else None


def validate(root):
    tasks_directory = root / "tasks"
    board_path = tasks_directory / "README.md"
    if not board_path.is_file():
        return None, [f"Task board not found: {board_path}"]

    board = board_path.read_text(encoding="utf-8")
    records = [
        {
            "id": match.group(1),
            "title": match.group(2).strip(),
            "link": match.group(3).strip(),
            "status": match.group(4).strip(),
            "dependencies": match.group(5).strip(),
        }
        for match in ROW_PATTERN.finditer(board)
    ]
    known_ids = {record["id"] for record in records}
    seen_ids = set()
    seen_links = set()
    errors = []

    for record in records:
        identifier = record["id"]
        if identifier in seen_ids:
            errors.append(f"Duplicate board identifier: {identifier}.")
        seen_ids.add(identifier)
        if record["link"] in seen_links:
            errors.append(f"Duplicate board link: {record['link']}.")
        seen_links.add(record["link"])
        if record["status"] not in STATUSES:
            errors.append(f"Task {identifier} has unknown status {record['status']}.")

        task_path = tasks_directory / record["link"]
        if not task_path.is_file():
            errors.append(f"Task file not found for {identifier}: {record['link']}.")
            continue
        task = task_path.read_text(encoding="utf-8")
        heading = HEADING_PATTERN.search(task)
        status = first_group(STATUS_PATTERN, task)
        dependencies = first_group(DEPENDS_PATTERN, task)

        if heading is None:
            errors.append(f"Task {identifier} has no valid heading.")
        else:
            if heading.group(1) != identifier:
                errors.append(f"Task {identifier} heading identifier differs.")
            if heading.group(2).strip() != record["title"]:
                errors.append(f"Task {identifier} title differs.")
        if status != record["status"]:
            errors.append(f"Task {identifier} status differs.")
        if dependencies != record["dependencies"]:
            errors.append(f"Task {identifier} dependencies differ.")

        if dependencies and dependencies != NO_DEPENDENCIES:
            for dependency in (value.strip() for value in dependencies.split(",")):
                if dependency not in known_ids:
                    errors.append(f"Task {identifier} has unknown dependency {dependency}.")

    for directory in sorted(path for path in tasks_directory.iterdir() if path.is_dir()):
        for task_file in sorted(directory.iterdir()):
            if not TASK_FILENAME.match(task_file.name):
                continue
            link = f"{directory.name}/{task_file.name}"
            if link not in seen_links:
                errors.append(f"Task file has no board row: {link}.")

    return records, errors


def main(argv):
    root = Path(argv[1]) if len(argv) > 1 else Path(__file__).resolve().parent.parent
    records, errors = validate(root.resolve())
    if errors:
        sys.stderr.write("\n".join(errors) + "\n")
        return 1
    sys.stdout.write(f"Task board valid: {len(records)} records.\n")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
