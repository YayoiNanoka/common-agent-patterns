const BLOCK_LINE: &str = "────────────────────────────────────────────────────────────";

/// 打印一段可能较长的控制台内容。
///
/// 统一使用标题和分割线包裹正文，让 LLM 输出、工具 Observation、最终答案等内容更容易阅读。
pub fn print_block(title: &str, content: impl AsRef<str>) {
    println!("{BLOCK_LINE}");
    println!("{title}");
    println!("{BLOCK_LINE}");
    println!("{}", content.as_ref());
    println!("{BLOCK_LINE}");
}
