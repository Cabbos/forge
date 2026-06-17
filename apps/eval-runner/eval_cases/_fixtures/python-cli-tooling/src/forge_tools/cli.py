import argparse


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(prog="forge-tools")
    parser.add_argument("--name", default=None)
    parser.add_argument("--format", choices=["text", "json"], default="text")
    return parser


def render_greeting(name: str | None, *, output_format: str) -> str:
    display_name = name or ""
    if output_format == "json":
        return '{"greeting": "hello", "name": "%s"}' % display_name
    return f"hello {display_name}"
