<!--
  @Component: 中间 DAG 面板
  @Description:
    使用 D3 渲染 SVG，dagre 计算布局。
    支持：
      - 平移和缩放（d3-zoom）
      - 节点点击选中
      - 选中后高亮链路，其余节点淡化
      - Ctrl/Cmd + 点击、右键长按拖拽框选：多选任务，弹出批量操作工具栏
      - 带动画的状态切换
      - 锁定节点显示🔒图标
      - 边上的箭头指示依赖方向
  @Author: Bin.H
  @Date: 2026-05-26
-->

<script setup>
import { ref, computed, onMounted, watch, nextTick } from "vue";
import * as d3 from "d3";
import {
    computeLayout,
    computeHighlight,
    NODE_WIDTH,
    NODE_DETAIL_START_Y,
    NODE_DETAIL_LINE_HEIGHT,
    nodeHeightFor,
} from "../composables/useLayout";
import { formatRecurSummary } from "../composables/useRecur";
import { formatDuration } from "../composables/useDuration";
import { tagChipStyle } from "../composables/useTagColor";
import constants from "../config/constants";

const props = defineProps({
    nodes: { type: Array, required: true },
    edges: { type: Array, required: true },
    // "depends"（默认）：显示真实依赖关系；"today-order"："今日任务"分类下显示用户
    // 手动排的今日工作顺序（此时 edges 传入的已经是 today_order_edges，不是真实依赖）
    mode: { type: String, default: "depends" },
    // 真实的依赖关系边，today-order 模式下用来算悬浮/选中节点的原始依赖链路虚线预览
    dependsEdges: { type: Array, default: () => [] },
    selected: { type: String, default: null }, // 选中任务的路径
    highlightSet: { type: Object, default: () => new Set() }, // Set(uuid)
    projectFilter: { type: String, default: null },
    tagFilter: { type: String, default: null }, // 按标签名过滤，和 projectFilter 同时生效取交集
    projects: { type: Object, default: () => ({}) }, // 项目路径 -> ProjectNode，按分类哨兵值筛选时用
    tags: { type: Object, default: () => ({}) }, // 标签名 -> { name, color, task_count }
    multiSelected: { type: Object, default: () => new Set() }, // Set(uuid)，框选/Ctrl 多选的任务
    hasActiveTimer: { type: Boolean, default: false }, // 是否已有任务（单个或一批）正在计时
    // 任务卡片上默认显示哪些信息、以及每项的标签文字（悬浮详情窗不受影响，总是显示全部信息、用固定标签）
    nodeDisplay: {
        type: Object,
        default: () => ({
            node_show_project: true,
            node_show_due: true,
            node_show_priority: true,
            node_show_recur: true,
            node_label_project: constants.DEFAULT_NODE_LABELS.project,
            node_label_due: constants.DEFAULT_NODE_LABELS.due,
            node_label_priority: constants.DEFAULT_NODE_LABELS.priority,
            node_label_recur: constants.DEFAULT_NODE_LABELS.recur,
        }),
    },
});

const emit = defineEmits([
    "select",
    "toggle-multi-select",
    "box-select",
    "clear-multi-select",
    "bulk-done",
    "bulk-delete",
    "bulk-today",
    "bulk-start-timer",
    "clear-tag-filter",
    "connect-nodes", // 拖拽节点边框的连接点建立依赖关系：{ fromUuid, fromSide, toUuid }
    "reconnect-edge", // 拖拽已有连线的终点：{ sourceUuid, oldTargetUuid, newTargetUuid }
]);

const svgRef = ref(null);
let zoomBehavior = null;
let layoutNodes = []; // 最近一次布局计算出的节点（含 x/y），供框选命中检测使用

// ----------------------------------------
// 节点高度自适应：详情行显示几项（项目/截止日期/优先级/重复）由设置决定，
// 行数越多节点越高。currentNodeHeight 是给 render() 之外那些非响应式的辅助函数
// （edgePath/hitTestBox/拖拽相关函数等）读的模块级变量，每次 render() 开头都会同步更新
// ----------------------------------------
const detailLineCount = computed(() => {
    const d = props.nodeDisplay;
    return [
        d.node_show_project,
        d.node_show_due,
        d.node_show_priority,
        d.node_show_recur,
    ].filter(Boolean).length;
});
let currentNodeHeight = nodeHeightFor(4);

// ----------------------------------------
// 悬浮任务详情窗：卡片上受设置隐藏的信息，悬浮时仍能在这里看到完整详情
// ----------------------------------------
const tooltipUuid = ref(null);
const tooltipPos = ref({ x: 0, y: 0 }); // 相对 .graph-container 左上角的坐标

const tooltipTask = computed(
    () => props.nodes.find((n) => n.uuid === tooltipUuid.value) || null,
);

/** 把鼠标位置换算成相对 .graph-container 左上角的坐标，用于定位悬浮详情窗 */
function updateTooltipPos(event) {
    if (!svgRef.value) return;
    const bounds = svgRef.value.getBoundingClientRect();
    tooltipPos.value = {
        x: event.clientX - bounds.left,
        y: event.clientY - bounds.top,
    };
}

/** 悬浮详情窗里的状态显示文字 */
function statusLabel(status) {
    return (
        {
            pending: constants.PENDING,
            completed: constants.COMPLETED,
            waiting: constants.WAITING,
            deleted: constants.DELETED,
        }[status] || status
    );
}

// ----------------------------------------
// today-order 模式：悬浮/选中某个任务时，虚线预览它在真实依赖图里的完整链路
// ----------------------------------------
const hoveredUuid = ref(null); // 当前鼠标悬浮的任务，只在 today-order 模式下跟踪

