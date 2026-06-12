#[cfg(test)]
mod tests {
    use crate::adapters::base::ToolCall;
    use crate::agent::session::tools::{
        canonical_json, tool_batch_signature, tool_category_signature,
    };

    fn tc(name: &str, input: serde_json::Value) -> ToolCall {
        ToolCall {
            id: format!("id-{name}"),
            name: name.to_string(),
            input,
        }
    }

    #[test]
    fn canonical_json_sorts_object_keys() {
        let value = serde_json::json!({"b": 2, "a": 1});
        assert_eq!(canonical_json(&value), "{\"a\":1,\"b\":2}");
    }

    #[test]
    fn canonical_json_sorts_nested_keys() {
        let value = serde_json::json!({"z": {"b": 2, "a": 1}});
        assert_eq!(canonical_json(&value), "{\"z\":{\"a\":1,\"b\":2}}");
    }

    #[test]
    fn canonical_json_handles_arrays_and_scalars() {
        let value = serde_json::json!([{"c": 3}, {"a": 1, "b": 2}, true]);
        assert_eq!(canonical_json(&value), "[{\"c\":3},{\"a\":1,\"b\":2},true]");
    }

    #[test]
    fn tool_batch_signature_is_deterministic() {
        let calls = vec![
            tc("read_file", serde_json::json!({"path": "b.rs"})),
            tc("read_file", serde_json::json!({"path": "a.rs"})),
        ];
        let sig = tool_batch_signature(&calls);
        assert!(sig.contains("read_file"));
        assert!(sig.starts_with("read_file:"));
        // Order should be sorted by the canonical JSON representation.
        let lines: Vec<&str> = sig.lines().collect();
        assert!(lines[0] < lines[1]);
    }

    #[test]
    fn tool_category_signature_deduplicates_names() {
        let calls = vec![
            tc("read_file", serde_json::json!({"path": "a.rs"})),
            tc("read_file", serde_json::json!({"path": "b.rs"})),
            tc("run_shell", serde_json::json!({"command": "echo hi"})),
        ];
        assert_eq!(tool_category_signature(&calls), "read_file,run_shell");
    }

    #[test]
    fn tool_category_signature_empty_for_no_calls() {
        assert_eq!(tool_category_signature(&[]), "");
    }
}
