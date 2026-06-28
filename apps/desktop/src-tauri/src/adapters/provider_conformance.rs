use crate::agent::event_sink::EventEmitter;
use crate::protocol::events::{ProviderUsageReason, StreamEvent};

use super::provider_registry::get_provider_definition;
use super::{adapter_family, anthropic, resolve_adapter_route, AdapterFamily};

#[derive(Debug, Clone, Copy)]
struct ProviderConformance {
    provider: &'static str,
    streaming_fixture: &'static str,
    expects_tools: bool,
    expects_usage: bool,
    expects_reasoning: bool,
    expected_family: AdapterFamily,
    api_base: Option<&'static str>,
}

#[derive(Default)]
struct CaptureEmitter {
    events: parking_lot::Mutex<Vec<StreamEvent>>,
}

impl CaptureEmitter {
    fn events(&self) -> Vec<StreamEvent> {
        self.events.lock().clone()
    }
}

impl EventEmitter for CaptureEmitter {
    fn emit(&self, event: StreamEvent) {
        self.events.lock().push(event);
    }
}

#[test]
fn provider_conformance_table_matches_registry_capabilities_and_routes() {
    for case in provider_conformance_cases() {
        let definition = get_provider_definition(case.provider)
            .unwrap_or_else(|| panic!("missing provider definition: {}", case.provider));
        let route = resolve_adapter_route(case.provider, case.api_base)
            .unwrap_or_else(|error| panic!("missing route for {}: {error}", case.provider));

        assert_eq!(
            route.family, case.expected_family,
            "adapter family for {}",
            case.provider
        );
        assert_eq!(
            adapter_family(definition.transport),
            Some(case.expected_family),
            "transport family for {}",
            case.provider
        );
        assert_eq!(
            definition.supports_tools, case.expects_tools,
            "tool support for {}",
            case.provider
        );
        assert_eq!(
            definition.supports_usage, case.expects_usage,
            "usage support for {}",
            case.provider
        );
        assert_eq!(
            definition.supports_thinking, case.expects_reasoning,
            "reasoning/thinking support for {}",
            case.provider
        );
        assert!(
            matches!(
                case.streaming_fixture,
                "anthropic_messages_sse" | "openai_chat_completions_sse"
            ),
            "known fixture family for {}",
            case.provider
        );
    }
}

#[test]
fn provider_conformance_unknown_usage_is_explicit_null_for_all_fixture_families() {
    for source in ["anthropic", "openai_compatible"] {
        let emitter = CaptureEmitter::default();

        anthropic::emit_usage_events(&emitter, "session-1", source, "unknown-model", None);

        assert!(emitter.events().iter().any(|event| matches!(
            event,
            StreamEvent::ProviderUsage {
                session_id,
                model: Some(model),
                input_tokens: None,
                output_tokens: None,
                estimated_cost_micros: None,
                source: Some(actual_source),
                reason: ProviderUsageReason::ProviderOmitted,
                ..
            } if session_id == "session-1"
                && model == "unknown-model"
                && actual_source == source
        )));
        let provider_usage_json = emitter
            .events()
            .into_iter()
            .find(|event| matches!(event, StreamEvent::ProviderUsage { .. }))
            .map(|event| serde_json::to_value(event).expect("serialize provider usage event"))
            .expect("provider usage event");

        assert_eq!(provider_usage_json["event_type"], "provider_usage");
        assert_eq!(provider_usage_json["provider_id"], source);
        assert_eq!(provider_usage_json["source"], source);
        assert_eq!(provider_usage_json["reason"], "provider_omitted");
        assert!(
            provider_usage_json
                .as_object()
                .expect("provider usage object")
                .contains_key("input_tokens"),
            "{source} input_tokens must be serialized as explicit null"
        );
        assert!(
            provider_usage_json
                .as_object()
                .expect("provider usage object")
                .contains_key("output_tokens"),
            "{source} output_tokens must be serialized as explicit null"
        );
        assert!(
            provider_usage_json
                .as_object()
                .expect("provider usage object")
                .contains_key("estimated_cost_micros"),
            "{source} estimated_cost_micros must be serialized as explicit null"
        );
        for key in [
            "cache_read_tokens",
            "cache_creation_tokens",
            "reasoning_tokens",
            "pricing_source",
        ] {
            assert!(
                provider_usage_json
                    .as_object()
                    .expect("provider usage object")
                    .contains_key(key),
                "{source} {key} must be serialized as explicit null"
            );
            assert_eq!(provider_usage_json[key], serde_json::Value::Null);
        }
        assert_eq!(provider_usage_json["input_tokens"], serde_json::Value::Null);
        assert_eq!(
            provider_usage_json["output_tokens"],
            serde_json::Value::Null
        );
        assert_eq!(
            provider_usage_json["estimated_cost_micros"],
            serde_json::Value::Null
        );
        assert!(
            !emitter
                .events()
                .iter()
                .any(|event| matches!(event, StreamEvent::Usage { .. })),
            "{source} omitted usage must not emit billing-grade usage facts"
        );
    }
}

