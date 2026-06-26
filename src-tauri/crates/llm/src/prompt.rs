/// Wrap a system instruction + user message in Qwen2.5 ChatML.
///
/// Exact format required by Qwen2.5-Instruct (arch `qwen2`, ChatML template):
/// ```text
/// <|im_start|>system
/// {system}<|im_end|>
/// <|im_start|>user
/// {user}<|im_end|>
/// <|im_start|>assistant
/// ```
/// The trailing newline after `<|im_start|>assistant` lets the model begin its
/// response without an extra blank line.
pub fn chatml(system: &str, user: &str) -> String {
    format!(
        "<|im_start|>system\n{system}<|im_end|>\n\
         <|im_start|>user\n{user}<|im_end|>\n\
         <|im_start|>assistant\n"
    )
}
