"""
数据层: 调用 task export 获取任务数据，并转换为前端所需的结构
"""

import json
import subprocess
from dataclasses import asdict, dataclass, field
from datetime import datetime, timezone

from constants import INBOX_PROJECT


@dataclass
class Task:
    """单个 Taskwarrior 任务"""

    uuid: str
    description: str
    status: str
    project: str | None
    tags: list[str] = field(default_factory=list)
    priority: str | None = None  # H / M / L
    urgency: float = 0.0
    due: str | None = None
    scheduled: str | None = None
    entry: str | None = None
    end: str | None = None
    depends: list[str] = field(default_factory=list)  # 依赖的任务 UUID 列表
    annotations: list[str] = field(default_factory=list)  # 任务备注列表
    # 派生字段，由数据层计算
    is_overdue: bool = False  # 是否过期
    is_due_today: bool = False  # 是否今天到期
    is_locked: bool = False  # 是否被锁定（有未完成的依赖任务且过期）
    blocking: list[str] = field(
        default_factory=list
    )  # 阻塞当前任务的未完成依赖任务 UUID 列表


@dataclass
class ProjectNode:
    """项目树节点"""

    path: str  # 项目路径，例如 "work.backend"
    name: str  # 项目名称，例如 "backend"
    depth: int  # 项目深度，根项目为 0
    children: list[str] = field(default_factory=list)  # 子节点完整路径
    pending_count: int = 0  # 包含子项目在内的待办任务数量
    completed_count: int = 0  # 包含子项目在内的已完成任务数量
    waiting_count: int = 0  # 包含子项目在内的等待任务数量
    overdue_count: int = 0  # 包含子项目在内的过期任务数量
    locked_count: int = 0  # 包含子项目在内的锁定任务数量
    overdue_blocking_count: int = 0  # 包含子项目在内的过期且阻塞其他任务的任务数量


@dataclass
class GraphData:
    """
    前端 DAG 渲染所需的全部数据。

    nodes: 任务节点列表
    edges: 依赖关系边列表，格式 {"source": uuid, "target": uuid}
           含义：source 必须在 target 之前完成（source 是 target 的前置任务）
    projects: 项目树节点字典，key 为完整路径
    project_roots: 根项目路径列表（排序后）
    """

    nodes: list[dict]
    edges: list[dict]
    projects: dict[str, dict]
    project_roots: list[str]


# ----------------------------------------
# 日期处理
# ----------------------------------------
def parse_datetime(s: str) -> datetime | None:
    """
    解析 Taskwarrior 的紧凑日期格式 "20260518T043412Z"。

    Taskwarrior 使用无连字符/冒号的紧凑 UTC 格式，
    与标准 ISO 8601 "2026-05-18T04:34:12Z" 不同。
    """
    if not s:
        return None

    try:
        return datetime.strptime(s, "%Y%m%dT%H%M%SZ").replace(tzinfo=timezone.utc)
    except ValueError:
        return None


def format_datetime(dt: datetime | None) -> str | None:
    """将 datetime 转换为前端友好的 ISO 字符串，保留时区信息"""
    if dt is None:
        return None
    return dt.strftime("%Y-%m-%dT%H:%M:%SZ")  # 输出格式示例: "2026-05-18T04:34:12Z"


