"""
API 路由定义

所有写操作都通过调用 task CLI 实现，操作成功后返回最新的完整图数据，
前端收到响应后直接替换本地状态，无需额外的刷新请求。
"""

import subprocess
from dataclasses import asdict

from fastapi import APIRouter, HTTPException
from pydantic import BaseModel

from data import build_graph_data

router = APIRouter()


# ----------------------------------------
# 请求体模型
# ----------------------------------------
class AddTaskRequest(BaseModel):
    """新建任务请求"""

    # 原生 taskwarrior 语法字符串，例如：
    # "修复登录 bug project:work.backend due:2026-05-20 priority:H"
    # 直接传给 task add，支持所有 taskwarrior 修饰符
    command: str


class ModifyTaskRequest(BaseModel):
    """修改任务请求"""

    uuid: str
    command: str


class TaskUUIDRequest(BaseModel):
    """仅需uuid的请求，如完成、删除"""

    uuid: str


# ----------------------------------------
# 辅助函数
# ----------------------------------------
def run_task(*args: str) -> str:
    """
    执行 task CLI 命令，返回 stdout

    失败时抛出 HTTPException(400)，错误信息来自 stderr。
    rc.confirmation=off 跳过所有交互确认。

    Args:
        *args (str): 命令内容

    Returns:
        (str) 命令执行结果
    """
    result = subprocess.run(
        ["task", "rc.confirmation=off", "rc.bulk=0", *args],
        capture_output=True,
        text=True,
    )

    if result.returncode != 0:
        raise HTTPException(status_code=400, detail=result.stderr.strip())

    return result.stdout


def graph_response() -> dict:
    """执行写操作后返回最新图数据的统一格式"""
    data = build_graph_data()
    return asdict(data)


# ----------------------------------------
# API 端点
# ----------------------------------------
@router.get("/tasks")
def get_tasks() -> dict:
    """
    获取所有任务数据

    数据包含：
    - nodes: 任务节点列表 (含派生字段 is_locked, is_overdue 等)
    - edges: 依赖关系边列表
    - projects: 项目树节点字典
    - project_roots: 根项目路径列表
    """
    try:
        data = build_graph_data()
        return asdict(data)
    except RuntimeError as e:
        raise HTTPException(status_code=500, detail=str(e))


@router.post("/task/add")
def add_task(req: AddTaskRequest) -> dict:
    """
    新建任务

    请求体：{"command": "description project:xx due:2026-05-20"}
    command 内容直接传给 task add，支持所有 taskwarrior 修饰符
    """
    tokens = req.command.strip().split()
    if not tokens:
        raise HTTPException(status_code=400, detail="命令不能为空")

    run_task("add", *tokens)

    return graph_response()


@router.post("/task/modify")
def modify_task(req: ModifyTaskRequest) -> dict:
    """
    修改任务

    请求体: {"uuid": "...", "command": "due:2026-05-20 priority:H"}
    command 内容直接传给 task <uuid> modify
    """
    tokens = req.command.strip().split()
    if not tokens:
        raise HTTPException(status_code=400, detail="修改参数不能为空")

    run_task(req.uuid, "modify", *tokens)

    return graph_response()


@router.post("/task/done")
def done_task(req: TaskUUIDRequest) -> dict:
    """将指定任务标记为完成"""
    run_task("done", req.uuid)
    return graph_response()


@router.post("/task/delete")
def delete_task(req: TaskUUIDRequest) -> dict:
    """删除指定任务"""
    run_task("delete", req.uuid)
    return graph_response()


@router.post("/task/start")
def start_task(req: TaskUUIDRequest) -> dict:
    """将任务标记为进行中 (Active)"""
    run_task("start", req.uuid)
    return graph_response()


@router.post("/task/stop")
def stop_task(req: TaskUUIDRequest) -> dict:
    """停止进行中的任务"""
    run_task("stop", req.uuid)
    return graph_response()
