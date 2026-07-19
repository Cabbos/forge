# Forge v5 「Console Craft」 Redesign Spec

Status: implementation brief for the v5 visual redesign of `apps/desktop`.
Supersedes the visual language of `v4-quiet-native` ("Warm Precision"). Structure, IA, protocol, and product terms are unchanged.

Design version string: `v5-console-craft` (replaces `data-design-version="v4-quiet-native"` in `AppShell.tsx`).

## Direction

Mainstream professional developer-tool aesthetic — a cool mission-control console: deep graphite surfaces, hairline borders as the primary spatial division, a single cyan live-light accent, dual-font discipline (mono chrome / sans reading), and a 3px left-border severity system for status and risk. Dense, quiet, keyboard-confident.

## Hard Guardrails (unchanged from v4)

- No changes to `StreamEvent`, IPC, protocol schema, or model response schema.
- No new visible modules, panels, entries, product terms, or abilities.
- No TSX structural changes. The only allowed TSX edits are: `data-design-version` bump in `AppShell.tsx`, and the font import in `main.tsx`.
- All visual change lands in `src/styles/*.css`, the style contract gate, e2e expectations, and docs.
- No nested cards, no decorative gradients/orbs/glow, no purple AI clichés. Radius stays ≤ 8px (6px for small controls).

## Font Stacks

- Add to `tokens.css`:
  - `--forge-font-sans: "Geist Variable", system-ui, -apple-system, "PingFang SC", sans-serif;`
  - `--forge-font-mono: "Geist Mono", "SF Mono", ui-monospace, "Fira Code", "PingFang SC", monospace;`
- Try `npm install @fontsource-variable/geist-mono` and import it in `main.tsx` next to the Geist import. If install fails (offline), keep the stack — SF Mono fallback on macOS is acceptable; do not block on this.
- Discipline: **mono for all UI chrome** (sidebar, titlebar, status bar, buttons, chips, tabs, menus, command palette, settings nav/labels, work-panel chrome, section labels, metadata, timestamps, kbd hints). **Sans only for reading**: assistant prose/markdown body and the composer textarea.
- Micro section labels: mono, 10–10.5px, uppercase, letter-spacing 0.08em, `--forge-text-faint`.
- Never use `font-style: italic` on CJK-capable text runs.

## Dark Theme Tokens (primary)

Replace values in `src/styles/tokens.css` (keep every token name; the contract gate requires `--forge-success-muted`, `--forge-amber-muted`, `--forge-amber-rgb`, `--forge-danger-rgb` to keep existing):

| Token | Value |
| --- | --- |
| `--forge-ink` | `#05080C` |
| `--forge-bg-depth` | `#070B10` |
| `--forge-bg-base` | `#0A0E14` |
| `--forge-bg-subtle` | `#0D1219` |
| `--forge-bg-surface` | `#10161F` |
| `--forge-bg-raised` | `#141B26` |
| `--forge-bg-raised-hover` | `#182130` |
| `--forge-bg-overlay` | `#182130` |
| `--forge-bg-composer` | `#10161F` |
| `--forge-border-subtle` | `#1C2635` |
| `--forge-border-muted` | `rgba(148, 163, 184, 0.16)` |
| `--forge-border-strong` | `#2A3648` |
| `--forge-text-primary` | `#E6E9EF` |
| `--forge-text-secondary` | `#B4BCC8` |
| `--forge-text-muted` | `#7C8694` |
| `--forge-text-faint` | `#7C8694` |
| `--forge-text-ghost` | `#556070` |
| `--forge-accent` | `#22D3EE` |
| `--forge-accent-rgb` | `34, 211, 238` |
| `--forge-accent-hover-strong` | `#4DE0F5` |
| `--forge-accent-foreground-strong` | `#052025` |
| `--forge-active` | `rgba(34, 211, 238, 0.14)` |
| `--forge-hover` | `rgba(148, 163, 184, 0.08)` |
| `--forge-focus-ring` | `rgba(34, 211, 238, 0.38)` |
| `--forge-pass` / `--forge-success-muted` | `#34D399` |
| `--forge-warning` | `#FBBF24` |
| `--forge-warning-strong` | `#F59E0B` |
| `--forge-warning-bright` | `#FCD34D` |
| `--forge-amber-muted` | `#D9A626` |
| `--forge-amber-rgb` | `251, 191, 36` |
| `--forge-danger` | `#F87171` |
| `--forge-danger-rgb` | `248, 113, 113` |
| `--forge-danger-muted` | `#F2888B` |
| `--forge-danger-soft` | `#FCA5A5` |
| `--forge-danger-hover` | `#EF6262` |
| `--forge-danger-bright` | `#FCA5A5` |

