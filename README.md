# Agent Patterns 项目结构笔记

这份文档主要用于记录当前项目的模块职责、接口边界和数据流，方便之后回看代码时快速恢复上下文。

## 1. 项目结构

```text
src/
├── main.rs                    # 当前测试入口，负责组装 LLM、工具注册表和 PlanExecuteReplanAgent
├── config.example.rs           # 可提交的配置示例，不包含真实密钥
├── config.rs                   # 本地真实配置，被 .gitignore 忽略
├── llm.rs                      # LLM API 通信封装
├── tools/
│   ├── mod.rs                  # tools 模块出口
│   ├── base.rs                 # Tool trait 和 ToolResult
│   ├── registry.rs             # 工具注册表，负责注册、保存、查找工具
│   ├── executor.rs             # 工具执行器，负责根据工具名执行具体工具
│   ├── parser.rs               # 通用 Action parser，目前 ReactAgent 主流程没有直接使用
│   └── builtin/
│       ├── mod.rs              # 内置工具模块出口
│       ├── calculator.rs        # 数学表达式计算工具
│       └── search.rs            # SerpApi 搜索工具
└── agents/
    ├── mod.rs                  # agents 模块出口
    ├── base.rs                 # Agent trait
    ├── types.rs                # AgentInput 等通用类型
    ├── summarizer.rs           # 通用 Agent 总结子 agent
    ├── react.rs                # ReAct Agent loop
    └── plan_execute_replan/
        ├── mod.rs              # Plan-Execute-Replan 模块出口
        ├── agent.rs            # P-E-RP 总控 Agent
        ├── planner.rs          # Planner 子 agent，负责生成计划
        ├── executor.rs         # Executor 子 agent，负责执行单个计划步骤
        ├── replanner.rs        # Replanner 子 agent，负责判断是否需要重新规划
        └── types.rs            # PlanState / PlanStep / StepResult / ReplanResult
```

当前 `src/agents` 使用 Rust 标准的小写模块目录，`main.rs` 中通过下面的方式接入：

```rust
mod agents;
```

## 2. 顶层模块总览

### `src/config.example.rs` / `src/config.rs`

职责：保存模型和工具调用所需的配置。

主要内容：

- `OPENAI_API_KEY`
- `OPENAI_BASE_URL`
- `OPENAI_MODEL`
- `AGENT_TEMPERATURE`
- `SERPAPI_API_KEY`

说明：

- `config.example.rs` 是可提交的配置模板。
- `config.rs` 是本地真实配置文件，被 `.gitignore` 忽略。
- `llm.rs` 使用 `OPENAI_*` 配置。
- `tools/builtin/search.rs` 使用 `SERPAPI_API_KEY`。

### `src/llm.rs`

职责：封装 OpenAI-compatible Chat Completions API 调用。

主要输入：

- `prompt: &str`

主要输出：

- `Result<String, LlmError>`

关键结构和函数：

- `LlmClient`
- `LlmError`
- `LlmClient::connect()`
- `LlmClient::chat(prompt)`
- `headers()`

内部流程：

```text
LlmClient::connect
  ↓
读取 config.rs / 环境变量
  ↓
解析 AGENT_TEMPERATURE
  ↓
构造 reqwest blocking Client
  ↓
返回 LlmClient
```

```text
LlmClient::chat(prompt)
  ↓
构造 /chat/completions URL
  ↓
构造 ChatCompletionRequest
  ↓
发送 HTTP 请求
  ↓
读取响应 body
  ↓
解析 ChatCompletionResponse
  ↓
取 choices[0].message.content
  ↓
返回模型文本
```

当前注意点：

- 当前使用 `reqwest::blocking::Client`，不是 async/tokio 写法。
- 当前只解析 `choices[0].message.content`。
- 当前没有实现 stream 流式输出。

### `src/tools/`

职责：工具系统。它把 LLM 输出中的工具调用转换成具体工具执行。

整体输入：

- 工具名：`name: &str`
- 工具参数：`args: serde_json::Value`

整体输出：

- `anyhow::Result<ToolResult>`

工具调用时的输入本质上是 JSON。比如 LLM 输出：

