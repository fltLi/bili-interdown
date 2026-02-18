import json
from collections import defaultdict
from typing import Any, Dict, List, Set, Tuple

def load_paths(filepath: str) -> Dict[int, List[List[Dict[str, Any]]]]:
    """
    从 JSON 文件加载路径数据，返回字典：
        节点 ID -> 到达该节点的所有路径（每条路径为从起点到该节点的节点列表）
    """
    with open(filepath, 'r', encoding='utf-8') as f:
        data: List[List[Dict[str, Any]]] = json.load(f)

    paths_by_id: Dict[int, List[List[Dict[str, Any]]]] = defaultdict(list)
    seen_paths: Dict[int, Set[Tuple[Tuple[Any, ...], ...]]] = defaultdict(set)

    for sublist in data:
        forward = list(reversed(sublist))          # 转为从起点到终点

        for i in range(len(forward)):
            prefix = forward[:i+1]                  # 起点到当前节点的路径
            node_id = prefix[-1]['id']

            # 用 (id, edge, choice) 的元组表示路径，用于去重
            rep = tuple(
                (node.get('id'), node.get('edge'), node.get('choice'))
                for node in prefix
            )
            if rep not in seen_paths[node_id]:
                seen_paths[node_id].add(rep)
                paths_by_id[node_id].append(prefix)

    return paths_by_id

def generate_mermaid(paths: List[List[Dict[str, Any]]], node_id: int) -> str:
    """根据路径列表生成 Mermaid 流程图源码（纵向 TD）。"""
    lines = ["graph TD"]
    nodes: Set[Tuple[str, str]] = set()          # (id_str, name)
    edges: Set[Tuple[str, str, str]] = set()      # (prev_id, curr_id, choice)

    for path in paths:
        for node in path:
            nid = str(node['id'])
            name = node['name']
            nodes.add((nid, name))

        for i in range(1, len(path)):
            prev = path[i-1]
            curr = path[i]
            prev_id = str(prev['id'])
            curr_id = str(curr['id'])
            choice = curr.get('choice', '')
            edges.add((prev_id, curr_id, choice))

    for nid, name in nodes:
        lines.append(f'    {nid}["{name}"]')

    for prev_id, curr_id, choice in edges:
        if choice:
            lines.append(f'    {prev_id} -- "{choice}" --> {curr_id}')
        else:
            lines.append(f'    {prev_id} --> {curr_id}')

    return "\n".join(lines)

def main() -> None:
    json_file = input("请输入 JSON 文件路径: ").strip()
    if not json_file:
        print("文件路径不能为空。")
        return

    try:
        paths_by_id = load_paths(json_file)
    except Exception as e:
        print(f"解析 JSON 失败: {e}")
        return

    print("路径加载完成。")

    while True:
        id_str = input("\n请输入节点 ID (直接回车退出): ").strip()
        if not id_str:
            break

        try:
            node_id = int(id_str)
        except ValueError:
            print("ID 必须是整数，请重新输入。")
            continue

        if node_id not in paths_by_id:
            print(f"未找到 ID 为 {node_id} 的路径。")
            continue

        paths = paths_by_id[node_id]
        mermaid_code = generate_mermaid(paths, node_id)

        output_file = f"{json_file}-{node_id}.md"
        with open(output_file, 'w', encoding='utf-8') as f:
            f.write(f"# 到达节点 {node_id} 的路径\n\n")
            f.write("```mermaid\n")
            f.write(mermaid_code)
            f.write("\n```\n")

        print(f"已生成文件: {output_file}")

if __name__ == "__main__":
    main()
