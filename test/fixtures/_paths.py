"""Repository root paths for tests (stable across test/unit/* and test/integration/*)."""
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
PROXY_SCRIPT = REPO_ROOT / "proxy" / "core" / "csp_proxy.py"
