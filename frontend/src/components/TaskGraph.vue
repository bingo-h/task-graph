<!--
  @Component: 中间 DAG 面板
  @Description:
    使用 D3 渲染 SVG，dagre 计算布局。
    支持：
      - 平移和缩放（d3-zoom）
      - 节点点击选中
      - 选中后高亮链路，其余节点淡化
      - 带动画的状态切换
      - 锁定节点显示🔒图标
      - 边上的箭头指示依赖方向
  @Author: Bin.H
  @Date: 2026-05-26
-->

<script setup>
import { ref, onMounted, watch, nextTick } from "vue";
import * as d3 from "d3";
import {
    computeLayout,
    NODE_WIDTH,
    NODE_HEIGHT,
} from "../composables/useLayout";

const props = defineProps({
    nodes: { type: Array, required: true },
    edges: { type: Array, required: true },
    selected: { type: String, default: null }, // 选中任务的路径
    highlightSet: { type: Object, default: () => new Set() }, // Set(uuid)
    projectFilter: { type: String, default: null },
    tagFilter: { type: String, default: null }, // 按标签名过滤，和 projectFilter 同时生效取交集
    tags: { type: Object, default: () => ({}) }, // 标签名 -> { name, color, task_count }
    multiSelected: { type: Object, default: () => new Set() }, // Set(uuid)，框选/Ctrl 多选的任务
    hasActiveTimer: { type: Boolean, default: false }, // 是否已有任务（单个或一批）正在计时
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
// 渲染函数
// ----------------------------------------
/**
 * 渲染画布
 */
function render() {
    const svg = d3.select(svgRef.value);
    if (!svg.node()) return;

    const { nodes, edges } = computeLayout(
        props.nodes,
        props.edges,
        props.projectFilter,
        props.tagFilter,
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
    const maxY = d3.max(nodes, (n) => n.y + NODE_HEIGHT) + 80;

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
        .attr("class", "edge")
        .attr("marker-end", "url(#arrow)")
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
        .attr("class", "edge");

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
        });

    nodeEnter
        .append("rect")
        .attr("width", NODE_WIDTH)
        .attr("height", NODE_HEIGHT)
        .attr("rx", 8)
        .attr("ry", 8);

    // 任务描述文字
    nodeEnter
        .append("text")
        .attr("class", "node-desc")
        .attr("x", 12)
        .attr("y", 22);

    // 状态/项目副标题
    nodeEnter
        .append("text")
        .attr("class", "node-sub")
        .attr("x", 12)
        .attr("y", 40);

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
        .attr("cy", NODE_HEIGHT / 2)
        .attr("r", 6)
        .on("mousedown", (event, n) => startConnectDrag(event, n, "left"))
        .on("click", (event) => event.stopPropagation());

    nodeEnter
        .append("circle")
        .attr("class", "connect-dot connect-dot-right")
        .attr("cx", NODE_WIDTH)
        .attr("cy", NODE_HEIGHT / 2)
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

    // 更新：按实际渲染宽度截断，避免中文等宽字体下按字符数截断仍超出节点边框
    nodeUpdate.select(".node-desc").each(function (n) {
        truncateToWidth(d3.select(this), n.description, NODE_WIDTH - 40);
    });

    // 更新副标题
    nodeUpdate.select(".node-sub").text((n) => subText(n));

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
}

// ----------------------------------------
// 辅助函数
// ----------------------------------------
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
    return `M${s.x + NODE_WIDTH},${s.y + NODE_HEIGHT / 2}L${t.x},${t.y + NODE_HEIGHT / 2}`;
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
    return t ? { x: t.x, y: t.y + NODE_HEIGHT / 2 } : null;
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
    if (n.is_lock) return "rect-locked";
    if (n.status === "waiting") return "rect-waiting";

    return "rect-pending";
}

/**
 * 节点副标题文字
 * @param {Object} n - 一个任务节点数据
 */
function subText(n) {
    const parts = [];

    if (n.project) parts.push(n.project.split(".").pop());
    if (n.due) parts.push(n.due.slice(0, 10));
    if (n.priority) parts.push(`[${n.priority}]`);

    return parts.join("  ");
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
            const sy2 = transform.applyY(n.y + NODE_HEIGHT);
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
        y: node.y + NODE_HEIGHT / 2,
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
        y: sourceNode.y + NODE_HEIGHT / 2,
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
onMounted(() => {
    const svg = d3.select(svgRef.value);

    // 定义箭头 marker
    // Debug: svg.select('defs').selectAll('marker').remove()
    svg.select("defs")
        .append("marker")
        .attr("id", "arrow")
        .attr("viewBox", "0 -5 10 10")
        .attr("refX", 10)
        .attr("refY", 0)
        .attr("markerWidth", 6)
        .attr("markerHeigth", 6)
        .attr("orient", "auto")
        .append("path")
        .attr("d", "M0,-5L10,0L0,5")
        .attr("class", "arrow-head");

    initZoom();
    render();
});

// 数据或高亮变化时重新渲染
watch(
    [
        () => props.nodes,
        () => props.edges,
        () => props.projectFilter,
        () => props.selected,
        () => props.highlightSet,
    ],
    async () => {
        await nextTick();
        render();
    },
    { deep: true },
);

// 切换项目后自动重置视图
watch(
    () => props.projectFilter,
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

/*
 * 注意：D3 动态生成的 SVG 元素（.node、.edge、.arrow-head 等）
 * 无法使用 scoped 样式，因为 scoped 会给选择器加上唯一属性前缀
 * 而 D3 插入的元素没有该属性。
 * 这些样式必须放在全局 style.css 中。
 */
</style>
