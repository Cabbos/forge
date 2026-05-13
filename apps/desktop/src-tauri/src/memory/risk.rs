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
        "token",
        "password",
        "passwd",
        "secret",
        "private key",
        "ssh-rsa",
        "-----begin",
        "credit card",
        "身份证",
        "客户名单",
        "客户资料",
        "商业机密",
    ];

    if sensitive_words.iter().any(|word| lower.contains(word)) {
        return true;
    }

    let patterns = [
        r"sk-[A-Za-z0-9_\-]{16,}",
        r"ghp_[A-Za-z0-9_]{16,}",
        r"AIza[0-9A-Za-z_\-]{20,}",
        r"AKIA[0-9A-Z]{16}",
        r"-----BEGIN [A-Z ]+PRIVATE KEY-----",
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
    fn allows_low_risk_preferences() {
        assert!(!should_reject_persistent_memory("以后都用中文和我交流"));
        assert!(!should_reject_persistent_memory(
            "这个项目方向是小白优先，开发者也舒服"
        ));
    }
}
