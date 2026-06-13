use crate::llm::LlmClient;

/// 通用 Agent 总结器使用的 prompt 模板。
///
/// 这个模板不绑定具体 agent pattern，用于根据已有执行信息生成阶段性或最终回答。
pub const SUMMARY_PROMPT_TEMPLATE: &str = r#"
你是一个 Agent 执行结果总结器。

你的任务是根据当前 Agent 的执行状态、已完成工作和未完成部分，生成一个面向用户的中文回答。

要求：
1. 如果任务未完成，必须明确说明任务未完全完成。
2. 只能基于已完成工作和已有信息回答，不要编造未完成部分。
3. 总结已经完成了什么。
4. 说明仍然缺少什么或后续应该继续做什么。
5. 如果已有信息足够，请给出尽可能有用的阶段性结论。

原始任务：
{task}

当前状态：
{status}

需要总结的原因：
{reason}

已完成工作：
{completed_work}

尚未完成或缺失的信息：
{remaining_work}

请输出中文回答。
"#;

/// 总结器的结构化输入。
///
/// 不同 agent pattern 可以把自己的内部状态转换成这个结构，从而复用同一个总结器。
#[derive(Debug, Clone)]
pub struct SummaryInput {
    /// 原始用户任务。
    pub task: String,
    /// 当前任务状态，例如“任务未完成”。
    pub status: String,
    /// 已经完成的工作、执行历史或观察结果。
    pub completed_work: String,
    /// 尚未完成的步骤、缺失信息或未知部分。
    pub remaining_work: String,
    /// 触发总结的原因，例如“达到最大执行步骤数”。
    pub reason: String,
}

/// 可复用的 Agent 总结子 agent。
///
/// 它不实现全局 `Agent` trait，因为它需要的是 `SummaryInput`，不是普通 `AgentInput`。
pub struct AgentSummarizer {
    llm: LlmClient,
}

impl AgentSummarizer {
    /// 创建一个通用总结器。
    pub fn new(llm: LlmClient) -> Self {
        Self { llm }
    }

    /// 根据结构化总结输入生成中文回答。
    pub fn summarize(&self, input: SummaryInput) -> anyhow::Result<String> {
        let prompt = self.build_prompt(&input);
        self.llm.chat(&prompt).map_err(Into::into)
    }

    /// 拼装总结器 prompt。
    fn build_prompt(&self, input: &SummaryInput) -> String {
        SUMMARY_PROMPT_TEMPLATE
            .replace("{task}", &input.task)
            .replace("{status}", &input.status)
            .replace("{reason}", &input.reason)
            .replace("{completed_work}", &input.completed_work)
            .replace("{remaining_work}", &input.remaining_work)
    }
}
