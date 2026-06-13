use reqwest::blocking::Client;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::config::SERPAPI_API_KEY;
use crate::tools::base::{Tool, ToolResult};

const SERPAPI_SEARCH_URL: &str = "https://serpapi.com/search";
const DEFAULT_RESULT_LIMIT: usize = 5;
const MAX_RESULT_LIMIT: usize = 10;

/// 内置搜索工具。
///
/// 当前实现基于 SerpApi 的 Google Search API，会把搜索结果整理成适合 Agent 阅读的文本。
pub struct SearchTool;

impl Tool for SearchTool {
    /// 返回搜索工具的固定名称。
    fn name(&self) -> &'static str {
        "search"
    }

    /// 返回搜索工具的用途说明。
    fn description(&self) -> &'static str {
        "Search Google through SerpApi and return concise organic search results for a query."
    }

    /// 返回搜索工具需要的参数 schema。
    ///
    /// `query` 是必填参数；`num_results` 和 `location` 是可选参数。
    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The search query."
                },
                "num_results": {
                    "type": "integer",
                    "description": "How many organic results to return. Default is 5, max is 10.",
                    "minimum": 1,
                    "maximum": 10
                },
                "location": {
                    "type": "string",
                    "description": "Optional Google search location, for example: Austin, Texas, United States."
                }
            },
            "required": ["query"]
        })
    }

    /// 执行搜索工具。
    ///
    /// 该函数会调用 SerpApi，读取 `organic_results`，并把标题、链接和摘要格式化成 Observation。
    fn execute(&self, args: Value) -> anyhow::Result<ToolResult> {
        let query = parse_query(&args)?;
        let limit = parse_result_limit(&args);
        let location = args.get("location").and_then(Value::as_str);
        let api_key = load_serpapi_api_key()?;

        let client = Client::new();
        let num = limit.to_string();
        let mut request = client.get(SERPAPI_SEARCH_URL).query(&[
            ("engine", "google"),
            ("q", query),
            ("api_key", api_key.as_str()),
            ("num", num.as_str()),
        ]);

        if let Some(location) = location {
            request = request.query(&[("location", location)]);
        }

        let response = request
            .send()
            .map_err(|error| anyhow::anyhow!("failed to send SerpApi request: {error}"))?;

        let status = response.status();
        let body = response
            .text()
            .map_err(|error| anyhow::anyhow!("failed to read SerpApi response: {error}"))?;

        if !status.is_success() {
            anyhow::bail!("SerpApi returned {status}: {body}");
        }

        let response: SerpApiResponse = serde_json::from_str(&body)
            .map_err(|error| anyhow::anyhow!("failed to parse SerpApi response: {error}"))?;

        if let Some(error) = response.error {
            anyhow::bail!("SerpApi returned error: {error}");
        }

        Ok(ToolResult {
            content: format_search_results(query, &response.organic_results, limit),
        })
    }
}

/// 从工具参数中读取搜索关键词。
fn parse_query(args: &Value) -> anyhow::Result<&str> {
    let query = args
        .get("query")
        .and_then(Value::as_str)
        .map(str::trim)
        .ok_or_else(|| anyhow::anyhow!("search tool requires string argument `query`"))?;

    if query.is_empty() {
        anyhow::bail!("search tool argument `query` cannot be empty");
    }

    Ok(query)
}

/// 从工具参数中读取希望返回的结果数量。
///
/// 如果 LLM 没有提供该参数，则使用默认值；如果超过上限，会自动截断到 `MAX_RESULT_LIMIT`。
fn parse_result_limit(args: &Value) -> usize {
    args.get("num_results")
        .and_then(Value::as_u64)
        .map(|limit| limit as usize)
        .unwrap_or(DEFAULT_RESULT_LIMIT)
        .clamp(1, MAX_RESULT_LIMIT)
}

/// 读取 SerpApi API key。
///
/// 优先使用 `config.rs` 中的 `SERPAPI_API_KEY`，如果为空，则尝试读取环境变量。
fn load_serpapi_api_key() -> anyhow::Result<String> {
    if !SERPAPI_API_KEY.trim().is_empty() {
        return Ok(SERPAPI_API_KEY.to_string());
    }

    std::env::var("SERPAPI_API_KEY")
        .map_err(|_| anyhow::anyhow!("SERPAPI_API_KEY is not configured"))
}

/// 将 SerpApi 的自然搜索结果格式化为 Agent 可读的 Observation。
fn format_search_results(query: &str, results: &[SerpApiOrganicResult], limit: usize) -> String {
    if results.is_empty() {
        return format!("No organic search results found for query: {query}");
    }

    let mut output = format!("Search results for query: {query}");

    for (index, result) in results.iter().take(limit).enumerate() {
        output.push_str(&format!(
            "\n\n{}. {}\n   Link: {}\n   Snippet: {}",
            index + 1,
            result.title.as_deref().unwrap_or("(no title)"),
            result.link.as_deref().unwrap_or("(no link)"),
            result.snippet.as_deref().unwrap_or("(no snippet)")
        ));
    }

    output
}

/// SerpApi 搜索接口的顶层响应结构。
#[derive(Debug, Deserialize)]
struct SerpApiResponse {
    /// SerpApi 返回的自然搜索结果列表。
    #[serde(default)]
    organic_results: Vec<SerpApiOrganicResult>,
    /// SerpApi 在业务失败时可能返回的错误信息。
    error: Option<String>,
}

/// SerpApi `organic_results` 中单条搜索结果的字段。
#[derive(Debug, Deserialize)]
struct SerpApiOrganicResult {
    /// 搜索结果标题。
    title: Option<String>,
    /// 搜索结果链接。
    link: Option<String>,
    /// 搜索结果摘要。
    snippet: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_result_limit_with_default_and_clamp() {
        assert_eq!(parse_result_limit(&json!({})), DEFAULT_RESULT_LIMIT);
        assert_eq!(parse_result_limit(&json!({"num_results": 2})), 2);
        assert_eq!(
            parse_result_limit(&json!({"num_results": 100})),
            MAX_RESULT_LIMIT
        );
    }

    #[test]
    fn formats_search_results() {
        let results = vec![SerpApiOrganicResult {
            title: Some("Rust".to_string()),
            link: Some("https://www.rust-lang.org/".to_string()),
            snippet: Some("A language empowering everyone.".to_string()),
        }];

        let output = format_search_results("Rust", &results, 5);

        assert!(output.contains("Search results for query: Rust"));
        assert!(output.contains("https://www.rust-lang.org/"));
    }
}
