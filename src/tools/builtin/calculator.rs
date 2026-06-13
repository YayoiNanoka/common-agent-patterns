use serde_json::{Value, json};

use crate::tools::base::{Tool, ToolResult};

/// 内置计算器工具。
///
/// 当前实现基于 `meval` crate，用于计算简单数学表达式。
pub struct CalculatorTool;

impl Tool for CalculatorTool {
    /// 返回计算器工具的固定名称。
    fn name(&self) -> &'static str {
        "calculator"
    }

    /// 返回计算器工具的用途说明。
    fn description(&self) -> &'static str {
        "Evaluate a mathematical expression and return the numeric result."
    }

    /// 返回计算器工具需要的参数 schema。
    ///
    /// 第一版只要求一个字符串类型的 `expression` 参数。
    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "expression": {
                    "type": "string",
                    "description": "A mathematical expression to evaluate, for example: 2 + 3 * (4 - 1)."
                }
            },
            "required": ["expression"]
        })
    }

    /// 执行数学表达式计算。
    ///
    /// 输入来自 LLM 输出的 JSON args，例如：`{"expression":"2 + 3 * 4"}`。
    fn execute(&self, args: Value) -> anyhow::Result<ToolResult> {
        let expression = parse_expression(&args)?;
        let result = evaluate_expression(expression)?;

        Ok(ToolResult {
            content: format!("Expression: {expression}\nResult: {result}"),
        })
    }
}

/// 从工具参数中读取表达式字符串。
fn parse_expression(args: &Value) -> anyhow::Result<&str> {
    let expression = args
        .get("expression")
        .and_then(Value::as_str)
        .map(str::trim)
        .ok_or_else(|| anyhow::anyhow!("calculator tool requires string argument `expression`"))?;

    if expression.is_empty() {
        anyhow::bail!("calculator tool argument `expression` cannot be empty");
    }

    Ok(expression)
}

/// 使用现成表达式求值库计算数学表达式。
fn evaluate_expression(expression: &str) -> anyhow::Result<f64> {
    meval::eval_str(expression)
        .map_err(|error| anyhow::anyhow!("failed to evaluate expression `{expression}`: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evaluates_basic_expression() {
        let result = evaluate_expression("2 + 3 * 4").unwrap();

        assert_eq!(result, 14.0);
    }

    #[test]
    fn evaluates_expression_with_parentheses() {
        let result = evaluate_expression("(2 + 3) * 4").unwrap();

        assert_eq!(result, 20.0);
    }

    #[test]
    fn rejects_empty_expression() {
        let error = parse_expression(&json!({"expression": "   "})).unwrap_err();

        assert!(error.to_string().contains("cannot be empty"));
    }
}
