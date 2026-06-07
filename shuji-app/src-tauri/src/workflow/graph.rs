//! 文移图 — 基于 DAG 的部门间任务流转追踪。
//!
//! 每次 `route_to` 产生一条有向边。用 DFS 环路检测保证图始终无环：
//! 如果 B 到 A 已有路径，创建新的 B 节点实例（B#2, B#3...）而非重连。

use std::collections::HashMap;
use std::path::Path;

use chrono::Local;
use serde::{Deserialize, Serialize};

/// 节点状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum NodeStatus {
    #[serde(rename = "active")]
    Active,
    #[serde(rename = "completed")]
    Completed,
    #[serde(rename = "failed")]
    Failed,
}

/// 图节点：一个部门的一次"出场"
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: usize,
    pub role: String,
    pub instance: u32,
    /// 当前承担的任务摘要
    pub task_summary: String,
    pub status: NodeStatus,
    pub created_at: String,
}

/// 有向边：一次 route_to 调用
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEdge {
    pub id: usize,
    pub source: usize,
    pub target: usize,
    pub task_id: String,
    pub description: String,
    pub timestamp: String,
}

/// 有向无环流程图
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowGraph {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    next_node_id: usize,
    next_edge_id: usize,
    /// 角色 → 当前活跃节点 ID（运行时状态，不持久化）
    #[serde(skip)]
    current_nodes: HashMap<String, usize>,
    /// 角色 → 已出现的实例数（运行时状态，不持久化）
    #[serde(skip)]
    instance_counts: HashMap<String, u32>,
}

impl Default for WorkflowGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl WorkflowGraph {
    /// 创建新图，默认包含一个 内阁#1 起始节点
    pub fn new() -> Self {
        let mut graph = Self {
            nodes: Vec::new(),
            edges: Vec::new(),
            next_node_id: 1,
            next_edge_id: 1,
            current_nodes: HashMap::new(),
            instance_counts: HashMap::new(),
        };
        let neige_id = graph.alloc_node("内阁", 1, "会话启动");
        graph.current_nodes.insert("内阁".to_string(), neige_id);
        graph
    }

    /// 从持久化数据重建运行时状态（反序列化后调用）
    pub fn rebuild_state(&mut self) {
        self.current_nodes.clear();
        self.instance_counts.clear();

        let mut max_instance: HashMap<String, u32> = HashMap::new();
        for node in &self.nodes {
            let entry = max_instance.entry(node.role.clone()).or_insert(0);
            *entry = (*entry).max(node.instance);
        }
        self.instance_counts = max_instance;

        // 每角色最后出现的节点 = 当前活跃节点
        let mut last_by_role: HashMap<String, usize> = HashMap::new();
        for node in &self.nodes {
            last_by_role.insert(node.role.clone(), node.id);
        }
        self.current_nodes = last_by_role;
    }

    /// 添加一条边：from_role → to_role，附带任务信息。
    /// 自动做环路检测，必要时创建 to_role 的新实例。
    pub fn add_edge(
        &mut self,
        from_role: &str,
        to_role: &str,
        task_id: &str,
        task_description: &str,
    ) {
        let from_id = match self.current_nodes.get(from_role) {
            Some(id) => *id,
            None => {
                log_console!("[graph] 找不到 {} 的活跃节点，忽略", from_role);
                return;
            }
        };

        let target_id = match self.current_nodes.get(to_role) {
            Some(candidate_id) if self.has_path(*candidate_id, from_id) => {
                // 环路！创建新实例
                let instance = self.instance_counts.get(to_role).copied().unwrap_or(0) + 1;
                self.alloc_node(to_role, instance, task_description)
            }
            Some(candidate_id) => {
                // 无环路，更新已有节点的任务摘要
                if let Some(node) = self.nodes.iter_mut().find(|n| n.id == *candidate_id) {
                    node.task_summary = task_description.to_string();
                }
                *candidate_id
            }
            None => {
                // to_role 首次出现
                let instance = 1;
                self.alloc_node(to_role, instance, task_description)
            }
        };

        // 创建边
        let edge = GraphEdge {
            id: self.next_edge_id,
            source: from_id,
            target: target_id,
            task_id: task_id.to_string(),
            description: task_description.to_string(),
            timestamp: Local::now().format("%H:%M:%S").to_string(),
        };
        self.next_edge_id += 1;
        self.edges.push(edge);

        log_console!(
            "[graph] {}#{:?} → {}#{} (task: {})",
            from_role,
            self.nodes.iter().find(|n| n.id == from_id).map(|n| n.instance),
            to_role,
            self.nodes.iter().find(|n| n.id == target_id).map(|n| n.instance).unwrap_or(0),
            task_id,
        );
    }