/** 当前用于计算链路预览的任务：悬浮优先，其次是选中的任务 */
const chainActiveUuid = computed(() => hoveredUuid.value ?? props.selected);

/** 该任务在真实依赖图里的完整链路（祖先+后代+自身），非 today-order 模式下为空集 */
const chainNodeSet = computed(() => {
    if (props.mode !== "today-order" || !chainActiveUuid.value) return new Set();
    return computeHighlight(chainActiveUuid.value, props.dependsEdges, "full");
});

/** 链路里两端都在这个集合中的边，才画得出虚线（另一端不在"今日任务"过滤后的当前视图里就没有坐标） */
const chainEdges = computed(() => {
    if (chainNodeSet.value.size === 0) return [];
    return props.dependsEdges.filter(
        (e) => chainNodeSet.value.has(e.source) && chainNodeSet.value.has(e.target),
    );
});

/** 链路里有多少成员不在当前"今日任务"视图内（只能用徽标提示数量，画不出虚线） */
const chainOffScreenCount = computed(() => {
    if (chainNodeSet.value.size === 0) return 0;
    const visibleUuids = new Set(layoutNodes.map((n) => n.uuid));
    let count = 0;
    for (const uuid of chainNodeSet.value) {
        if (uuid !== chainActiveUuid.value && !visibleUuids.has(uuid)) count++;
    }
    return count;
});

// ----------------------------------------
// 渲染函数
// ----------------------------------------
/**
 * 渲染画布
 */
function render() {
    const svg = d3.select(svgRef.value);
    if (!svg.node()) return;

    currentNodeHeight = nodeHeightFor(detailLineCount.value);

    const { nodes, edges } = computeLayout(
        props.nodes,
        props.edges,
        props.projectFilter,
        props.tagFilter,
        props.projects,
        currentNodeHeight,
    );

    if (nodes.length === 0) {
        // 如果过滤后发现没有任务，则清空画布
        svg.select("g.canvas").selectAll("*").remove();
        layoutNodes = [];
        return;
    }

    layoutNodes = nodes;

    // 计算画布尺寸
    const maxX = d3.max(nodes, (n) => n.x + NODE_WIDTH) + 80;
    const maxY = d3.max(nodes, (n) => n.y + currentNodeHeight) + 80;

    const canvas = svg.select("g.canvas");

    // 渲染边：每条边是一个 <g class="edge-wrap">，包含连线本身和终点的拖拽手柄
    // （手柄再拉出去，拖到空白处删除依赖、拖到另一个节点则改指向它）
    const uuidToPos = Object.fromEntries(nodes.map((n) => [n.uuid, n]));
    const edgeSel = canvas
        .selectAll("g.edge-wrap")
        .data(edges, (e) => `${e.source}-${e.target}`);

    const edgeEnter = edgeSel
        .enter()
        .append("g")
        .attr("class", "edge-wrap")
        .style("opacity", 0);

    edgeEnter
        .append("path")
        .attr("class", edgeClass())
        .attr("marker-end", edgeMarker())
        .attr("fill", "none")
        .attr("d", (e) => edgePath(e, uuidToPos));

    const handleEnter = edgeEnter
        .append("g")
        .attr("class", "edge-handle")
        .on("mousedown", (event, e) => startEdgeEndDrag(event, e));
    handleEnter.append("circle").attr("class", "edge-handle-hit").attr("r", 10);
    handleEnter.append("circle").attr("class", "edge-handle-dot").attr("r", 4);

    edgeEnter.style("opacity", 1);

    const edgeUpdate = edgeSel.merge(edgeEnter);

    // 链路高亮时只淡化节点，连线本身保持原样，不跟着透明化
    edgeUpdate
        .select("path.edge")
        .transition()
        .duration(300)
        .attr("d", (e) => edgePath(e, uuidToPos))
        .attr("class", edgeClass())
        .attr("marker-end", edgeMarker());

    edgeUpdate.select(".edge-handle").attr("transform", (e) => {
        const p = edgeEndpoint(e, uuidToPos);
        return p ? `translate(${p.x},${p.y})` : null;
    });

    edgeSel.exit().transition().duration(200).style("opacity", 0).remove();

    // 渲染节点
    const nodeSel = canvas.selectAll("g.node").data(nodes, (n) => n.uuid);

    // 任务进场动画
    const nodeEnter = nodeSel
        .enter()
        .append("g")
        .attr("class", "enter")
        .attr("transform", (n) => `translate(${n.x},${n.y})`)
        .style("opacity", 0)
        .style("cursor", "pointer")
        .on("click", (event, n) => {
            if (event.ctrlKey || event.metaKey) {
                emit("toggle-multi-select", n.uuid);
            } else {
                emit("select", n.uuid === props.selected ? null : n.uuid);
            }
        })
        .on("mouseenter", (event, n) => {
            if (props.mode === "today-order") hoveredUuid.value = n.uuid;
            tooltipUuid.value = n.uuid;
            updateTooltipPos(event);
        })
        .on("mousemove", (event) => {
            updateTooltipPos(event);
        })
        .on("mouseleave", (event, n) => {
            if (props.mode === "today-order" && hoveredUuid.value === n.uuid) {
                hoveredUuid.value = null;
            }
            if (tooltipUuid.value === n.uuid) tooltipUuid.value = null;
        });

    nodeEnter
        .append("rect")
        .attr("width", NODE_WIDTH)
        .attr("height", currentNodeHeight)
        .attr("rx", 8)
        .attr("ry", 8);

    // 任务描述文字
    nodeEnter
        .append("text")
        .attr("class", "node-desc")
        .attr("x", 12)
        .attr("y", 22);

    // 详情行：项目/截止日期/优先级/重复标记，各占一行，具体显示哪几项、按什么顺序排
    // 由 nodeDetailLines() 根据设置和这个任务实际有哪些字段动态决定，行位置固定、内容紧凑排列
    for (let i = 0; i < NODE_DETAIL_LINES; i++) {
        nodeEnter
            .append("text")
            .attr("class", "node-detail")
            .attr("x", 12)
            .attr("y", NODE_DETAIL_START_Y + i * NODE_DETAIL_LINE_HEIGHT);
    }

    // 锁定图标
    nodeEnter
        .append("text")
        .attr("class", "node-lock")
        .attr("x", NODE_WIDTH - 20)
        .attr("y", 22)
        .text("🔒");

    // 逾期/今日图标
    nodeEnter
        .append("text")
        .attr("class", "node-urgency")
        .attr("x", NODE_WIDTH - 20)
        .attr("y", 42);

    // 连接点：鼠标靠近节点时才会显示（CSS 控制），按住拖到另一个节点上可以建立依赖关系。
    // 右侧点拖出去：这个任务是目标任务的前置任务；左侧点拖出去：这个任务是目标任务的后置任务
    nodeEnter
        .append("circle")
        .attr("class", "connect-dot connect-dot-left")
        .attr("cx", 0)
        .attr("cy", currentNodeHeight / 2)
        .attr("r", 6)
        .on("mousedown", (event, n) => startConnectDrag(event, n, "left"))
        .on("click", (event) => event.stopPropagation());

    nodeEnter
        .append("circle")
        .attr("class", "connect-dot connect-dot-right")
        .attr("cx", NODE_WIDTH)
        .attr("cy", currentNodeHeight / 2)
        .attr("r", 6)
        .on("mousedown", (event, n) => startConnectDrag(event, n, "right"))
        .on("click", (event) => event.stopPropagation());

    nodeEnter.style("opacity", 1);

    // 更新节点位置和样式
    const nodeUpdate = nodeSel.merge(nodeEnter);

    nodeUpdate
        .transition()
        .duration(300)
        .attr("transform", (n) => `translate(${n.x},${n.y})`);

    // 更新 CSS 类名
    nodeUpdate.attr("class", (n) => nodeClass(n));

    // 更新背景颜色
    nodeUpdate
        .select("rect")
        .transition()
        .duration(300)
        .attr("class", (n) => rectClass(n));

    // 节点高度自适应：显示的详情行数变了（比如刚在设置里勾掉一项），
    // 已经存在的节点也要跟着调整矩形高度和连接点的垂直位置，不等下次重新创建才生效
    nodeUpdate.select("rect").attr("height", currentNodeHeight);
    nodeUpdate.selectAll(".connect-dot").attr("cy", currentNodeHeight / 2);

    // 更新：按实际渲染宽度截断，避免中文等宽字体下按字符数截断仍超出节点边框
    nodeUpdate.select(".node-desc").each(function (n) {
        truncateToWidth(d3.select(this), n.description, NODE_WIDTH - 40);
    });

    // 更新详情行：每个节点各自算出要显示哪几行文字（受设置里的显示开关影响），
    // 紧凑地绑到固定的 4 个 <text> 位置上，缺的项直接留空，不会串位
    nodeUpdate.each(function (n) {
        const lines = nodeDetailLines(n, props.nodeDisplay);
        const padded = Array.from(
            { length: NODE_DETAIL_LINES },
            (_, i) => lines[i] || "",
        );

        d3.select(this)
            .selectAll(".node-detail")
            .data(padded)
            .each(function (text) {
                truncateToWidth(d3.select(this), text, NODE_WIDTH - 24);
            });
    });

    // 更新锁状态
    nodeUpdate
        .select(".node-lock")
        .style("display", (n) => (n.is_locked ? null : "none"));

    // 更新紧急状态
    nodeUpdate.select(".node-urgency").text((n) => {
        if (n.is_overdue) return "⚠";
        if (n.is_due_today) return "📅";
        return "";
    });

    nodeSel.exit().transition().duration(200).style("opacity", 0).remove();

    renderChainOverlay();
}

