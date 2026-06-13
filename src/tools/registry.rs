use std::collections::HashMap;
use std::sync::Arc;

use crate::tools::base::Tool;
use crate::tools::builtin::search::SearchTool;

/// 工具注册表，负责保存和查找当前 Agent 可用的所有工具。
///
/// Registry 不负责解析 LLM 输出，也不负责执行工具，只维护工具集合及其描述信息。
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl ToolRegistry {
    /// 创建一个空的工具注册表。
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    /// 向注册表中添加一个工具。
    ///
    /// 如果工具名已经存在，会返回错误，避免 LLM 调用时出现歧义。
    pub fn register<T>(&mut self, tool: T) -> anyhow::Result<()>
    where
        T: Tool + 'static,
    {
        let name = tool.name().to_string();
        if self.tools.contains_key(&name) {
            anyhow::bail!("tool `{name}` is already registered");
        }

        self.tools.insert(name, Arc::new(tool));
        Ok(())
    }

    /// 根据工具名称查找已注册的工具。
    ///
    /// 返回 `Arc<dyn Tool>`，方便多个调用方共享同一个工具实例。
    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.get(name).cloned()
    }

    /// 渲染当前所有工具的描述信息。
    ///
    /// 生成的字符串可以拼接到 prompt 中，让 LLM 知道有哪些工具可用以及参数格式。
    pub fn render_tools_description(&self) -> String {
        let mut tools = self.tools.values().collect::<Vec<_>>();
        tools.sort_by_key(|tool| tool.name());

        tools
            .into_iter()
            .map(|tool| {
                format!(
                    "- {}\n  Description: {}\n  Parameters: {}",
                    tool.name(),
                    tool.description(),
                    tool.parameters_schema()
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

impl Default for ToolRegistry {
    /// 创建默认的空注册表。
    fn default() -> Self {
        Self::new()
    }
}

/// 构建 demo 阶段默认使用的工具注册表。
///
/// 当前只注册 mock 版 `SearchTool`，后续可以在这里继续添加 calculator 等内置工具。
pub fn build_default_registry() -> anyhow::Result<ToolRegistry> {
    let mut registry = ToolRegistry::new();
    registry.register(SearchTool)?;
    Ok(registry)
}