    /// 创建并注册一个节点，返回其 id
    fn alloc_node(&mut self, role: &str, instance: u32, task: &str) -> usize {
        let node = GraphNode {
            id: self.next_node_id,
            role: role.to_string(),
            instance,
            task_summary: task.to_string(),
            status: NodeStatus::Active,
            created_at: Local::now().format("%H:%M:%S").to_string(),
        };
        self.next_node_id += 1;

        self.instance_counts
            .entry(role.to_string())
            .and_modify(|c| *c = (*c).max(instance))
            .or_insert(instance);

        let id = node.id;
        self.nodes.push(node);
        self.current_nodes.insert(role.to_string(), id);
        id
    }

    /// DFS 检测 from_id 能否到达 to_id
    fn has_path(&self, from_id: usize, to_id: usize) -> bool {
        if from_id == to_id {
            return true;
        }
        let mut visited = vec![false; self.next_node_id.max(1)];
        let mut stack = vec![from_id];
        while let Some(current) = stack.pop() {
            if current == to_id {
                return true;
            }
            if current < visited.len() {
                if visited[current] {
                    continue;
                }
                visited[current] = true;
            }
            for edge in &self.edges {
                if edge.source == current && edge.target < visited.len() && !visited[edge.target] {
                    stack.push(edge.target);
                }
            }
        }
        false
    }

    /// 标记一个角色的当前节点为 completed
    pub fn mark_completed(&mut self, role: &str) {
        if let Some(id) = self.current_nodes.get(role) {
            if let Some(node) = self.nodes.iter_mut().find(|n| n.id == *id) {
                node.status = NodeStatus::Completed;
            }
        }
    }

    /// 标记一个角色的当前节点为 failed
    pub fn mark_failed(&mut self, role: &str) {
        if let Some(id) = self.current_nodes.get(role) {
            if let Some(node) = self.nodes.iter_mut().find(|n| n.id == *id) {
                node.status = NodeStatus::Failed;
            }
        }
    }

    // ── 持久化 ──

