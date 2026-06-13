use serde_json::Value;

/// 工具执行后的统一返回结果。
///
/// `content` 会作为 Observation 交回给 Agent，供后续推理使用。
#[derive(Debug, Clone)]
pub struct ToolResult {
    /// 工具返回给 Agent 的文本内容。
    pub content: String,
}

/// 所有工具都需要实现的基础抽象。
///
/// Tool 只关心自身的元信息、参数结构和执行逻辑，不负责注册、解析 LLM 输出或调度执行。
pub trait Tool: Send + Sync {
    /// 返回工具名称，用于 LLM 输出 Action 时匹配具体工具。
    fn name(&self) -> &'static str;

    /// 返回工具描述，用于拼接到 prompt 中告诉 LLM 何时使用该工具。
    fn description(&self) -> &'static str;

    /// 返回工具参数的 JSON Schema，用于告诉 LLM 该工具需要哪些参数。
    fn parameters_schema(&self) -> Value;

    /// 执行工具逻辑。
    ///
    /// `args` 是 parser 从 LLM 输出中解析出来的 JSON 参数。
    fn execute(&self, args: Value) -> anyhow::Result<ToolResult>;
}
