/**
 * 全局常量。
 *
 * 所有跨组件共享的固定字符串都在此定义，
 * 避免魔法字符串散落在代码各处。
 */

const constants = {
  /** 无项目归属任务的虚拟项目路径标识符 */
  INBOX_PROJECT: "(无项目)",

  /** 状态显示文字 */
  PENDING: "待办",
  COMPLETED: "已完成",
  WAITING: "等待中",
  DELETED: "已删除",

  /** 后端 API 路径 */
  API_BASE: "/api",
};

export default constants;
