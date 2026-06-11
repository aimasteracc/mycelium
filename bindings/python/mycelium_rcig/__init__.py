"""mycelium — thin CLI-wrapper SDK for the Mycelium engine (RFC-0111 Phase 2).

Embed the Mycelium code-intelligence engine in any Python app without a Rust
toolchain. Locates the prebuilt ``mycelium`` CLI, spawns it, and returns parsed
JSON::

    from mycelium_rcig import Mycelium
    m = Mycelium(root=".")
    m.index()
    hits = m.query("#login")
"""
from ._client import Mycelium
from ._resolve import resolve_binary
from ._run import MyceliumError

__all__ = ["Mycelium", "MyceliumError", "resolve_binary"]
__version__ = "0.0.0.dev0"
