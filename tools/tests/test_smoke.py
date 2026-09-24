import tomllib
from pathlib import Path


def test_project_declares_name_and_python_floor() -> None:
    pyproject = Path(__file__).resolve().parents[1] / "pyproject.toml"
    project = tomllib.loads(pyproject.read_text(encoding="utf-8"))["project"]
    assert project["name"] == "melomae-tools"
    assert project["requires-python"].startswith(">=")
