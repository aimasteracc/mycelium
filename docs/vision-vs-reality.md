# Mycelium — 当初设想 vs. 现在做到（易懂版）

> 把**最初头脑风暴**（2026-05-29 Cowork 启动会）画的蓝图，和**今天 v0.1.16+**
> 的真实状态放在一起，用大白话讲清楚：我们计划做到什么程度，现在做到了哪。
>
> 最后核对：2026-06-03（已逐条对照源码核实）。
> 更新（2026-06-03 晚）：**最后一公里的主动推送已闭环** —— RFC-0106
> (`mycelium/graphChanged` 推送) / RFC-0107 (作用域 delta 订阅 + CLI
> `watch --subscribe`) / RFC-0108 (反应式查询订阅) 已落地，据此从「❌ 差主动
> 推送」更新为 ✅。
> 来源：`wiki/wiki/ai-tech/mycelium-project-bootstrap.md` ·
> `wiki/raw/ai-tech/mycelium-cowork-session-2026-05-29.md`

---

## 三个原始比喻 → 落成了什么（创始人最初的构思框架）

| 比喻 | 落成 | 状态 |
|---|---|---|
| **jQuery**（选择器选节点） | **Hyphae 查询语言** —— CSS 选择器风格选**代码符号**（RFC-0003 / ADR-0006）+ 专门的 **jQuery 扩展**（RFC-0091，7 种 jQuery 式形式）。`class.AuthService > method:async:calls(UserRepo)`，伪类 `:calls()/:has()/:not()/:in()` 已实装，`mycelium query` 三端齐全。 | ✅ |
| **virtual DOM**（只 diff/patch 变化节点） | **反应式符号图**：Trunk（层级树）+ Synapse（关系网）是内存里的"虚拟镜像"；watch-mode（RFC-0008）文件一改**只重建该文件切片并原子换入**——正是 virtual DOM 的"只 patch 变化节点"。底层已翻成 **redb 默认**（mmap、有界内存）。**主动推送也已闭环**：`mycelium/graphChanged` server-initiated 通知 + 作用域订阅增量（RFC-0106/0107/0108）。 | ✅ 含主动推送 |
| **zen coding / Emmet**（极简 DSL 表达） | **AI 原生紧凑序列化**（RFC-0001/0004）：输出紧凑 DSL（text/TOON、msgpack），比 JSON **省约 70% token**（Charter §2 ≤30% JSON 硬指标），`mycelium_get_token_stats` 可验证。 | ✅ |

---

## 一句话总结

> 当初的比喻：**给 10 万本书的图书馆做一张「魔法目录卡」**，而且是一张
> **会自己更新的活卡片**——你一改代码，卡片自动跟着变。

好消息：连蓝图的**灵魂功能（活卡片 / 反应式增量更新）核心都做出来了**
（RFC-0008 watch-mode 已实现：`notify` 文件监听 + 防抖 + 单文件增量重建）。

两块原本还差的，现在也补上了：
- **watch CLI 端**——`mycelium watch` 子命令已落地（`crates/mycelium-cli/src/watch.rs`），
  含 `--subscribe <SPEC>`（RFC-0107 D5），达成"三端齐全"。
- **主动推送**——服务器侦测到变化后**主动通知** AI agent 已闭环：`mycelium/graphChanged`
  推送（RFC-0106）+ 作用域 per-batch delta 订阅（RFC-0107）+ 反应式查询订阅（RFC-0108）。
  从"卡片自动更新好了但 agent 得自己再查"（pull）升级成"卡片一变就主动喊 agent"（push）。

换句话说：**主体蓝图兑现，广度严重超额，且最后一公里的主动推送也闭环了**。

---

## 蓝图原文里的关键设计（头脑风暴当天敲定的）

技术栈四件套：
- **Rust** —— 性能 + 安全 ✅
- **tree-sitter** —— 语法解析，目标 **20 种语言** 🟡（现 10 种）
- **Salsa** —— 增量计算框架，"只重算改动部分" → 🟡 **已接入但只到 Phase 1**：
  `crates/mycelium-core/src/cortex.rs`（RFC-0011）搭好了 `#[salsa::db]` 数据库
  骨架 + 第一个 tracked query（`dependency_depth`）；"大多数查询仍直接跑在
  `Store` 上，按 RFC 逐个迁移"。增量"只重算改动部分"的目标另由自研 `notify`
  防抖 watch 循环达成（RFC-0008 watch-mode，已 Implemented）
- **BLAKE3** —— NodeId 哈希，低 8 位留 shard tag ✅

三个不可推翻的工程决策（写进 `decisions.jsonl`）：
1. **HashMap 先行**，bench 证明了再换 radix trie → ✅ **已换成 Patricia/radix trie**
2. NodeId = BLAKE3 低 64 位，低 8 位永远 0（留 shard tag）→ ✅
3. 路径分隔符 `>`，只禁空段+控制字符 → ✅

两根数据结构支柱：
- **Trunk**（树干）—— 管"谁在谁里面"的层级树 ✅
- **Synapse**（突触）—— 管"谁引用谁"的双向关系网 ✅

