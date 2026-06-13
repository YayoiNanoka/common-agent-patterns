use crate::tools::base::ToolResult;
use crate::tools::parser::AgentAction;
use crate::tools::registry::ToolRegistry;

/// 工具执行器，负责把结构化的工具调用分发给具体工具。
///
/// Executor 不保存工具实现细节，只通过 `ToolRegistry` 查找工具并调用其 `execute` 方法。
pub struct ToolExecutor {
    registry: ToolRegistry,
}

impl ToolExecutor {
    /// 使用一个工具注册表创建执行器。
    pub fn new(registry: ToolRegistry) -> Self {
        Self { registry }
    }

    /// 执行一个 Agent 动作。
    ///
    /// 当前只接受 `ToolCall`；如果收到 `Finish`，说明调用方流程有误，会返回错误。
    pub fn execute(&self, action: AgentAction) -> anyhow::Result<ToolResult> {
        match action {
            AgentAction::ToolCall { name, args } => self.execute_tool_call(&name, args),
            AgentAction::Finish { .. } => {
                anyhow::bail!("finish action should not be executed by ToolExecutor")
            }
        }
    }

    /// 根据工具名称和 JSON 参数执行具体工具。
    ///
    /// 参数校验由具体工具自己完成，executor 只负责查找工具并转发参数。
    pub fn execute_tool_call(
        &self,
        name: &str,
        args: serde_json::Value,
    ) -> anyhow::Result<ToolResult> {
        let tool = self
            .registry
            .get(name)
            .ok_or_else(|| anyhow::anyhow!("tool `{name}` is not registered"))?;

        tool.execute(args)
    }
}
