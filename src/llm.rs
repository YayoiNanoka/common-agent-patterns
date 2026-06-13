use reqwest::blocking::Client;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};
use std::fmt;

use crate::config::{AGENT_TEMPERATURE, OPENAI_API_KEY, OPENAI_BASE_URL, OPENAI_MODEL};

#[derive(Debug, Clone)]
pub struct LlmClient {
    client: Client,
    api_key: String,
    base_url: String,
    model: String,
    temperature: f32,
}

#[derive(Debug)]
pub enum LlmError {
    ConnectFailed { reason: String },
    ChatFailed { reason: String },
}

impl fmt::Display for LlmError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConnectFailed { reason } => write!(formatter, "LLM connect failed: {reason}"),
            Self::ChatFailed { reason } => write!(formatter, "LLM chat failed: {reason}"),
        }
    }
}

impl std::error::Error for LlmError {}

impl LlmClient {
    /// 初始化 LLM 客户端。
    ///
    /// 输入：从 `config.rs` 读取 OpenAI 相关配置；当 `OPENAI_API_KEY`
    /// 为空时，会尝试从环境变量 `OPENAI_API_KEY` 读取。
    ///
    /// 输出：返回一个可直接使用的 `LlmClient`；如果必要配置缺失或格式错误，
    /// 返回 `LlmError::ConnectFailed`。
    pub fn connect() -> Result<Self, LlmError> {
        let api_key = if OPENAI_API_KEY.is_empty() {
            std::env::var("OPENAI_API_KEY").map_err(|_| LlmError::ConnectFailed {
                reason: "OPENAI_API_KEY is not configured".to_string(),
            })?
        } else {
            OPENAI_API_KEY.to_string()
        };

        let temperature =
            AGENT_TEMPERATURE
                .parse::<f32>()
                .map_err(|error| LlmError::ConnectFailed {
                    reason: format!("invalid AGENT_TEMPERATURE: {error}"),
                })?;

        let client = Self {
            client: Client::new(),
            api_key,
            base_url: OPENAI_BASE_URL.trim_end_matches('/').to_string(),
            model: OPENAI_MODEL.to_string(),
            temperature,
        };

        println!("🔌 LLM 连接建立已完成。");

        Ok(client)
    }

    /// 将 prompt 发送给 OpenAI Chat Completions API，并返回模型生成的文本。
    ///
    /// 输入：`prompt` 是发送给模型的完整文本指令或问题，会作为 user message
    /// 传给 OpenAI。
    ///
    /// 输出：返回 assistant 生成的文本；如果 HTTP 请求失败、API 返回错误，
    /// 或响应中没有可用文本，则返回 `LlmError::ChatFailed`。
    pub fn chat(&self, prompt: &str) -> Result<String, LlmError> {
        let url = format!("{}/chat/completions", self.base_url);
        let request = ChatCompletionRequest {
            model: &self.model,
            messages: vec![ChatCompletionMessage {
                role: "user",
                content: prompt,
            }],
            temperature: self.temperature,
        };

        println!("🧠 正在调用 {} 模型...", self.model);

        let response = self
            .client
            .post(url)
            .headers(self.headers()?)
            .json(&request)
            .send()
            .map_err(|error| LlmError::ChatFailed {
                reason: format!("failed to send OpenAI request: {error}"),
            })?;

        let status = response.status();
        let body = response.text().map_err(|error| LlmError::ChatFailed {
            reason: format!("failed to read OpenAI response: {error}"),
        })?;

        if !status.is_success() {
            return Err(LlmError::ChatFailed {
                reason: format!("OpenAI API returned {status}: {body}"),
            });
        }

        let response: ChatCompletionResponse =
            serde_json::from_str(&body).map_err(|error| LlmError::ChatFailed {
                reason: format!("failed to parse OpenAI response: {error}; body: {body}"),
            })?;

        response
            .choices
            .into_iter()
            .next()
            .and_then(|choice| choice.message.content)
            .filter(|content| !content.trim().is_empty())
            .ok_or_else(|| LlmError::ChatFailed {
                reason: "OpenAI response did not contain assistant content".to_string(),
            })
    }

    /// 构造调用 OpenAI API 所需的 HTTP headers。
    ///
    /// 输入：使用当前 `LlmClient` 中保存的 API key。
    ///
    /// 输出：返回包含 `Authorization` 和 `Content-Type` 的 headers；
    /// 如果授权 header 无法创建，则返回 `LlmError::ChatFailed`。
    fn headers(&self) -> Result<HeaderMap, LlmError> {
        let mut headers = HeaderMap::new();
        let auth_value =
            HeaderValue::from_str(&format!("Bearer {}", self.api_key)).map_err(|error| {
                LlmError::ChatFailed {
                    reason: format!("invalid OpenAI API key header: {error}"),
                }
            })?;

        headers.insert(AUTHORIZATION, auth_value);
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        Ok(headers)
    }
}

#[derive(Debug, Serialize)]
struct ChatCompletionRequest<'a> {
    model: &'a str,
    messages: Vec<ChatCompletionMessage<'a>>,
    temperature: f32,
}

#[derive(Debug, Serialize)]
struct ChatCompletionMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<ChatCompletionChoice>,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionChoice {
    message: ChatCompletionResponseMessage,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionResponseMessage {
    content: Option<String>,
}