/**
 * 画出 today-order 模式下悬浮/选中节点的原始依赖链路虚线预览。
 * 独立于主 render()：悬浮切换时只重画这一层，不用跑一遍完整的节点/边数据绑定，
 * 避免鼠标在节点间移动时反复触发全量过渡动画。
 */
function renderChainOverlay() {
    const svg = d3.select(svgRef.value);
    const canvas = svg.select("g.canvas");
    if (canvas.empty()) return;

    let overlay = canvas.select("g.chain-overlay");
    if (overlay.empty()) {
        // 插到最前面（第一个子节点），这样边/节点始终画在虚线预览的上层
        overlay = canvas.insert("g", ":first-child").attr("class", "chain-overlay");
    }

    const uuidToPos = Object.fromEntries(layoutNodes.map((n) => [n.uuid, n]));
    const visibleChainEdges = chainEdges.value.filter(
        (e) => uuidToPos[e.source] && uuidToPos[e.target],
    );

    const sel = overlay
        .selectAll("path.chain-edge")
        .data(visibleChainEdges, (e) => `${e.source}-${e.target}`);

    sel.exit().remove();

    sel.enter()
        .append("path")
        .attr("class", "chain-edge")
        .attr("marker-end", "url(#arrow-magenta)")
        .attr("fill", "none")
        .merge(sel)
        .attr("d", (e) => edgePath(e, uuidToPos));
}

// ----------------------------------------
// 辅助函数
// ----------------------------------------
/** 边的 CSS 类名：today-order 模式下用不同颜色区分"手动排序边"和真实依赖边 */
function edgeClass() {
    return props.mode === "today-order" ? "edge edge-today-order" : "edge";
}

