use serde::Deserialize;
use serde_json::Value;

/// LLM 输出经过解析后得到的结构化 Agent 动作。
///
/// Agent 根据这个枚举决定是继续调用工具，还是结束任务并返回最终答案。
#[derive(Debug, Clone, PartialEq)]
pub enum AgentAction {
    /// 表示 LLM 希望调用某个工具。
    ToolCall { name: String, args: Value },
    /// 表示 LLM 已经给出最终答案，Agent 可以结束循环。
    Finish { answer: String },
}

/// 从 LLM 原始输出中解析出结构化的 Agent 动作。
///
/// 当前第一版要求输出中包含 `Action:`，并且 `Action:` 后面是 JSON。
pub fn parse_action(output: &str) -> anyhow::Result<AgentAction> {
    let action_json = extract_action_json(output)?;
    let raw_action: RawAction = serde_json::from_str(action_json)?;

    match raw_action {
        RawAction::Tool { name, args } => Ok(AgentAction::ToolCall {
            name,
            args: args.unwrap_or(Value::Object(Default::default())),
        }),
        RawAction::Finish { answer } => Ok(AgentAction::Finish { answer }),
    }
}

/// 从 LLM 输出文本中提取 `Action:` 后面的 JSON 字符串。
///
/// 该函数只负责截取文本，不负责理解 JSON 中的业务含义。
fn extract_action_json(output: &str) -> anyhow::Result<&str> {
    let (_, action_part) = output
        .split_once("Action:")
        .ok_or_else(|| anyhow::anyhow!("LLM output does not contain `Action:`"))?;

    let action_part = action_part.trim();
    if action_part.is_empty() {
        anyhow::bail!("LLM output contains empty `Action:`");
    }

    Ok(action_part)
}

/// 与 LLM 输出 JSON 格式直接对应的中间结构。
///
/// 该类型只用于反序列化，随后会被转换成更适合 Agent 使用的 `AgentAction`。
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum RawAction {
    /// LLM 请求调用工具时的原始 JSON 结构。
    Tool { name: String, args: Option<Value> },
    /// LLM 返回最终答案时的原始 JSON 结构。
    Finish { answer: String },
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_tool_action() {
        let action =
            parse_action(r#"Action: {"type":"tool","name":"search","args":{"query":"Rust"}}"#)
                .unwrap();

        assert_eq!(
            action,
            AgentAction::ToolCall {
                name: "search".to_string(),
                args: json!({"query": "Rust"}),
            }
        );
    }

    #[test]
    fn parses_finish_action() {
        let action = parse_action(r#"Action: {"type":"finish","answer":"最终答案"}"#).unwrap();

        assert_eq!(
            action,
            AgentAction::Finish {
                answer: "最终答案".to_string(),
            }
        );
    }
}
