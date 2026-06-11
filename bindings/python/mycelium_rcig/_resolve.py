"""Binary resolution for the Mycelium SDK (RFC-0111, Python Phase 2).

Locates the ``mycelium`` CLI binary. Resolution order:

1. the ``MYCELIUM_BIN`` environment variable (explicit override),
2. the ``mycelium`` command on ``PATH`` (``shutil.which``),
3. the bare command name, leaving discovery to the OS at spawn time.

All inputs (``env``, ``which``, ``platform``) are injectable so the logic is
unit-testable without a real binary. Python has no per-platform optional-package
mechanism like npm; binary bundling via platform wheels is a future follow-up.
"""
import os
import shutil
import sys


def binary_name(platform=None):
    """The binary file name for a platform (``.exe`` on Windows)."""
    plat = platform if platform is not None else sys.platform
    return "mycelium.exe" if plat.startswith("win") else "mycelium"


def resolve_binary(env=None, which=None, platform=None):
    """Resolve the ``mycelium`` binary path (or a PATH-resolvable command name)."""
    env = os.environ if env is None else env
    which = shutil.which if which is None else which

    override = env.get("MYCELIUM_BIN")
    if override:
        return override

    name = binary_name(platform)
    found = which(name)
    if found:
        return found

    return name