```text
Action: {"type":"tool","name":"search","args":{"query":"Rust agent framework","num_results":3}}
```

这里真正传给工具的是：

```json
{
  "query": "Rust agent framework",
  "num_results": 3
}
```

整体调用关系：

```text
ReactAgent 解析 LLM Action
  ↓
ToolExecutor::execute_tool_call(name, args)
  ↓
ToolRegistry::get(name)
  ↓
dyn Tool::execute(args)
  ↓
ToolResult
```

### `src/agents/`

职责：定义 Agent 抽象和具体 Agent loop。

当前已实现：

- `ReactAgent`
- `PlanExecuteReplanAgent`

整体输入：

- `AgentInput`

整体输出：

- `anyhow::Result<String>`

整体流程：

```text
AgentInput
  ↓
具体 Agent::run
  ↓
拼装 prompt
  ↓
LlmClient::chat
  ↓
解析 LLM 输出
  ↓
ToolExecutor / ToolRegistry / Tool
  ↓
更新 Agent 内部状态
  ↓
下一轮 loop 或返回 answer
```

### `src/main.rs`

职责：当前测试入口，用来测试完整 Plan-Execute-Replan Agent 链路。

当前流程：

```text
LlmClient::connect
  ↓
build_default_registry
  ↓
PlanExecuteReplanAgent::new
  ↓
传入 max_steps
  ↓
AgentInput::new
  ↓
agent.run
  ↓
打印最终 answer
```

## 3. 核心数据流

从用户任务到最终答案的大致流程：

```text
用户任务 String
  ↓
AgentInput { task }
  ↓
ReactAgent::run(input)
  ↓
ReactAgent::build_prompt(input, history)
  ↓
LlmClient::chat(prompt)
  ↓
LLM 输出 Thought + Action
  ↓
parse_react_action(llm_output)
  ↓
ReactAction::ToolCall 或 ReactAction::Finish
```

如果是工具调用：

```text
ReactAction::ToolCall { name, args }
  ↓
ToolExecutor::execute_tool_call(name, args)
  ↓
ToolRegistry::get(name)
  ↓
SearchTool::execute(args)
  ↓
ToolResult { content }
  ↓
Observation 追加到 history
  ↓
进入下一轮 ReAct loop
```

如果是最终答案：

```text
ReactAction::Finish { answer }
  ↓
ReactAgent::run 返回 answer
```

需要注意：用户输入不会直接进入 tool。用户输入先进入 Agent，Agent 通过 LLM 决定是否调用工具。工具真正收到的输入来自 LLM 输出的 `Action.args`。

## 4. tools 模块说明

`tools/` 是工具系统模块。它的职责是让 Agent 可以通过统一接口调用外部能力。

整体输入：

- `name`: 工具名称
- `args`: JSON 参数，也就是 `serde_json::Value`

整体输出：

- `ToolResult`

`ToolResult.content` 会作为 Observation 返回给 Agent。

### `src/tools/base.rs`

职责：定义所有工具都要遵守的基础接口。

核心结构：

```rust
pub struct ToolResult {
    pub content: String,
}
```

核心 trait：

```rust
pub trait Tool: Send + Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn parameters_schema(&self) -> serde_json::Value;
    fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult>;
}
```

接口含义：

- `name()`：返回工具名，用于匹配 LLM 输出中的 `Action.name`。
- `description()`：返回工具描述，用于拼接到 prompt 中。
- `parameters_schema()`：返回参数 schema，用于告诉 LLM 参数格式。
- `execute(args)`：真正执行工具逻辑。

输入输出：

```text
execute 输入: serde_json::Value
execute 输出: anyhow::Result<ToolResult>
```

### `src/tools/registry.rs`

职责：注册、保存、查找工具，并渲染工具描述。

核心结构：

```rust
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}
```

关键函数：

- `ToolRegistry::new()`
- `register(tool)`
- `get(name)`
- `render_tools_description()`
- `build_default_registry()`

内部流程：

```text
build_default_registry
  ↓
ToolRegistry::new
  ↓
registry.register(SearchTool)
  ↓
registry.register(CalculatorTool)
  ↓
返回 ToolRegistry
```

