"""Tests for tasks/validate_board.py.

Run: python -m unittest discover -s tasks -p "test_*.py" -v
"""

import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).with_name("validate_board.py")

BOARD_HEADER = "| # | Task | Status | Depends on |\n| --- | --- | --- | --- |\n"


def run_validator(*args):
    return subprocess.run(
        [sys.executable, str(SCRIPT), *args],
        capture_output=True,
        text=True,
        encoding="utf-8",
    )


def task(identifier, title, status="DONE", depends="—"):
    return (
        f"# {identifier} — {title}\n\n"
        f"**Status:** {status}\n"
        f"**Depends on:** {depends}\n"
    )


class TemporaryBoard:
    """Builds a throwaway repository root with a tasks/ board."""

    def __init__(self, rows, files):
        self.rows = rows
        self.files = files

    def __enter__(self):
        self._directory = tempfile.TemporaryDirectory(prefix="melomae-board-")
        root = Path(self._directory.name)
        tasks = root / "tasks"
        (tasks / "mvp").mkdir(parents=True)
        (tasks / "backlog").mkdir()
        (tasks / "README.md").write_text(BOARD_HEADER + "".join(self.rows), encoding="utf-8")
        for relative, content in self.files.items():
            path = tasks / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(content, encoding="utf-8")
        return root

    def __exit__(self, *exc):
        self._directory.cleanup()


class ValidateBoardTest(unittest.TestCase):
    def test_the_task_board_matches_its_task_records(self):
        result = run_validator()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertRegex(result.stdout, r"Task board valid: \d+ records\.")

    def test_a_consistent_board_passes(self):
        rows = [
            "| 00 | [Example](mvp/00-example.md) | DONE | — |\n",
            "| 01 | [Next](mvp/01-next.md) | TODO | 00 |\n",
            "| B-001 | [Later](backlog/B-001-later.md) | TODO | 01 |\n",
        ]
        files = {
            "mvp/00-example.md": task("00", "Example"),
            "mvp/01-next.md": task("01", "Next", "TODO", "00"),
            "backlog/B-001-later.md": task("B-001", "Later", "TODO", "01"),
        }
        with TemporaryBoard(rows, files) as root:
            result = run_validator(str(root))
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("Task board valid: 3 records.", result.stdout)

    def test_a_status_mismatch_is_rejected(self):
        rows = ["| 00 | [Example](mvp/00-example.md) | DONE | — |\n"]
        files = {"mvp/00-example.md": task("00", "Example", status="TODO")}
        with TemporaryBoard(rows, files) as root:
            result = run_validator(str(root))
        self.assertNotEqual(result.returncode, 0)
        self.assertRegex(result.stderr, r"(?i)status")

    def test_an_unlisted_file_in_any_task_directory_is_reported(self):
        rows = ["| 00 | [Example](mvp/00-example.md) | DONE | — |\n"]
        files = {
            "mvp/00-example.md": task("00", "Example"),
            "mvp-sprint1/110-unlisted.md": task("110", "Unlisted", "TODO"),
        }
        with TemporaryBoard(rows, files) as root:
            result = run_validator(str(root))
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("mvp-sprint1/110-unlisted.md", result.stderr)

    def test_an_unknown_dependency_is_rejected(self):
        rows = ["| 00 | [Example](mvp/00-example.md) | TODO | 07 |\n"]
        files = {"mvp/00-example.md": task("00", "Example", "TODO", "07")}
        with TemporaryBoard(rows, files) as root:
            result = run_validator(str(root))
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("unknown dependency 07", result.stderr)

    def test_a_status_outside_the_legend_is_rejected(self):
        rows = ["| 00 | [Example](mvp/00-example.md) | FINISHED | — |\n"]
        files = {"mvp/00-example.md": task("00", "Example", status="FINISHED")}
        with TemporaryBoard(rows, files) as root:
            result = run_validator(str(root))
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("unknown status FINISHED", result.stderr)

    def test_a_title_mismatch_is_rejected(self):
        rows = ["| 00 | [Example](mvp/00-example.md) | DONE | — |\n"]
        files = {"mvp/00-example.md": task("00", "Another title")}
        with TemporaryBoard(rows, files) as root:
            result = run_validator(str(root))
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("title differs", result.stderr)

    def test_a_duplicate_identifier_is_rejected(self):
        rows = [
            "| 00 | [Example](mvp/00-example.md) | DONE | — |\n",
            "| 00 | [Copy](mvp/00-copy.md) | DONE | — |\n",
        ]
        files = {
            "mvp/00-example.md": task("00", "Example"),
            "mvp/00-copy.md": task("00", "Copy"),
        }
        with TemporaryBoard(rows, files) as root:
            result = run_validator(str(root))
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Duplicate board identifier: 00.", result.stderr)

    def test_a_missing_board_is_reported(self):
        with tempfile.TemporaryDirectory(prefix="melomae-board-") as directory:
            result = run_validator(directory)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Task board not found", result.stderr)


if __name__ == "__main__":
    unittest.main()