/**
 * 生成边的 SVG path 路径（折线通过 dagre 给出的控制点）。
 * @param {Object} edge - 连线数据源对象
 * @param {Object} uuidToPos - 快速查表字典
 * @returns {string} 最终返回符合 SVG 规范的字符串轨迹命令（如 "M10,20 C30,40 ..."）
 */
function edgePath(edge, uuidToPos) {
    // 路线 A: 高精度的复杂折线 (优选)
    if (edge.points && edge.points.length > 0) {
        const line = d3
            .line()
            .x((p) => p.x)
            .y((p) => p.y)
            .curve(d3.curveBasis);
        return line(edge.points);
    }

    // 路线 B: 直接连接两节点
    const s = uuidToPos[edge.source];
    const t = uuidToPos[edge.target];
    if (!s || !t) return "";
    return `M${s.x + NODE_WIDTH},${s.y + currentNodeHeight / 2}L${t.x},${t.y + currentNodeHeight / 2}`;
}

/**
 * 算出一条边实际画出来的终点坐标，供拖拽手柄定位。
 * 一个节点有多条入边时，dagre 不会把它们都挤到同一个点，而是在左边框上错开分布，
 * 所以手柄不能简单固定在节点左边框正中间，得跟 edgePath 用同一份 points 数据才能对上。
 * curveBasis 这种曲线生成器保证画出来的线一定经过第一个/最后一个数据点，因此直接取
 * points 的最后一个点即可，和视觉上箭头落点完全一致。
 * @param {Object} edge
 * @param {Object} uuidToPos
 * @returns {{x:number, y:number}|null}
 */
function edgeEndpoint(edge, uuidToPos) {
    if (edge.points && edge.points.length > 0) {
        const last = edge.points[edge.points.length - 1];
        return { x: last.x, y: last.y };
    }

    const t = uuidToPos[edge.target];
    return t ? { x: t.x, y: t.y + currentNodeHeight / 2 } : null;
}

/**
 * 节点容器 CSS 动态生成
 * @param {Object} n - 一个任务节点数据
 * @returns {Array} 返回一系列 CSS 设置
 */
function nodeClass(n) {
    const inSet = props.highlightSet.has(n.uuid);
    // 1. 判断当前整个画布是不是处于"某条链路被高亮"的状态，且当前这个节点是不是不属于这条链路
    const dimmed = props.highlightSet.size > 0 && !inSet;
    // 2. 属于高亮链路但不是选中节点本身：需要单独给一个视觉提示，
    //    否则它和"完全没有选中任何节点"时的默认外观完全一样，看起来像没高亮
    const highlighted = inSet && n.uuid !== props.selected;

    return [
        "node", // 基础类名
        n.uuid === props.selected ? "selected" : "", // 如果当前节点被选中了，加上 .selected
        highlighted ? "highlighted" : "", // 链路上的相关节点，加上.highlighted
        dimmed ? "dimmed" : "", // 如果不相关，加上.dimmed让它变透明
        props.multiSelected.has(n.uuid) ? "multi-selected" : "", // 框选/Ctrl 多选命中
    ]
        .filter(Boolean)
        .join(" ");
}

/**
 * 节点矩形 CSS 背景颜色动态生成
 * @param {Object} n - 一个矩形节点
 * @returns {string} 返回对应状态的颜色
 */
function rectClass(n) {
    if (n.status === "completed") return "rect-done";
    if (n.is_overdue) return "rect-overdue";
    if (n.is_due_today) return "rect-today";
    if (n.is_locked) return "rect-locked";
    if (n.status === "waiting") return "rect-waiting";

    return "rect-pending";
}

// 最多同时显示这么多行（项目/截止日期/优先级/重复），节点高度由 detailLineCount 决定
const NODE_DETAIL_LINES = 4;

/**
 * 节点卡片上的详情行文字，每一项各占一行。设置里开启的项始终显示（这个任务没有对应
 * 字段值时显示"无"），关闭的项不显示——所以每个节点显示的行数、顺序都完全一致，
 * 只取决于设置，和具体某个任务有没有值无关，这样节点高度才能统一自适应。
 * @param {Object} n - 一个任务节点数据
 * @param {Object} display - 显示开关 + 标签文字
 * @returns {string[]}
 */
function nodeDetailLines(n, display) {
    const lines = [];
    const labels = constants.DEFAULT_NODE_LABELS;

    if (display.node_show_project) {
        const label = display.node_label_project || labels.project;
        const value = n.project ? n.project.split(".").pop() : "无";
        lines.push(`${label}：${value}`);
    }
    if (display.node_show_due) {
        const label = display.node_label_due || labels.due;
        const value = n.due ? n.due.slice(0, 10) : "无";
        lines.push(`${label}：${value}`);
    }
    if (display.node_show_priority) {
        const label = display.node_label_priority || labels.priority;
        lines.push(`${label}：${n.priority || "无"}`);
    }
    if (display.node_show_recur) {
        const label = display.node_label_recur || labels.recur;
        const value = n.is_recurring ? `🔁 ${formatRecurSummary(n.recur_rule)}` : "无";
        lines.push(`${label}：${value}`);
    }

    return lines;
}

/**
 * 按实际渲染宽度截断并加省略号，写入 SVG text 选区
 * 不能按字符数截断：中文等宽字符比英文宽很多，固定字符数在字符较宽时仍会超出节点边框
 * @param {import("d3").Selection} selection 单个 SVG text 元素的 d3 选区
 * @param {string} text
 * @param {number} maxWidth 允许的最大渲染宽度（像素）
 */
function truncateToWidth(selection, text, maxWidth) {
    selection.text(text);
    const el = selection.node();
    if (!el || el.getComputedTextLength() <= maxWidth) return;

    let shown = text;
    while (shown.length > 1 && el.getComputedTextLength() > maxWidth) {
        shown = shown.slice(0, -1);
        selection.text(shown + "…");
    }
}

