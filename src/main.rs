#[path = "Agents/mod.rs"]
mod agents;
mod config;
mod llm;
mod tools;

use agents::base::Agent;
use agents::react::ReactAgent;
use agents::types::AgentInput;
use llm::LlmClient;
use tools::registry::build_default_registry;

fn main() {
    // 初始化 LLM 客户端。
    // ReactAgent 内部会使用这个 client 不断调用模型，生成 Thought 和 Action。
    let llm = match LlmClient::connect() {
        Ok(llm) => llm,
        Err(error) => {
            eprintln!("Failed to connect LLM client: {error}");
            return;
        }
    };

    // 构建默认工具注册表。
    // 当前默认注册了 SearchTool，后续可以在 build_default_registry 中继续加入更多内置工具。
    let registry = match build_default_registry() {
        Ok(registry) => registry,
        Err(error) => {
            eprintln!("Failed to build tool registry: {error}");
            return;
        }
    };

    // 创建 ReactAgent。
    // max_steps 限制最多执行几轮“思考 -> 行动 -> 观察”，避免模型一直不返回最终答案。
    let agent = ReactAgent::new(llm, registry, 5);

    // 构造 Agent 输入。
    // 这个问题需要搜索最新资料，适合测试 ReAct loop 是否会调用 search tool。
    let input =
        AgentInput::new("请搜索 Rust 中适合构建 LLM agent 应用的框架，并用中文总结 3 个候选项。");

    // 运行完整 ReAct 循环。
    // 成功时会返回模型通过 finish action 给出的最终答案。
    match agent.run(input) {
        Ok(answer) => {
            println!("React agent answer:");
            println!("{answer}");
        }
        Err(error) => {
            eprintln!("React agent failed: {error:#}");
        }
    }
}
