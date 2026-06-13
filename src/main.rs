mod agents;
mod config;
mod console;
mod llm;
mod tools;

use agents::base::Agent;
use agents::plan_execute_replan::agent::PlanExecuteReplanAgent;
use agents::types::AgentInput;
use console::print_block;
use llm::LlmClient;
use tools::registry::build_default_registry;

fn main() {
    // 初始化 LLM 客户端。
    // Plan-Execute-Replan Agent 内部的 planner、executor、replanner 都会使用这个 client 调用模型。
    let llm = match LlmClient::connect() {
        Ok(llm) => llm,
        Err(error) => {
            eprintln!("Failed to connect LLM client: {error}");
            return;
        }
    };

    // 构建默认工具注册表。
    // 当前默认注册了 SearchTool 和 CalculatorTool，后续可以在 build_default_registry 中继续加入更多内置工具。
    let registry = match build_default_registry() {
        Ok(registry) => registry,
        Err(error) => {
            eprintln!("Failed to build tool registry: {error}");
            return;
        }
    };

    // 创建 Plan-Execute-Replan Agent。
    // 它会先规划步骤，再逐步执行，每一步执行后检查是否需要重新规划。
    let agent = PlanExecuteReplanAgent::new(llm, registry, 5);

    // 构造 Agent 输入。
    // 这个问题适合测试 planner 拆分任务、executor 调用 search tool、replanner 检查计划有效性。
    let input = AgentInput::new(
        "请调研 Rust 中适合构建 LLM agent 应用的框架，比较 3 个候选项，并给出中文建议。",
    );

    // 运行完整 Plan-Execute-Replan 流程。
    // 成功时会返回每个计划步骤的执行结果汇总。
    match agent.run(input) {
        Ok(answer) => {
            print_block("🏁 Plan-Execute-Replan agent answer", answer);
        }
        Err(error) => {
            eprintln!("Plan-Execute-Replan agent failed: {error:#}");
        }
    }
}
