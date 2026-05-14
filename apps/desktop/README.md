# Forge

A desktop AI coding agent built with Tauri 2.0 + React + Rust. Direct filesystem and shell access, skills system, parallel sub-agents.

## Quick Start

```bash
npm install
npm run tauri dev
```

Set your DeepSeek API key in Settings (`Cmd+,`) or `~/.forge/config.json`:

```json
{
  "providers": {
    "deepseek": {
      "api_key": "sk-..."
    }
  }
}
```

## Features

- **Multi-turn agent loop** — up to 10 tool-call rounds per message
- **Streaming UI** — thinking blocks, shell output line-by-line, tool call cards
- **Skills** — drop `SKILL.md` / `CLAUDE.md` into `~/.forge/skills/<name>/` and it's injected into the system prompt
- **Sub-agent dispatch** — `delegate_task` tool spawns parallel read-only sub-agents for independent research tasks
- **Permission gate** — dangerous commands (shell, file writes) require user confirmation
- **Session persistence** — IndexedDB, survives app restart (visual only, backend sessions are ephemeral)

## Architecture

```
React frontend (Vite + TypeScript)
    ↕ Tauri IPC events (StreamEvent)
Rust backend (Tokio async)
    ├── AgentSession — main agent loop
    ├── SubAgent — parallel sub-task dispatch
    ├── AnthropicAdapter — DeepSeek Anthropic-compatible API
    ├── ToolExecutor — file, shell, search, web tools
    └── Harness — hooks, permissions, skill loader
```

## Tools

| Tool | Description |
|------|-------------|
| `read_file` | Read file contents |
| `write_to_file` | Create or overwrite a file |
| `edit_file` | Targeted string replacement in file |
| `list_directory` | List directory contents |
| `search_files` | Glob-based file search |
| `search_content` | Regex-based content search |
| `run_shell` | Execute shell commands |
| `web_search` | Search the web |
| `web_fetch` | Fetch and extract web content |
| `delegate_task` | Dispatch parallel read-only sub-agent |

## Project Context

Place a `CLAUDE.md`, `AGENTS.md`, or `GEMINI.md` in your working directory — it's automatically injected into the AI's system prompt as project context.

## Config

```json
// ~/.forge/config.json
{
  "providers": {
    "deepseek": {
      "api_key": "sk-..."
    }
  },
  "model": "deepseek-v4-pro[1m]"
}
```

## Build

```bash
npm run build        # TypeScript check + Vite production build
npm run tauri build  # Full Tauri desktop app bundle
```