/**
 * 计算矩形框（屏幕坐标）与当前布局中各节点的命中结果
 * @param {number} x - 框选矩形左上角 x（相对 svg 左上角）
 * @param {number} y - 框选矩形左上角 y
 * @param {number} w - 框选矩形宽度
 * @param {number} h - 框选矩形高度
 * @returns {string[]} 与框选矩形有重叠的节点 uuid 列表
 */
function hitTestBox(x, y, w, h) {
    const transform = d3.zoomTransform(svgRef.value);
    const x2 = x + w;
    const y2 = y + h;

    return layoutNodes
        .filter((n) => {
            // 节点世界坐标换算成当前缩放/平移下的屏幕坐标，再判断矩形是否重叠
            const sx1 = transform.applyX(n.x);
            const sy1 = transform.applyY(n.y);
            const sx2 = transform.applyX(n.x + NODE_WIDTH);
            const sy2 = transform.applyY(n.y + currentNodeHeight);
            return sx1 < x2 && sx2 > x && sy1 < y2 && sy2 > y;
        })
        .map((n) => n.uuid);
}

/**
 * 初始化右键长按拖拽框选：按住右键拖出一个矩形，松开后选中矩形范围内的所有任务节点。
 * 用右键而非左键，是因为左键拖拽已经用于平移画布（d3-zoom）。
 */
function initBoxSelect() {
    const svg = d3.select(svgRef.value);
    const svgEl = svgRef.value;

    // 拖拽结束时松开的是右键，阻止浏览器/系统弹出右键菜单
    svg.on("contextmenu", (event) => event.preventDefault());

    svg.on("mousedown", (event) => {
        if (event.button !== 2) return; // 只响应右键
        event.preventDefault();

        const bounds = svgEl.getBoundingClientRect();
        const startX = event.clientX - bounds.left;
        const startY = event.clientY - bounds.top;
        let moved = false;
        let box = { x: startX, y: startY, w: 0, h: 0 };

        const boxEl = svg
            .append("rect")
            .attr("class", "select-box")
            .attr("x", startX)
            .attr("y", startY);

        function onMove(e) {
            const curX = e.clientX - bounds.left;
            const curY = e.clientY - bounds.top;
            if (Math.abs(curX - startX) > 2 || Math.abs(curY - startY) > 2) {
                moved = true;
            }

            box = {
                x: Math.min(startX, curX),
                y: Math.min(startY, curY),
                w: Math.abs(curX - startX),
                h: Math.abs(curY - startY),
            };
            boxEl
                .attr("x", box.x)
                .attr("y", box.y)
                .attr("width", box.w)
                .attr("height", box.h);

            // 实时预览命中的节点，松开后再统一通知父组件
            const hits = new Set(hitTestBox(box.x, box.y, box.w, box.h));
            svg.selectAll("g.node").classed("multi-selected", (n) =>
                hits.has(n.uuid),
            );
        }

        function onUp() {
            window.removeEventListener("mousemove", onMove);
            window.removeEventListener("mouseup", onUp);
            boxEl.remove();

            if (!moved) {
                // 未拖动，视为一次单纯的右键点击：清空多选
                emit("clear-multi-select");
                return;
            }

            emit("box-select", hitTestBox(box.x, box.y, box.w, box.h));
        }

        window.addEventListener("mousemove", onMove);
        window.addEventListener("mouseup", onUp);
    });
}

/** 按 uuid 在最近一次布局结果里查找节点（含 x/y），找不到返回 null */
function nodePosByUuid(uuid) {
    return layoutNodes.find((n) => n.uuid === uuid) || null;
}

/**
 * 从固定原点拖出一条临时连接线，实时跟踪鼠标悬停的目标节点；
 * 松开时回调 onDrop(hoveredUuid)（没有指向任何节点时为 null），由调用方决定接下来做什么。
 * 供"节点连接点建立依赖"和"已有连线终点改指向/删除"两个交互复用。
 *
 * @param {MouseEvent} event
 * @param {{x:number, y:number}} origin - 连线固定起点的世界坐标
 * @param {string} lineClass - 临时连线的额外 CSS 类（区分方向配色）
 * @param {string} excludeUuid - 命中检测时要排除的节点（一般是连线自己的起点任务）
 * @param {(hoveredUuid: string|null) => void} onDrop
 */
function dragConnectionLine(event, origin, lineClass, excludeUuid, onDrop) {
    event.stopPropagation(); // 阻止触发 d3-zoom 的平移，以及节点自身的点击选中
    event.preventDefault();

    const svg = d3.select(svgRef.value);
    const svgEl = svgRef.value;
    const bounds = svgEl.getBoundingClientRect();
    const canvas = svg.select("g.canvas");

    // 挂在 g.canvas 下面，直接用世界坐标画线，会自动跟着当前的平移/缩放走
    const lineEl = canvas
        .append("path")
        .attr("class", `connect-drag-line ${lineClass}`)
        .attr("marker-end", "url(#arrow)")
        .attr("d", `M${origin.x},${origin.y} L${origin.x},${origin.y}`);

    let hoveredUuid = null;

    function pointToWorld(clientX, clientY) {
        const transform = d3.zoomTransform(svgEl);
        return transform.invert([clientX - bounds.left, clientY - bounds.top]);
    }

    function onMove(e) {
        const [wx, wy] = pointToWorld(e.clientX, e.clientY);
        lineEl.attr("d", `M${origin.x},${origin.y} L${wx},${wy}`);

        const el = document.elementFromPoint(e.clientX, e.clientY);
        const targetGroup = el?.closest("g.node");
        const targetData = targetGroup ? d3.select(targetGroup).datum() : null;
        hoveredUuid =
            targetData && targetData.uuid !== excludeUuid ? targetData.uuid : null;

        svg.selectAll("g.node").classed(
            "connect-target",
            (n) => n.uuid === hoveredUuid,
        );
    }

    function onUp() {
        window.removeEventListener("mousemove", onMove);
        window.removeEventListener("mouseup", onUp);
        lineEl.remove();
        svg.selectAll("g.node").classed("connect-target", false);
        onDrop(hoveredUuid);
    }

    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
}