`render_tools_description()` 会遍历所有已注册工具，生成工具说明文本，后续会放进 ReAct prompt 中。

### `src/tools/executor.rs`

职责：根据工具名和 JSON 参数执行具体工具。

核心结构：

```rust
pub struct ToolExecutor {
    registry: ToolRegistry,
}
```

关键函数：

- `ToolExecutor::new(registry)`
- `execute_tool_call(name, args)`
- `execute(action)`

当前 ReAct 主流程主要使用：

```rust
execute_tool_call(name, args)
```

处理流程：

```text
name + args
  ↓
registry.get(name)
  ↓
拿到 Arc<dyn Tool>
  ↓
tool.execute(args)
  ↓
ToolResult
```

注意点：

- executor 不做参数 schema 校验。
- `parameters_schema()` 主要是给 LLM 看。
- 具体工具自己负责解析和校验 `args`。

### `src/tools/parser.rs`

职责：通用 Action parser。

核心类型：

```rust
pub enum AgentAction {
    ToolCall {
        name: String,
        args: serde_json::Value,
    },
    Finish {
        answer: String,
    },
}
```

关键函数：

```rust
parse_action(output: &str) -> anyhow::Result<AgentAction>
```

支持的输出格式：

```text
Action: {"type":"tool","name":"search","args":{"query":"Rust"}}
```

或：

```text
Action: {"type":"finish","answer":"最终答案"}
```

当前注意点：

- 这个模块是通用 parser。
- 当前 `ReactAgent` 主流程没有直接使用它。
- `ReactAgent` 在 `src/agents/react.rs` 内部实现了自己的 `parse_react_action()`。
- 后续如果想减少重复，可以考虑把两处 parser 合并。

### `src/tools/builtin/calculator.rs`

职责：内置计算器工具，当前通过 `meval` crate 计算简单数学表达式。

实现的工具：

```rust
pub struct CalculatorTool;
```

工具名：

```text
calculator
```

输入 JSON：

```json
{
  "expression": "2 + 3 * (4 - 1)"
}
```

字段说明：

- `expression`：必填，字符串类型的数学表达式。

输出：

```text
ToolResult.content = 表达式和计算结果
```

输出示例：

```text
Expression: 2 + 3 * 4
Result: 14
```

关键函数：

- `CalculatorTool::execute(args)`
- `parse_expression(args)`
- `evaluate_expression(expression)`

内部流程：

```text
CalculatorTool::execute(args)
  ↓
parse_expression(args)
  ↓
evaluate_expression(expression)
  ↓
ToolResult { content }
```

注意点：

- calculator 的输入仍然是 `serde_json::Value`。
- 具体表达式求值交给 `meval`，当前项目不自己实现数学 parser。
- 表达式为空、缺少 `expression`、表达式非法时会返回错误。

### `src/tools/builtin/search.rs`

职责：内置搜索工具，当前通过 SerpApi 调用 Google Search API。

实现的工具：

```rust
pub struct SearchTool;
```

工具名：

```text
search
```

输入 JSON：

```json
{
  "query": "Rust agent framework",
  "num_results": 3,
  "location": "optional"
}
```

字段说明：

- `query`：必填，搜索关键词。
- `num_results`：可选，返回结果数量，默认 5，最大 10。
- `location`：可选，搜索地区。

输出：

```text
ToolResult.content = 格式化后的搜索结果文本
```

关键函数：

- `SearchTool::execute(args)`
- `parse_query(args)`
- `parse_result_limit(args)`
- `load_serpapi_api_key()`
- `format_search_results(query, results, limit)`

内部流程：

```text
SearchTool::execute(args)
  ↓
parse_query(args)
  ↓
parse_result_limit(args)
  ↓
load_serpapi_api_key()
  ↓
reqwest GET https://serpapi.com/search
  ↓
解析 SerpApiResponse
  ↓
format_search_results(...)
  ↓
ToolResult { content }
```

## 5. tools 模块处理一次请求的流程

以 search 工具为例，LLM 输出：

```text
Thought: 我需要搜索 Rust agent framework。
Action: {"type":"tool","name":"search","args":{"query":"Rust agent framework","num_results":3}}
```

处理流程：

