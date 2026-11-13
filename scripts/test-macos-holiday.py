"""Compile the actual native holiday implementation with its actual sprite/window types."""
from pathlib import Path
import re
import subprocess
import tempfile

root = Path(__file__).resolve().parent.parent
with tempfile.TemporaryDirectory() as folder:
    folder = Path(folder)
    types = "import Foundation\nimport CoreGraphics\nimport simd\n"
    for filename, name in [("SnowSimulation.swift", "Sprite"), ("WindowTracker.swift", "WindowSurface")]:
        source = (root / "Sources/Snowhush" / filename).read_text(encoding="utf-8")
        match = re.search(rf"(?ms)^struct {name} \{{.*?^\}}", source)
        assert match, f"Missing native type {name}"
        types += match[0] + "\n"
    (folder / "Types.swift").write_text(types, encoding="utf-8")
    subprocess.run(["swiftc", str(folder / "Types.swift"),
                    str(root / "Sources/Snowhush/SnowLayer.swift"),
                    str(root / "Sources/Snowhush/HolidayScene.swift"),
                    str(root / "scripts/HolidaySmoke.swift"), "-o", str(folder / "holiday-tests")], check=True)
    subprocess.run([str(folder / "holiday-tests")], check=True)
