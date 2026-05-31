"""
task-web 后端服务入口

启动方式:
    uvicorn main:app --reload --port 8765

API 路由:
    GET /api/tasks         -> 获取所有任务
    POST /api/task/add     -> 新建任务
    POST /api/task/modify  -> 修改任务
    POST /api/task/done    -> 完成任务
    POST /api/task/delete  -> 删除任务
"""

from fastapi import FastAPI
from fastapi.middleware.cors import CORSMiddleware
from api import router

app = FastAPI(title="task-web", version="0.1.0")

# 允许前端跨域访问
app.add_middleware(
    CORSMiddleware,
    allow_origins=["http://localhost:5173", "http://127.0.0.1:5173"],
    allow_methods=["*"],
    allow_headers=["*"]
)

# 挂载 API 路由
app.include_router(router, prefix="/api")
