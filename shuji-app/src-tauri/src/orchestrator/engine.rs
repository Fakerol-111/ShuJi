use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::agent::r#trait::{Agent, AgentDecision, AgentInput};
use crate::models::chat::{ChatMessage, ChatOption, approval_options, issue_options};
use crate::models::project::*;
use crate::models::role::Role;
use crate::state_machine::states::ProjectState;
use crate::storage::shuji_dir::ShujiDir;
use crate::logging::logger::Logger;

/// Safe truncation at char boundary — avoids panics on multi-byte strings
fn safe_truncate(s: &str, max: usize) -> &str {
    let end = s.floor_char_boundary(max.min(s.len()));
    &s[..end]
}

pub struct WorkflowEngine {
    agents: HashMap<Role, Box<dyn Agent>>,
    shuji_dir: ShujiDir,
    logger: Logger,
    phase_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSnapshot {
    pub overall: OverallStatus,
    pub phases: Vec<PhaseSnapshot>,
    pub overall_progress: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseSnapshot {
    pub index: u32,
    pub design: String,
    pub execution: String,
}

impl WorkflowEngine {
    pub fn new(agents: HashMap<Role, Box<dyn Agent>>, shuji_dir: ShujiDir, phase_count: u32) -> Self {
        let logger = Logger::new(&shuji_dir.root());
        Self { agents, shuji_dir, logger, phase_count }
    }

    // ── Public entry: process emperor input + auto-advance ──────────

    pub async fn process_and_advance(
        &self,
        project: &mut Project,
        emperor_input: &str,
    ) -> anyhow::Result<Vec<ChatMessage>> {
        let mut messages: Vec<ChatMessage> = Vec::new();

        // Case 1: New goal
        if project.overall == OverallStatus::NotStarted {
            if !emperor_input.is_empty() {
                project.goal = emperor_input.to_string();
                self.save_and_log(project, "目标已接收").await?;
                messages.push(ChatMessage::new("内阁",
                    &format!("遵旨。陛下目标是「{}」，臣已记录，立刻安排中书省设计方案。", emperor_input)));
            }
        }

        // Case 2: Pending emperor decision — process the input
        if self.has_pending_decision(project) && !emperor_input.is_empty() {
            let decision = EmperorDecision {
                choice: emperor_input.to_uppercase(),
                comment: None,
            };
            self.logger.log("皇帝", "皇帝", "决策",
                &format!("皇帝御批：{}", decision.choice),
                &serde_json::to_string(&decision).unwrap_or_default()).await;

            self.apply_decision(project, &decision).await;
        }

        // Auto-advance loop
        loop {
            if self.has_pending_decision(project) {
                // Only push decision message if last message doesn't already have options
                let already_has = messages.last().map_or(false, |m| !m.options.is_empty());
                if !already_has {
                    messages.push(self.build_decision_message(project));
                }
                break;
            }

            if project.state == ProjectState::Delivered {
                messages.push(ChatMessage::new("内阁",
                    "项目已全部完成，交付归档。陛下若有新的目标，随时吩咐。"));
                break;
            }
            if project.state == ProjectState::Terminated {
                messages.push(ChatMessage::new("内阁", "项目已终止。"));
                break;
            }
            if project.state == ProjectState::Paused {
                messages.push(ChatMessage::new("内阁", "项目已暂缓。陛下可随时下旨恢复。"));
                break;
            }

            // Run one workflow step
            let step_msg = self.run_one_step(project).await?;
            if let Some(msg) = step_msg {
                messages.push(msg);
            }
        }

        Ok(messages)
    }

    pub fn snapshot(&self, project: &Project) -> ProjectSnapshot {
        ProjectSnapshot {
            overall: project.overall.clone(),
            phases: project.phases.iter().map(|p| PhaseSnapshot {
                index: p.index,
                design: p.design.label(p.index),
                execution: p.execution.label(p.index),
            }).collect(),
            overall_progress: self.calc_progress(project),
        }
    }

    // ── Decision helpers ───────────────────────────────────────

    fn has_pending_decision(&self, project: &Project) -> bool {
        project.overall == OverallStatus::PendingApproval ||
        project.overall == OverallStatus::Escalated ||
        project.phases.iter().any(|p|
            p.design == PhaseDesignStatus::PendingApproval ||
            p.design == PhaseDesignStatus::Escalated ||
            matches!(p.execution, PhaseExecutionStatus::Blocked { .. })
        )
    }

    fn build_decision_message(&self, project: &Project) -> ChatMessage {
        let mut options = approval_options();
        let content = if project.overall == OverallStatus::PendingApproval || project.overall == OverallStatus::Escalated {
            "整体方案已通过门下省审查，呈请皇帝御批。".to_string()
        } else if let Some(p) = project.phases.iter().find(|p|
            p.design == PhaseDesignStatus::PendingApproval || p.design == PhaseDesignStatus::Escalated
        ) {
            format!("阶段{}设计已通过门下省审查，呈请皇帝御批。", p.index)
        } else if let Some(p) = project.phases.iter().find(|p|
            matches!(p.execution, PhaseExecutionStatus::Blocked { .. })
        ) {
            if let PhaseExecutionStatus::Blocked { reason } = &p.execution {
                options = issue_options();
                format!("阶段{}执行遇到问题：{}\n请陛下定夺。", p.index, reason)
            } else { unreachable!() }
        } else {
            "等待陛下指示。".to_string()
        };

        ChatMessage::new("内阁", &content).with_options(options)
    }

    async fn apply_decision(&self, project: &mut Project, decision: &EmperorDecision) {
        match decision.choice.as_str() {
            "A" => {
                if project.overall == OverallStatus::PendingApproval || project.overall == OverallStatus::Escalated {
                    project.overall = OverallStatus::Approved;
                    project.phases = (0..self.phase_count).map(|i| PhaseRuntime {
                        index: i + 1, design: PhaseDesignStatus::NotStarted, execution: PhaseExecutionStatus::NotStarted,
                    }).collect();
                    self.logger.log_transition("整体方案已批准", "").await;
                } else {
                    // Approve current pending phase
                    if let Some(pi) = project.phases.iter().position(|p|
                        p.design == PhaseDesignStatus::PendingApproval || p.design == PhaseDesignStatus::Escalated
                    ) {
                        project.phases[pi].design = PhaseDesignStatus::Approved;
                        self.logger.log_transition(&format!("阶段{}设计已批准", pi + 1), "").await;
                    }
                    // Handle blocked execution — restart phase from redesign
                    if let Some(pi) = project.phases.iter().position(|p|
                        matches!(p.execution, PhaseExecutionStatus::Blocked { .. })
                    ) {
                        project.phases[pi].design = PhaseDesignStatus::Designing;
                        project.phases[pi].execution = PhaseExecutionStatus::NotStarted;
                        self.logger.log_transition(&format!("阶段{}退回修改，重新设计", pi + 1), "").await;
                    }
                }
            }
            "B" => {
                if project.overall == OverallStatus::PendingApproval || project.overall == OverallStatus::Escalated {
                    project.overall = OverallStatus::Designing;
                    self.logger.log_transition("皇帝准但，整体方案返回修改", "").await;
                } else if let Some(pi) = project.phases.iter().position(|p|
                    p.design == PhaseDesignStatus::PendingApproval || p.design == PhaseDesignStatus::Escalated
                ) {
                    project.phases[pi].design = PhaseDesignStatus::Designing;
                    self.logger.log_transition(&format!("阶段{}准但，返回修改", pi + 1), "").await;
                } else if let Some(pi) = project.phases.iter().position(|p|
                    matches!(p.execution, PhaseExecutionStatus::Blocked { .. })
                ) {
                    project.phases[pi].execution = PhaseExecutionStatus::TaskBreakdown;
                    self.logger.log_transition(&format!("阶段{}继续执行", pi + 1), "").await;
                }
            }
            "C" => {
                if project.overall == OverallStatus::PendingApproval || project.overall == OverallStatus::Escalated {
                    project.overall = OverallStatus::Rejected(1);
                    self.logger.log_transition("皇帝驳回整体方案", "").await;
                } else if let Some(pi) = project.phases.iter().position(|p|
                    p.design == PhaseDesignStatus::PendingApproval || p.design == PhaseDesignStatus::Escalated
                ) {
                    project.phases[pi].design = PhaseDesignStatus::Rejected(1);
                    self.logger.log_transition(&format!("阶段{}驳回", pi + 1), "").await;
                } else if let Some(pi) = project.phases.iter().position(|p|
                    matches!(p.execution, PhaseExecutionStatus::Blocked { .. })
                ) {
                    project.state = ProjectState::Terminated;
                    self.logger.log_transition("皇帝终止项目", "").await;
                }
            }
            "D" => {
                project.state = ProjectState::Paused;
                self.logger.log_transition("皇帝暂缓项目", "").await;
            }
            "E" => {
                if project.overall == OverallStatus::PendingApproval || project.overall == OverallStatus::Escalated {
                    project.overall = OverallStatus::Designing;
                    self.logger.log_transition("皇帝钦此，按圣意修改", "").await;
                } else if let Some(pi) = project.phases.iter().position(|p|
                    p.design == PhaseDesignStatus::PendingApproval || p.design == PhaseDesignStatus::Escalated
                ) {
                    project.phases[pi].design = PhaseDesignStatus::Designing;
                    self.logger.log_transition(&format!("阶段{}钦此，按圣意修改", pi + 1), "").await;
                }
            }
            _ => {}
        }
    }

    // ── One step execution (returns a ChatMessage if anything happened) ──

    async fn run_one_step(&self, project: &mut Project) -> anyhow::Result<Option<ChatMessage>> {
        match project.overall {
            OverallStatus::NotStarted | OverallStatus::Designing => {
                return self.step_overall_design(project).await;
            }
            OverallStatus::Rejected(n) if n < 3 => {
                // Capture count BEFORE setting to Designing, so step_overall_design can use it
                let retain_count = n;
                project.overall = OverallStatus::Designing;
                self.save_and_log(project, "整体方案重新设计中").await?;
                let result = self.step_overall_design_with_prev_count(project, retain_count).await;
                return result;
            }
            _ => {}
        }

        if project.overall == OverallStatus::Approved {
            return self.step_phases(project).await;
        }

        Ok(None)
    }

    // ── Overall design step ────────────────────────────────────

    /// First call — prev_count = 0
    async fn step_overall_design(&self, project: &mut Project) -> anyhow::Result<Option<ChatMessage>> {
        self.step_overall_design_with_prev_count(project, 0).await
    }

    /// Retry call with explicit previous reject count to avoid infinite loop
    async fn step_overall_design_with_prev_count(&self, project: &mut Project, prev_count: u32) -> anyhow::Result<Option<ChatMessage>> {
        project.overall = OverallStatus::Designing;
        self.save_and_log(project, "整体方案设计中").await?;

        let agent = self.agents.get(&Role::Zhongshu).unwrap();
        let input = AgentInput {
            role: Role::Zhongshu,
            task_description: format!("为项目「{}」做整体方案设计。皇帝目标：{}", project.name, project.goal),
            context_messages: vec![],
            project_dir: self.shuji_dir.root(),
            working_dir: PathBuf::from(&project.working_dir),
        };
        let output = agent.execute(&input).await?;
        self.logger.log_agent(Role::Zhongshu, "设计", "整体方案设计完成",
            safe_truncate(&output.content, 200)).await;

        for doc in &output.documents {
            self.shuji_dir.write_document("designs", "overall_design.md", &doc.content).await?;
        }

        // Phase 2: Menxia reviews
        let review_agent = self.agents.get(&Role::Menxia).unwrap();
        let review_input = AgentInput {
            role: Role::Menxia,
            task_description: "审查整体方案设计".into(),
            context_messages: vec![],
            project_dir: self.shuji_dir.root(),
            working_dir: PathBuf::from(&project.working_dir),
        };
        let review_output = review_agent.execute(&review_input).await?;
        self.logger.log_agent(Role::Menxia, "审查", "审查整体方案",
            safe_truncate(&review_output.content, 200)).await;

        self.shuji_dir.write_document("reviews", "review_overall.md", &review_output.content).await?;

        match review_agent.parse_decision(&review_output) {
            AgentDecision::Rejected { .. } => {
                // Use the running counter, ignore mock's count
                let new_count = prev_count + 1;
                project.overall = OverallStatus::Rejected(new_count);
                self.save_and_log(project, "整体方案驳回").await?;
                return Ok(Some(ChatMessage::new("内阁", &format!(
                    "门下省上报：整体方案审查未通过（驳回第{}次），已退回中书省修改。", new_count))));
            }
            _ => {
                // Passed — move to emperor decision
                project.overall = OverallStatus::PendingApproval;
                self.save_and_log(project, "整体方案待批").await?;

                let neige = self.agents.get(&Role::Neige).unwrap();
                let neige_input = AgentInput {
                    role: Role::Neige,
                    task_description: "整理整体方案审查结果，呈奏折".into(),
                    context_messages: vec![],
                    project_dir: self.shuji_dir.root(),
                    working_dir: PathBuf::from(&project.working_dir),
                };
                let neige_output = neige.execute(&neige_input).await?;
                self.logger.log_agent(Role::Neige, "拟奏折", "整理审查结果呈皇帝",
                    safe_truncate(&neige_output.content, 200)).await;

                for doc in &neige_output.documents {
                    self.shuji_dir.write_document("reports", "memorial_overall.md", &doc.content).await?;
                }

                return Ok(Some(ChatMessage::new("内阁",
                    "整体方案已完成设计并通过门下省审查，呈请皇帝御批。")
                    .with_options(approval_options())));
            }
        }
    }

    // ── Phase execution loop ───────────────────────────────────

    async fn step_phases(&self, project: &mut Project) -> anyhow::Result<Option<ChatMessage>> {
        let pc = project.phases.len();
        for i in 0..pc {
            let phase = &project.phases[i];

            // Check blocked phases
            if matches!(phase.execution, PhaseExecutionStatus::Blocked { .. }) {
                return Ok(Some(
                    ChatMessage::new("内阁", &format!("阶段{}执行遇到阻塞，请陛下定夺。", i + 1))
                        .with_options(issue_options())))
            }
            if phase.execution == PhaseExecutionStatus::MinorIssue {
                // Auto-fix and continue
                // (In mock mode just continue)
            }

            // Can we start designing this phase?
            let can_design = if i == 0 {
                phase.design == PhaseDesignStatus::NotStarted
            } else {
                let prev = &project.phases[i - 1];
                phase.design == PhaseDesignStatus::NotStarted &&
                (prev.design == PhaseDesignStatus::Approved || prev.execution != PhaseExecutionStatus::NotStarted)
            };

            if can_design {
                return self.step_phase_design(project, i).await;
            }

            // Handle design states
            match &phase.design {
                PhaseDesignStatus::Designing => return self.step_phase_review(project, i, 0).await,
                PhaseDesignStatus::Rejected(n) if *n < 3 => {
                    let prev = *n;
                    project.phases[i].design = PhaseDesignStatus::Designing;
                    self.save_and_log(project, &format!("阶段{}重新设计中", i + 1)).await?;
                    // Pass prev count to avoid infinite reject loop
                    let result = self.step_phase_design_with_count(project, i, prev).await;
                    return result;
                }
                PhaseDesignStatus::Rejected(_) | PhaseDesignStatus::Escalated => {
                    project.phases[i].design = PhaseDesignStatus::Escalated;
                    self.save_and_log(project, &format!("阶段{}驳回升级", i + 1)).await?;
                    return Ok(Some(
                        ChatMessage::new("内阁", &format!("门下省上报：阶段{}设计已驳回3次，呈请皇帝御批。", i + 1))
                            .with_options(approval_options())
                    ));
                }
                PhaseDesignStatus::PendingApproval => {
                    // Wait — decision needed (handled by has_pending_decision)
                    // phase design pending — waiting for emperor
                }
                PhaseDesignStatus::Approved => {
                    // Execute phase
                    if phase.execution == PhaseExecutionStatus::NotStarted {
                        return self.step_phase_exec(project, i).await;
                    }
                    match &phase.execution {
                        PhaseExecutionStatus::TaskBreakdown
                        | PhaseExecutionStatus::Testing
                        | PhaseExecutionStatus::Implementing
                        | PhaseExecutionStatus::Checking
                        | PhaseExecutionStatus::Standards
                        | PhaseExecutionStatus::Logging => {
                            return self.step_phase_exec(project, i).await;
                        }
                        PhaseExecutionStatus::Completed => continue,
                        _ => {}
                    }
                }
                _ => {}
            }
        }

        // All phases done?
        if project.phases.iter().all(|p| p.execution == PhaseExecutionStatus::Completed) {
            project.state = ProjectState::Delivered;
            self.save_and_log(project, "已交付").await?;
            return Ok(Some(ChatMessage::new("尚书省", "所有阶段已完成，项目交付。")));
        }

        Ok(None)
    }

    async fn step_phase_design(&self, project: &mut Project, idx: usize) -> anyhow::Result<Option<ChatMessage>> {
        project.phases[idx].design = PhaseDesignStatus::Designing;
        self.save_and_log(project, &format!("阶段{}设计中", idx + 1)).await?;

        let agent = self.agents.get(&Role::Zhongshu).unwrap();
        let input = AgentInput {
            role: Role::Zhongshu,
            task_description: format!("为阶段{}做详细设计", idx + 1),
            context_messages: vec![],
            project_dir: self.shuji_dir.root(),
            working_dir: PathBuf::from(&project.working_dir),
        };
        let output = agent.execute(&input).await?;
        self.logger.log_agent(Role::Zhongshu, "详细设计", &format!("阶段{}详细设计完成", idx + 1),
            safe_truncate(&output.content, 200)).await;

        for doc in &output.documents {
            self.shuji_dir.write_document("designs", &format!("phase{}_design.md", idx + 1), &doc.content).await?;
        }

        // Directly to review
        return self.step_phase_review(project, idx, 0).await;
    }

    /// Same as step_phase_design but passes a previously-rejected count
    async fn step_phase_design_with_count(&self, project: &mut Project, idx: usize, prev_count: u32) -> anyhow::Result<Option<ChatMessage>> {
        project.phases[idx].design = PhaseDesignStatus::Designing;
        self.save_and_log(project, &format!("阶段{}设计中", idx + 1)).await?;

        let agent = self.agents.get(&Role::Zhongshu).unwrap();
        let input = AgentInput {
            role: Role::Zhongshu,
            task_description: format!("为阶段{}做详细设计", idx + 1),
            context_messages: vec![],
            project_dir: self.shuji_dir.root(),
            working_dir: PathBuf::from(&project.working_dir),
        };
        let output = agent.execute(&input).await?;
        self.logger.log_agent(Role::Zhongshu, "详细设计", &format!("阶段{}详细设计完成", idx + 1),
            safe_truncate(&output.content, 200)).await;

        for doc in &output.documents {
            self.shuji_dir.write_document("designs", &format!("phase{}_design.md", idx + 1), &doc.content).await?;
        }

        return self.step_phase_review(project, idx, prev_count).await;
    }

    async fn step_phase_review(&self, project: &mut Project, idx: usize, prev_count: u32) -> anyhow::Result<Option<ChatMessage>> {
        let agent = self.agents.get(&Role::Menxia).unwrap();
        let input = AgentInput {
            role: Role::Menxia,
            task_description: format!("审查阶段{}详细设计", idx + 1),
            context_messages: vec![],
            project_dir: self.shuji_dir.root(),
            working_dir: PathBuf::from(&project.working_dir),
        };
        let output = agent.execute(&input).await?;
        self.logger.log_agent(Role::Menxia, "审查", &format!("审查阶段{}详细设计", idx + 1),
            safe_truncate(&output.content, 200)).await;

        self.shuji_dir.write_document("reviews", &format!("review_phase{}.md", idx + 1), &output.content).await?;

        match agent.parse_decision(&output) {
            AgentDecision::Rejected { .. } => {
                let new_count = prev_count + 1;
                project.phases[idx].design = PhaseDesignStatus::Rejected(new_count);
                self.save_and_log(project, &format!("阶段{}设计驳回", idx + 1)).await?;
                Ok(Some(ChatMessage::new("内阁",
                    &format!("门下省上报：阶段{}设计审查未通过（驳回第{}次），已退回修改。", idx + 1, new_count))))
            }
            _ => {
                project.phases[idx].design = PhaseDesignStatus::PendingApproval;
                self.save_and_log(project, &format!("阶段{}设计待批", idx + 1)).await?;

                let neige = self.agents.get(&Role::Neige).unwrap();
                let neige_input = AgentInput {
                    role: Role::Neige,
                    task_description: format!("整理阶段{}审查结果，呈奏折", idx + 1),
                    context_messages: vec![],
                    project_dir: self.shuji_dir.root(),
                    working_dir: PathBuf::from(&project.working_dir),
                };
                let neige_output = neige.execute(&neige_input).await?;
                self.logger.log_agent(Role::Neige, "拟奏折", &format!("整理阶段{}审查结果呈皇帝", idx + 1),
                    safe_truncate(&neige_output.content, 200)).await;

                for doc in &neige_output.documents {
                    self.shuji_dir.write_document("reports", &format!("memorial_phase{}.md", idx + 1), &doc.content).await?;
                }

                Ok(Some(ChatMessage::new("内阁",
                    &format!("阶段{}设计已完成并通过审查，呈请皇帝御批。", idx + 1))
                    .with_options(approval_options())))
            }
        }
    }

    async fn step_phase_exec(&self, project: &mut Project, idx: usize) -> anyhow::Result<Option<ChatMessage>> {
        let pi = idx + 1;

        if project.phases[idx].execution == PhaseExecutionStatus::NotStarted {
            project.phases[idx].execution = PhaseExecutionStatus::TaskBreakdown;
            self.save_and_log(project, &format!("阶段{}执行：吏部拆解任务", pi)).await?;

            let agent = self.agents.get(&Role::LiBuP).unwrap();
            let input = AgentInput {
                role: Role::LiBuP,
                task_description: format!("拆解阶段{}的任务", pi),
                context_messages: vec![],
                project_dir: self.shuji_dir.root(),
                working_dir: PathBuf::from(&project.working_dir),
            };
            let output = agent.execute(&input).await?;
            self.logger.log_agent(Role::LiBuP, "任务拆解", &format!("阶段{}任务拆解完成", pi),
                safe_truncate(&output.content, 200)).await;

            for doc in &output.documents {
                self.shuji_dir.write_document("execution", &format!("tasks_phase{}.md", pi), &doc.content).await?;
            }

            // After task breakdown, continue to next execution step immediately
            project.phases[idx].execution = PhaseExecutionStatus::Testing;
            self.save_and_log(project, &format!("阶段{}执行：兵部测试", pi)).await?;
        }

        // 兵部 → 工部 → 刑部 → 礼部 → 户部
        let exec_steps = vec![
            (PhaseExecutionStatus::Testing, Role::Bingbu, "兵部测试"),
            (PhaseExecutionStatus::Implementing, Role::Gongbu, "工部编码"),
            (PhaseExecutionStatus::Checking, Role::Xingbu, "刑部检查"),
            (PhaseExecutionStatus::Standards, Role::LiBuR, "礼部检查"),
            (PhaseExecutionStatus::Logging, Role::Hubu, "户部记录"),
        ];

        for (status, role, step_name) in exec_steps {
            if project.phases[idx].execution == status {
                let agent = self.agents.get(&role).unwrap();
                let input = AgentInput {
                    role,
                    task_description: format!("阶段{}的{}工作", pi, step_name),
                    context_messages: vec![],
                    project_dir: self.shuji_dir.root(),
                    working_dir: PathBuf::from(&project.working_dir),
                };
                let output = agent.execute(&input).await?;
                self.logger.log_agent(role, step_name, &format!("阶段{} {}", pi, step_name),
                    safe_truncate(&output.content, 200)).await;

                // Check execution issues
                match agent.parse_decision(&output) {
                    AgentDecision::ExecutionIssue { is_blocking, reason } => {
                        if is_blocking {
                            project.phases[idx].execution = PhaseExecutionStatus::Blocked { reason: reason.clone() };
                            self.save_and_log(project, &format!("阶段{}执行阻塞", pi)).await?;
                            return Ok(Some(ChatMessage::new("内阁",
                                &format!("兵部上报：阶段{}执行发现严重问题：{}\n请陛下定夺。", pi, reason))
                                .with_options(issue_options())));
                        } else {
                            project.phases[idx].execution = PhaseExecutionStatus::MinorIssue;
                            self.save_and_log(project, &format!("阶段{}执行有轻微问题", pi)).await?;
                            // Continue — minor issue doesn't block
                        }
                    }
                    _ => {}
                }

                // Advance to next execution state
                let next = match status {
                    PhaseExecutionStatus::Testing => PhaseExecutionStatus::Implementing,
                    PhaseExecutionStatus::Implementing => PhaseExecutionStatus::Checking,
                    PhaseExecutionStatus::Checking => PhaseExecutionStatus::Standards,
                    PhaseExecutionStatus::Standards => PhaseExecutionStatus::Logging,
                    PhaseExecutionStatus::Logging => PhaseExecutionStatus::Completed,
                    _ => PhaseExecutionStatus::Completed,
                };
                project.phases[idx].execution = next;
                self.save_and_log(project, &format!("阶段{} {}完成", pi, step_name)).await?;
                return Ok(None); // No chat message for routine execution steps
            }
        }

        Ok(None)
    }

    // ── Helpers ───────────────────────────────────────────────

    fn calc_progress(&self, project: &Project) -> f64 {
        let total = self.phase_count as f64 * 2.0 + 1.0;
        let mut done = 0.0;
        if project.overall == OverallStatus::Approved { done += 1.0; }
        for phase in &project.phases {
            if phase.design == PhaseDesignStatus::Approved { done += 1.0; }
            if phase.execution == PhaseExecutionStatus::Completed { done += 1.0; }
        }
        done / total * 100.0
    }

    async fn save_and_log(&self, project: &mut Project, event: &str) -> anyhow::Result<()> {
        project.updated_at = chrono::Utc::now().to_rfc3339();
        self.shuji_dir.save_project(project).await?;
        self.logger.log_transition(event, "").await;
        Ok(())
    }
}

// Keep EmperorDecision for internal use
#[derive(Debug, Clone, Serialize, Deserialize)]
struct EmperorDecision {
    choice: String,
    comment: Option<String>,
}