```text
parse_react_action
  ↓
ReactAction::ToolCall {
      name: "search",
      args: {"query":"Rust agent framework","num_results":3}
  }
  ↓
ToolExecutor::execute_tool_call("search", args)
  ↓
ToolRegistry::get("search")
  ↓
SearchTool::execute(args)
  ↓
ToolResult {
      content: "Search results for query: ..."
  }
```

这个流程中，各层职责是：

- `ReactAgent`：决定是否要调用工具，并把 LLM 输出解析成结构化 Action。
- `ToolExecutor`：根据工具名分发调用。
- `ToolRegistry`：保存工具并按名称查找工具。
- `SearchTool`：解析自己的参数，调用 SerpApi，返回结果。

## 6. agents 模块说明

`agents/` 负责 Agent 抽象和具体 Agent loop。

### `src/agents/types.rs`

职责：定义 Agent 运行时的输入类型。

核心结构：

```rust
pub struct AgentInput {
    pub task: String,
}
```

关键函数：

```rust
AgentInput::new(task)
```

当前输入很简单，只包含用户任务本身。后续如果需要上下文、会话历史或运行配置，可以继续扩展这个结构。

### `src/agents/base.rs`

职责：定义所有 Agent 都应实现的 trait。

核心 trait：

```rust
pub trait Agent {
    fn run(&self, input: AgentInput) -> anyhow::Result<String>;
}
```

输入：

- `AgentInput`

输出：

- `anyhow::Result<String>`

这里返回 `Result` 是因为 Agent loop 中可能出现多种错误：

- LLM 调用失败
- LLM 输出格式不对
- Action JSON 解析失败
- 工具不存在
- 工具执行失败
- 超过最大循环次数

### `src/agents/summarizer.rs`

职责：通用 Agent 总结子 agent。

它不绑定具体 agent pattern，用于在“任务未完成但已有部分信息”或“需要把执行轨迹整理成答案”时，让 LLM 基于已有信息生成中文总结。

核心结构：

```rust
pub struct SummaryInput {
    pub task: String,
    pub status: String,
    pub completed_work: String,
    pub remaining_work: String,
    pub reason: String,
}
```

```rust
pub struct AgentSummarizer {
    llm: LlmClient,
}
```

关键内容：

- `SUMMARY_PROMPT_TEMPLATE`
- `SummaryInput`
- `AgentSummarizer::new(llm)`
- `AgentSummarizer::summarize(input)`

输入：

- `SummaryInput`

输出：

- `anyhow::Result<String>`

当前用途：

- `PlanExecuteReplanAgent` 到达外层 `max_steps` 但任务未完成时，会把 `PlanState` 转换成 `SummaryInput`，再调用 summarizer 生成阶段性答案。

后续可复用场景：

- `ReactAgent` 到达最大推理轮次但未 finish 时，也可以把 history 转换成 `SummaryInput` 后复用这个 summarizer。

### `src/agents/react.rs`

职责：实现 ReAct Agent loop。

核心结构：

```rust
pub struct ReactAgent {
    llm: LlmClient,
    executor: ToolExecutor,
    tools_description: String,
    max_steps: usize,
}
```

关键内容：

- `REACT_PROMPT_TEMPLATE`
- `ReactAgent::new(llm, registry, max_steps)`
- `build_prompt(input, history)`
- `execute_tool(name, args)`
- `run(input)`
- `parse_react_action(output)`
- `extract_action_json(output)`

初始化流程：

```text
ReactAgent::new
  ↓
registry.render_tools_description()
  ↓
ToolExecutor::new(registry)
  ↓
保存 llm / executor / tools_description / max_steps
```

运行流程：

```text
ReactAgent::run(input)
  ↓
let history = Vec::new()
  ↓
for step in 1..=max_steps
  ↓
build_prompt(input, history)
  ↓
llm.chat(prompt)
  ↓
parse_react_action(llm_output)
```

如果模型返回工具调用：

```text
ReactAction::ToolCall { name, args }
  ↓
execute_tool(name, args)
  ↓
ToolResult
  ↓
history.push(llm_output + Observation)
  ↓
下一轮循环
```

如果模型返回最终答案：

