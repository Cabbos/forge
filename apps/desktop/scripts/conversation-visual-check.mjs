import { chromium } from "@playwright/test";
import { writeFileSync } from "fs";

// Build a standalone mock HTML page with inlined CSS
const cssFiles = [
  "src/styles/globals.css",
  "src/styles/tokens.css",
  "src/styles/markdown.css",
  "src/styles/messages.css",
  "src/styles/composer.css",
  "src/styles/process.css",
];

let css = "";
for (const f of cssFiles) {
  try {
    const { readFileSync } = await import("fs");
    css += readFileSync(f, "utf8") + "\n";
  } catch {}
}

const html = `<!DOCTYPE html>
<html lang="zh-CN">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Conversation Visual Check</title>
<style>
${css}
body { margin: 0; padding: 0; background: #F5F0E8; }
</style>
</head>
<body>
<div class="forge-session-operating-surface" data-conversation-theme="light" style="min-height:100vh;padding-top:2rem;">
  <div class="forge-conversation-lane" style="margin:0 auto;">

    <!-- Turn 1: user short -->
    <section class="forge-conversation-turn" data-turn-rail="user">
      <div class="forge-message-block" data-block-role="user">
        <div class="forge-user-message-row" data-message-length="short">
          <div class="forge-message-with-actions user-command-note" data-long="false">
            <div class="markdown-content">
              <p>请帮我分析一下这个路径的文件：<code class="forge-inline-code forge-inline-code-file">/Users/cabbos/project/crusted-spinning-lynx-agent/src/hooks/useSession.ts</code></p>
            </div>
          </div>
        </div>
      </div>
    </section>

    <!-- Turn 2: assistant with table -->
    <section class="forge-conversation-turn" data-turn-rail="assistant">
      <div class="forge-message-block" data-block-role="assistant">
        <div class="forge-message-with-actions assistant-paper">
          <span class="forge-assistant-avatar">F</span>
          <span class="forge-assistant-name">Forge</span>
          <div class="markdown-content">
            <p>好的，我来分析一下这个文件。</p>
            <table>
              <thead><tr><th>函数名</th><th>作用</th></tr></thead>
              <tbody>
                <tr><td><code>create</code></td><td>创建新会话</td></tr>
                <tr><td><code>resume</code></td><td>恢复已有会话</td></tr>
                <tr><td><code>deleteConversation</code></td><td>删除会话</td></tr>
              </tbody>
            </table>
            <p>路径中的关键部分：</p>
            <ul>
              <li><code>src/hooks</code> — 自定义 Hook 目录</li>
              <li><code>useSession.ts</code> — 会话管理 Hook</li>
            </ul>
          </div>
        </div>
      </div>
    </section>

    <!-- Turn 3: tool done -->
    <section class="forge-conversation-turn" data-turn-rail="assistant">
      <div class="forge-message-block" data-block-role="trace">
        <div class="tool-machine">
          <div class="forge-log-line forge-evidence-row tool-machine-plate" data-state="done" data-tone="default">
            <span style="flex-shrink:0;color:var(--forge-text-faint);">&#x2713;</span>
            <span class="forge-log-line-command tool-machine-name">已读取文件</span>
            <span class="forge-log-line-input tool-machine-input">src/hooks/useSession.ts</span>
            <span class="forge-log-status forge-log-line-status" data-tone="success" data-status="done">&#x2713;</span>
          </div>
        </div>
      </div>
    </section>

    <!-- Turn 4: shell -->
    <section class="forge-conversation-turn" data-turn-rail="assistant">
      <div class="forge-message-block" data-block-role="trace">
        <div class="shell-reel">
          <div class="shell-reel-header">
            <div class="shell-reel-body">
              <div class="forge-log-line forge-evidence-row" data-state="done" data-tone="default">
                <span style="flex-shrink:0;color:var(--forge-text-faint);font-family:monospace;">$</span>
                <span class="forge-log-line-command">cat /Users/cabbos/project/crusted-spinning-lynx-agent/src/hooks/useSession.ts | head -20</span>
                <span class="forge-log-status forge-log-line-status" data-tone="success" data-status="done">&#x2713;</span>
              </div>
            </div>
          </div>
        </div>
      </div>
    </section>

    <!-- Turn 5: user long -->
    <section class="forge-conversation-turn" data-turn-rail="user">
      <div class="forge-message-block" data-block-role="user">
        <div class="forge-user-message-row" data-message-length="long">
          <div class="forge-message-with-actions user-command-note" data-long="true">
            <div class="markdown-content">
              <p>这是一个比较长的用户消息，用来测试长消息在桌面端的表现。还要检查 inline code 的换行行为：<code class="forge-inline-code">/very/long/path/that/might/overflow/without/proper/wrapping/handling/in/the/css</code></p>
            </div>
          </div>
        </div>
      </div>
    </section>

    <!-- Turn 6: assistant with long inline code -->
    <section class="forge-conversation-turn" data-turn-rail="assistant">
      <div class="forge-message-block" data-block-role="assistant">
        <div class="forge-message-with-actions assistant-paper">
          <span class="forge-assistant-avatar">F</span>
          <span class="forge-assistant-name">Forge</span>
          <div class="markdown-content">
            <p>关于路径处理，代码中使用了 <code class="forge-inline-code">workspaceFromPath</code> 函数来规范化路径。这里有一个很长的路径示例：<code class="forge-inline-code">/Users/cabbos/project/crusted-spinning-lynx-agent/src-tauri/src/continuity/filters.rs</code> 看看它是否会在容器内正确换行。</p>
          </div>
        </div>
      </div>
    </section>

  </div>

  <!-- Composer -->
  <div class="forge-composer-frame">
    <div class="forge-conversation-lane" style="margin:0 auto;">
      <div class="forge-composer">
        <div class="forge-composer-textarea-wrap">
          <textarea class="forge-composer-textarea" placeholder="输入消息..." rows="1"></textarea>
        </div>
        <div class="forge-composer-toolbar">
          <div class="forge-composer-tool-cluster">
            <div class="forge-composer-tool-buttons">
              <button class="forge-composer-tool">@</button>
              <button class="forge-composer-tool">#</button>
            </div>
            <span class="forge-composer-hint">Enter 发送</span>
          </div>
          <div class="forge-composer-control-cluster">
            <button class="forge-composer-model">Claude 4.7</button>
            <button class="forge-composer-send" data-ready="true">&#x2191;</button>
          </div>
        </div>
      </div>
    </div>
  </div>
</div>
</body>
</html>`;

writeFileSync("scripts/conversation-visual-check.html", html);

const browser = await chromium.launch({ headless: true });
const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
await page.goto(`file://${process.cwd()}/scripts/conversation-visual-check.html`);
await page.waitForTimeout(500);
await page.screenshot({ path: "scripts/conversation-visual-check.png", fullPage: true });
console.log("Screenshot: scripts/conversation-visual-check.png");
await browser.close();
