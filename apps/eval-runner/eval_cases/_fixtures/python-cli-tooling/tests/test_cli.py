import json

from forge_tools.cli import build_parser, render_greeting


def test_parser_defaults_to_text_format() -> None:
    args = build_parser().parse_args([])

    assert args.format == "text"


def test_render_greeting_uses_world_when_name_missing() -> None:
    assert render_greeting(None, output_format="text") == "hello world"


def test_render_greeting_json_is_parseable() -> None:
    payload = json.loads(render_greeting("Ada", output_format="json"))

    assert payload == {"greeting": "hello", "name": "Ada"}
