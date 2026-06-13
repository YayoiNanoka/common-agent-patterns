use serde::Deserialize;

use crate::agents::plan_execute_replan::types::PlanStep;
use crate::console::print_block;
use crate::llm::LlmClient;

/// Planner 子 agent 的 prompt 模板。
///
/// Planner 只负责把用户任务拆解成步骤，不执行任何步骤。
pub const PLANNER_PROMPT_TEMPLATE: &str = r#"
你是一个任务规划器。你的任务是将用户提出的问题拆解成若干个简单、明确、可执行的步骤。

要求：
1. 每个步骤必须是独立的、可执行的子任务。
2. 步骤之间必须符合逻辑顺序。
3. 不要执行任务，只负责生成计划。
4. 只输出 JSON，不要输出解释、Markdown、代码块或额外文本。

用户任务：
{question}

请严格按照以下 JSON 格式输出：

{
  "steps": [
    "步骤1",
    "步骤2",
    "步骤3"
  ]
}
"#;

/// 负责生成初始计划的子 agent。
pub struct Planner {
    llm: LlmClient,
}

impl Planner {
    /// 创建 Planner。
    pub fn new(llm: LlmClient) -> Self {
        Self { llm }
    }

    /// 根据用户任务生成计划步骤。
    pub fn run(&self, question: &str) -> anyhow::Result<Vec<PlanStep>> {
        println!("🧩 Planner 开始生成计划...");
        let prompt = self.build_prompt(question);
        let output = self.llm.chat(&prompt)?;
        print_block("📨 Planner LLM 输出", &output);

        let steps = self.parse_plan_response(&output)?;
        println!("✅ Planner 生成计划完成，共 {} 步。", steps.len());
        print_block("📋 Planner 生成的计划", render_steps_for_console(&steps));

        Ok(steps)
    }

    /// 拼装发送给 LLM 的 planner prompt。
    fn build_prompt(&self, question: &str) -> String {
        PLANNER_PROMPT_TEMPLATE.replace("{question}", question)
    }

    /// 解析 Planner 返回的 JSON，并转换成 `PlanStep`。
    pub fn parse_plan_response(&self, output: &str) -> anyhow::Result<Vec<PlanStep>> {
        parse_plan_response(output)
    }
}

/// 将计划步骤转换成适合控制台展示的编号文本。
fn render_steps_for_console(steps: &[PlanStep]) -> String {
    steps
        .iter()
        .enumerate()
        .map(|(index, step)| format!("{}. {}", index + 1, step.description))
        .collect::<Vec<_>>()
        .join("\n")
}

/// 解析 Planner 返回的 JSON，并转换成计划步骤列表。
pub fn parse_plan_response(output: &str) -> anyhow::Result<Vec<PlanStep>> {
    let json_text = extract_json_object(output)?;
    let response: PlannerResponse = serde_json::from_str(json_text)?;

    if response.steps.is_empty() {
        anyhow::bail!("planner returned empty steps");
    }

    Ok(response
        .steps
        .into_iter()
        .map(PlanStep::new)
        .collect::<Vec<_>>())
}

/// 从 LLM 输出中提取 JSON 对象文本。
///
/// 正常情况下 Planner 只会输出 JSON；这里仍然做一次截取，提升一点容错能力。
fn extract_json_object(output: &str) -> anyhow::Result<&str> {
    let start = output
        .find('{')
        .ok_or_else(|| anyhow::anyhow!("planner output does not contain JSON object"))?;
    let end = output
        .rfind('}')
        .ok_or_else(|| anyhow::anyhow!("planner output does not contain JSON object"))?;

    Ok(output[start..=end].trim())
}

/// Planner JSON 响应的中间结构。
#[derive(Debug, Deserialize)]
struct PlannerResponse {
    /// Planner 生成的步骤文本列表。
    steps: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证 Planner JSON 能解析为计划步骤。
    #[test]
    fn parses_plan_response() {
        let steps = parse_plan_response(r#"{"steps":["步骤1","步骤2"]}"#).unwrap();

        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0], PlanStep::new("步骤1"));
    }
}
