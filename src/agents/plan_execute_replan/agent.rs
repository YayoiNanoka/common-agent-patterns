use crate::agents::base::Agent;
use crate::agents::plan_execute_replan::executor::PlanStepExecutor;
use crate::agents::plan_execute_replan::planner::Planner;
use crate::agents::plan_execute_replan::replanner::Replanner;
use crate::agents::plan_execute_replan::types::{PlanState, StepResult};
use crate::agents::summarizer::{AgentSummarizer, SummaryInput};
use crate::agents::types::AgentInput;
use crate::console::print_block;
use crate::llm::LlmClient;
use crate::tools::registry::ToolRegistry;

/// Plan-Execute-Replan 总控 Agent。
///
/// 它负责创建计划状态、调用 Planner 生成计划、调用 Executor 执行每个步骤，
/// 并在每个步骤完成后调用 Replanner 判断是否需要调整后续计划。
pub struct PlanExecuteReplanAgent {
    planner: Planner,
    executor: PlanStepExecutor,
    replanner: Replanner,
    summarizer: AgentSummarizer,
    max_steps: usize,
}

impl PlanExecuteReplanAgent {
    /// 创建 Plan-Execute-Replan Agent。
    ///
    /// `llm` 会被 clone 给三个子 agent；`registry` 会交给 Executor 子 agent 管理工具执行。
    pub fn new(llm: LlmClient, registry: ToolRegistry, max_steps: usize) -> Self {
        Self {
            planner: Planner::new(llm.clone()),
            executor: PlanStepExecutor::new(llm.clone(), registry),
            replanner: Replanner::new(llm.clone()),
            summarizer: AgentSummarizer::new(llm),
            max_steps,
        }
    }

    /// 根据当前计划状态生成最终答案文本。
    fn build_final_answer(&self, state: &PlanState) -> String {
        let mut answer = format!("任务：{}\n\n执行结果：", state.task);

        for result in &state.step_results {
            answer.push_str(&format!(
                "\n\n{}. {}\n{}",
                result.step_index + 1,
                result.step.description,
                result.output
            ));
        }

        answer
    }

    /// 执行单个步骤后的 replan 检查，并按需修改计划状态。
    fn maybe_replan(&self, state: &mut PlanState, step_result: &StepResult) -> anyhow::Result<()> {
        let current_step = step_result.step.clone();
        let replan_result = self.replanner.run(state, &current_step, step_result)?;

        println!("🔁 Replan 检查结果: {}", replan_result.reason);

        if replan_result.need_replan {
            println!(
                "🧭 需要重新规划，新的后续步骤数: {}",
                replan_result.new_steps.len()
            );
            state.replace_remaining_steps(replan_result.new_steps);
            print_block("📋 更新后的计划", state.render_plan());
        }

        Ok(())
    }

    /// 在达到最大执行步数时，调用通用 summarizer 生成阶段性答案。
    fn build_incomplete_answer(&self, state: &PlanState) -> anyhow::Result<String> {
        self.summarizer.summarize(SummaryInput {
            task: state.task.clone(),
            status: "任务未完成，Plan-Execute-Replan Agent 已达到最大执行步骤数。".to_string(),
            completed_work: state.render_history(),
            remaining_work: state.render_remaining_steps(),
            reason: format!("达到最大执行步骤数: {}", self.max_steps),
        })
    }
}

impl Agent for PlanExecuteReplanAgent {
    /// 运行完整 Plan-Execute-Replan 流程，并返回最终结果汇总。
    fn run(&self, input: AgentInput) -> anyhow::Result<String> {
        println!("🧭 Plan-Execute-Replan Agent 开始运行。");
        println!("📌 用户任务: {}", input.task);

        let plan = self.planner.run(&input.task)?;
        let mut state = PlanState::new(input.task, plan);
        let mut executed_steps = 0;

        print_block("📋 初始计划", state.render_plan());

        while !state.is_finished() && executed_steps < self.max_steps {
            let step = state
                .current_step()
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("missing current plan step"))?;

            println!(
                "\n================ P-E-RP Step {}/{} ================",
                state.current_step_index + 1,
                state.total_steps
            );
            println!("▶️ 当前步骤: {}", step.description);

            let step_result = self.executor.run(&state, &step)?;
            print_block("✅ 当前步骤执行结果", &step_result.output);

            state.push_step_result(step_result.clone());
            self.maybe_replan(&mut state, &step_result)?;
            state.advance_step();
            executed_steps += 1;
        }

        if state.is_finished() {
            println!("🏁 Plan-Execute-Replan Agent 执行完成。");
            Ok(self.build_final_answer(&state))
        } else {
            println!(
                "⚠️ Plan-Execute-Replan Agent 达到最大执行步骤数 {}，开始生成阶段性答案。",
                self.max_steps
            );
            self.build_incomplete_answer(&state)
        }
    }
}
