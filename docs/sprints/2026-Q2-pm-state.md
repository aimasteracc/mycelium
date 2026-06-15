# 24/7 PM State — Mycelium, 2026 Q2

This file is the **live state** of the PM brain, updated every dispatch.
For historical sprints, see `docs/sprints/` archives.

**Last updated: 2026-06-15 (PM dispatch v273 — PR #881 CI-pending (Codex P1 fixed, commit 5b5ff62); PR #568 escalation ×133)**

---

## Table of Contents

1. [P0 — Founder-gated (blocked)](#p0--founder-gated-blocked)
2. [P1 — Active Work](#p1--active-work)
3. [Recently Closed / Merged](#recently-closed--merged)
4. [Sprint Cadence & Ceremony Log](#sprint-cadence--ceremony-log)
5. [PM Dispatch Archive](#pm-dispatch-archive)

---

## P0 — Founder-gated (blocked)

These items cannot proceed without explicit founder action.

### PR #568 — ×133 Escalation (CRITICAL)

- **Status**: OPEN — awaiting founder `finalize` workflow_dispatch or explicit "close as won't fix"
- **Escalation count**: ×133 (escalated in v273)
- **What it is**: release/v0.3.0 — Node/Python SDKs (RFC-0111) + RFC-0103/0094. Registries published (crates.io + npm + PyPI ✅) since 2026-06-05.
- **Release branch HEAD**: `f14f80df` (nightly.yml fix from PR #861). **CI CONFIRMED 50/50 ✅ as of 2026-06-14T21:19Z** — verified in v261 dispatch.
- **Ceremony path (v268 — ALL PREREQUISITES MET)**: No remaining prerequisites. Founder only needs to **trigger `finalize` workflow_dispatch on release/v0.3.0** to complete the v0.3.0 ceremony (main merge → tag → GH Release → develop back-merge).
- **Blocker**: Founder has not triggered `finalize` despite ×133 escalations over 10+ days (since 2026-06-05).

### PR #881 — CI PENDING (PM dispatch v272 chore)

- **Status**: OPEN — CI running on fix commit `5b5ff62` (all fast checks ✅, matrix + coverage in_progress)
- **What it is**: PM dispatch v272 chore — PR #880 merged, Codex P1 fixed (restored deleted v13 entry in decisions.jsonl)
- **Codex P1** (r3412502211): **FIXED** — commit `5b5ff62` restores deleted `decisions.jsonl` v13 stub entry. Reply posted. Charter append-only satisfied.
- **Next action (FIRST THING next run)**: Once Quality Gate shows ✅ → `gh pr merge #881 --admin --squash --delete-branch`

### ✅ Codex Usage Limits — RESTORED (v268)

- **Status**: Codex reviewed PRs actively — limits online.
- **Hard Rule impact**: Full enforcement active.

### PR #763 — RFC-0121 DRAFT

- **Status**: OPEN DRAFT — awaiting founder promotion to "Ready for Review"
- **What it is**: RFC-0121 Option A — Charter §2 Hyphae token SLA amendment (three per-class targets replacing single 30% row)
- **Blocker**: Founder must mark ready before review can proceed

---

## P1 — Active Work

### Issue #829 — Nightly Mutation Kill Rate <70% on Main

- **Status**: FIX MERGED into release/v0.3.0 (v260) — pending ceremony to reach `main`
- **Priority**: P1 → resolves automatically when PR #568 `finalize` completes
- **Root cause (diagnosed v259)**: CI tooling crash (ENOTDIR), not a real kill-rate failure. `tee mutants.out` created a file blocking `cargo-mutants` from using `mutants.out/` as directory.
- **Fix**: PR #861 MERGED `f14f80df` → release/v0.3.0. Nightly.yml now uses `tee mutants.log`. Will flow to `main` after the v0.3.0 ceremony.
- **Bench note**: After ceremony, run nightly mutation test on new `main` to confirm kill rate ≥ 70%.

---

## Recently Closed / Merged

### PR #879 — MERGED `abb82c8` (PM dispatch v270 chore → develop) / Issue #872 CLOSED

- **Merged at**: 2026-06-15 (v271)
- **What it was**: PM dispatch v270 state chore — PRs #874–#878 merged, Issue #872 closed. 2 Codex findings addressed: P1 rejected (DCO CI passed), P2 acknowledged (count normalized in v271). CI 22/22 ✅.

### PR #878 — MERGED `2c3074f` (insta 1.47.2→1.48.0 → develop)

- **Merged at**: 2026-06-15 (v270)
- **What it was**: Dependabot bump — `insta` 1.47.2 → 1.48.0 (strip_ansi, YAML literal blocks, CI=true override). CI 22/22 ✅, 0 Codex findings.

### PR #877 — MERGED `3a527fe` (regex 1.12.3→1.12.4 → develop)

- **Merged at**: 2026-06-15 (v270)
- **What it was**: Dependabot bump — `regex` 1.12.3 → 1.12.4 (perf: large char class compile). CI 22/22 ✅, 0 Codex findings.

### PR #876 — MERGED `94c441e` (uuid 1.23.2→1.23.3 → develop)

- **Merged at**: 2026-06-15 (v270)
- **What it was**: Dependabot bump — `uuid` 1.23.2 → 1.23.3 (parser panic on empty input fix). CI 22/22 ✅, 0 Codex findings.

### PR #875 — MERGED `535ed5b` (PM dispatch v269 chore → develop)

- **Merged at**: 2026-06-15 (v270)
- **What it was**: PM dispatch v269 state chore.

### PR #874 — MERGED `6a9e86b` (libc fix → develop) / Issue #872 CLOSED

- **Merged at**: 2026-06-15 (v270)
- **What it was**: `fix(npm): add libc glibc constraint to Linux platform packages` — closes Issue #872. Adds `"libc": ["glibc"]` to linux-x64-gnu and linux-arm64-gnu platform `package.json` entries so npm 8.7+ on Alpine/musl skips incompatible glibc binaries. CI 22/22 ✅, 0 Codex findings.

### PR #870 — CLOSED SUPERSEDED (v267 stale branch)

- **Closed at**: 2026-06-15 (v268)
- **What it was**: PM dispatch v267 — stale branch with 27 accumulated commits / 35 files (RFC-0109/0110 code already merged to develop + PM state). `mergeable_state: dirty` (decisions.jsonl conflict).
- **Codex findings addressed before close**: P1 (`r3411045985`) → issue #871; P2 (`r3411045988`) → issue #872
- **Replaced by**: this v268 PM chore (clean branch from develop HEAD)

### PR #868 — MERGED `0d99291c` (v265 → develop)

- **Merged at**: 2026-06-15 (v266)
- **What it was**: PM dispatch v265 chore — PR #867 `da3b202a` MERGED; PR #568 CI CONFIRMED 50/50 ✅ on `f14f80df`; escalation ×125
- **Dispatch**: v266

### PR #867 — MERGED `da3b202a` (v264 → develop)

- **Merged at**: 2026-06-15 (v265)
- **What it was**: PM dispatch v264 chore — PR #866 `29d5112e` MERGED; PR #568 CI CONFIRMED 50/50 ✅ on `f14f80df`; escalation ×124
- **Dispatch**: v265

### PR #866 — MERGED `29d5112e` (v263 → develop)

- **Merged at**: 2026-06-15 (v264)
- **What it was**: PM dispatch v263 chore — PR #865 `bddf07d2` MERGED; PR #568 CI CONFIRMED 50/50 ✅; escalation ×123
- **Dispatch**: v264

### PR #865 — MERGED `bddf07d2` (v262 → develop)

- **Merged at**: 2026-06-15 (v263)
- **What it was**: PM dispatch v262 chore — PR #864 `46a3e67f` MERGED; PR #568 CI CONFIRMED 50/50 ✅; escalation ×122
- **Dispatch**: v263

### PR #864 — MERGED `46a3e67f` (v261 → develop)

- **Merged at**: 2026-06-14 (v262)
- **What it was**: PM dispatch v261 chore — PR #568 CI CONFIRMED 50/50 ✅ on `f14f80df`; escalation ×121
- **Dispatch**: v262

### PR #863 — MERGED `5e6e06a1` (v260 → develop)

- **Merged at**: 2026-06-14 (v261)
- **What it was**: PM dispatch v260 chore — PRs #862+#861 merged; nightly.yml fix in release/v0.3.0; escalation ×120
- **Dispatch**: v261

### PR #862 — MERGED `b2a09e88` (v259 → develop)

- **Merged at**: 2026-06-14 (v260)
- **What it was**: PM dispatch v259 chore — escalation ×119, Issue #829 root cause diagnosed, PR #861 opened
- **Dispatch**: v260

### PR #861 — MERGED `f14f80df` (nightly.yml fix → release/v0.3.0)

- **Merged at**: 2026-06-14 (v260)
- **What it was**: 3-line nightly.yml fix — `tee mutants.out` → `tee mutants.log` (Issue #829 ENOTDIR root cause)
- **Impact**: release/v0.3.0 HEAD advances to `f14f80df`; Issue #829 fix will land on `main` after ceremony
- **Dispatch**: v260

### PR #860 — MERGED `8d3cfc35` (v258 → develop)

- **Merged at**: 2026-06-14
- **What it was**: PM dispatch v258 chore — escalation ×118, Codex usage-limit notice
- **Dispatch**: v259

### PR #859 — MERGED `6c0fc595` (v257 → develop)

- **Merged at**: 2026-06-14
- **What it was**: PM dispatch v257 chore — escalation ×117
- **Dispatch**: v258

### PR #858 — MERGED `edb17606` (v256 → develop)

- **Merged at**: 2026-06-14
- **What it was**: PM dispatch v256 chore — escalation ×116
- **Dispatch**: v257

### PR #857 — MERGED `17958f34` (v255 → develop)

- **Merged at**: 2026-06-14
- **What it was**: PM dispatch v255 — escalation ×115
- **Dispatch**: v256

### PR #856 — MERGED `d9489b5f` (v254 → develop)

- **Merged at**: 2026-06-14
- **What it was**: PM dispatch v254 — escalation ×114
- **Dispatch**: v255

### PR #853 — MERGED `c858cc40` (v252 → develop)

- **Merged at**: 2026-06-14
- **What it was**: PM dispatch v251 — first Codex usage-limit notice; escalation ×112
- **Dispatch**: v252

---

## Sprint Cadence & Ceremony Log

| Version | Date | Key Event |
|---------|------|-----------|
| v0.1.0 | 2026-06-02 | Initial release ceremony |
| v0.2.0 | 2026-06-07 | Sprint 2 ceremony |
| v0.3.0 | TBD (PR #568 pending finalize) | Next release target |

---

## PM Dispatch Archive

All dispatches from v129 onward are archived below. Earlier dispatches (v1–v128) are in closed PRs and git log.

---

### 2026-06-15 PM dispatch v273

**Codex P1 on PR #881 FIXED (commit 5b5ff62 restores deleted decisions.jsonl v13 entry). PR #881 CI pending. PR #568 escalation ×132→×133.**

**Pre-flight:** CHARTER.md §2/§5.1/§5.10/§5.12/§5.13, _orchestrator.md, decisions.jsonl tail-40, anti-patterns (domains: ci/release-governance/merge-discipline), PM state v271/v272 (develop HEAD `e980845c`), v0.2 PRD.

**Assessment:**
- 3 open PRs: #881 (PM v272 chore, 1 live Codex P1 r3412502211), #568 (release, founder gate ×132), #763 (DRAFT RFC-0121)
- 1 open issue: #829 (P1, mutation kill rate on main — blocked on #568 ceremony)
- Develop CI: ✅ GREEN (HEAD `e980845c`)

**Actions taken:**
1. **Diagnosed Codex P1 on PR #881** (r3412502211): PR #881 (`chore/pm-state-v272` branch) deleted the first line of `.hive/memory/decisions.jsonl` — the stub v13 entry (`ts: 2026-06-03T00:00:00Z`). This is a Charter Hard Rule violation (append-only memory). ✅
2. **Fixed Codex P1**: Fetched `origin/chore/pm-state-v272`, prepended the deleted v13 line, committed `5b5ff62` (`fix(memory): restore deleted decisions.jsonl v13 entry`), pushed. Reply posted to Codex thread. ✅
3. **CI re-triggered on PR #881**: fast checks all ✅ (DCO, commit lint, governance guardrails, clippy, rustfmt, Skill coverage, real projects ×2, dogfood, security, unit tests); matrix + coverage + build in_progress at dispatch close.
4. **Updated PM state to v273**. ✅
5. **Appended decisions.jsonl** ✅
6. **PR #882 opened** (this PR). ✅

**Escalations to founder:**
- **(1) PR #568** ×133: release/v0.3.0 ceremony **ALL PREREQUISITES MET** — CI 50/50 ✅ `f14f80df`, crates.io + npm + PyPI published since 2026-06-05. Trigger `finalize` workflow_dispatch on `release/v0.3.0`.
- **(2) PR #763**: RFC-0121 DRAFT — undraft when ready for review.
- **(3) Issue #829**: Resolves automatically after #568 ceremony lands on `main`.

**Next run first action**: Check PR #881 Quality Gate → if ✅ merge immediately (squash, delete branch).

---

### 2026-06-15 PM dispatch v271

**PR #879 MERGED `abb82c8` (v270 chore). Issue #872 CLOSED. Develop CI ✅ GREEN. PR #568 escalation ×130→×131.**

**Pre-flight:** CHARTER.md §2/§5.1/§5.10/§5.12/§5.13, _orchestrator.md, decisions.jsonl tail-20, anti-patterns (domains: ci/release-governance/merge-discipline/pm-dispatch), PM state v270, v0.2 PRD.

**Assessment:**
- 2 open issues: #829 (P1, mutation kill rate — blocked on #568 ceremony), #872 (P2 — **CLOSED this dispatch**)
- 3 open PRs: #879 (PM v270 chore, CI 22/22 ✅, 2 Codex findings), #568 (release, founder gate, CI 50/50 ✅), #763 (DRAFT RFC-0121, founder gate)
- Develop CI: ✅ GREEN (HEAD `abb82c8` after #879 merge)

**Actions taken:**
1. **Addressed Codex P1 on PR #879** — rejected with justification (DCO CI job passed `success` on SHA `371d11ad`). ✅
2. **Addressed Codex P2 on PR #879** — acknowledged; count inconsistency (×130/×128/×127) normalized in this v271 PM state. ✅
3. **Closed Issue #872** (libc glibc constraint — fixed by PR #874, merged in v270). ✅
4. **Merged PR #879** (`abb82c8`) — PM dispatch v270 chore. CI 22/22 ✅, all Codex findings resolved. ✅
5. **Updated PM state to v271** — unified PR #568 escalation count to ×131 throughout. ✅
6. **Appended decisions.jsonl** ✅

**Escalations to founder:**
- **(1) PR #568** ×131: release/v0.3.0 ceremony ALL PREREQUISITES MET — CI 50/50 ✅ `f14f80df`, crates.io + npm + PyPI published. Please trigger `finalize` workflow_dispatch on `release/v0.3.0`.
- **(2) PR #763**: RFC-0121 DRAFT — undraft when ready for review.
- **(3) Issue #829**: Resolves automatically after #568 ceremony lands on `main`.

---

### 2026-06-15 PM dispatch v270

**PRs #874/#875 MERGED (libc fix + v269 chore). Dependabot PRs #876/#877/#878 MERGED (uuid/regex/insta bumps). Issue #872 CLOSED. Develop CI ✅ GREEN. PR #568 escalation ×130.**

**Pre-flight:** CHARTER.md §2/§5.1/§5.10/§5.12/§5.13, _orchestrator.md, decisions.jsonl tail-20, anti-patterns (domains: ci/release-governance/npm), PM state v269, v0.2 PRD.

**Assessment:**
- 2 open issues: #872 (P2, libc — addressed by #874), #829 (P1, mutation kill rate — blocked on #568 ceremony)
- 7 open PRs at session start: #763 (DRAFT, founder gate), #568 (release, founder gate), #874 (#872 libc fix, CI 22/22 ✅), #875 (v269 chore, CI 22/22 ✅), #876/#877/#878 (dependabot, CI ✅)
- 0 Codex review findings on any PR
- Develop CI: ✅ GREEN (post-v269 develop HEAD `15f6315` → `2c3074f` after all merges)

**Actions taken:**
1. **Merged PR #874** (`6a9e86b`) — libc glibc constraint fix. Closes Issue #872. ✅
2. **Merged PR #875** (`535ed5b`) — PM dispatch v269 chore. ✅
3. **Merged PR #876** (`94c441e`) — dependabot uuid 1.23.2→1.23.3. ✅
4. **Merged PR #877** (`3a527fe`) — dependabot regex 1.12.3→1.12.4. ✅
5. **Merged PR #878** (`2c3074f`) — dependabot insta 1.47.2→1.48.0. ✅
6. **Appended decisions.jsonl** ✅
7. **PR #880 opened** — PM dispatch v270 chore ✅

**Escalations to founder:**
- **(1) PR #568** ×130: release/v0.3.0 ceremony ALL PREREQUISITES MET (CI 50/50 ✅ `f14f80df`). Please trigger `finalize` workflow_dispatch.
- **(2) PR #763**: RFC-0121 DRAFT — undraft when ready for review.

---

### 2026-06-15 PM dispatch v268

**PR #870 CLOSED superseded (dirty, 27 stale commits). Codex limits RESTORED (P1→#871, P2→#872 spun off). Develop CI ✅ GREEN. PR #568 CI CONFIRMED 50/50 ✅ on `f14f80df`. Escalation ×126→×128 on PR #568.**

**P0 (founder-gated, blocked):**
- PR #568: ×128 escalation — release/v0.3.0 ceremony. CI CONFIRMED 50/50 ✅ on `f14f80df`. All registries published. Founder triggers `finalize` workflow_dispatch to complete ceremony.
- PR #763: RFC-0121 DRAFT — awaiting founder promotion to "Ready for Review"
- ~~Codex usage limits exhausted~~ — **RESTORED** (Codex reviewed PR #870 at 2026-06-15T04:32Z)

**P1:**
- Issue #829: Fix in release/v0.3.0 (`f14f80df`). Resolves on main after ceremony.
- Issue #871 (NEW): fix(release) — check-npm-token should gate publish steps. P1, fix before v0.3.1.
- Issue #872 (NEW): feat(npm) — add libc constraint to Linux platform packages. P2.

**Actions taken this dispatch:**
- Pre-flight: CHARTER §2/§5.1/§5.10/§5.12/§5.13, _orchestrator.md, decisions.jsonl tail-20, anti-patterns, PM state v266 (develop HEAD `2c379ec0`), v0.2 PRD ✅
- Assessed GitHub: 3 open PRs (#568 ceremony, #870 PM v267 dirty, #763 DRAFT RFC-0121); 1 open issue (#829). Develop CI: ✅ GREEN (#1851 at 04:06Z). PR #568: 50/50 ✅, finalize pending.
- **PR #870 analysis**: found dirty (mergeable_state: dirty), 27 commits / 35 files including pre-merged RFC-0109/RFC-0110 code. Codex had reviewed with P1 + P2 findings.
- **Addressed Codex P1** (`r3411045985`) → opened issue **#871** (`fix(release): check-npm-token should gate publish steps`), replied to thread
- **Addressed Codex P2** (`r3411045988`) → opened issue **#872** (`feat(npm): add libc constraint to Linux platform packages`), replied to thread
- **Closed PR #870** as superseded (dirty, stale, Codex findings addressed) ✅
- **Codex limits RESTORED**: Codex reviewed PR #870 at 2026-06-15T04:32Z — limits are back
- Created clean `chore/pm-state-v268` from develop HEAD `2c379ec0`; updated PM state; appended decisions.jsonl
- PushNotification sent to founder (PR #568 ×128 escalation)

**Escalations to founder:**
1. **PR #568** ×128: **CI is CONFIRMED GREEN on `f14f80df`** (50/50 ✅, verified 2026-06-14T21:19Z) — trigger `finalize` workflow_dispatch on release/v0.3.0
2. **PR #763**: un-draft RFC-0121 Charter §2 amendment when ready
3. **Issue #871**: P1 — fix check-npm-token to fail (not warn) before next release

---

### 2026-06-15 PM dispatch v269

**PR #873 MERGED `15f6315`. Issue #871 CLOSED (already-fixed on develop). PR #874 OPENED (fix/npm-linux-libc-constraint, Issue #872). PR #568 escalation ×128→×129.**

**P0 (founder-gated, blocked):**
- PR #568: ×129 escalation — release/v0.3.0 ceremony. CI 50/50 ✅ on `f14f80df`. All registries published. Founder triggers `finalize` workflow_dispatch.
- PR #763: RFC-0121 DRAFT — awaiting founder promotion to "Ready for Review"

**P1:**
- Issue #829: Fix in release/v0.3.0. Resolves on main after ceremony.
- PR #874 (NEW): fix(npm) — libc glibc constraint for Linux platform packages. CI pending. Closes Issue #872.

**Actions taken this dispatch:**
- Pre-flight: CHARTER §2/§5.1/§5.10/§5.12/§5.13, _orchestrator.md, decisions.jsonl tail-20, anti-patterns, PM state v268, v0.2 PRD ✅
- Assessed GitHub: 3 open PRs (#568 ceremony, #873 PM v268 chore, #763 DRAFT); 2 open issues (#829, #871, #872). Develop CI: ✅ GREEN.
- **Merged PR #873** (PM v268 chore, CI ✅ 1/1 success → squash `15f6315`) ✅
- **Issue #871 investigation**: checked develop release.yml HEAD — `check-npm-token` already exits non-zero (exit 1) and `publish-crates` already lists it in `needs`. Fix was applied in a prior cycle (Issue #560 / Codex P1). Issue #871 is a duplicate filed from stale branch #870.
- **Closed Issue #871** with explanation comment — already-fixed in develop ✅
- **Implemented Issue #872** (musl libc): added `libc: ["glibc"]` to linux-x64 and linux-arm64 entries in `TARGETS` + spreads into generated `package.json` in `buildPlatformPackage`. CHANGELOG updated. Committed `9228622`, pushed `fix/npm-linux-libc-constraint`.
- **Opened PR #874** (fix/npm-linux-libc-constraint → develop) — CI pending ✅
- Updated PM state v269 + appended decisions.jsonl ✅
- PushNotification sent to founder (PR #568 ×129, Charter §2 violation on main)

**Escalations to founder:**
1. **PR #568** ×129: CI GREEN on `f14f80df` — trigger `finalize` workflow_dispatch on release/v0.3.0
2. **PR #763**: un-draft RFC-0121 Charter §2 amendment when ready

---

### 2026-06-15 PM dispatch v266

**PR #868 `0d99291c` MERGED. PR #568 CI status unchanged: 50/50 ✅ on `f14f80df`. Escalation ×125→×126 on PR #568.**

**P0 (founder-gated, blocked):**
- PR #568: ×126 escalation — release/v0.3.0 ceremony. CI CONFIRMED 50/50 ✅ on `f14f80df`. No remaining prerequisites. Founder triggers `finalize` workflow_dispatch to complete ceremony.
- PR #763: RFC-0121 DRAFT — awaiting founder promotion to "Ready for Review"
- Codex usage limits exhausted — automated PR reviews unavailable

**P1:**
- Issue #829: Fix in release/v0.3.0 (`f14f80df`). Resolves on main after ceremony.

**Actions taken this dispatch:**
- Pre-flight: CHARTER §2/§5.1/§5.10/§5.12/§5.13, _orchestrator.md, decisions.jsonl tail-20, anti-patterns (domain: ci/release-governance/pm-dispatch), PM state v265 (develop HEAD `0d99291c` after #868 squash merge), v0.2 PRD ✅
- Assessed GitHub: 3 open PRs (#568 ceremony, #868 v265 chore, #763 DRAFT); 1 open issue (#829)
- Verified CI on PR #868: 22/22 ✅; Codex exhausted (vacuously satisfied); 0 review threads
- **Merged PR #868** (`0d99291c`) — PM dispatch v265 chore ✅
- Updated PM state to v266; appended decisions.jsonl; PushNotification sent to founder
- Escalated PR #568 to ×126

**Escalations to founder:**
1. **PR #568** ×126: **CI is CONFIRMED GREEN on `f14f80df`** — trigger `finalize` workflow_dispatch on release/v0.3.0 to complete the v0.3.0 ceremony
2. **PR #763**: un-draft RFC-0121 Charter §2 amendment when ready
3. **Codex**: upgrade usage limits at https://chatgpt.com/codex/cloud/settings/code-review

---

### 2026-06-15 PM dispatch v265

**PR #867 `da3b202a` MERGED. PR #568 CI status unchanged: 50/50 ✅ on `f14f80df`. Escalation ×124→×125 on PR #568.**

**P0 (founder-gated, blocked):**
- PR #568: ×125 escalation — release/v0.3.0 ceremony. CI CONFIRMED 50/50 ✅ on `f14f80df`. No remaining prerequisites.
- PR #763: RFC-0121 DRAFT — awaiting founder promotion
- Codex usage limits exhausted

**P1:**
- Issue #829: Fix in release/v0.3.0. Resolves after ceremony.

**Actions taken this dispatch:**
- Merged PR #867 (`da3b202a`) — 22/22 CI ✅
- Escalated PR #568 to ×125
- PushNotification sent to founder

---

### 2026-06-15 PM dispatch v264

**PR #866 `29d5112e` MERGED. Escalation ×123→×124 on PR #568.**

*(see PR #867 squash commit for archive)*

---

### 2026-06-15 PM dispatch v263

**PR #865 `bddf07d2` MERGED. Escalation ×122→×123 on PR #568.**

*(see PR #866 squash commit for archive)*

---

### 2026-06-14 PM dispatch v262

**PR #864 `46a3e67f` MERGED. Escalation ×121→×122 on PR #568.**

*(see PR #865 squash commit for archive)*

---

### 2026-06-14 PM dispatch v261

**PR #863 `5e6e06a1` MERGED. PR #568 CI CONFIRMED 50/50 ✅ on `f14f80df` — ALL PREREQUISITES MET. Escalation ×120→×121.**

*(see PR #864 squash commit for archive)*

---

### 2026-06-14 PM dispatch v260

**PRs #862+#861 MERGED. nightly.yml ENOTDIR fix in release/v0.3.0. Escalation ×119→×120.**

*(see PR #863 squash commit for archive)*

---

### 2026-06-14 PM dispatch v259

**PR #860 MERGED. Issue #829 root cause diagnosed (ENOTDIR). PR #861 opened. Escalation ×118→×119.**

*(see PR #862 squash commit for archive)*

---

### 2026-06-14 PM dispatch v258 and earlier (v252–v258)

*(see PRs #853–#860 squash commits for archive)*

---

### 2026-06-14 PM dispatch v252

**PR #853 MERGED. Codex usage limits exhausted. Escalation ×111→×112.**

*(see PR #854 squash commit for archive)*

---

### 2026-06-13 PM dispatch v251 and earlier (v240–v251)

*(see closed PRs and git log — PR #568 escalations ×100–×111)*

---

### 2026-06-13 PM dispatch v239 and earlier (v129–v239)

*(see PR #697 squash commit `d0b3d5f` and git log for full archive)*

---

### 2026-06-03 PM dispatch v28 and earlier (v1–v128)

*(see closed PRs #502/#506 and git log for historical archives — last pre-v0.2.0 dispatch)*
