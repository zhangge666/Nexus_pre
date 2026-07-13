/** 本文件实现知识图谱页面，用 Canvas 2D 渲染力导向节点图。 */
import type React from "react";
import { useEffect, useRef, useState, useCallback } from "react";
import { Network, RefreshCw, ZoomIn, ZoomOut } from "lucide-react";
import { getGraphData } from "../core";
import type { GraphNode, GraphEdge } from "../core";
import { Topbar } from "../components/Topbar";
import { EmptyState } from "../components/EmptyState";

const SOURCE_COLORS: Record<string, string> = {
  orbit: "hsl(234,56%,60%)",
  muse:  "hsl(38,92%,58%)",
  quill: "hsl(142,44%,52%)",
  echo:  "hsl(200,50%,55%)",
};

const RELATION_DASH: Record<string, number[]> = {
  derived_from: [],
  related:      [6, 4],
  references:   [3, 3],
  duplicate:    [8, 3],
};

function getNodeColor(source: string): string {
  return SOURCE_COLORS[source.split(":")[0]] ?? "hsl(220,10%,50%)";
}

interface NodeWithPos extends GraphNode {
  x: number; y: number; vx: number; vy: number;
}

export default function GraphPage(): React.JSX.Element {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [nodes, setNodes] = useState<NodeWithPos[]>([]);
  const [edges, setEdges] = useState<GraphEdge[]>([]);
  const [selected, setSelected] = useState<GraphNode | null>(null);
  const [loading, setLoading] = useState(true);
  const [scale, setScale] = useState(1);
  const animRef = useRef<number>(0);
  const nodesRef = useRef<NodeWithPos[]>([]);

  const loadGraph = useCallback(async () => {
    setLoading(true);
    try {
      const { nodes: rawNodes, edges: rawEdges } = await getGraphData();
      const canvas = canvasRef.current;
      const w = canvas?.width ?? 800;
      const h = canvas?.height ?? 500;
      const positioned: NodeWithPos[] = rawNodes.map((n, i) => ({
        ...n,
        x: w / 2 + Math.cos((i / rawNodes.length) * Math.PI * 2) * 180,
        y: h / 2 + Math.sin((i / rawNodes.length) * Math.PI * 2) * 130,
        vx: 0, vy: 0,
      }));
      setNodes(positioned);
      nodesRef.current = positioned;
      setEdges(rawEdges);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { void loadGraph(); }, [loadGraph]);

  // 力导向模拟 + Canvas 渲染
  useEffect(() => {
    if (nodes.length === 0) return;
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d")!;
    let frame = 0;

    function tick(): void {
      const ns = nodesRef.current;
      // 斥力
      for (let i = 0; i < ns.length; i++) {
        for (let j = i + 1; j < ns.length; j++) {
          const dx = ns[j].x - ns[i].x;
          const dy = ns[j].y - ns[i].y;
          const dist = Math.sqrt(dx * dx + dy * dy) || 1;
          const force = 4000 / (dist * dist);
          ns[i].vx -= (dx / dist) * force;
          ns[i].vy -= (dy / dist) * force;
          ns[j].vx += (dx / dist) * force;
          ns[j].vy += (dy / dist) * force;
        }
      }
      // 弹力（边约束）
      for (const edge of edges) {
        const a = ns.find((n) => n.id === edge.from);
        const b = ns.find((n) => n.id === edge.to);
        if (!a || !b) continue;
        const dx = b.x - a.x;
        const dy = b.y - a.y;
        const dist = Math.sqrt(dx * dx + dy * dy) || 1;
        const strength = (dist - 160) * 0.03;
        a.vx += (dx / dist) * strength;
        a.vy += (dy / dist) * strength;
        b.vx -= (dx / dist) * strength;
        b.vy -= (dy / dist) * strength;
      }
      // 中心引力
      const cx = canvas!.width / 2;
      const cy = canvas!.height / 2;
      for (const n of ns) {
        n.vx += (cx - n.x) * 0.005;
        n.vy += (cy - n.y) * 0.005;
        n.vx *= 0.85;
        n.vy *= 0.85;
        n.x += n.vx;
        n.y += n.vy;
      }

      // 渲染
      ctx.clearRect(0, 0, canvas!.width, canvas!.height);

      // 边
      for (const edge of edges) {
        const a = ns.find((n) => n.id === edge.from);
        const b = ns.find((n) => n.id === edge.to);
        if (!a || !b) continue;
        ctx.save();
        ctx.beginPath();
        ctx.setLineDash(RELATION_DASH[edge.relation] ?? []);
        ctx.strokeStyle = edge.relation === "duplicate"
          ? "hsl(0,65%,58%,0.5)"
          : "hsla(220,10%,100%,0.12)";
        ctx.lineWidth = 1.5;
        ctx.moveTo(a.x, a.y);
        ctx.lineTo(b.x, b.y);
        ctx.stroke();
        ctx.restore();
      }

      // 节点
      for (const n of ns) {
        const r = n.kind === "card" ? 10 : 14;
        const isSelected = selected?.id === n.id;
        ctx.save();
        ctx.beginPath();
        ctx.arc(n.x, n.y, isSelected ? r + 4 : r, 0, Math.PI * 2);
        ctx.fillStyle = getNodeColor(n.source);
        ctx.globalAlpha = isSelected ? 1 : 0.85;
        ctx.fill();
        if (isSelected) {
          ctx.strokeStyle = "hsl(220,15%,96%)";
          ctx.lineWidth = 2;
          ctx.stroke();
        }
        ctx.restore();

        // 标签
        ctx.save();
        ctx.font = "11px Inter, system-ui, sans-serif";
        ctx.fillStyle = "hsl(220,15%,85%)";
        ctx.textAlign = "center";
        const label = n.title.length > 12 ? n.title.slice(0, 12) + "…" : n.title;
        ctx.fillText(label, n.x, n.y + r + 14);
        ctx.restore();
      }

      frame++;
      animRef.current = requestAnimationFrame(tick);
    }

    animRef.current = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(animRef.current);
  }, [nodes, edges, selected]);

  function handleCanvasClick(e: React.MouseEvent<HTMLCanvasElement>): void {
    const rect = canvasRef.current!.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const y = e.clientY - rect.top;
    const hit = nodesRef.current.find((n) => {
      const r = n.kind === "card" ? 10 : 14;
      return Math.hypot(n.x - x, n.y - y) < r + 4;
    });
    setSelected(hit ?? null);
  }

  return (
    <div className="page-enter graph-page">
      <Topbar
        title="知识图谱"
        subtitle={`${nodes.length} 个节点 · ${edges.length} 条关联`}
        actions={
          <div className="topbar-actions">
            <button className="icon-button" onClick={() => setScale((s) => Math.min(s + 0.2, 3))} title="放大"><ZoomIn size={16} /></button>
            <button className="icon-button" onClick={() => setScale((s) => Math.max(s - 0.2, 0.4))} title="缩小"><ZoomOut size={16} /></button>
            <button className="secondary-button" onClick={() => void loadGraph()}><RefreshCw size={14} />重置</button>
          </div>
        }
      />

      {loading && (
        <div className="graph-loading">
          <EmptyState icon={<Network size={40} />} title="加载图谱数据…" />
        </div>
      )}

      <div className="graph-container">
        <canvas
          ref={canvasRef}
          className="graph-canvas"
          width={900}
          height={560}
          onClick={handleCanvasClick}
          style={{ transform: `scale(${scale})`, transformOrigin: "center top" }}
        />

        {/* 图例 */}
        <div className="graph-legend">
          {Object.entries(SOURCE_COLORS).map(([src, color]) => (
            <div key={src} className="legend-item">
              <span className="legend-dot" style={{ background: color }} />
              <span>{src.charAt(0).toUpperCase() + src.slice(1)}</span>
            </div>
          ))}
        </div>

        {/* 节点详情面板 */}
        {selected && (
          <div className="graph-node-panel">
            <button className="graph-panel-close icon-button" onClick={() => setSelected(null)}>✕</button>
            <div className="graph-panel-dot" style={{ background: getNodeColor(selected.source) }} />
            <h3>{selected.title}</h3>
            <p className="graph-panel-meta">{selected.source} · {selected.kind}</p>
            <div className="graph-panel-links">
              {edges.filter((e) => e.from === selected.id || e.to === selected.id).map((e, i) => {
                const other = nodesRef.current.find((n) => n.id === (e.from === selected.id ? e.to : e.from));
                return other ? (
                  <div key={i} className="graph-panel-link">
                    <span className="link-relation">{e.relation}</span>
                    <span className="link-target">{other.title}</span>
                  </div>
                ) : null;
              })}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