---

## 计分卡：计划 vs. 现状

图例：✅ 完成   🟡 部分   ⚠️ 换路线达成   ❌ 没做

### 一、地基（核心数据结构 + 第一轮 spike）

| 计划 | 状态 | 现在的真相 |
|---|---|---|
| Trunk 层级树（upsert/lookup/ancestors/descendants） | ✅ | 已从 HashMap 升级到 **Patricia/radix trie** |
| Synapse 双向关系网，多 EdgeKind | ✅ | 每种 edge kind 独立邻接表 |
| NodeId = BLAKE3 + shard tag | ✅ | 有专门测试守护字节不变量 |
| TDD（先红后绿） | ✅ | 全程强制，pre-commit hook 把关 |
| 19 NodeKind / 15 EdgeKind | ✅ | EdgeKind 还新增 `TypeImports`（v0.1.12/0.1.13） |
| MessagePack 持久化（`.mycelium/index.rmp`） | ✅ | `rmp-serde` 往返一致 |

### 二、查询 + 语言（"目录卡能用起来"）

| 计划 | 状态 | 现在的真相 |
|---|---|---|
| 20 种语言 tree-sitter 集成 | 🟡 | **10 种 pack 落地**：c, cpp, csharp, go, java, javascript, python, ruby, rust, typescript（`Language` 枚举预留 20 种，pack 实装 10 种） |
| Hyphae 查询语言（CSS 选择器风格） | ✅ | v1 + jQuery 扩展（RFC-0091），`mycelium query` 端到端可用 |
| CLI：index / query / 各种结构查询 | ✅ | 完整 CLI |
| 单文件改动只重建受影响切片 | ✅ | **RFC-0008 已实现**：watch 事件触发，仅重建该文件 |

### 三、三端齐全（CLI + MCP + Skill）

| 计划 | 状态 | 现在的真相 |
|---|---|---|
| MCP server，全套查询 | ✅ | ~89 个工具 |
| CLI ↔ MCP 严格 1:1 对齐 | ✅ | CI 强制（Three-Surface Rule / RFC-0090） |
| 每个能力都有 Skill 覆盖 | ✅ | 10 个 Skill 分类，100% 覆盖 |
| 发布到 crates.io / npm / PyPI | ✅ | 三个仓库都已发布 |

### 四、"活目录卡"——反应式增量（**蓝图的灵魂**）

| 计划 | 状态 | 现在的真相 |
|---|---|---|
| 增量计算（"只重算改动部分"） | ✅ | watch 触发单文件增量重建（RFC-0008）；Salsa memoization 另在 cortex.rs Phase 1（RFC-0011） |
| 文件监听 file watcher | ✅ | `notify::RecommendedWatcher` + 防抖 + 后台 loop |
| watch 三端齐全（CLI+MCP+Skill） | ✅ | MCP `start_watch`/`stop_watch`/`watch_status` + **CLI `mycelium watch` 子命令已落地**（`crates/mycelium-cli/src/watch.rs`，含 `--subscribe <SPEC>`，RFC-0107 D5） |
| 重建期间查询看到一致快照 | ✅ | 原子换入该文件的 nodes/edges |
| **主动推送 / 订阅传输给 agent** | ✅ | **已闭环**：`mycelium/graphChanged` 通知（RFC-0106，`push.rs`）+ 作用域 per-batch delta 订阅（RFC-0107，`subscription.rs`）+ 反应式查询订阅（RFC-0108，Salsa Phase 2）。469 处 `subscribe`，`contract_subscription.rs` 守护 |

### 五、规模 + 打磨

| 计划 | 状态 | 现在的真相 |
|---|---|---|
| 10 万节点**查询**性能 SLA | ✅ | <30s，多个 SLA 测试；**redb 路径新增 100k SLA 门禁**（warm <5ms/<1ms，nightly `redb-sla-100k`，RFC-0100 Phase 3）。Charter §2 已按 RFC-0104 标为 warm 契约，冷数字待 nightly 实测 |
| 6+ 语言 | ✅ | 10 种（达标） |
| 治理体系（CHARTER/RFC/ADR/CI） | ✅ | RFC 至 0104 + 8 ADR + 完整发布仪式 + **supersede 守卫（CI 强制）** |
| 大规模 index（几十万文件） | 🟡 | **R2 增量持久化 + R3 内存边界已解决**：redb 已翻为**默认**后端（mmap 有界内存 + 按文件 ACID 增量写，RFC-0100 Phase 3）。剩 **R1 并行抽取**未做（仍串行）。见 scale-gap-analysis.md |
| Salsa 框架 / 分层存储 / LSP | 🟡/❌ | Salsa 接入 Phase 1（cortex.rs，符号抽取半步；边仍全量重抽）；分层存储、LSP 未做 |

### 六、v0.1.13 之后新增（这份文档上次核对后做的）

