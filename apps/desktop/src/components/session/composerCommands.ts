export interface ComposerCommand {
  prefix: string;
  text: string;
  desc: string;
}

export const COMPOSER_COMMANDS: ComposerCommand[] = [
  { prefix: "/cr", text: "/code-review", desc: "检查有没有风险" },
  { prefix: "/fix", text: "/fix", desc: "帮我修一个问题" },
  { prefix: "/explain", text: "/explain", desc: "解释清楚" },
  { prefix: "/refactor", text: "/refactor", desc: "整理代码结构" },
  { prefix: "/test", text: "/test", desc: "运行相关检查" },
  { prefix: "/docs", text: "/docs", desc: "补充说明文档" },
  { prefix: "/compact", text: "/compact", desc: "压缩当前上下文" },
];