Icon role colors (`--forge-icon-*`): context `#7FA8C9`, action `#22D3EE`, reasoning `#8B93A8`, safety `#34D399`, automation `#F87171`, neutral `#7C8694`.

LED: running `#22D3EE`, done `#34D399`, error `#F87171`, idle `#556070`; glow shadows use matching rgb at 0.3 alpha.

Material/glass/composer/code tokens: derive from the same ladder (surfaces become the hex values above or their rgba forms at 0.94–0.99 alpha; glass tints keep the same alpha structure). Shadows lose the warm tint: `0 10px 28px rgba(0, 0, 0, 0.35)` family.

Work-panel dark tokens (`--forge-work-panel-*`): canvas `#0B0F15`, sheet `#111824`, row `#182130`, row-active `#1E2939`, context `#131A25`, border `rgba(148, 163, 184, 0.10)`, shadow unchanged shape.

Meter: filled `#34D399`, empty `#1C2635`. Film strip: bg `rgba(20, 27, 38, 0.94)`, perf `rgba(230, 233, 239, 0.08)`, header `rgba(7, 11, 16, 0.86)`. Object materials: paper/metal/receipt/ticket from surface/raised ladder with `#1C2635` borders.

## Light Theme Tokens (secondary, cool daylight)

Rewrite the light block in `globals.css` (selector structure unchanged — the gate asserts `.forge-app-shell[data-theme="light"],` and `body:has(.forge-app-shell[data-theme="light"])` scopes) and the light work-panel overrides at the end of `tokens.css`:

| Token | Value |
| --- | --- |
| `--forge-bg-base` | `#F4F6F8` |
| `--forge-bg-depth` | `#EDF1F4` |
| `--forge-bg-subtle` | `#F7F9FB` |
| `--forge-bg-surface` | `#FCFDFE` |
| `--forge-bg-raised` | `#EFF3F6` |
| `--forge-bg-raised-hover` | `#E5EBF0` |
| `--forge-bg-overlay` | `#FFFFFF` |
| `--forge-border-subtle` | `#DCE3EA` |
| `--forge-border-muted` | `rgba(30, 41, 59, 0.14)` |
| `--forge-border-strong` | `#C3CED9` |
| `--forge-text-primary` | `#131A23` |
| `--forge-text-secondary` | `#3D4754` |
| `--forge-text-muted` | `#66707D` |
| `--forge-text-faint` | `#8A94A1` |
| `--forge-text-ghost` | `#AEB7C1` |
| `--forge-accent` | `#0891B2` |
| `--forge-active` | `rgba(8, 145, 178, 0.10)` |
| `--forge-hover` | `rgba(30, 41, 59, 0.05)` |
| `--forge-focus-ring` | `rgba(8, 145, 178, 0.35)` |
| success | `#059669`, warning | `#B45309` family, danger | `#DC2626` family |
| `--forge-composer-surface` / `-focus` | `#FFFFFF` |
| `--forge-composer-border` | `#C3CED9` |
| material raised/popover/surface | `#FFFFFF` |
| code bg | `#F0F3F6` family, code text `#1B2531` |

Light work-panel: canvas `#E9EDF1`, sheet `#FCFDFE`, row `#EFF3F6`, row-active `#E3EAF0`, context `#F4F6F8`, border `rgba(30, 41, 59, 0.10)`.

## Component Detail Recipes