| 新增 | 状态 | 现在的真相 |
|---|---|---|
| **`mycelium context` 一站式架构上下文**（RFC-0101） | ✅ | 三端齐全；CLI+MCP 共用 `mycelium_core::context` 一个 builder（字节一致 by construction）；related_files/edge_kinds/Hyphae 路由 |
| **自适应输出预算 OutputBudget**（RFC-0102） | ✅ | 移进 `mycelium_core::budget`，CLI+MCP 同一预算同一截断（字节一致） |
| **redb 翻为默认存储后端**（RFC-0100 Phase 3） | ✅ | mmap ACID、有界内存、按文件增量写；老快照仍可读（软迁移）；跨平台 CI 绿 |
| **god-file 收缩**（#428） | 🟡 | `mcp/lib.rs` 12k→5.6k（测试外提）、`redb_backend.rs` 拆出 `redb_codec`；剩 lib.rs 内 tool 实现可继续拆 |

---

## 进度可视化（按当初的路线图阶段）

```
地基 · 核心数据结构        ██████████ 100%  ✅ 已交付（还超额换了 radix trie）
查询 · Hyphae + 语言       ████████░░  80%  🟡 Hyphae✅ + 10/20 语言
三端 · CLI/MCP/Skill       ██████████ 100%  ✅ 三端齐全 + 三仓库发布
反应式 · 增量自动更新       ██████████  95%  ✅ 监听+增量重建+主动推送(RFC-0106/0107/0108)全上线
规模 · SLA/LSP/分层         ██████░░░░  60%  🟡 SLA✅ 10语言✅；LSP/分层存储❌
```

**力气花在哪**（这次诚实复盘后，反应式核心其实也做了，只剩推送）：

```
                    蓝图说的价值    实际投入的力气
反应式核心（监听+增量）  ████████        ███████   ✅ 基本兑现（RFC-0008）
反应式·主动推送          ████            ████      ✅ 最后一公里已闭环（RFC-0106/0107/0108）
广度：语言数量          ██              ███████   10 种
广度：agent 工具         ███             ████████  ~89 工具 · 三端
治理 / 发布仪式          █               ███████   RFC 至 0097 · 全套仪式
```

---

## 诚实结论

**做对了什么**
- 四个架构赌注站得住（其中 Salsa 用自研 watch 循环替代，殊途同归）：Rust ✅、
  tree-sitter ✅、radix trie（如约替换 HashMap）✅、BLAKE3 shard tag ✅。
- **蓝图的灵魂功能（反应式增量）真的做出来了**——RFC-0008 文件监听 + 单文件
  增量重建（MCP 端）。这不是"又一个静态代码图"，warm graph 的核心在了。
- 广度严重超额：10 语言、~90 工具、三端 1:1、三仓库发布、自托管 CI。
- "加一种语言只动 `packs/<lang>/`、不碰核心"的约束守住了。
- **存储愿景兑现**（v0.1.13 之后）：redb 已翻为**默认**（mmap 有界内存 + 按文件
  ACID 增量写）——scale-gap 的 **R2 增量持久化 + R3 内存边界两块解决了**（只剩
  R1 并行抽取）。`mycelium context` 一站式上下文 + OutputBudget 也已三端落地。

**已闭环（原本列为"还差的"，现已落地）**
- **主动推送 / 订阅传输层** —— `mycelium/graphChanged` 推送（RFC-0106）+ 作用域
  delta 订阅（RFC-0107）+ 反应式查询订阅（RFC-0108）。"活卡片"升级成"会主动喊
  你的活卡片"，最后一公里闭环。
- **watch CLI 端** —— `mycelium watch` 子命令落地（含 `--subscribe`），三端齐全达成。

**还差的（明确且不大）**
1. **语言数 10/20** —— 明确但次要的广度缺口。
4. **Salsa 只到 Phase 1** —— cortex.rs 搭好了数据库骨架 + 1 个 tracked query
   （`dependency_depth`），其余查询仍直接跑在 `Store` 上，按 RFC 逐个迁移。

**给下一阶段的战略问题**
- push/subscription 已闭环（RFC-0106/0107/0108），差异化兑现。下一步重心
  转向 **加宽语言（10→20）** 与 **Salsa Phase 2+ 迁移**（更多 tracked query
  从 Store 迁到记忆化）。蓝图措辞偏重的"看不见但谁动一下大家都能感觉到"的
  菌丝网，最后那下"感觉到"（push）已经做出来了。

---

## 源文档

- 项目启动总览：`wiki/wiki/ai-tech/mycelium-project-bootstrap.md`
- 原始 Cowork 头脑风暴日志：`wiki/raw/ai-tech/mycelium-cowork-session-2026-05-29.md`
- 反应式核心 RFC：[`rfcs/0008-watch-mode.md`](../rfcs/0008-watch-mode.md)（Implemented）·
  Salsa 增量记忆化 [`rfcs/0011`](../rfcs/) → `crates/mycelium-core/src/cortex.rs`（Phase 1）
- 当前治理：[`CHARTER.md`](../CHARTER.md) · [`rfcs/`](../rfcs/) · [`docs/adr/`](adr/)
- 当前迭代状态：[`docs/sprints/2026-Q2-pm-state.md`](sprints/2026-Q2-pm-state.md)