/**
 * 拖拽节点边框上的连接点，建立任务依赖关系。
 * 右侧点拖出去：这个节点是目标节点的前置任务；左侧点拖出去：这个节点是目标节点的后置任务
 * （具体转换成 depends 怎么写，由父组件根据 fromSide 解析，这里只负责交互和命中检测）
 *
 * @param {MouseEvent} event
 * @param {Object} node - 起点任务（带 x/y 的布局节点）
 * @param {"left"|"right"} side
 */
function startConnectDrag(event, node, side) {
    const origin = {
        x: side === "right" ? node.x + NODE_WIDTH : node.x,
        y: node.y + currentNodeHeight / 2,
    };

    dragConnectionLine(
        event,
        origin,
        `connect-drag-line-${side}`,
        node.uuid,
        (hoveredUuid) => {
            if (!hoveredUuid) return;
            emit("connect-nodes", {
                fromUuid: node.uuid,
                fromSide: side,
                toUuid: hoveredUuid,
            });
        },
    );
}

/**
 * 拖拽一条已有依赖连线的终点：连线起点（前置任务）固定不动，
 * 拖到空白处删除这条依赖，拖到另一个节点则把依赖关系改指向它。
 *
 * @param {MouseEvent} event
 * @param {{source:string, target:string}} edge - 被拖拽的这条边
 */
function startEdgeEndDrag(event, edge) {
    const sourceNode = nodePosByUuid(edge.source);
    if (!sourceNode) return;

    const origin = {
        x: sourceNode.x + NODE_WIDTH,
        y: sourceNode.y + currentNodeHeight / 2,
    };

    dragConnectionLine(
        event,
        origin,
        "connect-drag-line-right",
        edge.source,
        (hoveredUuid) => {
            if (hoveredUuid === edge.target) return; // 松回原处，没有变化

            emit("reconnect-edge", {
                sourceUuid: edge.source,
                oldTargetUuid: edge.target,
                newTargetUuid: hoveredUuid, // null 表示拖到空白处，删除这条依赖
            });
        },
    );
}

/**
 * 初始化缩放
 */
function initZoom() {
    const svg = d3.select(svgRef.value);
    const canvas = svg.select("g.canvas");

    zoomBehavior = d3
        .zoom()
        .scaleExtent([0.2, 3])
        .on("zoom", (e) => canvas.attr("transform", e.transform));

    svg.call(zoomBehavior);
}

/**
 * 重置缩放到可以显示所有节点
 */
function resetZoom() {
    const svg = d3.select(svgRef.value);
    const svgEl = svgRef.value;
    if (!svgEl || !zoomBehavior) return;

    // 📐 1. 获取【屏幕/浏览器视口】的实际宽高（比如：你的屏幕是 1000 x 600 像素）
    const { width, height } = svgEl.getBoundingClientRect();

    // 📐 2. 获取【主画板 canvas】的几何边界（BBox = Bounding Box）
    // 也就是说，不管图有多复杂，用一个虚拟的隐形大矩形把整张有向图死死框住
    const canvas = svg.select("g.canvas");
    const bbox = canvas.node()?.getBBox(); // canvas.node() 用于获取d3中的SVG的<g>标签
    if (!bbox || bbox.width === 0) return;

    // 📐 3. 核心计算 A：算出完美的【缩放比例（scale）】
    // 屏幕宽 / 图的总宽 = 宽度缩放比； 屏幕高 / 图的总高 = 高度缩放比。
    // 乘以 0.9 是为了在四周留出 10% 的安全页边距（留白），显得不那么拥挤。
    const scale = Math.min(
        (0.9 * width) / bbox.width,
        (0.9 * height) / bbox.height,
        1.5, // 约束上限：即使图很小，也最多放大到 1.5 倍，防止字变成大饼
    );

    // 📐 4. 核心计算 B：算出完美的【平移距离（tx, ty）】
    // 运用初中几何数学：用屏幕的中心点坐标，减去缩放后的图形中心点坐标
    // 从而算出图的左上角应该往右挪多少像素（tx）、往下挪多少像素（ty），才能让大矩形刚好悬浮在屏幕正中央！
    const tx = (width - scale * bbox.width) / 2 - scale * bbox.x;
    const ty = (height - scale * bbox.height) / 2 - scale * bbox.y;

    // 🚀 5. 发射军令，全军转场！
    svg.transition()
        .duration(500) // 开启 500 毫秒的电影级平滑过渡动画
        .call(
            zoomBehavior.transform,
            d3.zoomIdentity.translate(tx, ty).scale(scale), // 强行把算出来的完美坐标和缩放尺寸灌进去！
        );
}

// ----------------------------------------
// 生命周期
// ----------------------------------------
/**
 * 定义一个箭头 marker，颜色由 markerClass 对应的 CSS 决定。
 * 不同类型的边（真实依赖/今日排序/链路预览虚线）线条颜色不同，箭头必须跟着换一个
 * 对应颜色的 marker，不能全部共用一个固定颜色的箭头，否则线和箭头对不上。
 * @param {import("d3").Selection} defs
 * @param {string} id
 * @param {string} markerClass
 */
