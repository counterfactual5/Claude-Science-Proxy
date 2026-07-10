import ast
import json
import pathlib
import shutil
import subprocess
import sys
import tempfile
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[3]
PROXY_DIR = ROOT / "proxy"
TAURI_CONF = ROOT / "desktop" / "src-tauri" / "tauri.conf.json"


# Modules that csp_proxy.py directly imports (from proxy.*).
CSP_PROXY_DEPS = {
    "proxy.core.http_transport",
    "proxy.dsml.dsml_shim",
    "proxy.registry.model_discovery",
    "proxy.registry.model_registry",
    "proxy.registry.model_sort",
    "proxy.compat.openai_chat_compat",
    "proxy.compat.responses_compat",
    "proxy.compat.anthropic_compat",
    "proxy.policy.provider_policy",
}

def module_to_path(mod):
    """Convert 'proxy.core.http_transport' -> 'core/http_transport.py'"""
    # Remove 'proxy.' prefix if present
    if mod.startswith("proxy."):
        mod = mod[6:]
    return mod.replace(".", "/") + ".py"


class ProxyPackagingSmoke(unittest.TestCase):
    def test_tauri_proxy_resources_import_from_minimal_bundle_dir(self):
        conf = json.loads(TAURI_CONF.read_text())
        resources = conf["bundle"]["resources"]
        proxy_resources = {
            pathlib.Path(dst).name
            for dst in resources.values()
            if str(dst).startswith("proxy/")
        }
        # Check that all dependencies are bundled
        for dep in CSP_PROXY_DEPS:
            dep_name = pathlib.Path(module_to_path(dep)).name
            self.assertIn(dep_name, proxy_resources, f"Missing bundled dependency: {dep_name}")

        # Smoke test: can we import csp_proxy from a minimal bundle dir?
        old_dont_write = sys.dont_write_bytecode
        sys.dont_write_bytecode = True
        with tempfile.TemporaryDirectory() as td:
                    bundle_root = pathlib.Path(td)
                    for dep in CSP_PROXY_DEPS:
                        src = PROXY_DIR / module_to_path(dep)
                        dst = bundle_root / "proxy" / module_to_path(dep)
                        dst.parent.mkdir(parents=True, exist_ok=True)
                        shutil.copy2(src, dst)
                    # Also copy csp_proxy itself
                    src = PROXY_DIR / "core" / "csp_proxy.py"
                    dst = bundle_root / "proxy" / "core" / "csp_proxy.py"
                    dst.parent.mkdir(parents=True, exist_ok=True)
                    shutil.copy2(src, dst)
                    # Add __init__.py files for package structure
                    for subdir in ("core", "dsml", "registry", "compat", "policy"):
                        (bundle_root / "proxy" / subdir / "__init__.py").write_text("")
                    (bundle_root / "proxy" / "__init__.py").write_text("")
                    env = {
                        "PYTHONDONTWRITEBYTECODE": "1",
                        "PYTHONPATH": str(bundle_root),
                    }
                    try:
                        result = subprocess.run(
                            [sys.executable, "-S", "-c", "import proxy.core.csp_proxy"],
                            cwd=td,
                            env=env,
                            capture_output=True,
                            text=True,
                        )
                        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                    finally:
                        sys.dont_write_bytecode = old_dont_write

    def test_tauri_bundle_includes_ops_scripts_for_packaged_smoke(self):
        conf = json.loads(TAURI_CONF.read_text())
        resources = set(conf["bundle"]["resources"].values())
        self.assertTrue(
            {
                "scripts/sandbox/launch-virtual-sandbox.sh",
                "scripts/sandbox/stop-science-sandbox.sh",
                "scripts/maintenance/doctor.sh",
                "scripts/ci/verify-proxy.sh",
            }.issubset(resources)
        )


if __name__ == "__main__":
    unittest.main()