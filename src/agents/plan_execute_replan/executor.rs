use serde::Deserialize;
use serde_json::Value;

use crate::agents::plan_execute_replan::types::{PlanState, PlanStep, StepResult};
use crate::console::print_block;
use crate::llm::LlmClient;
use crate::tools::executor::ToolExecutor;
use crate::tools::registry::ToolRegistry;

/// Executor 子 agent 的 prompt 模板。
///
/// Executor 负责执行单个计划步骤，可以选择调用工具，也可以直接 finish 当前步骤。
pub const EXECUTOR_PROMPT_TEMPLATE: &str = r#"
你是一个计划步骤执行器。你的任务是根据原始任务、完整计划、历史执行结果和当前步骤，决定下一步应该调用工具，还是直接完成当前步骤。

原始任务：
{question}

完整计划：
{plan}

历史步骤与结果：
{history}

当前步骤：
{current_step}

可用工具：
{tools}

当前轮次说明：
{round_instruction}

请严格按照以下格式输出：

Thought: 简要说明你为什么要采取这个行动。
Action: 一个 JSON 对象。

如果需要调用工具，Action 必须是：

{"type":"tool","name":"工具名称","args":{"参数名":"参数值"}}

如果当前步骤已经可以完成，Action 必须是：

{"type":"finish","answer":"当前步骤的执行结果"}

规则：
1. 每次只能输出一个 Thought 和一个 Action。
2. Action 必须是合法 JSON。
3. 不要输出除 Thought 和 Action 之外的任何内容。
4. 只能调用“可用工具”中列出的工具。
"#;

const DEFAULT_MAX_TOOL_ROUNDS: usize = 3;

/// 负责执行单个计划步骤的子 agent。
pub struct PlanStepExecutor {
    llm: LlmClient,
    tool_executor: ToolExecutor,
    tools_description: String,
    max_tool_rounds: usize,
}

impl PlanStepExecutor {
    /// 创建计划步骤执行器。
    pub fn new(llm: LlmClient, registry: ToolRegistry) -> Self {
        let tools_description = registry.render_tools_description();
        let tool_executor = ToolExecutor::new(registry);

        Self {
            llm,
            tool_executor,
            tools_description,
            max_tool_rounds: DEFAULT_MAX_TOOL_ROUNDS,
        }
    }

    /// 执行当前计划步骤，并返回 `StepResult`。
    ///
    /// 如果 LLM 选择调用工具，会把工具 Observation 追加到当前步骤的临时 history 中，
    /// 然后继续让 LLM 判断当前步骤是否可以 finish。
    pub fn run(&self, state: &PlanState, step: &PlanStep) -> anyhow::Result<StepResult> {
        let mut local_history = Vec::new();

        println!("🛠 Executor 开始执行当前步骤: {}", step.description);

        for round in 1..=self.max_tool_rounds {
            let is_last_round = round == self.max_tool_rounds;
            println!("🔂 Executor tool round {round}/{}", self.max_tool_rounds);

            let prompt = self.build_prompt(state, step, &local_history, is_last_round);
            let output = self.llm.chat(&prompt)?;
            print_block("📨 Executor LLM 输出", &output);

            let action = self.parse_executor_response(&output)?;

            match action {
                ExecutorAction::ToolCall { name, args } => {
                    if is_last_round {
                        println!(
                            "⚠️ Executor 最后一轮仍收到工具调用请求，当前步骤将以部分完成结束。"
                        );
                        return Ok(StepResult::partial(
                            state.current_step_index,
                            step.clone(),
                            build_partial_step_output(
                                "当前步骤执行器已达到最大工具调用轮次，但模型仍请求继续调用工具。",
                                &local_history,
                                Some((&name, &args)),
                            ),
                        ));
                    }

                    println!("🔧 Executor 决定调用工具: {name}");
                    println!("📦 工具参数: {args}");

                    let observation = self.tool_executor.execute_tool_call(&name, args)?;
                    print_block("👀 Tool Observation", &observation.content);

                    local_history.push(format!("{output}\nObservation: {}", observation.content));
                }
                ExecutorAction::Finish { answer } => {
                    println!("✅ Executor 完成当前步骤。");
                    return Ok(StepResult::success(
                        state.current_step_index,
                        step.clone(),
                        answer,
                    ));
                }
            }
        }

        Ok(StepResult::partial(
            state.current_step_index,
            step.clone(),
            build_partial_step_output(
                "当前步骤执行器已达到最大工具调用轮次，未获得明确的 finish 结果。",
                &local_history,
                None,
            ),
        ))
    }