function defineArrowMarker(defs, id, markerClass) {
    defs
        .append("marker")
        .attr("id", id)
        .attr("viewBox", "0 -5 10 10")
        .attr("refX", 10)
        .attr("refY", 0)
        .attr("markerWidth", 6)
        .attr("markerHeigth", 6)
        .attr("orient", "auto")
        .append("path")
        .attr("d", "M0,-5L10,0L0,5")
        .attr("class", markerClass);
}

/** 边当前应该用哪个箭头 marker，和 edgeClass() 的配色规则保持一致 */
function edgeMarker() {
    return props.mode === "today-order" ? "url(#arrow-green)" : "url(#arrow)";
}

onMounted(() => {
    const svg = d3.select(svgRef.value);

    // 定义箭头 marker
    // Debug: svg.select('defs').selectAll('marker').remove()
    const defs = svg.select("defs");
    defineArrowMarker(defs, "arrow", "arrow-head");
    defineArrowMarker(defs, "arrow-green", "arrow-head-green");
    defineArrowMarker(defs, "arrow-magenta", "arrow-head-magenta");

    initZoom();
    initBoxSelect();
    render();
});

// 数据或高亮变化时重新渲染
watch(
    [
        () => props.nodes,
        () => props.edges,
        () => props.mode,
        () => props.dependsEdges,
        () => props.projectFilter,
        () => props.tagFilter,
        () => props.selected,
        () => props.highlightSet,
        () => props.multiSelected,
        () => props.nodeDisplay,
    ],
    async () => {
        await nextTick();
        render();
    },
    { deep: true },
);

// 悬浮任务变化时（today-order 模式下的链路预览）只重画虚线预览这一层，
// 不用跑完整的 render()，避免鼠标划过节点时反复触发全量过渡动画
watch(hoveredUuid, () => {
    renderChainOverlay();
});

// 切换项目/标签过滤后自动重置视图
watch(
    [() => props.projectFilter, () => props.tagFilter],
    async () => {
        await nextTick();
        render();
        setTimeout(resetZoom, 350);
    },
);

// 标签筛选提示条的颜色：用该标签自己的颜色，没设置就用默认洋红
const tagFilterColor = computed(
    () => props.tags[props.tagFilter]?.color || "#8250df",
);

defineExpose({ resetZoom });
</script>

<template>
    <div class="graph-container">
        <svg ref="svgRef" class="graph-svg">
            <defs />
            <g class="canvas" />
        </svg>

        <!-- 重置视图按钮 -->
        <button class="reset-zoom-btn" title="重置视图按钮" @click="resetZoom">
            ⊙
        </button>

        <!-- 标签筛选提示：点击某个标签跳转过来后显示，点 ✕ 清除筛选 -->
        <div
            v-if="tagFilter"
            class="tag-filter-badge"
            :style="{
                borderColor: tagFilterColor,
                color: tagFilterColor,
            }"
        >
            <span>🏷 {{ tagFilter }}</span>
            <button
                class="tag-filter-clear"
                title="清除标签筛选"
                @click="emit('clear-tag-filter')"
            >
                ✕
            </button>
        </div>

        <!-- today-order 模式下悬浮/选中节点时，提示原依赖链路里还有多少成员不在"今日任务"视图里
             （这些成员画不出虚线，因为它们在当前过滤后的布局里没有坐标） -->
        <div v-if="chainOffScreenCount > 0" class="chain-offscreen-badge">
            🔗 原依赖链路中还有 {{ chainOffScreenCount }} 个任务不在今日任务里
        </div>

        <!-- 悬浮任务详情窗：卡片本身可能因为显示设置隐藏了一些信息，这里始终展示全部 -->
        <div
            v-if="tooltipTask"
            class="node-tooltip"
            :style="{
                left: tooltipPos.x + 18 + 'px',
                top: tooltipPos.y + 18 + 'px',
            }"
        >
            <div class="node-tooltip-desc">{{ tooltipTask.description }}</div>

            <div class="node-tooltip-row">
                <span class="node-tooltip-label">状态</span>
                {{ statusLabel(tooltipTask.status) }}
            </div>
            <div v-if="tooltipTask.project" class="node-tooltip-row">
                <span class="node-tooltip-label">项目</span>
                {{ tooltipTask.project }}
            </div>
            <div v-if="tooltipTask.priority" class="node-tooltip-row">
                <span class="node-tooltip-label">优先级</span>
                {{ tooltipTask.priority }}
            </div>
            <div v-if="tooltipTask.due" class="node-tooltip-row">
                <span class="node-tooltip-label">截止</span>
                {{ tooltipTask.due.slice(0, 10) }} {{ tooltipTask.due.slice(11, 16) }}
            </div>
            <div v-if="tooltipTask.scheduled" class="node-tooltip-row">
                <span class="node-tooltip-label">计划开始</span>
                {{ tooltipTask.scheduled.slice(0, 10) }}
            </div>
            <div v-if="tooltipTask.is_recurring" class="node-tooltip-row">
                <span class="node-tooltip-label">重复</span>
                🔁 {{ formatRecurSummary(tooltipTask.recur_rule) }}
            </div>
            <div v-if="tooltipTask.depends.length" class="node-tooltip-row">
                <span class="node-tooltip-label">前置任务</span>
                {{ tooltipTask.depends.length }} 个
            </div>
            <div v-if="tooltipTask.total_seconds > 0" class="node-tooltip-row">
                <span class="node-tooltip-label">累计耗时</span>
                {{ formatDuration(tooltipTask.total_seconds) }}
            </div>
            <div class="node-tooltip-row">
                <span class="node-tooltip-label">紧迫度</span>
                {{ tooltipTask.urgency.toFixed(2) }}
            </div>

            <div v-if="tooltipTask.tags?.length" class="node-tooltip-tags">
                <span
                    v-for="tag in tooltipTask.tags"
                    :key="tag"
                    class="node-tooltip-tag"
                    :style="tagChipStyle(tags[tag]?.color)"
                >
                    {{ tag }}
                </span>
            </div>
        </div>

        <!-- 批量操作工具栏：框选 / Ctrl+点击多选后出现 -->
        <div v-if="multiSelected.size > 0" class="multi-select-toolbar">
            <span class="ms-count">已选中 {{ multiSelected.size }} 个任务</span>

            <button
                class="ms-btn"
                @click="emit('bulk-done', [...multiSelected])"
            >
                ✔ 完成
            </button>
            <button
                class="ms-btn"
                @click="emit('bulk-today', [...multiSelected])"
            >
                ☀ 设为今日任务
            </button>
            <button
                class="ms-btn"
                :disabled="hasActiveTimer"
                :title="hasActiveTimer ? '已有任务在计时，请先停止' : ''"
                @click="emit('bulk-start-timer', [...multiSelected])"
            >
                ▶ 开始计时
            </button>
            <button
                class="ms-btn ms-btn-danger"
                @click="emit('bulk-delete', [...multiSelected])"
            >
                🗑 删除
            </button>
            <button class="ms-btn ms-btn-ghost" @click="emit('clear-multi-select')">
                ✕ 取消选择
            </button>
        </div>
    </div>
