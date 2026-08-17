/**
 * @file DAG 布局计算
 * @module useLayout
 * @description
 *  使用 dagre 自动计算节点坐标，支持按项目过滤。
 *  dagre 负责层次布局，D3 负责渲染，两者职责分离。
 * @author Bin.H
 * @date 2026-05-27
 */

import dagre from "@dagrejs/dagre";
import config from "../config/constants";

// 节点尺寸常量
export const NODE_WIDTH = 200;
const RANK_SEP = 80;
const NODE_SEP = 24;

// 节点高度是"自适应"的：只有描述这一行是固定的，详情行（项目/截止日期/优先级/重复）
// 显示几行由图谱显示设置决定，行数越多节点越高，见 nodeHeightFor()
const NODE_HEADER_HEIGHT = 34; // 描述行占用的高度
export const NODE_DETAIL_LINE_HEIGHT = 16; // 每条详情行的高度
export const NODE_DETAIL_START_Y = NODE_HEADER_HEIGHT + 6; // 第一条详情行的基线 y 坐标
const NODE_PADDING_BOTTOM = 14;

/**
 * 根据要显示的详情行数算出节点应有的高度
 * @param {number} detailLineCount - 0-4，项目/截止日期/优先级/重复里有几项被设置为显示
 */
export function nodeHeightFor(detailLineCount) {
  return (
    NODE_HEADER_HEIGHT +
    Math.max(0, detailLineCount) * NODE_DETAIL_LINE_HEIGHT +
    NODE_PADDING_BOTTOM
  );
}

/**
 * 计算 DAG 布局。
 *
 * @param {Array}  nodes          - 任务节点列表
 * @param {Array}  edges          - 依赖关系边列表 [{source, target}]
 * @param {string|null} projectFilter - 过滤项目路径（或分类哨兵值，见 filterNodes），null 表示显示全部
 * @param {string|null} tagFilter - 过滤标签名，null 表示不按标签过滤；和 projectFilter 同时生效（取交集）
 * @param {Object} [projects] - 项目路径 -> ProjectNode 字典，仅按分类哨兵值筛选时需要
 * @param {number} [nodeHeight] - 当前应使用的节点高度（由显示设置决定的详情行数算出），默认按 4 行算
 * @returns {{ nodes: Array, edges: Array }}
 *   nodes 每项附加 { x, y } 坐标（节点中心点）
 *   edges 每项附加 { points } 折线控制点数组
 */
export function computeLayout(
  nodes,
  edges,
  projectFilter,
  tagFilter,
  projects = {},
  nodeHeight = nodeHeightFor(4),
) {
  // 按项目、标签过滤（两者同时指定时取交集）
  const visibleNodes = filterNodes(nodes, projectFilter, projects).filter(
    (n) => !tagFilter || n.tags?.includes(tagFilter),
  );
  const visibleUUIDs = new Set(visibleNodes.map((n) => n.uuid));

  // 只保留两端都可见的边
  const visibleEdges = edges.filter(
    (e) => visibleUUIDs.has(e.source) && visibleUUIDs.has(e.target),
  );

  if (visibleNodes.length === 0) {
    return { nodes: [], edges: [] };
  }

  // 初始化 dagre 图
  const g = new dagre.graphlib.Graph();
  g.setGraph({
    rankdir: "LR", // 从左到右布局
    ranksep: RANK_SEP,
    nodesep: NODE_SEP,
    marginx: 40,
    marginy: 40,
  });
  g.setDefaultEdgeLabel(() => ({}));

  // 添加节点
  for (const node of visibleNodes) {
    g.setNode(node.uuid, { width: NODE_WIDTH, height: nodeHeight });
  }

  // 添加边
  for (const edge of visibleEdges) {
    g.setEdge(edge.source, edge.target);
  }

  // 执行布局计算
  dagre.layout(g);

  // 提取布局结果
  const layoutNodes = visibleNodes.map((node) => {
    const pos = g.node(node.uuid);
    return {
      ...node,
      x: pos.x - NODE_WIDTH / 2, // 转为左上角坐标
      y: pos.y - nodeHeight / 2,
    };
  });

  const layoutEdges = visibleEdges.map((edge) => {
    const e = g.edge(edge.source, edge.target);
    return {
      ...edge,
      points: e ? e.points : [],
    };
  });

  return { nodes: layoutNodes, edges: layoutEdges };
}

