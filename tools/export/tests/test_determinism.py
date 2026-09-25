import subprocess
import sys
from pathlib import Path

import pytest

# The probe imports export.py, which imports torch.
pytest.importorskip("torch")

EXPORT_DIR = Path(__file__).resolve().parents[1]


def test_script_and_package_paths_import_the_same_wrapper():
    # The dynamo exporter writes the wrapper's module path into node
    # metadata. A different module path changes the ONNX file bytes.
    probe = (
        "import runpy, sys; "
        f"sys.path.insert(0, {str(EXPORT_DIR)!r}); "
        f"g = runpy.run_path({str(EXPORT_DIR / 'export.py')!r}); "
        "print(g['RealApollo'].__module__)"
    )
    result = subprocess.run(
        [sys.executable, "-c", probe], capture_output=True, text=True, check=True
    )
    assert result.stdout.strip().splitlines()[-1] == "export.real_dsp"
