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
});

const emit = defineEmits(["select"]);

const svgRef = ref(null);
let zoomBehavior = null;

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
    );

    if (nodes.length === 0) {
        // 如果过滤后发现没有任务，则清空画布
        svg.select("g.canvas").selectAll("*").remove();
        return;
    }

    // 计算画布尺寸
    const maxX = d3.max(nodes, (n) => n.x + NODE_WIDTH) + 80;
    const maxY = d3.max(nodes, (n) => n.y + NODE_HEIGHT) + 80;

    const canvas = svg.select("g.canvas");

    // 渲染边
    const uuidToPos = Object.fromEntries(nodes.map((n) => [n.uuid, n]));
    const edgeSel = canvas
        .selectAll("path.edge")
        .data(edges, (e) => `${e.source}-${e.target}`);

    edgeSel
        .enter()
        .append("path")
        .attr("class", "edge")
        .attr("marker-end", "url(#arrow)")
        .attr("fill", "none")
        .attr("d", (e) => edgePath(e, uuidToPos))
        .style("opacity", 1);

    edgeSel
        .transition()
        .duration(300)
        .attr("d", (e) => edgePath(e, uuidToPos))
        .attr("class", (e) => {
            const dimmed =
                props.highlightSet.size > 0 &&
                !props.highlightSet.has(e.source) &&
                !props.highlightSet.has(e.target);
            return `edge ${dimmed ? "dimmed" : ""}`;
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
        .on("click", (_, n) => {
            console.log("node clicked:", n.uuid); // 加这行
            emit("select", n.uuid === props.selected ? null : n.uuid);
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

    // 更新
    nodeUpdate.select(".node-desc").text((n) => truncate(n.description, 15));

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
 * 节点容器 CSS 动态生成
 * @param {Object} n - 一个任务节点数据
 * @returns {Array} 返回一系列 CSS 设置
 */
function nodeClass(n) {
    // 1. 判断当前整个画布是不是处于“某条链路被高亮”的状态，且当前这个节点是不是不属于这条链路
    const dimmed =
        props.highlightSet.size > 0 && !props.highlightSet.has(n.uuid);

    return [
        "node", // 基础类名
        n.uuid === props.selected ? "selected" : "", // 如果当前节点被选中了，加上 .selected
        dimmed ? "dimmed" : "", // 如果不相关，加上.dimmed让它变透明
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
 * 截断超长文字并加省略号
 * @param {string} text
 * @param {number} maxLen
 * @returns {string} 返回截断后的字符串
 */
function truncate(text, maxLen) {
    return text.length > maxLen ? text.slice(0, maxLen) + "..." : text;
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
    font-size: 18px;
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
