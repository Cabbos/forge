use crate::adapters::base::AdapterError;

pub(crate) const MAX_MODEL_RETRY_ATTEMPTS: usize = 2;

pub(crate) fn should_retry_adapter_error(error: &AdapterError, retries_used: usize) -> bool {
    retries_used < MAX_MODEL_RETRY_ATTEMPTS && adapter_error_is_retryable(error)
}

fn adapter_error_is_retryable(error: &AdapterError) -> bool {
    match error {
        AdapterError::Api { code, message } => {
            http_status_code_is_retryable(parse_status_code(code))
                || is_rate_limit_code(code)
                || transient_error_message(message)
        }
        AdapterError::Http(message) => {
            http_status_code_is_retryable(parse_http_status_from_message(message))
                || transient_error_message(message)
        }
        AdapterError::Stream(message) => transient_error_message(message),
        AdapterError::MissingApiKey => false,
    }
}

fn parse_status_code(value: &str) -> Option<u16> {
    value.trim().parse::<u16>().ok()
}

fn parse_http_status_from_message(message: &str) -> Option<u16> {
    let message = message.trim();
    let rest = message.strip_prefix("HTTP ")?;
    let digits = rest
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>();
    parse_status_code(&digits)
}

fn http_status_code_is_retryable(status: Option<u16>) -> bool {
    matches!(status, Some(408 | 409 | 425 | 429 | 500 | 502 | 503 | 504))
}

fn is_rate_limit_code(code: &str) -> bool {
    let normalized = code.trim().to_ascii_lowercase();
    matches!(
        normalized.as_str(),
        "rate_limit" | "rate_limited" | "too_many_requests"
    )
}

fn transient_error_message(message: &str) -> bool {
    let normalized = message.to_ascii_lowercase();
    [
        "timed out",
        "timeout",
        "connection reset",
        "temporarily unavailable",
    ]
    .iter()
    .any(|needle| normalized.contains(needle))
}

#[cfg(test)]
mod tests {
    use super::{should_retry_adapter_error, MAX_MODEL_RETRY_ATTEMPTS};
    use crate::adapters::base::AdapterError;

    #[test]
    fn retries_structured_api_transient_status_codes() {
        for code in ["429", "500", "502", "503", "504"] {
            assert!(should_retry_adapter_error(
                &AdapterError::Api {
                    code: code.to_string(),
                    message: "upstream failed".to_string(),
                },
                0,
            ));
        }
    }

    #[test]
    fn does_not_retry_non_transient_api_status_or_missing_key() {
        assert!(!should_retry_adapter_error(
            &AdapterError::Api {
                code: "400".to_string(),
                message: "bad request".to_string(),
            },
            0,
        ));
        assert!(!should_retry_adapter_error(&AdapterError::MissingApiKey, 0));
    }

    #[test]
    fn retries_http_messages_only_when_status_prefix_is_transient() {
        assert!(should_retry_adapter_error(
            &AdapterError::Http("HTTP 503: unavailable".to_string()),
            0,
        ));
        assert!(!should_retry_adapter_error(
            &AdapterError::Http("provider returned 500 candidate tokens".to_string()),
            0,
        ));
    }

    #[test]
    fn retries_timeout_and_connection_stream_errors() {
        assert!(should_retry_adapter_error(
            &AdapterError::Stream("request timed out".to_string()),
            0,
        ));
        assert!(should_retry_adapter_error(
            &AdapterError::Stream("connection reset by peer".to_string()),
            0,
        ));
    }

    #[test]
    fn retry_budget_stops_after_max_attempts() {
        assert!(!should_retry_adapter_error(
            &AdapterError::Http("HTTP 503: unavailable".to_string()),
            MAX_MODEL_RETRY_ATTEMPTS,
        ));
    }
}
