#!/usr/bin/env node
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import process from "node:process";
import { pathToFileURL } from "node:url";

const ROW_PROMPTS = {
  "1": `/fix @src/App.tsx
这个 demo 页面里有一个按钮点击后没有明显反馈。请先定位原因，再做最小修复，并运行相关检查。只改当前 demo 项目。`,
  "2": `在当前 demo 项目里做一个很小的 CSS layout polish，只改样式文件。目标是让主要按钮点击反馈更明显，但不要重构组件，不要改业务逻辑。完成后说明改了哪些文件。`,
  "3": `请在当前 demo 项目运行合适的 build/check 命令，并总结命令、结果和任何失败原因。不要修改文件。`,
};

const COMMON_FIELDS = [
  "Forge prompt",
  "Forge final answer",
  "Confirmation behavior",
  "Screenshot or transcript reference",
];

const ROW_RESULT_FIELDS = {
  "1": "Row #1 visible feedback fix result",
  "2": "Row #2 style-only polish result",
  "3": "Row #3 command-only check result",
};

export function createDisposableLoopManualTemplate({ row = "1", includePrompt = true } = {}) {
  const normalizedRow = String(row);
  const fields = [...COMMON_FIELDS];
  if (normalizedRow === "all") {
    fields.push(...Object.values(ROW_RESULT_FIELDS));
  } else {
    fields.push(ROW_RESULT_FIELDS[normalizedRow]);
  }

  const template = Object.fromEntries(fields.map((field) => [field, ""]));
  if (includePrompt) {
    template["Forge prompt"] = normalizedRow === "all"
      ? Object.entries(ROW_PROMPTS).map(([key, prompt]) => `Row #${key}:\n${prompt}`).join("\n\n")
      : ROW_PROMPTS[normalizedRow];
  }
  return template;
}

function printHelp() {
  console.log(`Usage: node scripts/create-disposable-loop-manual-json.mjs [--json] [--row <all|1|2|3>] [--out <path>] [--empty-prompt]

Creates a correctly-shaped manual evidence JSON template for Phase 8 disposable loop archive input.

Options:
  --json          Print the JSON template.
  --row VALUE     Row scope: all, 1, 2, or 3. Defaults to 1.
  --out PATH      Write the JSON template to PATH.
  --empty-prompt  Leave "Forge prompt" empty instead of pre-filling the row prompt.
  -h, --help      Show this help.
`);
}

function parseArgs(argv) {
  const options = {
    json: false,
    row: "1",
    out: null,
    includePrompt: true,
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--json") {
      options.json = true;
    } else if (arg === "--empty-prompt") {
      options.includePrompt = false;
    } else if (arg === "--row") {
      const value = argv[index + 1];
      if (!["all", "1", "2", "3"].includes(value)) throw new Error("--row must be one of: all, 1, 2, 3");
      options.row = value;
      index += 1;
    } else if (arg === "--out") {
      const value = argv[index + 1];
      if (!value) throw new Error("--out requires a path");
      options.out = value;
      index += 1;
    } else if (arg === "-h" || arg === "--help") {
      options.help = true;
    } else {
      throw new Error(`Unknown argument: ${arg}`);
    }
  }

  return options;
}

function main(argv = process.argv.slice(2)) {
  let options;
  try {
    options = parseArgs(argv);
  } catch (error) {
    console.error(error.message);
    return 2;
  }

  if (options.help) {
    printHelp();
    return 0;
  }

  const template = createDisposableLoopManualTemplate(options);
  const output = `${JSON.stringify(template, null, 2)}\n`;
  if (options.out) {
    const outputPath = resolve(options.out);
    mkdirSync(dirname(outputPath), { recursive: true });
    writeFileSync(outputPath, output);
  }

  if (options.json || !options.out) {
    process.stdout.write(output);
  }
  return 0;
}

if (process.argv[1] && pathToFileURL(process.argv[1]).href === import.meta.url) {
  process.exitCode = main();
}
