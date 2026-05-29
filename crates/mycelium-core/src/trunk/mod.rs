//! Trunk — 容器树存储层（RFC-0089：Patricia Trie 实现）。
//!
//! 代码容器关系天然是一棵树：文件包含类，类包含方法，方法包含局部项。
//! Trunk 以 Patricia Trie 存储这棵树，使得：
//!
//! - **精确查找**（按路径）为 `O(path_len)` ≈ O(1)（路径长度有界）。
//! - **所有后代** 枚举为 `O(K)`，K = 子树节点数（非全图 O(N) 扫描）。
//! - **所有祖先** 枚举为 `O(depth)`。
//!
//! ## v0.1 → v0.5（RFC-0089）
//!
//! v0.1 的 `HashMap` 实现在 `descendants@100k` 节点时耗时 256 µs（O(N) 全表扫描）。
//! Patricia Trie 按前缀组织，`descendants` 降为 O(K)，目标 < 10 µs@100k。
//!
//! ## 快速示例
//!
//! ```
//! use mycelium_core::trunk::{Trunk, TrunkPath};
//!
//! let mut trunk = Trunk::new();
//!
//! let auth_service = trunk.upsert(TrunkPath::parse("src/auth.rs>AuthService").unwrap());
//! let login = trunk.upsert(TrunkPath::parse("src/auth.rs>AuthService>login").unwrap());
//!
//! // 精确查找
//! assert_eq!(trunk.lookup_path("src/auth.rs>AuthService>login"), Some(login));
//!
//! // login 的祖先包含 AuthService
//! let ancestors: Vec<_> = trunk.ancestors(login).collect();
//! assert_eq!(ancestors, vec![auth_service]);
//!
//! // AuthService 的后代包含 login
//! let descendants: Vec<_> = trunk.descendants(auth_service).collect();
//! assert_eq!(descendants, vec![login]);
//! ```

mod path;
#[cfg(test)]
mod tests;

pub use path::TrunkPath;

use hashbrown::HashMap;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

use crate::types::NodeId;

// ── 内部 Trie 节点 ─────────────────────────────────────────────────────────────

/// Patricia Trie 的一个节点。
///
/// 路径分隔符 `>` 作为层级边界；每段（`>`之间的文本）是 edge label。
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
struct TrieNode {
    /// 若该路径被 `upsert` 过，存储对应的 [`NodeId`]。
    id: Option<NodeId>,
    /// 子节点：下一路径段 → 子节点。
    children: HashMap<SmolStr, Self>,
}

impl TrieNode {
    /// 沿路径段向下导航（不创建节点），返回叶节点的不可变引用。
    fn navigate<'a>(&'a self, segments: &[&str]) -> Option<&'a Self> {
        let mut cur = self;
        for seg in segments {
            cur = cur.children.get(*seg)?;
        }
        Some(cur)
    }

    /// 沿路径段向下导航（不创建节点），返回叶节点的可变引用。
    fn navigate_mut<'a>(&'a mut self, segments: &[&str]) -> Option<&'a mut Self> {
        let mut cur = self;
        for seg in segments {
            cur = cur.children.get_mut(*seg)?;
        }
        Some(cur)
    }

    /// 沿路径段向下导航（沿途自动创建节点），返回叶节点的可变引用。
    fn navigate_or_create<'a>(&'a mut self, segments: &[&str]) -> &'a mut Self {
        let mut cur = self;
        for seg in segments {
            cur = cur.children.entry(SmolStr::new(seg)).or_default();
        }
        cur
    }
}

// ── 递归辅助函数 ───────────────────────────────────────────────────────────────

/// 从 `node` 出发收集所有后代节点的 [`NodeId`]（DFS，不含 `node` 自身）。
fn collect_descendant_ids(node: &TrieNode, out: &mut Vec<NodeId>) {
    for child in node.children.values() {
        if let Some(id) = child.id {
            out.push(id);
        }
        collect_descendant_ids(child, out);
    }
}

/// 从 `root` 按路径段递归移除子树。
///
/// - 若 `segments` 长度为 1，直接从 `root.children` 删除该 key。
/// - 否则递归进入子节点，完成后清理空内部节点。
fn remove_trie_subtree(root: &mut TrieNode, segments: &[&str]) {
    let Some((first, rest)) = segments.split_first() else {
        return;
    };
    if rest.is_empty() {
        root.children.remove(*first);
    } else {
        if let Some(child) = root.children.get_mut(*first) {
            remove_trie_subtree(child, rest);
        }
        // 清理已空的内部节点（no id, no children）。
        // 先持有不可变引用判断，再持有可变引用删除——两次借用不重叠，NLL 允许。
        let should_remove = root
            .children
            .get(*first)
            .is_some_and(|c| c.id.is_none() && c.children.is_empty());
        if should_remove {
            root.children.remove(*first);
        }
    }
}

// ── 公开结构体 ─────────────────────────────────────────────────────────────────

/// Trunk 存储层。
///
/// 以 Patricia Trie 按前缀组织路径，`descendants` 为 O(K)；
/// 以 `by_id` `HashMap` 提供 O(1) 反向查找（id → 路径）。
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Trunk {
    root: TrieNode,
    /// `NodeId` → 完整路径字符串（反向索引，O(1)）。
    by_id: HashMap<NodeId, String>,
}

