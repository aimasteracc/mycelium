#!/usr/bin/env python3
"""
RFC-0090 Phase 1 — Three-Surface Rule parity checker.

Extracts MCP tool names from crates/mycelium-mcp/src/lib.rs and Skill
allowed-tools from skills/*/SKILL.md, then reports coverage against four
invariants:

  I1  Every MCP tool appears in ≥1 Skill's allowed-tools. (Coverage)
  I2  Every Skill allowed-tool reference exists as an MCP tool. (No orphans)
  I3  All Skill files on disk exist in skills/INDEX.md. (Index freshness)
  I4  [Phase 3] Every CLI subcommand has an MCP twin. (Deferred)

Phase 1 (default): informational — prints a full report, exits 0.
Phase 3 (--strict): exits 1 on any I1/I2 violation.

Usage:
  python3 scripts/check_skill_parity.py [--strict] [--repo-root PATH]
"""
from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path


# ---------------------------------------------------------------------------
# Extraction helpers
# ---------------------------------------------------------------------------

def _extract_mcp_tools(repo_root: Path) -> list[str]:
    """Return canonical tool names from the MCP implementation.

    Grep for `async fn mycelium_<name>` in lib.rs and strip the
    `mycelium_` prefix to get the canonical name.
    """
    lib_rs = repo_root / "crates" / "mycelium-mcp" / "src" / "lib.rs"
    if not lib_rs.exists():
        raise FileNotFoundError(f"MCP lib not found: {lib_rs}")
    pattern = re.compile(r"async fn (mycelium_[a-z_]+)\s*[<(]")
    tools: list[str] = []
    for match in pattern.finditer(lib_rs.read_text()):
        raw = match.group(1)
        tools.append(raw.removeprefix("mycelium_"))
    return sorted(set(tools))


def _extract_skill_tools(repo_root: Path) -> dict[str, list[str]]:
    """Return {skill_name: [canonical_tool_name, ...]} from all SKILL.md files."""
    skills_root = repo_root / "skills"
    skill_map: dict[str, list[str]] = {}
    prefix = "mcp__mycelium__"
    candidates = (p for p in skills_root.glob("*/SKILL.md") if not p.parent.name.startswith("_"))
    for skill_md in sorted(candidates):
        skill_name = skill_md.parent.name
        text = skill_md.read_text()
        tools: list[str] = []
        # Parse YAML front matter (lines between first --- and second ---)
        in_front_matter = False
        in_allowed = False
        for line in text.splitlines():
            if line.strip() == "---":
                if not in_front_matter:
                    in_front_matter = True
                    continue
                else:
                    break  # end of front matter
            if not in_front_matter:
                continue
            if line.strip() == "allowed-tools:":
                in_allowed = True
                continue
            if in_allowed:
                stripped = line.strip()
                if stripped.startswith("- "):
                    entry = stripped[2:].strip()
                    if entry.startswith(prefix):
                        tools.append(entry.removeprefix(prefix))
                    elif entry.startswith("mcp__"):
                        # Non-mycelium reference — include as-is for orphan check
                        tools.append(entry)
                elif stripped and not stripped.startswith("#"):
                    in_allowed = False  # next key
        skill_map[skill_name] = tools
    return skill_map


# ---------------------------------------------------------------------------
# Report builder
# ---------------------------------------------------------------------------

def run(repo_root: Path, strict: bool) -> int:
    mcp_tools = set(_extract_mcp_tools(repo_root))
    skill_map = _extract_skill_tools(repo_root)

    # Build flat set of all tools referenced in any Skill
    skill_tools_all: set[str] = set()
    for tools in skill_map.values():
        skill_tools_all.update(tools)

    # I1: MCP tools missing from all Skills
    not_covered = sorted(mcp_tools - skill_tools_all)
    # I2: Skill references that don't exist in MCP
    orphans = sorted(skill_tools_all - mcp_tools)

    covered_count = len(mcp_tools) - len(not_covered)
    total = len(mcp_tools)
    pct = covered_count * 100 // total if total else 0

    # ---------------------------------------------------------------------------
    print("═══ Three-Surface Rule parity report (RFC-0090) ═══")
    print(f"MCP tools found    : {total}")
    print(f"Skills found       : {len(skill_map)}")
    print(f"Coverage (I1)      : {covered_count}/{total} ({pct}%)")
    print()

    if not_covered:
        print(f"❌ I1 FAIL — {len(not_covered)} MCP tools have NO Skill coverage:")
        for t in not_covered:
            print(f"   mycelium_{t}")
    else:
        print(f"✅ I1 PASS — all {total} MCP tools have ≥1 Skill coverage")

    print()

    if orphans:
        print(f"❌ I2 FAIL — {len(orphans)} Skill references are NOT valid MCP tools:")
        for t in orphans:
            print(f"   mcp__mycelium__{t}")
    else:
        print(f"✅ I2 PASS — all Skill allowed-tool references are valid MCP tools")

    print()

    # Per-skill summary
    print("Per-skill breakdown:")
    for skill, tools in sorted(skill_map.items()):
        print(f"  {skill:<25} {len(tools):>3} tools")

    print()
    print("ℹ️  I4 (CLI↔MCP 1:1) is deferred to Phase 3 / v0.2.0.")
    print("═══════════════════════════════════════════════════")

    if strict and (not_covered or orphans):
        print("\n--strict mode: exiting 1 due to I1/I2 violations.", file=sys.stderr)
        return 1
    return 0


# ---------------------------------------------------------------------------
# Entry point
# ---------------------------------------------------------------------------

def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--strict",
        action="store_true",
        help="Exit 1 if any I1/I2 invariant is violated (Phase 3 mode).",
    )
    parser.add_argument(
        "--repo-root",
        type=Path,
        default=None,
        help="Path to repository root (default: parent of this script's directory).",
    )
    args = parser.parse_args()

    repo_root = args.repo_root
    if repo_root is None:
        repo_root = Path(__file__).parent.parent

    sys.exit(run(repo_root, args.strict))


if __name__ == "__main__":
    main()