```text
ReactAction::Finish { answer }
  ↓
return Ok(answer)
```

如果超过最大轮次：

```text
return Err("ReactAgent reached max steps ...")
```

当前设计细节：

- `ReactAgent` 不单独解析 `Thought`。
- history 直接保存完整的 LLM 输出和 Observation。
- parser 作为 `react.rs` 内部函数实现，提高 ReAct 逻辑阅读连贯性。
- ReAct loop 依赖模型严格输出 `Action: {...}`。

### `src/agents/plan_execute_replan/`

职责：实现 Plan-Execute-Replan 模式。

这个模块把一个复杂任务拆成三个阶段：

```text
Plan
  ↓
Execute
  ↓
Replan
```

整体输入：

- `AgentInput { task }`

整体输出：

- `anyhow::Result<String>`

整体状态：

- `PlanState`

整体流程：

```text
PlanExecuteReplanAgent::run(input)
  ↓
Planner::run(task)
  ↓
PlanState::new(task, plan)
  ↓
循环执行每个 PlanStep
  ↓
PlanStepExecutor::run(state, step)
  ↓
StepResult
  ↓
PlanState 记录 StepResult
  ↓
Replanner::run(state, current_step, step_result)
  ↓
按需修改后续计划
  ↓
所有步骤完成后汇总最终 answer
```

#### `src/agents/plan_execute_replan/types.rs`

职责：定义 P-E-RP 模式内部共享的数据结构。

核心结构：

```rust
pub struct PlanState {
    pub task: String,
    pub plan: Vec<PlanStep>,
    pub current_step_index: usize,
    pub total_steps: usize,
    pub step_results: Vec<StepResult>,
    pub replan_count: usize,
}
```

`PlanState` 的作用：

- 保存原始任务。
- 保存当前计划。
- 记录当前执行到第几个步骤。
- 保存每个步骤的执行结果。
- 记录重新规划次数。

其他结构：

- `PlanStep`：计划中的单个步骤，核心字段是 `description`。
- `StepResult`：单个步骤的执行结果，包含 step、输出文本、成功状态。成功完成用 `StepResult::success`，轮次耗尽但已有部分信息时用 `StepResult::partial`。
- `ReplanResult`：Replanner 的判断结果，包含是否需要 replan、原因、新的后续步骤。

关键函数：

- `PlanState::new(task, plan)`
- `PlanState::current_step()`
- `PlanState::is_finished()`
- `PlanState::render_plan()`
- `PlanState::render_history()`
- `PlanState::render_remaining_steps()`
- `PlanState::push_step_result(result)`
- `PlanState::advance_step()`
- `PlanState::replace_remaining_steps(new_steps)`
- `render_plan_steps(steps)`

#### `src/agents/plan_execute_replan/planner.rs`

职责：Planner 子 agent，负责根据用户任务生成初始计划。

输入：

- `question: &str`

输出：

- `anyhow::Result<Vec<PlanStep>>`

关键内容：

- `PLANNER_PROMPT_TEMPLATE`
- `Planner`
- `Planner::new(llm)`
- `Planner::run(question)`
- `build_prompt(question)`
- `parse_plan_response(output)`

内部流程：

```text
Planner::run(question)
  ↓
打印 Planner 开始生成计划
  ↓
build_prompt(question)
  ↓
llm.chat(prompt)
  ↓
打印 Planner LLM 原始输出
  ↓
parse_plan_response(output)
  ↓
打印计划步骤数量和每个步骤
  ↓
Vec<PlanStep>
```

Planner 的 LLM 输出要求是 JSON：

```json
{
  "steps": [
    "步骤1",
    "步骤2"
  ]
}
```

#### `src/agents/plan_execute_replan/executor.rs`

职责：Executor 子 agent，负责执行单个 `PlanStep`。

输入：

- `state: &PlanState`
- `step: &PlanStep`

输出：

- `anyhow::Result<StepResult>`

关键内容：

- `EXECUTOR_PROMPT_TEMPLATE`
- `PlanStepExecutor`
- `PlanStepExecutor::new(llm, registry)`
- `PlanStepExecutor::run(state, step)`
- `build_prompt(state, step, local_history)`
- `parse_executor_response(output)`