impl Trunk {
    /// 创建空 Trunk。
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// 当前存储的节点总数。
    #[must_use]
    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    /// 若无节点返回 `true`。
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }

    /// 插入路径，返回其（稳定的）[`NodeId`]。
    ///
    /// 幂等：同一路径插入两次返回同一 id，不产生重复状态。
    ///
    /// **注意**：祖先路径不自动实体化。若要让 `src/auth.rs>AuthService`
    /// 可按节点查询，需单独 `upsert`。
    pub fn upsert(&mut self, path: TrunkPath) -> NodeId {
        let s = path.into_string();
        let id = NodeId(path_to_id(&s));
        // 在 Trie 中插入
        let segments: Vec<&str> = s.split(path::SEPARATOR).collect();
        let node = self.root.navigate_or_create(&segments);
        node.id.get_or_insert(id);
        // 反向索引
        self.by_id.entry(id).or_insert(s);
        id
    }

    /// 按精确路径查找 id。不存在返回 `None`。
    ///
    /// **区分精确匹配与前缀匹配**：从未 `upsert` 过的祖先路径返回 `None`，
    /// 即使其后代路径存在。
    #[must_use]
    pub fn lookup_path(&self, path: &str) -> Option<NodeId> {
        let segments: Vec<&str> = path.split(path::SEPARATOR).collect();
        self.root.navigate(&segments)?.id
    }

    /// 返回 [`NodeId`] 对应的路径字符串。不在 trunk 中返回 `None`。
    #[must_use]
    pub fn path_of(&self, id: NodeId) -> Option<&str> {
        self.by_id.get(&id).map(String::as_str)
    }

    /// 枚举 `id` 路径的所有**严格祖先**节点 id，以**子到根**顺序返回。
    ///
    /// 只返回**已实体化**的祖先（即被 `upsert` 过的路径）。
    pub fn ancestors(&self, id: NodeId) -> impl Iterator<Item = NodeId> + '_ {
        let path = self.by_id.get(&id).cloned();
        AncestorIter {
            trunk: self,
            remaining: path,
        }
    }

    /// 枚举 `id` 路径的所有**严格后代**节点 id（顺序不确定）。
    ///
    /// O(K)，K = 子树内节点数（RFC-0089：不再是全图 O(N)）。
    pub fn descendants(&self, id: NodeId) -> impl Iterator<Item = NodeId> + '_ {
        let mut ids = Vec::new();
        if let Some(path) = self.by_id.get(&id) {
            let segments: Vec<&str> = path.split(path::SEPARATOR).collect();
            if let Some(node) = self.root.navigate(&segments) {
                collect_descendant_ids(node, &mut ids);
            }
        }
        ids.into_iter()
    }

    /// 移除 `id`（不级联后代）。返回 `true` 表示节点存在。
    ///
    /// 后代节点和边保持不变。使用 [`Self::remove_subtree`] 来级联删除。
    pub fn remove(&mut self, id: NodeId) -> bool {
        let Some(path) = self.by_id.remove(&id) else {
            return false;
        };
        let segments: Vec<&str> = path.split(path::SEPARATOR).collect();
        if let Some(node) = self.root.navigate_mut(&segments) {
            node.id = None;
        }
        true
    }

    /// 枚举所有已实体化路径字符串（顺序不确定）。
    pub fn all_paths(&self) -> impl Iterator<Item = &str> + '_ {
        self.by_id.values().map(String::as_str)
    }

    /// Iterate all **symbol** nodes — paths that contain at least one `>`
    /// (i.e., non-file nodes). Yields `(NodeId, path_str)` pairs.
    ///
    /// O(V) — no trie navigation. Prefer this over `all_paths()` +
    /// `lookup_path()` loops in graph-algorithm code.
    pub fn symbol_nodes(&self) -> impl Iterator<Item = (NodeId, &str)> + '_ {
        self.by_id
            .iter()
            .filter(|(_, p)| p.contains('>'))
            .map(|(&id, p)| (id, p.as_str()))
    }

    /// 移除 `id` 及其所有后代。返回移除的节点数。
    pub fn remove_subtree(&mut self, id: NodeId) -> usize {
        let Some(path) = self.by_id.get(&id).cloned() else {
            return 0;
        };
        // 先收集所有待删 id（id 本身 + 所有后代）
        let mut ids_to_remove: Vec<NodeId> = vec![id];
        {
            let segments: Vec<&str> = path.split(path::SEPARATOR).collect();
            if let Some(node) = self.root.navigate(&segments) {
                collect_descendant_ids(node, &mut ids_to_remove);
            }
        }
        let count = ids_to_remove.len();
        // 从反向索引删除
        for nid in &ids_to_remove {
            self.by_id.remove(nid);
        }
        // 从 Trie 摘除整个子树
        let segments: Vec<&str> = path.split(path::SEPARATOR).collect();
        remove_trie_subtree(&mut self.root, &segments);
        count
    }
}

// ── 祖先迭代器 ─────────────────────────────────────────────────────────────────

struct AncestorIter<'a> {
    trunk: &'a Trunk,
    remaining: Option<String>,
}

impl Iterator for AncestorIter<'_> {
    type Item = NodeId;

    fn next(&mut self) -> Option<NodeId> {
        loop {
            let path = self.remaining.take()?;
            let parent_path = path::parent(&path)?.to_owned();
            self.remaining = Some(parent_path.clone());
            if let Some(id) = self.trunk.lookup_path(&parent_path) {
                return Some(id);
            }
            // 跳过未实体化的祖先，继续向上
        }
    }
}

// ── NodeId 派生 ────────────────────────────────────────────────────────────────

/// 从路径字符串派生稳定的 [`NodeId`]。
///
/// 使用 BLAKE3 截断到 64 位。低 8 位保留为 shard tag（v0.1 均为 0）。
fn path_to_id(path: &str) -> u64 {
    let hash = blake3::hash(path.as_bytes());
    let bytes: [u8; 8] = hash.as_bytes()[..8].try_into().expect("blake3 ≥ 8 bytes");
    let raw = u64::from_le_bytes(bytes);
    // 低 8 位置 0，预留 shard tag
    raw & 0xFFFF_FFFF_FFFF_FF00
}
