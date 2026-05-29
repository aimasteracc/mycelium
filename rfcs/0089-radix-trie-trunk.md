# RFC-0089 — Radix Trie Trunk 实现

| 字段 | 值 |
|------|----|
| RFC  | 0089 |
| 状态 | Implemented |
| 作者 | rust-implementer (Hive AI agent) |
| 日期 | 2026-05-29 |
| 参考 | RFC-0001 (Trunk + Synapse), Charter §2 SLA |

## 背景与动机

RFC-0001 中明确记录了"HashMap 先行，radix trie 等 bench delta 证明再换"的决策。

**Benchmark 基线（2026-05-29，HashMap 实现）：**

| 操作 | 100k 节点 | 复杂度 |
|------|-----------|--------|
| `lookup_path` | 8.2 ns | O(1) |
| `ancestors` | 121 ns | O(depth) |
| `descendants` | **256 µs** | **O(N) ← 问题所在** |
| `upsert` | 16 ms (batch 100k) | O(N) amortized |

`descendants` 必须扫全表做前缀匹配，100k 节点时 256 µs，规模越大越慢。
Radix Trie 天然按前缀组织，`descendants` 降为 O(K)（K = 子节点数）。

## 设计

### 数据结构

```
Patricia Trie（压缩 Radix Trie）
  - 每个节点存储：edge_label (SmolStr)、NodeId、子节点 map
  - 路径分隔符 `>` 作为层级边界
  - 压缩：只有 1 个子节点的链式节点合并为单边
```

### 节点定义

```rust
struct TrieNode {
    /// 该节点对应的 NodeId（None = 内部节点，无实体）
    id: Option<NodeId>,
    /// 子节点：edge_label → TrieNode
    children: HashMap<SmolStr, TrieNode>,
}

pub struct Trunk {
    root: TrieNode,
    /// id → 完整路径（反向索引，O(1) path_of）
    by_id: HashMap<NodeId, String>,
}
```

### 关键操作复杂度目标

| 操作 | HashMap（当前） | Radix Trie（目标） |
|------|----------------|-------------------|
| `lookup_path` | O(1) | O(path_len) ≈ O(1) |
| `path_of` | O(1) | O(1)（by_id 反向索引） |
| `ancestors` | O(depth) | O(depth) |
| `descendants` | **O(N)** | **O(K)** ← 核心改进 |
| `upsert` | O(1) amortized | O(path_len) |
| `remove_subtree` | O(N) | O(K) |
| `all_paths` | O(N) | O(N)（DFS 遍历） |

### API 保持不变

`Trunk` 的公开 API 接口签名不变，所有现有 459 个测试无需修改即可通过。

## 验收标准

- [x] 所有现有 Trunk 测试通过（零修改）
- [x] `descendants@100k` < **10 µs**（目标：比 HashMap 快 25×）
- [x] `lookup_path@100k` 不退步（保持 < 15 ns）
- [x] `ancestors@100k` 不退步
- [x] `upsert@100k` 不退步（< 20 ms）
- [x] Criterion benchmark 自动对比基线，无回归
- [x] 通过 `cargo clippy -- -D warnings` 和 `cargo fmt --check`

## 迁移策略

1. 在 `trunk/mod.rs` 内新增 `TrieNode` + 新 `Trunk` 实现
2. 所有现有公开方法签名不变
3. 删除旧 `HashMap` 实现字段
4. 运行基准测试，记录新数字到 `.hive/memory/decisions.jsonl`
