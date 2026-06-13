use serde::Deserialize;
use serde_json::Value;

use crate::agents::base::Agent;
use crate::agents::types::AgentInput;
use crate::llm::LlmClient;
use crate::tools::base::ToolResult;
use crate::tools::executor::ToolExecutor;
use crate::tools::registry::ToolRegistry;

/// ReAct Agent 使用的 prompt 模板。
///
/// 模板中包含工具说明、用户任务和历史轨迹，要求模型输出 `Thought` 与 `Action`。
const REACT_PROMPT_TEMPLATE: &str = r#"You are a ReAct style agent.
你是一个可以调用外部工具的智能助手。
你需要通过“思考 -> 行动 -> 观察”的方式逐步解决用户问题。

可用工具如下：
{tools_description}

请严格按照以下格式输出：
Thought: 你对当前问题的分析，以及下一步打算做什么。
Action: 一个 JSON 对象，表示你要执行的动作。

如果你需要调用工具，Action 必须使用以下格式：
{"type":"tool","name":"工具名称","args":{"参数名":"参数值"}}

如果你已经获得足够信息，可以回答用户问题，Action 必须使用以下格式：
{"type":"finish","answer":"最终答案"}

规则：
每次只能输出一个 Thought 和一个 Action。
Action 必须是合法 JSON。
不要输出除 Thought 和 Action 之外的其他内容。
只能调用“可用工具”中列出的工具。
如果工具返回的信息不足，你可以继续调用工具。
当你能够回答用户问题时，必须使用 type = "finish" 返回最终答案。

现在，请开始解决以下问题：
Question:
{task}

History:
{history}"#;

/// ReAct Agent 的第一版实现。
///
/// 该 Agent 内部负责拼装 prompt、调用 LLM、解析 Action、执行工具，并把 Observation 追加进 history。
pub struct ReactAgent {
    llm: LlmClient,
    executor: ToolExecutor,
    tools_description: String,
    max_steps: usize,
}

impl ReactAgent {
    /// 创建一个 ReAct Agent。
    ///
    /// `registry` 用于查找和执行工具；`max_steps` 用于限制循环次数，避免模型一直不返回最终答案。
    pub fn new(llm: LlmClient, registry: ToolRegistry, max_steps: usize) -> Self {
        let tools_description = registry.render_tools_description();
        let executor = ToolExecutor::new(registry);

        println!("🤖 ReAct Agent 初始化完成。最大循环轮次: {max_steps}");
        println!("🧰 已加载工具:\n{tools_description}");

        Self {
            llm,
            executor,
            tools_description,
            max_steps,
        }
    }

    /// 根据用户输入和历史轨迹拼装最终发送给 LLM 的 prompt。
    ///
    /// history 第一版直接保存完整的 LLM 输出和 Observation，不单独解析 Thought。
    fn build_prompt(&self, input: &AgentInput, history: &[String]) -> String {
        let history_text = if history.is_empty() {
            "(empty)".to_string()
        } else {
            history.join("\n\n")
        };

        REACT_PROMPT_TEMPLATE
            .replace("{tools_description}", &self.tools_description)
            .replace("{task}", &input.task)
            .replace("{history}", &history_text)
    }

    /// 执行一个工具调用，并把工具结果转换成 Observation 文本。
    fn execute_tool(&self, name: &str, args: Value) -> anyhow::Result<ToolResult> {
        self.executor.execute_tool_call(name, args)
    }
}

