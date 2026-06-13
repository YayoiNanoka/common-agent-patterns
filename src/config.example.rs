/// OpenAI-compatible API key.
///
/// Leave this empty to read from the `OPENAI_API_KEY` environment variable.
pub const OPENAI_API_KEY: &str = "";

/// OpenAI-compatible API base URL.
///
/// The default value below points to DeepSeek's OpenAI-compatible endpoint.
pub const OPENAI_BASE_URL: &str = "https://api.deepseek.com/v1";

/// Model name used by the LLM client.
pub const OPENAI_MODEL: &str = "deepseek-v4-flash";

/// Sampling temperature used by the Agent.
pub const AGENT_TEMPERATURE: &str = "0.7";

/// SerpApi API key.
///
/// Leave this empty to read from the `SERPAPI_API_KEY` environment variable.
pub const SERPAPI_API_KEY: &str = "";
