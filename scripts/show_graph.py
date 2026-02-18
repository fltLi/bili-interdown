import json
from typing import Any, Dict, List

def generate(json_file: str) -> None:
    """读取 JSON 文件并生成 Mermaid 流程图，保存为同名的 .md 文件。"""
    with open(json_file, 'r', encoding='utf-8') as f:
        data: Dict[str, Any] = json.load(f)

    nodes: List[Dict[str, Any]] = data['graph']['nodes']
    node_names: Dict[int, str] = {node['id']: node['name'] for node in nodes}

    lines = ['graph TD']

    # 添加所有节点
    for node in nodes:
        node_id = node['id']
        raw_name = node['name']
        clean_name = raw_name.replace('\n', ' ').replace('"', '\\"')
        lines.append(f'    n{node_id}["{clean_name}"]')

    # 添加边（选择关系）
    for node in nodes:
        src_id = node['id']
        if 'choices' in node:
            for choice in node['choices']:
                target_id = choice['target']
                if target_id not in node_names:
                    print(f'警告：目标节点 {target_id} 不存在，跳过')
                    continue
                choice_name = choice['name'].replace('\n', ' ').replace('"', '\\"')
                lines.append(f'    n{src_id} -->|"{choice_name}"| n{target_id}')

    mermaid_code = "```mermaid\n" + '\n'.join(lines) + "\n```\n"
    output_file = f"{json_file}.md"
    with open(output_file, "w", encoding="utf-8") as f:
        f.write(mermaid_code)
    print(f"已生成文件: {output_file}")

def main() -> None:
    json_file = input("请输入 JSON 文件路径: ").strip()
    if not json_file:
        print("文件路径不能为空。")
        return
    generate(json_file)

if __name__ == "__main__":
    main()
