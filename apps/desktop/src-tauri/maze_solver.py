"""
5x5 迷宫 BFS 求解器
起点 (0,0) → 终点 (4,4)
3 堵墙：[(1,1), (2,2), (3,3)]
"""

from collections import deque

# ─── 迷宫定义 ───
ROWS, COLS = 5, 5
START = (0, 0)
END = (4, 4)
WALLS = [(1, 1), (2, 2), (3, 3)]

# 四方向：上 下 左 右
DIRECTIONS = [(-1, 0), (1, 0), (0, -1), (0, 1)]


def bfs(maze, start, end):
    """
    BFS 搜索最短路径
    返回：(路径坐标列表, 搜索步数)
    """
    rows, cols = len(maze), len(maze[0])
    visited = [[False] * cols for _ in range(rows)]
    # queue: (row, col, path)
    queue = deque()
    queue.append((start[0], start[1], [start]))
    visited[start[0]][start[1]] = True

    while queue:
        r, c, path = queue.popleft()

        # 到达终点
        if (r, c) == end:
            return path

        for dr, dc in DIRECTIONS:
            nr, nc = r + dr, c + dc
            if 0 <= nr < rows and 0 <= nc < cols:
                if not visited[nr][nc] and maze[nr][nc] == 0:
                    visited[nr][nc] = True
                    queue.append((nr, nc, path + [(nr, nc)]))

    return None  # 无解


def print_maze(maze, path=None):
    """打印迷宫，可选标注路径"""
    print("  " + "─" * (COLS * 4 - 1))
    for r in range(ROWS):
        row_str = "│ "
        for c in range(COLS):
            cell = maze[r][c]
            if path and (r, c) in path:
                idx = path.index((r, c))
                row_str += f"\033[92m{idx:2d}\033[0m "  # 绿色路径编号
            elif cell == 1:
                row_str += "██ "
            elif (r, c) == START:
                row_str += " S "
            elif (r, c) == END:
                row_str += " E "
            else:
                row_str += " · "
            row_str += "│ "
        print(row_str)
        print("  " + "─" * (COLS * 4 - 1))
    print()


def main():
    # 构建迷宫矩阵: 0=通路, 1=墙
    maze = [[0] * COLS for _ in range(ROWS)]
    for wr, wc in WALLS:
        maze[wr][wc] = 1
    # 确保起点终点不是墙
    maze[START[0]][START[1]] = 0
    maze[END[0]][END[1]] = 0

    print("=" * 46)
    print("  5x5 迷宫 BFS 求解器")
    print(f"  起点: {START}  终点: {END}")
    print(f"  墙体: {WALLS}")
    print("=" * 46)
    print()
    print("█ 迷宫地图（绿色数字 = BFS 路径编号）：")
    print()

    # 求解
    path = bfs(maze, START, END)

    # 打印迷宫（带路径）
    print_maze(maze, path)

    if path:
        print(f"✅ BFS 找到最短路径，共 {len(path)} 步！")
        print(f"   路径坐标: {path}")
        print()
        # 格式化输出
        print("   路径（逐行）：")
        for i, (r, c) in enumerate(path):
            arrow = " → " if i < len(path) - 1 else "   "
            print(f"      ({r},{c}){arrow}", end="")
            if (i + 1) % 4 == 0:
                print()
        print()
        print()
        print(f"   总步数: {len(path) - 1} 步（从起点到终点移动次数）")
    else:
        print("❌ 无法找到从起点到终点的路径！")


if __name__ == "__main__":
    main()
