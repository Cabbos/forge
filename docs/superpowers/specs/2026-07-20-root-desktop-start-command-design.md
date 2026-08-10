# Root Desktop Start Command Design

## Goal

Make the full Forge desktop application directly startable from the monorepo root while preserving the existing frontend-only development workflow.

## Command Contract

- `npm run dev:desktop` starts the complete Tauri desktop application, including the Vite frontend and Rust backend.
- `npm run dev:desktop:web` starts only the desktop Vite frontend on port 1420.
- The scripts delegate to `apps/desktop`; no wrapper script or shared package is introduced.

## Compatibility

The current `dev:desktop` behavior is retained under `dev:desktop:web`. Any developer or automation that intentionally needs only the browser frontend can migrate to the explicit command. Repository documentation should describe the root command as the shortest full-app startup path.

## Verification

Verify the script definitions through a focused Node contract test, then launch `npm run dev:desktop` from the repository root and confirm that both Vite and the Tauri Rust process start. Stop the development process after the startup evidence is captured.
