#!/usr/bin/env python3
"""Extract CSS blocks from globals.css into domain files."""
import re

with open("src/styles/globals.css") as f:
    lines = f.readlines()

def find_block(lines, start_text, end_text, after=0):
    """Find block from first line matching start_text to line matching end_text."""
    start = None
    for i in range(after, len(lines)):
        if start_text in lines[i] and lines[i].startswith("  "):
            start = i
        if start is not None and end_text in lines[i] and i > start:
            depth = 0
            for j in range(i, len(lines)):
                for ch in lines[j]:
                    if ch == '{': depth += 1
                    elif ch == '}': depth -= 1
                if depth == 0 and j > start:
                    return start, j
    return None, None

def find_single_rule(lines, text, after=0):
    """Find a single CSS rule matching text."""
    for i in range(after, len(lines)):
        if text in lines[i] and lines[i].startswith("  ."):
            depth = 0
            for j in range(i, len(lines)):
                for ch in lines[j]:
                    if ch == '{': depth += 1
                    elif ch == '}': depth -= 1
                if depth == 0 and j > i:
                    return i, j
    return None, None

# Find blocks
tb_s, tb_e = find_block(lines, ".forge-titlebar {", ".forge-titlebar-button:hover")
sb_s, sb_e = find_block(lines, ".forge-sidebar-brand {", 'data-forge-motion="sidebar-entry"')
sm_s, sm_e = find_single_rule(lines, ".forge-sidebar-menu {", after=sb_e or 0)
sh_s, sh_e = find_block(lines, ".forge-sidebar-history-row {", ".forge-sidebar-history-delete:hover")
st_s, st_e = find_block(lines, ".forge-settings-dialog {", ".forge-settings-danger-zone")

found = []
for name, s, e in [("titlebar", tb_s, tb_e), ("sidebar-main", sb_s, sb_e),
                     ("sidebar-menu", sm_s, sm_e), ("sidebar-history", sh_s, sh_e),
                     ("settings", st_s, st_e)]:
    if s is not None:
        print(f"  {name}: L{s+1}-L{e+1} ({e-s+1} lines)")
        found.append((name, s, e))
    else:
        print(f"  {name}: NOT FOUND")

# Collect indices to remove
remove = set()
for _, s, e in found:
    for i in range(s, e + 1):
        remove.add(i)

print(f"\nRemoving {len(remove)} lines from globals.css")

# Extract content
blocks = {name: "".join(lines[s:e+1]) for name, s, e in found}

# Write new files
with open("src/styles/titlebar.css", "w") as f:
    f.write("@layer components {\n" + blocks.get("titlebar", "") + "}\n")

sb = "@layer components {\n"
sb += "  .forge-sidebar {\n    border-right: 1px solid var(--forge-border-subtle);\n"
sb += "    background:\n      linear-gradient(180deg, rgba(255, 252, 236, 0.34), transparent 10rem),\n"
sb += "      var(--sidebar);\n    padding-inline: 0.75rem;\n  }\n\n"
sb += blocks.get("sidebar-main", "") + "\n"
sb += blocks.get("sidebar-menu", "") + "\n"
sb += blocks.get("sidebar-history", "")
sb += "\n}\n"
with open("src/styles/sidebar.css", "w") as f:
    f.write(sb)

with open("src/styles/settings.css", "w") as f:
    f.write("@layer components {\n" + blocks.get("settings", "") + "}\n")

# Rebuild globals.css
new_lines = [line for i, line in enumerate(lines) if i not in remove]

# Add imports after empty-workbench import
for i, line in enumerate(new_lines):
    if '@import "./empty-workbench.css"' in line:
        new_lines.insert(i + 1, '@import "./titlebar.css";\n')
        new_lines.insert(i + 2, '@import "./sidebar.css";\n')
        new_lines.insert(i + 3, '@import "./settings.css";\n')
        break

content = "".join(new_lines)
content = re.sub(r'\n{3,}', '\n\n', content)

with open("src/styles/globals.css", "w") as f:
    f.write(content)

print(f"Created titlebar.css")
print(f"Created sidebar.css")
print(f"Created settings.css")
print(f"globals.css: {len(content.splitlines())} lines")