Executor 每次 LLM 输出格式：

```text
Thought: ...
Action: {"type":"tool","name":"search","args":{"query":"..."}}
```

或者：

```text
Thought: ...
Action: {"type":"finish","answer":"当前步骤的执行结果"}
```

内部流程：

```text
PlanStepExecutor::run(state, step)
  ↓
打印当前步骤开始执行
  ↓
for round in 1..=max_tool_rounds
  ↓
打印当前 tool round
  ↓
build_prompt(state, step, local_history)
  ↓
llm.chat(prompt)
  ↓
打印 Executor LLM 原始输出
  ↓
parse_executor_response(output)
```

如果解析到工具调用：

```text
ExecutorAction::ToolCall { name, args }
  ↓
如果这是最后一轮:
    返回 StepResult::partial，不再继续调用工具
  ↓
打印工具名和参数
  ↓
ToolExecutor::execute_tool_call(name, args)
  ↓
打印 Tool Observation
  ↓
local_history.push(output + Observation)
  ↓
继续下一轮，让 LLM 判断当前 step 是否可以 finish
```

如果解析到完成：

```text
ExecutorAction::Finish { answer }
  ↓
打印当前步骤完成
  ↓
StepResult::success(...)
```

注意点：

- Executor 子 agent 内部也有一个小循环。
- 这个小循环只服务于当前 step。
- 最后一轮 prompt 会明确告诉 LLM：这是最后一轮，必须基于已有信息 finish，不要继续调用工具。
- 如果最后一轮模型仍然要求调用工具，executor 不再报错，而是返回 `StepResult::partial`。
- 如果达到 `max_tool_rounds` 仍没有明确 finish，executor 也返回 `StepResult::partial`。
- 工具调用结果会进入 `local_history`，不会立刻写入全局 `PlanState.step_results`。
- 当前 step finish 后，`agent.rs` 才会把 `StepResult` 写入 `PlanState`。

#### `src/agents/plan_execute_replan/replanner.rs`

职责：Replanner 子 agent，负责根据当前步骤执行结果判断是否需要修改后续计划。

输入：

- `state: &PlanState`
- `current_step: &PlanStep`
- `step_result: &StepResult`

输出：

- `anyhow::Result<ReplanResult>`

关键内容：

- `REPLANNER_PROMPT_TEMPLATE`
- `Replanner`
- `Replanner::new(llm)`
- `Replanner::run(state, current_step, step_result)`
- `build_prompt(state, current_step, step_result)`
- `parse_replan_response(output)`

Replanner 的 LLM 输出要求是 JSON：

```json
{
  "need_replan": false,
  "reason": "说明为什么需要或不需要重新规划",
  "new_steps": []
}
```

如果需要重新规划：

```json
{
  "need_replan": true,
  "reason": "说明为什么需要重新规划",
  "new_steps": [
    "新的步骤1",
    "新的步骤2"
  ]
}
```

内部流程：

```text
Replanner::run(state, current_step, step_result)
  ↓
打印开始检查当前步骤结果
  ↓
build_prompt(...)
  ↓
llm.chat(prompt)
  ↓
打印 Replanner LLM 原始输出
  ↓
parse_replan_response(output)
  ↓
打印 need_replan 和 reason
  ↓
如果 need_replan = true，打印新的后续步骤
  ↓
ReplanResult
```

#### `src/agents/plan_execute_replan/agent.rs`

职责：P-E-RP 总控 Agent，负责把 Planner、Executor、Replanner 串成一个完整循环。

核心结构：

```rust
pub struct PlanExecuteReplanAgent {
    planner: Planner,
    executor: PlanStepExecutor,
    replanner: Replanner,
    summarizer: AgentSummarizer,
    max_steps: usize,
}
```

关键函数：

- `PlanExecuteReplanAgent::new(llm, registry, max_steps)`
- `run(input)`
- `maybe_replan(state, step_result)`
- `build_final_answer(state)`
- `build_incomplete_answer(state)`

初始化流程：

