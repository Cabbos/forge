#!/usr/bin/env python3
"""Phase 7: insert imports at correct positions, then remove blocks."""
import re

with open("src/styles/globals.css") as f:
    lines = f.readlines()

# Find block boundaries
blocks = []
depth = 0
block_start = None
for i, line in enumerate(lines):
    old_depth = depth
    for ch in line:
        if ch == '{': depth += 1
        elif ch == '}': depth -= 1
    if old_depth == 1 and depth == 2 and block_start is None:
        block_start = i
    if old_depth == 2 and depth == 1 and block_start is not None:
        blocks.append((block_start, i))
        block_start = None

# Find empty-workbench, titlebar, sidebar, settings blocks
ew_blocks = []
tb_blocks = []
sb_blocks = []
st_blocks = []

for start, end in blocks:
    sel = lines[start].strip().split('{')[0].strip()
    if sel in ew_selectors or '.forge-readiness' in sel or 'empty-readiness' in sel or sel == '.forge-empty':
        ew_blocks.append((start, end))
    elif '.forge-titlebar' in sel:
        tb_blocks.append((start, end))
    elif '.forge-sidebar' in sel:
        sb_blocks.append((start, end))
    elif '.forge-settings' in sel:
        st_blocks.append((start, end))

print(f"Empty-workbench: {len(ew_blocks)} blocks")
print(f"Titlebar: {len(tb_blocks)} blocks")
print(f"Sidebar: {len(sb_blocks)} blocks")
print(f"Settings: {len(st_blocks)} blocks")

# Collect indices to remove
remove = set()
for blocks_list in [ew_blocks, tb_blocks, sb_blocks, st_blocks]:
    for start, end in blocks_list:
        for i in range(start, end + 1):
            remove.add(i)

print(f"Total lines to remove: {len(remove)}")

# Find insertion points (first block of each domain)
ew_insert = min(s for s, e in ew_blocks) if ew_blocks else None
tb_insert = min(s for s, e in tb_blocks) if tb_blocks else None
sb_insert = min(s for s, e in sb_blocks) if sb_blocks else None
st_insert = min(s for s, e in st_blocks) if st_blocks else None

print(f"Insert empty-workbench at L{ew_insert+1}")
print(f"Insert titlebar at L{tb_insert+1}")
print(f"Insert sidebar at L{sb_insert+1}")
print(f"Insert settings at L{st_insert+1}")

# Build new file with imports at correct positions
new_lines = []
for i, line in enumerate(lines):
    if i in remove:
        # Insert import at the first block of each domain
        if i == ew_insert:
            new_lines.append('@import "./empty-workbench.css";\n')
        elif i == tb_insert:
            new_lines.append('@import "./titlebar.css";\n')
        elif i == sb_insert:
            new_lines.append('@import "./sidebar.css";\n')
        elif i == st_insert:
            new_lines.append('@import "./settings.css";\n')
        # Skip the line (it's being removed)
    else:
        new_lines.append(line)

content = "".join(new_lines)
content = re.sub(r'\n{3,}', '\n\n', content)

with open("src/styles/globals.css", "w") as f:
    f.write(content)

print(f"\nglobals.css: {len(content.splitlines())} lines")
