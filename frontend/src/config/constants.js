/**
 * 全局常量。
 *
 * 所有跨组件共享的固定字符串都在此定义，
 * 避免魔法字符串散落在代码各处。
 */

const constants = {
  /** 无项目归属任务的虚拟项目路径标识符，需要和后端 commands.rs 里的 INBOX_PROJECT 保持一致 */
  INBOX_PROJECT: "无项目",

  /**
   * 按项目分类（计划中/进行中/已归档/回收站）筛选任务图谱时使用的哨兵值，
   * 前缀 __stage__ 加分组 key（对应 ProjectNode.group 字段），
   * 用于和真实项目路径区分开，不会和任何实际项目路径冲突
   */
  STAGE_FILTER_PREFIX: "__stage__",
  stageFilter(group) {
    return `${this.STAGE_FILTER_PREFIX}${group}`;
  },

  /**
   * "今日任务"分类的哨兵值：选中它时任务看板显示所有 is_today 的任务，
   * 依赖图边替换成用户手动排的今日顺序（today_order_edges），而不是真实 depends
   */
  TODAY_PROJECT: "__today__",

  /** 状态显示文字 */
  PENDING: "待办",
  COMPLETED: "已完成",
  WAITING: "等待中",
  DELETED: "已删除",

  /** 后端 API 路径 */
  API_BASE: "/api",

  /**
   * 任务图谱节点卡片上，项目/截止日期/优先级/重复标记这几项详情行的默认标签文字，
   * 用户可以在设置里分别自定义；修改后"重置"按钮会把对应项恢复成这里的默认值
   */
  DEFAULT_NODE_LABELS: {
    project: "项目",
    due: "截止时间",
    priority: "优先级",
    recur: "重复",
  },
};

export default constants;