```text
PlanExecuteReplanAgent::new(llm, registry, max_steps)
  ↓
Planner::new(llm.clone())
  ↓
PlanStepExecutor::new(llm.clone(), registry)
  ↓
Replanner::new(llm.clone())
  ↓
AgentSummarizer::new(llm)
  ↓
保存 max_steps
```

运行流程：

```text
run(input)
  ↓
打印 Agent 开始运行和用户任务
  ↓
Planner::run(input.task)
  ↓
PlanState::new(task, plan)
  ↓
打印初始计划
  ↓
while !state.is_finished()
```

现在实际循环条件是：

```text
while !state.is_finished() && executed_steps < max_steps
```

每个计划步骤：

```text
打印 Step 当前轮次和当前步骤
  ↓
PlanStepExecutor::run(state, step)
  ↓
打印当前步骤完成
  ↓
state.push_step_result(step_result)
  ↓
maybe_replan(state, step_result)
  ↓
state.advance_step()
```

重新规划：

```text
maybe_replan
  ↓
Replanner::run(...)
  ↓
打印 Replan 检查结果
  ↓
如果 need_replan:
      state.replace_remaining_steps(new_steps)
      打印更新后的计划
```

完成：

```text
所有步骤执行完
  ↓
打印 P-E-RP Agent 执行完成
  ↓
build_final_answer(state)
```

如果达到最大执行步骤数：

```text
executed_steps >= max_steps
  ↓
打印达到最大执行步骤数
  ↓
PlanState -> SummaryInput
  ↓
AgentSummarizer::summarize(input)
  ↓
返回阶段性答案
```

## 7. 一次完整 ReAct 请求的数据流

下面是一条完整链路：

```text
main.rs
  ↓
LlmClient::connect()
  ↓
build_default_registry()
  ↓
ReactAgent::new(llm, registry, max_steps)
  ↓
AgentInput::new(task)
  ↓
agent.run(input)
```

进入 ReAct loop 后：

```text
AgentInput.task
  ↓
ReactAgent::build_prompt
  ↓
REACT_PROMPT_TEMPLATE + tools_description + history
  ↓
LlmClient::chat(prompt)
  ↓
LLM output:
Thought: ...
Action: {...}
  ↓
parse_react_action
```

如果 Action 是 tool：

```text
ReactAction::ToolCall
  ↓
ToolExecutor::execute_tool_call
  ↓
ToolRegistry::get
  ↓
SearchTool::execute
  ↓
SerpApi
  ↓
ToolResult.content
  ↓
Observation
  ↓
history
  ↓
下一轮 prompt
```

如果 Action 是 finish：

```text
ReactAction::Finish
  ↓
返回最终 answer
```

## 8. 一次完整 Plan-Execute-Replan 请求的数据流

下面是一条完整 P-E-RP 链路：

```text
main.rs
  ↓
LlmClient::connect()
  ↓
build_default_registry()
  ↓
PlanExecuteReplanAgent::new(llm, registry, max_steps)
  ↓
AgentInput::new(task)
  ↓
agent.run(input)
```

进入 P-E-RP agent 后：

```text
AgentInput.task
  ↓
Planner::run(task)
  ↓
PLANNER_PROMPT_TEMPLATE + question
  ↓
LlmClient::chat(prompt)
  ↓
Planner JSON output:
{"steps":["...","..."]}
  ↓
parse_plan_response
  ↓
Vec<PlanStep>
  ↓
PlanState::new(task, plan)
```

执行某个计划步骤：

```text
PlanState.current_step()
  ↓
PlanStepExecutor::run(state, step)
  ↓
EXECUTOR_PROMPT_TEMPLATE + question + plan + history + current_step + tools
  ↓
LlmClient::chat(prompt)
  ↓
Executor output:
Thought: ...
Action: {...}
  ↓
parse_executor_response
```

如果 Executor 决定调用工具：

```text
ExecutorAction::ToolCall { name, args }
  ↓
ToolExecutor::execute_tool_call(name, args)
  ↓
ToolRegistry::get(name)
  ↓
SearchTool::execute(args)
  ↓
ToolResult.content
  ↓
local_history 追加 Observation
  ↓
继续 Executor 当前 step 内部下一轮
```

如果 Executor 达到当前 step 的最大工具轮次：