impl Agent for ReactAgent {
    /// 运行 ReAct 循环，直到模型返回 finish，或者达到最大步数。
    fn run(&self, input: AgentInput) -> anyhow::Result<String> {
        let mut history = Vec::new();

        println!("🚀 ReAct Agent 开始运行。");
        println!("📌 用户任务: {}", input.task);

        for step in 1..=self.max_steps {
            println!(
                "\n================ ReAct Step {step}/{} ================",
                self.max_steps
            );
            println!("🧾 当前 history 条数: {}", history.len());

            let prompt = self.build_prompt(&input, &history);
            println!(
                "📝 Prompt 已组装完成，长度: {} 字符",
                prompt.chars().count()
            );

            let llm_output = self.llm.chat(&prompt)?;
            println!("📨 LLM 原始输出:\n{llm_output}");

            let action = parse_react_action(&llm_output)?;

            match action {
                ReactAction::ToolCall { name, args } => {
                    println!("🔧 解析到工具调用: {name}");
                    println!("📦 工具参数: {args}");

                    let observation = self.execute_tool(&name, args)?;
                    println!("👀 Observation:\n{}", observation.content);

                    history.push(format!(
                        "Step {step}:\n{llm_output}\nObservation: {}",
                        observation.content
                    ));
                }
                ReactAction::Finish { answer } => {
                    println!("✅ ReAct Agent 返回最终答案。");
                    return Ok(answer);
                }
            }
        }

        anyhow::bail!(
            "ReactAgent reached max steps ({}) without final answer",
            self.max_steps
        )
    }
}

/// ReAct LLM 输出中 Action 部分的结构化结果。
///
/// 该类型只在 `react.rs` 内部使用，避免把 ReAct 专用解析细节泄漏到通用工具模块。
#[derive(Debug, Clone, PartialEq)]
enum ReactAction {
    /// 模型决定调用工具。
    ToolCall { name: String, args: Value },
    /// 模型决定结束任务并返回最终答案。
    Finish { answer: String },
}

/// 解析 ReAct LLM 输出中的 `Action:` 行。
///
/// 第一版不单独解析 Thought，只截取 `Action:` 后面的 JSON 并转换成 `ReactAction`。
fn parse_react_action(output: &str) -> anyhow::Result<ReactAction> {
    let action_json = extract_action_json(output)?;
    let raw_action: RawReactAction = serde_json::from_str(action_json)?;

    match raw_action {
        RawReactAction::Tool { name, args } => Ok(ReactAction::ToolCall {
            name,
            args: args.unwrap_or(Value::Object(Default::default())),
        }),
        RawReactAction::Finish { answer } => Ok(ReactAction::Finish { answer }),
    }
}

/// 从 ReAct 输出中提取 `Action:` 后面的 JSON 字符串。
///
/// 该函数允许 `Action:` 前面有 Thought 文本，但要求 `Action:` 后面是合法 JSON。
fn extract_action_json(output: &str) -> anyhow::Result<&str> {
    let (_, action_part) = output
        .split_once("Action:")
        .ok_or_else(|| anyhow::anyhow!("React output does not contain `Action:`"))?;

    let action_part = action_part.trim();
    if action_part.is_empty() {
        anyhow::bail!("React output contains empty `Action:`");
    }

    Ok(action_part)
}

/// 与 ReAct prompt 约定的 Action JSON 直接对应的中间结构。
///
/// 该类型只用于反序列化，随后会被转换成 `ReactAction`。
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum RawReactAction {
    /// 工具调用格式：`{"type":"tool","name":"search","args":{...}}`。
    Tool { name: String, args: Option<Value> },
    /// 最终答案格式：`{"type":"finish","answer":"..."}`。
    Finish { answer: String },
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// 验证 ReAct 工具调用 Action 可以被解析。
    #[test]
    fn parses_tool_action() {
        let output = r#"Thought: I need to search.
Action: {"type":"tool","name":"search","args":{"query":"Rust agent framework"}}"#;

        let action = parse_react_action(output).unwrap();

        assert_eq!(
            action,
            ReactAction::ToolCall {
                name: "search".to_string(),
                args: json!({"query": "Rust agent framework"}),
            }
        );
    }

    /// 验证 ReAct 最终答案 Action 可以被解析。
    #[test]
    fn parses_finish_action() {
        let output = r#"Thought: I know the answer.
Action: {"type":"finish","answer":"最终答案"}"#;

        let action = parse_react_action(output).unwrap();

        assert_eq!(
            action,
            ReactAction::Finish {
                answer: "最终答案".to_string(),
            }
        );
    }
}