/**
 * 按项目路径过滤节点
 * @description
 *  null：显示全部
 *  "无项目"：只显示无项目归属的任务
 *  "__stage__xxx" 分类哨兵值：显示 ProjectNode.group === "xxx" 的项目下的任务
 *    （对应项目树里点击"计划中/进行中/已归档/回收站"分组标题）
 *  其他路径：显示该项目及所有子项目的任务
 * @param {Array} nodes - 所有任务
 * @param {String} projectFilter - 项目过滤
 * @param {Object} projects - 项目路径 -> ProjectNode 字典，按分类哨兵值筛选时用来查每个任务所属项目的 group
 */
function filterNodes(nodes, projectFilter, projects = {}) {
  if (!projectFilter) return nodes;

  if (projectFilter === config.INBOX_PROJECT) {
    return nodes.filter((n) => !n.project);
  }

  if (projectFilter === config.TODAY_PROJECT) {
    return nodes.filter((n) => n.is_today);
  }

  if (projectFilter.startsWith(config.STAGE_FILTER_PREFIX)) {
    const group = projectFilter.slice(config.STAGE_FILTER_PREFIX.length);
    return nodes.filter((n) => n.project && projects[n.project]?.group === group);
  }

  return nodes.filter(
    (n) =>
      n.project === projectFilter || n.project?.startsWith(projectFilter + "."),
  );
}

/**
 * 计算高亮节点集合
 *
 * @param {string} selectedUUID - 当前选中的任务UUID
 * @param {Array} edges - 所有边
 * @param {string} mode - 高亮模式
 *  - ancestors: 默认，从根到当期节点的整条链路，不包含后续任务
 *  - neighbors: 仅高亮直接上下游 (父节点 + 子节点)
 *  - full: 选中节点所在的整条链路，包含后续任务
 *
 * @return {Set<string>} 需要高亮的任务UUID集合
 */
export function computeHighlight(selectedUUID, edges, mode) {
  if (!selectedUUID) return new Set();

  // 构建邻接表
  const parents = buildAdjacency(edges, "reverse"); // 直接父节点
  const children = buildAdjacency(edges, "forward"); // 直接子节点

  switch (mode) {
    case "neighbors":
      return new Set([
        selectedUUID,
        ...(parents[selectedUUID] || []),
        ...(children[selectedUUID] || []),
      ]);

    case "full":
      // 整条链路，向上找所有祖先+向下找所有后代
      return new Set([
        selectedUUID,
        ...walkGraph(selectedUUID, parents),
        ...walkGraph(selectedUUID, children),
      ]);

    case "ancestors":
      // 默认：只向上找祖先链路
      return new Set([selectedUUID, ...walkGraph(selectedUUID, parents)]);
  }
}

/**
 * 构建邻接表
 *
 * @param {Array} edges
 * @param {string} direction - forward 表示向下找子节点，reverse表示向上找父节点
 */
function buildAdjacency(edges, direction) {
  const adj = {};

  for (const e of edges) {
    const [from, to] =
      direction === "forward" ? [e.source, e.target] : [e.target, e.source];

    if (!adj[from]) adj[from] = [];
    adj[from].push(to);
  }

  return adj;
}

/**
 * 从起点沿邻接表做 BFS，返回所有可达节点的 UUID 集合 (不含起点)
 *
 * @param {string} start - 当前选中的任务UUID
 * @param {{}} adj - 邻接表
 *
 * @returns {Set<string>} 所有可达的任务节点
 */
function walkGraph(start, adj) {
  const visited = new Set();
  const queue = [...(adj[start] || [])];

  while (queue.length > 0) {
    const uuid = queue.shift();

    if (visited.has(uuid)) continue;
    visited.add(uuid);

    queue.push(...(adj[uuid] || []));
  }

  return visited;
}

/**
 * 判断新增一条"successor 依赖 precursor"的边是否会在依赖图里形成环
 * @description precursor 如果已经（直接或传递）依赖 successor，加上这条边就会闭环；
 *  自己连自己也算一种环，一并挡掉
 *
 * @param {Array} edges - 当前所有依赖边 [{source, target}]，source 是 target 的前置任务
 * @param {string} precursorUuid - 拟新增边的前置任务
 * @param {string} successorUuid - 拟新增边的后置任务（依赖 precursorUuid 的那个）
 * @returns {boolean}
 */
export function wouldCreateCycle(edges, precursorUuid, successorUuid) {
  if (precursorUuid === successorUuid) return true;

  const forward = buildAdjacency(edges, "forward");
  return walkGraph(successorUuid, forward).has(precursorUuid);
}