    /// 拼装执行当前步骤所需的 prompt。
    fn build_prompt(
        &self,
        state: &PlanState,
        step: &PlanStep,
        local_history: &[String],
        is_last_round: bool,
    ) -> String {
        let history = render_executor_history(state, local_history);
        let round_instruction = if is_last_round {
            "注意：这是当前步骤执行器的最后一轮。你不能再调用工具，必须基于已有信息完成当前步骤，并输出 finish Action。"
        } else {
            "你可以根据需要调用工具，或者在信息足够时完成当前步骤。"
        };

        EXECUTOR_PROMPT_TEMPLATE
            .replace("{question}", &state.task)
            .replace("{plan}", &state.render_plan())
            .replace("{history}", &history)
            .replace("{current_step}", &step.description)
            .replace("{tools}", &self.tools_description)
            .replace("{round_instruction}", round_instruction)
    }

    /// 解析 Executor 的 LLM 输出。
    pub fn parse_executor_response(&self, output: &str) -> anyhow::Result<ExecutorAction> {
        parse_executor_response(output)
    }
}

/// Executor LLM 输出中的结构化动作。
#[derive(Debug, Clone, PartialEq)]
pub enum ExecutorAction {
    /// 当前步骤需要调用工具。
    ToolCall { name: String, args: Value },
    /// 当前步骤已经执行完成。
    Finish { answer: String },
}

/// 解析 Executor 输出中的 `Action:` JSON。
pub fn parse_executor_response(output: &str) -> anyhow::Result<ExecutorAction> {
    let action_json = extract_action_json(output)?;
    let raw_action: RawExecutorAction = serde_json::from_str(action_json)?;

    match raw_action {
        RawExecutorAction::Tool { name, args } => Ok(ExecutorAction::ToolCall {
            name,
            args: args.unwrap_or(Value::Object(Default::default())),
        }),
        RawExecutorAction::Finish { answer } => Ok(ExecutorAction::Finish { answer }),
    }
}

/// 从 Executor 输出中提取 `Action:` 后面的 JSON。
fn extract_action_json(output: &str) -> anyhow::Result<&str> {
    let (_, action_part) = output
        .split_once("Action:")
        .ok_or_else(|| anyhow::anyhow!("executor output does not contain `Action:`"))?;

    let action_part = action_part.trim();
    if action_part.is_empty() {
        anyhow::bail!("executor output contains empty `Action:`");
    }

    Ok(action_part)
}

/// 将全局历史和当前 step 内部工具调用历史合并成 prompt history。
fn render_executor_history(state: &PlanState, local_history: &[String]) -> String {
    let global_history = state.render_history();

    if local_history.is_empty() {
        return global_history;
    }

    format!(
        "{global_history}\n\n当前步骤内的工具调用记录：\n{}",
        local_history.join("\n\n")
    )
}

/// 构造步骤部分完成时的输出文本。
fn build_partial_step_output(
    reason: &str,
    local_history: &[String],
    requested_tool: Option<(&str, &Value)>,
) -> String {
    let history = if local_history.is_empty() {
        "(empty)".to_string()
    } else {
        local_history.join("\n\n")
    };

    let mut output = format!("{reason}\n\n已有当前步骤内观察记录：\n{history}");

    if let Some((name, args)) = requested_tool {
        output.push_str(&format!(
            "\n\n最后一轮模型仍请求工具调用：\nTool: {name}\nArgs: {args}"
        ));
    }

    output
}

/// Executor Action JSON 的中间反序列化结构。
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum RawExecutorAction {
    /// 工具调用动作。
    Tool { name: String, args: Option<Value> },
    /// 当前步骤完成动作。
    Finish { answer: String },
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// 验证 Executor 工具调用 Action 可以解析。
    #[test]
    fn parses_tool_action() {
        let action = parse_executor_response(
            r#"Thought: search first
Action: {"type":"tool","name":"search","args":{"query":"Rust"}}"#,
        )
        .unwrap();

        assert_eq!(
            action,
            ExecutorAction::ToolCall {
                name: "search".to_string(),
                args: json!({"query": "Rust"}),
            }
        );
    }

    /// 验证 Executor finish Action 可以解析。
    #[test]
    fn parses_finish_action() {
        let action = parse_executor_response(
            r#"Thought: done
Action: {"type":"finish","answer":"步骤完成"}"#,
        )
        .unwrap();

        assert_eq!(
            action,
            ExecutorAction::Finish {
                answer: "步骤完成".to_string(),
            }
        );
    }
}
