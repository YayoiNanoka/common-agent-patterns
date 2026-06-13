use crate::agents::types::AgentInput;

/// 所有 Agent 都需要实现的基础 trait。
///
/// 调用方只需要传入用户任务，具体如何拼装 prompt、调用 LLM、维护循环由具体 Agent 实现决定。
pub trait Agent {
    /// 运行 Agent，并返回最终答案。
    ///
    /// 返回 `anyhow::Result<String>` 是为了把 LLM 调用失败、工具执行失败、解析失败等错误向上传递。
    fn run(&self, input: AgentInput) -> anyhow::Result<String>;
}
