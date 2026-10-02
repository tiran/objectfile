"""Guard against the hand-maintained type stub drifting from the real module.

pyo3-stub-gen does not yet support our pyo3 version, so `_objectfile.pyi` is
written by hand. This checks every public name the compiled module exports is
declared in the stub.
"""

from __future__ import annotations

import re
from pathlib import Path

import objectfile
from objectfile import _objectfile


def test_stub_declares_every_public_name() -> None:
    stub = Path(objectfile.__file__).parent / "_objectfile.pyi"
    text = stub.read_text()
    declared = set(re.findall(r"^(?:class|def)\s+(\w+)", text, re.MULTILINE))

    runtime = {name for name in dir(_objectfile) if not name.startswith("_")}

    missing = runtime - declared
    assert not missing, f"_objectfile.pyi is missing: {sorted(missing)}"
