# RFC-0004 — Hyphae Executor (Phase 2)

| 字段 | 值 |
|------|----|
| RFC  | 0004 |
| 状态 | Implemented |
| 作者 | rust-implementer (Hive AI agent) |
| 日期 | 2026-05-29 |
| 参考 | RFC-0003 (Hyphae lexer + parser) |

## 背景

RFC-0003 完成了 Hyphae DSL 的词法器和解析器，产出 `Ast` 类型。
本 RFC 实现执行层：把 `Ast` 在 `Store` 上求值，返回匹配的符号路径集合。

这是达成 Charter §2 **≤ 30% AI token 效率 SLA** 的关键路径：
AI 用一条 Hyphae 查询（如 `.function:calls(.class)`，20 字符）
取代多次 JSON API 调用（每次 ~1000 token），同时引擎只返回 AI
要求的字段，大幅压缩响应 token 数量。

## 设计

### 核心类型

```rust
/// 执行一个 Hyphae Ast 并返回匹配的符号路径。
pub struct Evaluator<'s> {
    store: &'s Store,
}

impl<'s> Evaluator<'s> {
    pub fn new(store: &'s Store) -> Self;

    /// 对 `ast` 求值，返回去重后按字母序排列的路径列表。
    pub fn eval(&self, ast: &Ast) -> Vec<String>;
}
```

### 语义

#### 选择器

| Hyphae | 语义 | 实现 |
|--------|------|------|
| `#name` | 精确匹配名称段（路径最后一段 == name） | `Store::all_symbols` 过滤 |
| `.kind` | 匹配节点类型 | `Store::symbol_count_by_kind` → `Store::all_symbols` |
| `*` | 匹配所有符号节点 | `Store::all_symbols(None, None)` |

#### 伪类

| 伪类 | 语义 | 实现 |
|------|------|------|
| `:calls(arg)` | 调用了 arg 选择器命中集合中任意节点的符号 | Synapse outgoing Calls |
| `:callers(arg)` | 被 arg 命中的节点调用的符号 | Synapse incoming Calls |
| `:imports(arg)` | 导入了 arg 命中节点的符号 | Synapse outgoing Imports |
| `:extends(arg)` | 继承了 arg 命中节点的符号 | Synapse outgoing Extends |

#### 组合器

| 组合器 | 语义 | 实现 |
|--------|------|------|
| `A > B` | B 的直接父节点是 A 命中的节点 | Trunk ancestors |
| `A B`（空格） | A 命中的节点的任意后代 | Trunk descendants |
| `A ~ B` | 与 A 有相同直接父节点的 B | Trunk siblings |

#### 逗号列表

`A, B` 返回 A 和 B 的并集（去重）。

### MCP 集成

新工具 `mycelium_query`：

```json
请求: { "query": ".function:calls(.class)", "limit": 20 }
响应: { "results": ["src/auth.rs>login", ...], "count": 5, "query": "..." }
错误: { "error": "parse error: ..." }
```

这是达成 ≤ 30% token SLA 的接口：AI 用短查询换精准结果。

## 验收标准

- [x] `#name` 精确匹配符号名称段
- [x] `.kind` 按节点类型过滤
- [x] `*` 返回所有符号
- [x] `:calls(arg)` 找到调用 arg 结果集的符号
- [x] `:callers(arg)` 找到被 arg 结果集调用的符号
- [x] `A > B` 组合器：B 的直接父节点在 A 集合中
- [x] `A B` 组合器：B 是 A 集合的后代
- [x] `A, B` 逗号：A 和 B 的并集去重
- [x] 空查询返回空列表
- [x] 解析错误返回 `{ "error": "..." }`
- [x] `mycelium_query` MCP 工具正常工作
- [x] 结果去重 + 字母序排列
- [x] 所有先前测试继续通过