fn provider_conformance_cases() -> &'static [ProviderConformance] {
    &[
        ProviderConformance {
            provider: "deepseek",
            streaming_fixture: "anthropic_messages_sse",
            expects_tools: true,
            expects_usage: true,
            expects_reasoning: true,
            expected_family: AdapterFamily::AnthropicCompatible,
            api_base: None,
        },
        ProviderConformance {
            provider: "anthropic",
            streaming_fixture: "anthropic_messages_sse",
            expects_tools: true,
            expects_usage: true,
            expects_reasoning: true,
            expected_family: AdapterFamily::AnthropicCompatible,
            api_base: None,
        },
        ProviderConformance {
            provider: "kimi",
            streaming_fixture: "anthropic_messages_sse",
            expects_tools: true,
            expects_usage: true,
            expects_reasoning: true,
            expected_family: AdapterFamily::AnthropicCompatible,
            api_base: None,
        },
        ProviderConformance {
            provider: "glm",
            streaming_fixture: "anthropic_messages_sse",
            expects_tools: true,
            expects_usage: true,
            expects_reasoning: true,
            expected_family: AdapterFamily::AnthropicCompatible,
            api_base: None,
        },
        ProviderConformance {
            provider: "minimax",
            streaming_fixture: "anthropic_messages_sse",
            expects_tools: true,
            expects_usage: true,
            expects_reasoning: true,
            expected_family: AdapterFamily::AnthropicCompatible,
            api_base: None,
        },
        ProviderConformance {
            provider: "ollama",
            streaming_fixture: "anthropic_messages_sse",
            expects_tools: true,
            expects_usage: false,
            expects_reasoning: false,
            expected_family: AdapterFamily::AnthropicCompatible,
            api_base: None,
        },
        ProviderConformance {
            provider: "custom_anthropic",
            streaming_fixture: "anthropic_messages_sse",
            expects_tools: true,
            expects_usage: false,
            expects_reasoning: false,
            expected_family: AdapterFamily::AnthropicCompatible,
            api_base: Some("http://127.0.0.1:9000/anthropic"),
        },
        ProviderConformance {
            provider: "openai",
            streaming_fixture: "openai_chat_completions_sse",
            expects_tools: true,
            expects_usage: true,
            expects_reasoning: false,
            expected_family: AdapterFamily::OpenAiCompatible,
            api_base: None,
        },
        ProviderConformance {
            provider: "openrouter",
            streaming_fixture: "openai_chat_completions_sse",
            expects_tools: true,
            expects_usage: true,
            expects_reasoning: true,
            expected_family: AdapterFamily::OpenAiCompatible,
            api_base: None,
        },
        ProviderConformance {
            provider: "alibaba",
            streaming_fixture: "openai_chat_completions_sse",
            expects_tools: true,
            expects_usage: true,
            expects_reasoning: true,
            expected_family: AdapterFamily::OpenAiCompatible,
            api_base: None,
        },
        ProviderConformance {
            provider: "gemini",
            streaming_fixture: "openai_chat_completions_sse",
            expects_tools: true,
            expects_usage: true,
            expects_reasoning: true,
            expected_family: AdapterFamily::OpenAiCompatible,
            api_base: None,
        },
        ProviderConformance {
            provider: "xai",
            streaming_fixture: "openai_chat_completions_sse",
            expects_tools: true,
            expects_usage: true,
            expects_reasoning: true,
            expected_family: AdapterFamily::OpenAiCompatible,
            api_base: None,
        },
        ProviderConformance {
            provider: "groq",
            streaming_fixture: "openai_chat_completions_sse",
            expects_tools: true,
            expects_usage: true,
            expects_reasoning: false,
            expected_family: AdapterFamily::OpenAiCompatible,
            api_base: None,
        },
        ProviderConformance {
            provider: "mistral",
            streaming_fixture: "openai_chat_completions_sse",
            expects_tools: true,
            expects_usage: true,
            expects_reasoning: false,
            expected_family: AdapterFamily::OpenAiCompatible,
            api_base: None,
        },
        ProviderConformance {
            provider: "custom_openai",
            streaming_fixture: "openai_chat_completions_sse",
            expects_tools: true,
            expects_usage: false,
            expects_reasoning: false,
            expected_family: AdapterFamily::OpenAiCompatible,
            api_base: Some("http://127.0.0.1:9000/v1"),
        },
    ]
}
