/// Plan-Execute-Replan 模式中的整体计划状态。
///
/// `PlanState` 由总控 agent 创建和维护，用于记录原始任务、当前计划、执行进度、
/// 每一步的执行结果以及重新规划次数。
#[derive(Debug, Clone)]
pub struct PlanState {
    /// 用户提出的原始任务。
    pub task: String,
    /// 当前计划，由多个 `PlanStep` 组成。
    pub plan: Vec<PlanStep>,
    /// 当前执行到第几个 step，从 0 开始。
    pub current_step_index: usize,
    /// 当前计划总步数。
    pub total_steps: usize,
    /// 已完成步骤的执行结果。
    pub step_results: Vec<StepResult>,
    /// 已发生的重新规划次数。
    pub replan_count: usize,
}

impl PlanState {
    /// 根据原始任务和初始计划创建计划状态。
    pub fn new(task: impl Into<String>, plan: Vec<PlanStep>) -> Self {
        let total_steps = plan.len();

        Self {
            task: task.into(),
            plan,
            current_step_index: 0,
            total_steps,
            step_results: Vec::new(),
            replan_count: 0,
        }
    }

    /// 返回当前正在执行的计划步骤。
    pub fn current_step(&self) -> Option<&PlanStep> {
        self.plan.get(self.current_step_index)
    }

    /// 判断当前计划是否已经执行完成。
    pub fn is_finished(&self) -> bool {
        self.current_step_index >= self.plan.len()
    }

    /// 将当前完整计划渲染成 prompt 中可读的文本。
    pub fn render_plan(&self) -> String {
        render_plan_steps(&self.plan)
    }

    /// 将历史步骤和执行结果渲染成 prompt 中可读的文本。
    pub fn render_history(&self) -> String {
        if self.step_results.is_empty() {
            return "(empty)".to_string();
        }

        self.step_results
            .iter()
            .map(|result| {
                format!(
                    "{}. {}\nResult: {}",
                    result.step_index + 1,
                    result.step.description,
                    result.output
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    /// 将尚未执行的步骤渲染成 prompt 中可读的文本。
    pub fn render_remaining_steps(&self) -> String {
        if self.current_step_index >= self.plan.len() {
            return "(empty)".to_string();
        }

        render_plan_steps(&self.plan[self.current_step_index..])
    }

    /// 记录当前步骤的执行结果。
    pub fn push_step_result(&mut self, result: StepResult) {
        self.step_results.push(result);
    }

    /// 前进到下一个计划步骤。
    pub fn advance_step(&mut self) {
        self.current_step_index += 1;
    }

    /// 使用新的后续步骤替换当前步骤之后的剩余计划。
    ///
    /// 当前步骤已经执行完成，因此保留 `0..=current_step_index` 的部分，只替换后续步骤。
    pub fn replace_remaining_steps(&mut self, new_steps: Vec<PlanStep>) {
        let keep_len = self.current_step_index + 1;
        self.plan.truncate(keep_len);
        self.plan.extend(new_steps);
        self.total_steps = self.plan.len();
        self.replan_count += 1;
    }
}

/// 计划中的单个可执行步骤。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanStep {
    /// 当前步骤的自然语言描述。
    pub description: String,
}

impl PlanStep {
    /// 创建一个新的计划步骤。
    pub fn new(description: impl Into<String>) -> Self {
        Self {
            description: description.into(),
        }
    }
}

/// 单个计划步骤的执行结果。
#[derive(Debug, Clone)]
pub struct StepResult {
    /// 当前步骤在计划中的下标。
    pub step_index: usize,
    /// 被执行的计划步骤。
    pub step: PlanStep,
    /// 当前步骤最终得到的结果文本。
    pub output: String,
    /// 当前步骤是否成功完成。
    pub success: bool,
}

impl StepResult {
    /// 创建一个成功的步骤执行结果。
    pub fn success(step_index: usize, step: PlanStep, output: impl Into<String>) -> Self {
        Self {
            step_index,
            step,
            output: output.into(),
            success: true,
        }
    }

    /// 创建一个部分完成的步骤执行结果。
    ///
    /// 通常用于达到执行轮次上限但已有部分可用信息的情况。
    pub fn partial(step_index: usize, step: PlanStep, output: impl Into<String>) -> Self {
        Self {
            step_index,
            step,
            output: output.into(),
            success: false,
        }
    }
}

/// Replanner 对当前计划状态给出的判断结果。
#[derive(Debug, Clone)]
pub struct ReplanResult {
    /// 是否需要重新规划后续步骤。
    pub need_replan: bool,
    /// 需要或不需要重新规划的原因。
    pub reason: String,
    /// 如果需要重新规划，这里保存新的后续步骤。
    pub new_steps: Vec<PlanStep>,
}

impl ReplanResult {
    /// 创建一个 replanner 判断结果。
    pub fn new(need_replan: bool, reason: impl Into<String>, new_steps: Vec<PlanStep>) -> Self {
        Self {
            need_replan,
            reason: reason.into(),
            new_steps,
        }
    }
}

/// 将计划步骤列表渲染成编号文本。
pub fn render_plan_steps(steps: &[PlanStep]) -> String {
    if steps.is_empty() {
        return "(empty)".to_string();
    }

    steps
        .iter()
        .enumerate()
        .map(|(index, step)| format!("{}. {}", index + 1, step.description))
        .collect::<Vec<_>>()
        .join("\n")
}
