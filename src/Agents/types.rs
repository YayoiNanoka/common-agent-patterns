/// Agent 运行时接收的输入。
///
/// 第一版只保留用户任务本身，后续如果需要上下文、会话历史或配置项，可以继续扩展这个结构体。
#[derive(Debug, Clone)]
pub struct AgentInput {
    /// 用户希望 Agent 完成的任务。
    pub task: String,
}

impl AgentInput {
    /// 创建一个新的 Agent 输入。
    pub fn new(task: impl Into<String>) -> Self {
        Self { task: task.into() }
    }
}