    /// 保存到 `.shuji/workflow_graph.json`
    pub async fn save_to(&self, working_dir: &Path) {
        let dir = working_dir.join(".shuji");
        let _ = tokio::fs::create_dir_all(&dir).await;
        let path = dir.join("workflow_graph.json");
        let tmp = dir.join("workflow_graph.json.tmp");
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = tokio::fs::write(&tmp, &json).await;
            let _ = tokio::fs::rename(&tmp, &path).await;
        }
    }

    /// 从 `.shuji/workflow_graph.json` 加载
    pub async fn load_from(working_dir: &Path) -> Option<Self> {
        let path = working_dir.join(".shuji").join("workflow_graph.json");
        let data = tokio::fs::read_to_string(&path).await.ok()?;
        let mut graph: Self = serde_json::from_str(&data).ok()?;
        graph.rebuild_state();
        Some(graph)
    }

    /// 尝试加载，若无文件则新建
    pub async fn load_or_new(working_dir: &Path) -> Self {
        Self::load_from(working_dir)
            .await
            .unwrap_or_else(|| {
                let g = Self::new();
                log_console!("[graph] 新建文移图（无持久化文件）");
                g
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_graph_has_neige() {
        let g = WorkflowGraph::new();
        assert_eq!(g.nodes.len(), 1);
        assert_eq!(g.nodes[0].role, "内阁");
        assert_eq!(g.current_nodes.get("内阁"), Some(&1));
    }

    #[test]
    fn test_add_edge_simple() {
        let mut g = WorkflowGraph::new();
        g.add_edge("内阁", "尚书令", "task_001", "执行任务");
        assert_eq!(g.nodes.len(), 2);
        assert_eq!(g.edges.len(), 1);
        assert_eq!(g.edges[0].source, 1);
        assert_eq!(g.edges[0].target, 2);
    }

    #[test]
    fn test_reuse_node_when_no_cycle() {
        let mut g = WorkflowGraph::new();
        g.add_edge("内阁", "尚书令", "t1", "任务A");
        g.add_edge("内阁", "工部", "t2", "任务B");
        g.add_edge("工部", "尚书令", "t3", "任务C");
        assert_eq!(g.nodes.len(), 3, "不应创建新尚书令节点");
        let shangshu = g.nodes.iter().find(|n| n.role == "尚书令").unwrap();
        assert_eq!(shangshu.task_summary, "任务C");
    }

    #[test]
    fn test_creates_new_instance_on_cycle() {
        let mut g = WorkflowGraph::new();
        g.add_edge("内阁", "尚书令", "t1", "执行");
        g.add_edge("尚书令", "工部", "t2", "编码");
        g.add_edge("工部", "内阁", "t3", "汇报");
        let neige_nodes: Vec<&GraphNode> = g.nodes.iter().filter(|n| n.role == "内阁").collect();
        assert_eq!(neige_nodes.len(), 2);
        assert_eq!(neige_nodes[1].instance, 2);
        assert_eq!(g.edges[2].target, 4);
    }

    #[test]
    fn test_multi_hop_cycle() {
        let mut g = WorkflowGraph::new();
        g.add_edge("内阁", "中书令", "t1", "设计");
        g.add_edge("中书令", "门下侍中", "t2", "审查");
        g.add_edge("门下侍中", "内阁", "t3", "批复");
        let neige_nodes: Vec<&GraphNode> = g.nodes.iter().filter(|n| n.role == "内阁").collect();
        assert_eq!(neige_nodes.len(), 2);
        assert_eq!(neige_nodes[1].instance, 2);
    }

    #[test]
    fn test_has_path() {
        let mut g = WorkflowGraph::new();
        g.add_edge("内阁", "尚书令", "t1", "执行");
        g.add_edge("尚书令", "工部", "t2", "编码");
        assert!(g.has_path(1, 3));
        assert!(!g.has_path(3, 1));
    }

    #[test]
    fn test_persistence_round_trip() {
        let tmp = tempfile::TempDir::new().unwrap();
        let mut g = WorkflowGraph::new();
        g.add_edge("内阁", "尚书令", "t1", "执行");
        g.add_edge("尚书令", "工部", "t2", "编码");

        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(g.save_to(tmp.path()));
        let loaded = rt.block_on(WorkflowGraph::load_from(tmp.path())).unwrap();
        assert_eq!(loaded.nodes.len(), 3);
        assert_eq!(loaded.edges.len(), 2);
    }

    #[test]
    fn test_rebuild_state() {
        let mut g = WorkflowGraph::new();
        g.add_edge("内阁", "尚书令", "t1", "A");
        g.add_edge("尚书令", "工部", "t2", "B");
        g.add_edge("工部", "内阁", "t3", "C");

        let json = serde_json::to_string(&g).unwrap();
        let mut restored: WorkflowGraph = serde_json::from_str(&json).unwrap();
        restored.rebuild_state();

        assert_eq!(restored.nodes.len(), 4);
        assert_eq!(restored.edges.len(), 3);
        assert_eq!(*restored.current_nodes.get("内阁").unwrap(), 4);
        assert_eq!(*restored.instance_counts.get("内阁").unwrap(), 2);
    }

    #[test]
    fn test_mark_completed() {
        let mut g = WorkflowGraph::new();
        g.add_edge("内阁", "尚书令", "t1", "执行");
        g.mark_completed("尚书令");
        assert_eq!(
            g.nodes.iter().find(|n| n.role == "尚书令").unwrap().status,
            NodeStatus::Completed
        );
    }
}
