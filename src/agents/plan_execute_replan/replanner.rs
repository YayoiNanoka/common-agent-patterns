use serde::Deserialize;

use crate::agents::plan_execute_replan::types::{PlanState, PlanStep, ReplanResult, StepResult};
use crate::console::print_block;
use crate::llm::LlmClient;

/// Replanner 子 agent 的 prompt 模板。
///
/// Replanner 根据当前步骤执行结果判断是否需要调整后续计划。
pub const REPLANNER_PROMPT_TEMPLATE: &str = r#"
你是一个计划检查器。你的任务是根据当前计划的执行情况，判断是否需要重新规划。

你需要检查：
1. 当前步骤的执行结果是否偏离原始任务。
2. 当前计划是否仍然能够完成用户目标。
3. 是否因为工具结果、错误信息或新信息导致原计划需要调整。
4. 如果需要重新规划，请给出新的后续步骤。

原始任务：
{question}

当前计划：
{plan}

当前步骤：
{current_step}

当前步骤执行结果：
{step_result}

历史步骤与结果：
{history}

请严格按照以下 JSON 格式输出：

{
  "need_replan": false,
  "reason": "说明为什么需要或不需要重新规划",
  "new_steps": []
}

如果需要重新规划，请输出：

{
  "need_replan": true,
  "reason": "说明为什么需要重新规划",
  "new_steps": [
    "新的步骤1",
    "新的步骤2"
  ]
}

规则：
1. 只输出 JSON。
2. 不要输出解释、Markdown、代码块或额外文本。
3. 如果不需要重新规划，new_steps 必须是空数组。
4. 如果需要重新规划，new_steps 必须包含新的后续执行步骤。
"#;

/// 负责判断是否需要重新规划的子 agent。
pub struct Replanner {
    llm: LlmClient,
}

impl Replanner {
    /// 创建 Replanner。
    pub fn new(llm: LlmClient) -> Self {
        Self { llm }
    }

    /// 根据当前计划状态和最新步骤结果判断是否需要重新规划。
    pub fn run(
        &self,
        state: &PlanState,
        current_step: &PlanStep,
        step_result: &StepResult,
    ) -> anyhow::Result<ReplanResult> {
        println!("🔍 Replanner 开始检查当前步骤结果...");

        let prompt = self.build_prompt(state, current_step, step_result);
        let output = self.llm.chat(&prompt)?;
        print_block("📨 Replanner LLM 输出", &output);

        let result = self.parse_replan_response(&output)?;
        println!("🔁 Replanner 判断: need_replan = {}", result.need_replan);
        println!("📝 原因: {}", result.reason);

        if result.need_replan {
            print_block(
                "📋 Replanner 新的后续步骤",
                render_steps_for_console(&result.new_steps),
            );
        }

        Ok(result)
    }

    /// 拼装发送给 LLM 的 replanner prompt。
    fn build_prompt(
        &self,
        state: &PlanState,
        current_step: &PlanStep,
        step_result: &StepResult,
    ) -> String {
        REPLANNER_PROMPT_TEMPLATE
            .replace("{question}", &state.task)
            .replace("{plan}", &state.render_plan())
            .replace("{current_step}", &current_step.description)
            .replace("{step_result}", &step_result.output)
            .replace("{history}", &state.render_history())
    }

    /// 解析 Replanner 返回的 JSON。
    pub fn parse_replan_response(&self, output: &str) -> anyhow::Result<ReplanResult> {
        parse_replan_response(output)
    }
}

/// 将 replanner 给出的后续步骤转换成适合控制台展示的编号文本。
fn render_steps_for_console(steps: &[PlanStep]) -> String {
    steps
        .iter()
        .enumerate()
        .map(|(index, step)| format!("{}. {}", index + 1, step.description))
        .collect::<Vec<_>>()
        .join("\n")
}

/// 解析 Replanner 返回的 JSON，并转换成 `ReplanResult`。
pub fn parse_replan_response(output: &str) -> anyhow::Result<ReplanResult> {
    let json_text = extract_json_object(output)?;
    let response: ReplannerResponse = serde_json::from_str(json_text)?;

    if !response.need_replan && !response.new_steps.is_empty() {
        anyhow::bail!("replanner returned new_steps while need_replan is false");
    }

    if response.need_replan && response.new_steps.is_empty() {
        anyhow::bail!("replanner requested replan but returned empty new_steps");
    }

    Ok(ReplanResult::new(
        response.need_replan,
        response.reason,
        response
            .new_steps
            .into_iter()
            .map(PlanStep::new)
            .collect::<Vec<_>>(),
    ))
}

/// 从 LLM 输出中提取 JSON 对象文本。
fn extract_json_object(output: &str) -> anyhow::Result<&str> {
    let start = output
        .find('{')
        .ok_or_else(|| anyhow::anyhow!("replanner output does not contain JSON object"))?;
    let end = output
        .rfind('}')
        .ok_or_else(|| anyhow::anyhow!("replanner output does not contain JSON object"))?;

    Ok(output[start..=end].trim())
}

/// Replanner JSON 响应的中间结构。
#[derive(Debug, Deserialize)]
struct ReplannerResponse {
    /// 是否需要重新规划。
    need_replan: bool,
    /// 判断原因。
    reason: String,
    /// 新的后续步骤。
    new_steps: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证不需要 replan 的响应可以解析。
    #[test]
    fn parses_no_replan_response() {
        let result = parse_replan_response(
            r#"{"need_replan":false,"reason":"计划仍然有效","new_steps":[]}"#,
        )
        .unwrap();

        assert!(!result.need_replan);
        assert!(result.new_steps.is_empty());
    }

    /// 验证需要 replan 的响应可以解析。
    #[test]
    fn parses_replan_response() {
        let result = parse_replan_response(
            r#"{"need_replan":true,"reason":"需要调整","new_steps":["新步骤1","新步骤2"]}"#,
        )
        .unwrap();

        assert!(result.need_replan);
        assert_eq!(result.new_steps.len(), 2);
    }
}