1. **Hairline materiality**: 1px `--forge-border-subtle` dividers are the primary separation everywhere. Remove warm-tinted inset highlights (`0 1px 0 rgba(238,234,225,…)` insets) from material shadows; keep only neutral black-alpha shadows for floating layers.
2. **Buttons**: mono labels (11.5–12.5px), 6px radius, hairline border, 150ms hover. Primary/send: accent fill, `#052025` text/icon; on light theme accent fill `#0891B2` with `#FFFFFF` text. Disabled keeps geometry, opacity 0.45.
3. **Focus rings**: `box-shadow: 0 0 0 2px var(--forge-focus-ring)` (replace thin outline) on all interactive chrome; keep `:focus-visible` only.
4. **Scrollbars**: 6px wide/high, transparent track, thumb `var(--forge-border-strong)` with 2px transparent border + `background-clip: content-box`, hover thumb `--forge-text-ghost`. Apply globally (`::-webkit-scrollbar`) — check existing rules first and unify.
5. **Severity/status rows**: 3px left border, color-coded (running → accent, review/warning → amber, alert/error → danger, ok → success). Applies to `HealthAlertBanner`, `RecoveryNoticeBanner`, `NetworkStatusBanner`, background task items (already border-left based — retune), `forge-status-*` rows, and confirm cards.
6. **Live state**: pulsing dot, 1.5s ease-in-out (retune `pulse-dot`); LED glow shadows recolored per LED tokens; no other ambient animation.
7. **Section labels**: uppercase mono micro-labels for sidebar sections, settings group titles, work-panel headers, status-bar label, history section headers.
8. **User message note**: cool `--forge-bg-raised` fill, 3px left border `--forge-border-strong`, 8px radius, sans text. Not a bubble, no accent fill.
9. **Composer**: `--forge-composer-border` hairline; focus-within → accent border + `var(--forge-composer-shadow-focus)`; running state border `rgba(var(--forge-accent-rgb), 0.26)`; send button square 6px radius accent fill. Tool chips: mono 11.5px, 6px radius, hairline.
10. **Command palette & menus**: mono; selected row = `var(--forge-active)` fill + 2px accent left rail; hairline separators; kbd hints in ghost mono.
11. **Work panel tabs**: mono 11.5px; active tab = 2px accent bottom border, not a pill.
12. **Markdown/prose**: sans; code stays mono; table surface uses light/dark ladder (replace `rgba(255, 251, 244, 0.72)` warm tint with theme-token surface); blockquote left border 3px `--forge-border-strong`.
13. **Assistant avatar/rail**: keep rail geometry; avatar becomes 6px-radius square mono "F" mark on `--forge-bg-raised` with hairline border (no emoji, no new icon library).

## Contract & Test Sync (mandatory, same commit)

- `scripts/check-conversation-style.mjs`: update every asserted value to the v5 system (light hexes, composer states/materials, send-button colors, message note, markdown table, etc.). Keep the structural assertions (selectors, radius, layout rules) unless the recipe above intentionally changes them; update `data-design-version` assertion to `v5-console-craft`.
- `e2e/messages.spec.ts`: update token expectations (currently `#1B1A17`, `#12110F`, `#B88A56`, light `#DDD2C3`/`#FEFCF8` family, dark ladder assertions near lines 2294–2685) to v5 values. Inspect `e2e/process.spec.ts` (uses `--forge-bg-depth`) and `e2e/guardrails.spec.ts` (hex lists) — update only what is theme-coupled.
- `docs/product/forge-design-language.md`: rewrite as the v5 language (positioning, signals, dark/light ladders, dual-font rule, hairline materiality, severity border system, guardrails, verification). Keep the doc structure.
- `README.md`, `apps/desktop/README.md`, `CHANGELOG.md`: add the v5 redesign entry.

## Verification

1. `npm run build` (includes `check:conversation-style`) passes.
2. `npm run test:e2e -- e2e/acceptance.spec.ts` passes; run `e2e/messages.spec.ts` if time allows.
3. `scripts/acceptance.sh --dry-run` stays aligned.
4. Screenshots (Playwright + e2e mock fixtures, dev server on :1420): app shell with active session, composer focus, settings dialog, history view, command palette — dark and light — saved under `apps/desktop/artifacts/v5-screenshots/`.
