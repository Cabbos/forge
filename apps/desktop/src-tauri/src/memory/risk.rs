use regex::Regex;

pub fn should_reject_persistent_memory(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return true;
    }

    let lower = trimmed.to_lowercase();
    let sensitive_words = [
        "api key",
        "apikey",
        "password",
        "passwd",
        "secret",
        "private key",
        "ssh-rsa",
        "-----begin",
        "credit card",
        "身份证",
        "密码",
        "密钥",
        "令牌",
        "私钥",
        "访问令牌",
        "认证令牌",
        "客户名单",
        "客户资料",
        "商业机密",
    ];

    if sensitive_words.iter().any(|word| lower.contains(word)) {
        return true;
    }

    let patterns = [
        r"(?:^|[^A-Za-z0-9])sk-[A-Za-z0-9_\-]{16,}",
        r"ghp_[A-Za-z0-9_]{16,}",
        r"gho_[A-Za-z0-9_]{16,}",
        r"ghu_[A-Za-z0-9_]{16,}",
        r"ghs_[A-Za-z0-9_]{16,}",
        r"ghr_[A-Za-z0-9_]{16,}",
        r"github_pat_[A-Za-z0-9_]{20,}",
        r"AIza[0-9A-Za-z_\-]{20,}",
        r"AKIA[0-9A-Z]{16}",
        r"-----BEGIN [A-Z ]+PRIVATE KEY-----",
        r"(?i)\btoken\s*[:=]\s*[A-Za-z0-9._~+/=-]{8,}",
        r"(?i)\bmy\s+token\s+(?:is|=|:)\s*[A-Za-z0-9._~+/=-]{8,}",
        r"(?i)\b(?:auth|access)\s+token(?:\s*(?:is|=|:))?\s+[A-Za-z0-9._~+/=-]{8,}",
        r"(?i)\bbearer\s+[A-Za-z0-9._~+/=-]{8,}",
    ];

    patterns.iter().any(|pattern| {
        Regex::new(pattern)
            .map(|regex| regex.is_match(trimmed))
            .unwrap_or(false)
    })
}

#[cfg(test)]
mod tests {
    use super::should_reject_persistent_memory;

    #[test]
    fn rejects_api_keys_and_tokens() {
        assert!(should_reject_persistent_memory(
            "my API key is sk-1234567890abcdefghijkl"
        ));
        assert!(should_reject_persistent_memory(
            "token: ghp_1234567890abcdefghijkl"
        ));
        assert!(should_reject_persistent_memory(
            "-----BEGIN OPENSSH PRIVATE KEY-----"
        ));
    }

    #[test]
    fn rejects_raw_github_tokens_without_token_label() {
        assert!(should_reject_persistent_memory(
            "github_pat_1234567890abcdef_1234567890abcdef"
        ));
        assert!(should_reject_persistent_memory(
            "gho_1234567890abcdefghijkl"
        ));
        assert!(should_reject_persistent_memory(
            "ghu_1234567890abcdefghijkl"
        ));
        assert!(should_reject_persistent_memory(
            "ghs_1234567890abcdefghijkl"
        ));
        assert!(should_reject_persistent_memory(
            "ghr_1234567890abcdefghijkl"
        ));
    }

    #[test]
    fn rejects_secret_token_phrases() {
        assert!(should_reject_persistent_memory("token: abcdefghijklmnop"));
        assert!(should_reject_persistent_memory(
            "my token is abcdefghijklmnop"
        ));
        assert!(should_reject_persistent_memory(
            "auth token abcdefghijklmnop"
        ));
        assert!(should_reject_persistent_memory(
            "access token abcdefghijklmnop"
        ));
        assert!(should_reject_persistent_memory("bearer abcdefghijklmnop"));
    }

    #[test]
    fn rejects_chinese_password_phrase() {
        assert!(should_reject_persistent_memory(
            "以后默认数据库密码是 abcdefghijklmnop"
        ));
    }

    #[test]
    fn allows_non_secret_token_phrases() {
        assert!(!should_reject_persistent_memory(
            "Use design tokens for color and spacing"
        ));
        assert!(!should_reject_persistent_memory(
            "We need to lower the token budget"
        ));
        assert!(!should_reject_persistent_memory(
            "Document the tokenization strategy"
        ));
    }

    #[test]
    fn allows_low_risk_preferences() {
        assert!(!should_reject_persistent_memory("以后都用中文和我交流"));
        assert!(!should_reject_persistent_memory(
            "这个项目方向是小白优先，开发者也舒服"
        ));
    }

    #[test]
    fn allows_task_summary_paths_that_contain_sk_substring() {
        assert!(!should_reject_persistent_memory(
            "/tmp/forge-eval-continuity-pipeline-task-summary-0ccz9xhz/workspace/src/task-summary.ts"
        ));
        assert!(!should_reject_persistent_memory(
            "Evidence: file_changes=[/tmp/task-summary-abcdefghijklmnop/src/task-summary.ts]"
        ));
    }
}