</template>

<style scoped>
/* 图容器：占满中间剩余空间 */
.graph-container {
    flex: 1;
    position: relative;
    overflow: hidden;
    background: var(--bg);
}

.graph-svg {
    width: 100%;
    height: 100%;
}

/* 重置视图按钮 */
.reset-zoom-btn {
    position: absolute;
    bottom: 16px;
    right: 16px;
    width: 36px;
    height: 36px;
    border-radius: 50%;
    background: var(--bg-panel);
    border: 1px solid var(--border);
    color: var(--fg-dim);
    font-size: 1.3846rem;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: all 0.15s;
}

.reset-zoom-btn:hover {
    color: var(--fg);
    border-color: var(--fg-dark);
}

/* 标签筛选提示条：左上角悬浮，颜色跟随该标签自己的颜色 */
.tag-filter-badge {
    position: absolute;
    top: 16px;
    left: 16px;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 5px 8px 5px 12px;
    border-radius: 999px;
    background: var(--bg-panel);
    border: 1px solid;
    font-size: 0.85rem;
    font-weight: 600;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
}

.tag-filter-clear {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    color: inherit;
    opacity: 0.7;
    transition:
        opacity 0.15s,
        background 0.15s;
}
.tag-filter-clear:hover {
    opacity: 1;
    background: var(--bg-select);
}

/* today-order 模式下的"链路里还有任务不在视图内"提示（放右上角，避免和左上角的标签筛选提示条重叠） */
.chain-offscreen-badge {
    position: absolute;
    top: 16px;
    right: 16px;
    padding: 6px 12px;
    border-radius: 999px;
    background: var(--bg-panel);
    border: 1px solid var(--magenta);
    color: var(--magenta);
    font-size: 0.8462rem;
    font-weight: 600;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
    pointer-events: none;
}

/* 悬浮任务详情窗：跟着鼠标出现在任务卡片右下方，展示不受卡片显示设置影响的完整信息 */
.node-tooltip {
    position: absolute;
    z-index: 30;
    min-width: 200px;
    max-width: 280px;
    padding: 10px 12px;
    background: var(--bg-popup);
    border: 1px solid var(--border);
    border-radius: 8px;
    box-shadow: 0 10px 28px rgba(0, 0, 0, 0.25);
    pointer-events: none;
}

.node-tooltip-desc {
    font-size: 0.9231rem;
    font-weight: 700;
    color: var(--fg);
    margin-bottom: 6px;
    word-break: break-word;
}

.node-tooltip-row {
    display: flex;
    gap: 8px;
    font-size: 0.8rem;
    color: var(--fg);
    line-height: 1.6;
}

.node-tooltip-label {
    flex-shrink: 0;
    width: 56px;
    color: var(--fg-dim);
}

.node-tooltip-tags {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    margin-top: 6px;
}

.node-tooltip-tag {
    padding: 1px 7px;
    border-radius: 3px;
    font-size: 0.75rem;
}

/* 批量操作工具栏：框选 / Ctrl+点击多选后，悬浮在画布顶部中间 */
.multi-select-toolbar {
    position: absolute;
    top: 16px;
    left: 50%;
    transform: translateX(-50%);
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 12px;
    background: var(--bg-panel);
    border: 1px solid var(--border);
    border-radius: 10px;
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.12);
}

.ms-count {
    font-size: 0.85rem;
    color: var(--fg-dim);
    margin-right: 4px;
    white-space: nowrap;
}

.ms-btn {
    padding: 5px 10px;
    font-size: 0.85rem;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--bg);
    color: var(--fg);
    white-space: nowrap;
    transition: all 0.15s;
}

.ms-btn:hover:not(:disabled) {
    border-color: var(--blue);
    color: var(--blue);
}

.ms-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
}

.ms-btn-danger:hover:not(:disabled) {
    border-color: var(--red);
    color: var(--red);
}

.ms-btn-ghost {
    background: transparent;
    border-color: transparent;
    color: var(--fg-dim);
}

/*
 * 注意：D3 动态生成的 SVG 元素（.node、.edge、.arrow-head 等）
 * 无法使用 scoped 样式，因为 scoped 会给选择器加上唯一属性前缀
 * 而 D3 插入的元素没有该属性。
 * 这些样式必须放在全局 style.css 中。
 */
</style>