# ----------------------------------------
# 数据加载
# ----------------------------------------
def load_tasks() -> list[Task]:
    """
    调用 `task export` 获取所有任务并解析为 Task 对象列表。

    task export 输出 JSON 数组，每个元素是一个任务对象。
    deleted 状态的任务会被过滤掉，不参与图的构建。
    """
    result = subprocess.run(
        ["task", "export"],
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        raise RuntimeError(f"task export 失败: {result.stderr}")

    raw_tasks = json.loads(result.stdout)
    tasks = []

    now = datetime.now(timezone.utc)

    for raw in raw_tasks:
        # 过滤已删除任务，不显示在图中
        if raw.get("status") == "deleted":
            continue

        due_dt = parse_datetime(raw.get("due"))

        # 计算逾期和今日到期状态
        is_overdue = False
        is_due_today = False
        if due_dt and raw.get("status") == "pending":
            delta = due_dt - now
            is_overdue = delta.total_seconds() < 0
            is_due_today = 0 <= delta.total_seconds() < 86400

        task = Task(
            uuid=raw.get("uuid"),
            description=raw.get("description"),
            status=raw.get("status"),
            project=raw.get("project"),
            tags=raw.get("tags"),
            priority=raw.get("priority"),
            urgency=raw.get("urgency", 0.0),
            due=format_datetime(due_dt),
            scheduled=format_datetime(parse_datetime(raw.get("scheduled"))),
            entry=format_datetime(parse_datetime(raw.get("entry"))),
            end=format_datetime(parse_datetime(raw.get("end"))),
            depends=raw.get("depends", []),
            is_overdue=is_overdue,
            is_due_today=is_due_today,
        )
        tasks.append(task)

    # 构建反向依赖和锁定状态
    _build_derived_fields(tasks)

    return tasks


def _build_derived_fields(tasks: list[Task]):
    """
    计算派生字段

    1. blocking: 对每个任务，找出哪些任务依赖它
    2. is_locked: 如果任务的前置任务未完成，则该任务被锁定
    """
    uuid_map = {t.uuid: t for t in tasks}

    # 构建 blocking 关系
    for task in tasks:
        for dep_uuid in task.depends:
            if dep_uuid in uuid_map:
                uuid_map[dep_uuid].blocking.append(task.uuid)

    # 计算锁定状态
    for task in tasks:
        if task.status != "pending":
            continue

        for dep_uuid in task.depends:
            dep = uuid_map.get(dep_uuid)
            # 前置任务存在且未完成
            if dep and dep.status not in ("completed",):
                task.is_locked = True
                break


# ----------------------------------------
# 项目树构建
# ----------------------------------------
def build_project_tree(tasks: list[Task]) -> tuple[dict[str, ProjectNode], list[str]]:
    """
    从任务列表构建树

    Returns:
        nodes - 项目路径到 ProjectNode 的映射
        roots - 根项目路径列表（排序后），开头插入“无项目”虚拟节点
    """
    nodes: dict[str, ProjectNode] = {}

    # 步骤1：为每个路径段创建节点
    for task in tasks:
        if not task.project:
            continue

        parts = task.project.split(".")

        for i in range(1, len(parts) + 1):
            path = ".".join(parts[:i])
            if path not in nodes:
                name = parts[i - 1]
                depth = i - 1
                nodes[path] = ProjectNode(path=path, name=name, depth=depth)

    # 步骤2：建立父子关系
    for path in list(nodes.keys()):
        parts = path.split(".")
        if len(parts) > 1:
            parent_path = ".".join(parts[:-1])
            if parent_path in nodes and path not in nodes[parent_path].children:
                nodes[parent_path].children.append(path)

    for node in nodes.values():
        node.children.sort()

    # 步骤 3: 将任务计数聚合到精确匹配的项目节点
    for task in tasks:
        if not task.project or task.project not in nodes:
            continue

        node = nodes[task.project]

        if task.status == "pending":
            node.pending_count += 1
            if task.is_overdue:
                node.overdue_count += 1
            if task.is_locked:
                node.locked_count += 1
        elif task.status == "completed":
            node.completed_count += 1
        elif task.status == "waiting":
            node.waiting_count += 1

    # 步骤 4: 从深到浅向上传播计数
    sorted_path = sorted(nodes.keys(), key=lambda p: p.count("."), reverse=True)
    for path in sorted_path:
        parts = path.split(".")

        if len(parts) < 2:
            continue

        parent_path = ".".join(parts[:-1])

        if parent_path not in nodes:
            continue

        child = nodes[path]
        parent = nodes[parent_path]
        parent.pending_count += child.pending_count
        parent.completed_count += child.completed_count
        parent.waiting_count += child.waiting_count
        parent.overdue_count += child.overdue_count
        parent.locked_count += child.locked_count

    # 步骤 5: 收集根节点
    roots = sorted(p for p in nodes if "." not in p)

    # 步骤 6: 处理无项目归属任务
    inbox_tasks = [
        t
        for t in tasks
        if not t.project and t.status in ("pending", "waiting", "completed")
    ]

    if inbox_tasks:
        inbox = ProjectNode(path=INBOX_PROJECT, name=INBOX_PROJECT, depth=0)
        for t in inbox_tasks:
            if t.status == "pending":
                inbox.pending_count += 1
                if t.is_overdue:
                    inbox.overdue_count += 1
                if t.is_locked:
                    inbox.locked_count += 1
            elif t.status == "completed":
                inbox.completed_count += 1
            elif t.status == "waiting":
                inbox.waiting_count += 1

        nodes[INBOX_PROJECT] = inbox
        roots: list[str] = [INBOX_PROJECT] + roots

    return nodes, roots


def build_graph_data() -> GraphData:
    """
    组装前端 DAG 所需的完整数据

    Returns:
        nodes - 每个元素包含任务的所有字段
        edges - 前置任务关系
    """
    tasks = load_tasks()
    project_nodes, roots = build_project_tree(tasks)

    nodes = [asdict(obj=t) for t in tasks]

    # 构建边: depends 表示“我依赖这些任务”
    # 边的方向为：前置任务 -> 当前任务 (表示执行顺序)
    edges = []
    for task in tasks:
        for dep_uuid in task.depends:
            edges.append({"source": dep_uuid, "target": task.uuid})

    projects = {path: asdict(node) for path, node in project_nodes.items()}

    return GraphData(
        nodes=nodes,
        edges=edges,
        projects=projects,
        project_roots=roots,
    )