```text
max_tool_rounds exhausted
  ↓
StepResult::partial(...)
  ↓
PlanState::push_step_result(step_result)
  ↓
继续进入 Replanner 或后续步骤
```

如果 Executor 决定完成当前步骤：

```text
ExecutorAction::Finish { answer }
  ↓
StepResult::success(...)
  ↓
PlanState::push_step_result(step_result)
```

之后进入 Replanner：

```text
Replanner::run(state, current_step, step_result)
  ↓
REPLANNER_PROMPT_TEMPLATE + question + plan + current_step + step_result + history
  ↓
LlmClient::chat(prompt)
  ↓
Replanner JSON output:
{"need_replan":false,"reason":"...","new_steps":[]}
  ↓
parse_replan_response
  ↓
ReplanResult
```

如果不需要 replan：

```text
PlanState::advance_step()
  ↓
继续执行下一个 PlanStep
```

如果需要 replan：

```text
ReplanResult { need_replan: true, new_steps }
  ↓
PlanState::replace_remaining_steps(new_steps)
  ↓
PlanState::advance_step()
  ↓
按更新后的计划继续执行
```

全部步骤完成后：

```text
PlanState::is_finished() == true
  ↓
PlanExecuteReplanAgent::build_final_answer(state)
  ↓
返回最终汇总
```

如果外层 P-E-RP 达到最大执行步骤数：

```text
executed_steps >= max_steps
  ↓
state.render_history()
  ↓
state.render_remaining_steps()
  ↓
SummaryInput
  ↓
AgentSummarizer::summarize
  ↓
返回“任务未完成但基于已有信息生成”的阶段性答案
```

当前 P-E-RP 控制台输出重点：

- Planner 开始生成计划、LLM 原始输出、计划步骤。
- Executor 当前步骤、tool round、LLM 原始输出、工具调用参数、Observation。
- Replanner 检查开始、LLM 原始输出、是否 replan、原因、新后续步骤。
- 总控 Agent 的初始计划、每个步骤进度、更新后的计划、最终完成。
- 达到外层最大执行步骤数时，会打印预算耗尽并进入 summarizer。

## 9. 当前设计注意点

1. `src/config.rs` 不提交，`src/config.example.rs` 提交。

2. tool 的输入统一是 `serde_json::Value`。

3. `SearchTool` 当前实际支持的参数是：

```text
query
num_results
location
```

4. `ToolExecutor` 不校验参数 schema。

5. `parameters_schema()` 主要给 LLM 看，具体工具负责解析和校验参数。

6. `tools/parser.rs` 当前不是 ReAct 主流程使用的 parser。

7. `ReactAgent` 的 parser 在 `src/agents/react.rs` 内部。

8. `ReactAgent` 当前不单独解析 Thought，history 直接拼接完整 LLM 输出和 Observation。

9. `llm.rs` 当前使用 blocking HTTP 请求。

10. 当前没有实现 LLM stream 流式输出。

11. ReAct loop 当前强依赖模型输出合法的 `Action` JSON。

12. `src/agents` 当前使用小写目录，符合 Rust 常见模块命名习惯。

13. `AgentSummarizer` 是通用总结子 agent，当前被 P-E-RP 用于达到最大执行步骤数后的阶段性总结，后续也可以被 ReAct 复用。

14. `PlanExecuteReplanAgent` 当前由 `Planner`、`PlanStepExecutor`、`Replanner` 和通用 `AgentSummarizer` 组成。

15. P-E-RP 的外层 `max_steps` 通过 `PlanExecuteReplanAgent::new(llm, registry, max_steps)` 传入。

16. P-E-RP 的 `Executor` 子 agent 内部有自己的工具调用小循环，工具 Observation 先进入当前 step 的 `local_history`。

17. P-E-RP 的全局执行历史保存在 `PlanState.step_results` 中，当前 step 返回 `StepResult::success` 或 `StepResult::partial` 后才写入。

18. P-E-RP 的 replanner 只修改当前步骤之后的剩余计划，已经完成的步骤不会被覆盖。

19. P-E-RP 当前同样依赖模型严格输出约定格式：Planner/Replanner 输出 JSON，Executor 输出 `Thought` + `Action` JSON。
