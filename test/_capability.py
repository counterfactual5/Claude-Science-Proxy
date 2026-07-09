"""Shared capability probe: whether loopback bind+connect is permitted by the environment.
Shared by sub-runners (bash CLI invocation) and unittest (import)."""
import socket


def loopback_available():
    """Whether bind+connect on 127.0.0.1 works. Sandboxes that block loopback return False (no raise)."""
    import os
    if os.environ.get("CSP_FORCE_NO_LOOPBACK") == "1":
        return False
    srv = cli = None
    try:
        srv = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        srv.bind(("127.0.0.1", 0))
        srv.listen(1)
        port = srv.getsockname()[1]
        cli = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        cli.settimeout(1.0)
        cli.connect(("127.0.0.1", port))
        return True
    except OSError:
        return False
    finally:
        for s in (cli, srv):
            try:
                if s:
                    s.close()
            except OSError:
                pass


if __name__ == "__main__":
    import sys
    print("1" if loopback_available() else "0")
    sys.exit(0)
