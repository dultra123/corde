import { useRef, useState } from "react";
import { useT } from "../i18n";
import { COLORS, classColor } from "../lib/color";
import { useTelemetry } from "../store/telemetry";
import { TrackLearner } from "./map/learner";
import { registerWidget, type WidgetProps } from "./registry";
import { fitCanvas, useElementSize, useFrameDraw } from "./shared/hooks";

interface Bounds {
  minX: number;
  maxX: number;
  minZ: number;
  maxZ: number;
}

function extend(b: Bounds, x: number, z: number): void {
  if (x < b.minX) b.minX = x;
  if (x > b.maxX) b.maxX = x;
  if (z < b.minZ) b.minZ = z;
  if (z > b.maxZ) b.maxZ = z;
}

function MapWidget(_props: WidgetProps) {
  const { t } = useT();
  const hostRef = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const size = useElementSize(hostRef);
  const learner = useRef(new TrackLearner()).current;
  const trackRef = useRef<string>("");
  const [progress, setProgress] = useState(0);
  const [sector, setSector] = useState(0);
  // smoothed bounds so the auto-scale does not jitter while learning
  const boundsRef = useRef<Bounds | null>(null);

  useFrameDraw(() => {
    const canvas = canvasRef.current;
    if (!canvas || size.width < 10 || size.height < 10) return;
    const ctx = fitCanvas(canvas, size);
    if (!ctx) return;
    const { frame } = useTelemetry.getState();
    const W = size.width;
    const H = size.height;
    ctx.clearRect(0, 0, W, H);
    if (!frame) return;

    if (frame.session.track_name !== trackRef.current) {
      trackRef.current = frame.session.track_name;
      learner.reset();
      boundsRef.current = null;
    }
    const player = frame.player;
    if (player) {
      learner.learn(
        player.motion.pos[0],
        player.motion.pos[2],
        player.lap.lap_fraction,
        player.lap.current_sector,
        player.in_pits || player.lap.in_pit_lane,
      );
    }
    if (!learner.closed) {
      for (const s of frame.standings) {
        if (s.is_player || s.in_garage || s.control === "nobody") continue;
        learner.observe(s.pos[0], s.pos[2], s.lap_fraction, s.in_pits);
      }
    }
    const p = learner.progress;
    if (Math.abs(p - progress) > 0.01 || (p === 1 && progress !== 1)) setProgress(p);
    const curSector = player?.lap.current_sector ?? 0;
    if (curSector !== sector) setSector(curSector);

    // bounds: track path when known, otherwise every car on track
    const path = learner.path();
    const b: Bounds = { minX: Infinity, maxX: -Infinity, minZ: Infinity, maxZ: -Infinity };
    for (const [x, z] of path) extend(b, x, z);
    if (!learner.closed) for (const s of frame.standings) extend(b, s.pos[0], s.pos[2]);
    if (!Number.isFinite(b.minX)) return;
    const prev = boundsRef.current;
    if (!prev || learner.closed) boundsRef.current = b;
    else {
      // only grow while learning
      boundsRef.current = {
        minX: Math.min(prev.minX, b.minX),
        maxX: Math.max(prev.maxX, b.maxX),
        minZ: Math.min(prev.minZ, b.minZ),
        maxZ: Math.max(prev.maxZ, b.maxZ),
      };
    }
    const bb = boundsRef.current;
    const pad = 16;
    const bw = Math.max(1, bb.maxX - bb.minX);
    const bh = Math.max(1, bb.maxZ - bb.minZ);
    const scale = Math.min((W - pad * 2) / bw, (H - pad * 2) / bh);
    const ox = (W - bw * scale) / 2;
    const oy = (H - bh * scale) / 2;
    // rFactor family: x east, z south → flip z so the map reads north-up
    const px = (x: number) => ox + (x - bb.minX) * scale;
    const py = (z: number) => H - (oy + (z - bb.minZ) * scale);

    // track
    if (path.length > 1) {
      ctx.lineCap = "round";
      ctx.lineJoin = "round";
      const segments = learner.segments();
      const drawSegments = () => {
        for (const pts of segments) {
          if (pts.length < 2) continue;
          ctx.beginPath();
          for (let i = 0; i < pts.length; i += 1) {
            const [x, z] = pts[i] as [number, number];
            if (i === 0) ctx.moveTo(px(x), py(z));
            else ctx.lineTo(px(x), py(z));
          }
          if (learner.closed) ctx.closePath();
          ctx.stroke();
        }
      };
      ctx.strokeStyle = COLORS.line;
      ctx.lineWidth = Math.max(5, Math.min(11, W / 40));
      drawSegments();
      ctx.strokeStyle = learner.closed ? "#3a4656" : "#2a3340";
      ctx.lineWidth = Math.max(2.5, Math.min(6, W / 80));
      drawSegments();

      // current sector, highlighted
      if (learner.closed && player) {
        const [s2, s3] = learner.sectorStarts;
        if (s2 !== null && s3 !== null) {
          const starts = [0, s2, s3];
          const ends = [s2, s3, 1];
          const idx = Math.max(0, Math.min(2, curSector - 1));
          const from = starts[idx] as number;
          const to = ends[idx] as number;
          const steps = 60;
          ctx.beginPath();
          for (let i = 0; i <= steps; i += 1) {
            const f = from + ((to - from) * i) / steps;
            const pt = learner.at(f);
            if (!pt) continue;
            if (i === 0) ctx.moveTo(px(pt[0]), py(pt[1]));
            else ctx.lineTo(px(pt[0]), py(pt[1]));
          }
          ctx.strokeStyle = "rgba(255, 180, 84, 0.45)";
          ctx.lineWidth = Math.max(2.5, Math.min(6, W / 80));
          ctx.stroke();
          // sector boundaries
          ctx.fillStyle = COLORS.dim;
          for (const f of [0, s2, s3]) {
            const pt = learner.at(f);
            if (!pt) continue;
            ctx.beginPath();
            ctx.arc(px(pt[0]), py(pt[1]), 2.2, 0, Math.PI * 2);
            ctx.fill();
          }
        }
        // start / finish line
        const sf = learner.at(0);
        const ahead = learner.at(0.004);
        if (sf && ahead) {
          const dx = px(ahead[0]) - px(sf[0]);
          const dy = py(ahead[1]) - py(sf[1]);
          const len = Math.hypot(dx, dy) || 1;
          const nx = -dy / len;
          const ny = dx / len;
          const half = Math.max(5, Math.min(9, W / 45));
          ctx.strokeStyle = COLORS.text;
          ctx.lineWidth = 2;
          ctx.beginPath();
          ctx.moveTo(px(sf[0]) - nx * half, py(sf[1]) - ny * half);
          ctx.lineTo(px(sf[0]) + nx * half, py(sf[1]) + ny * half);
          ctx.stroke();
        }
      }
    }

    // cars (opponents first, then the player on top)
    const r = Math.max(3, Math.min(5.5, W / 70));
    for (const s of frame.standings) {
      if (s.is_player || s.in_garage) continue;
      const x = px(s.pos[0]);
      const y = py(s.pos[2]);
      ctx.beginPath();
      ctx.arc(x, y, r, 0, Math.PI * 2);
      ctx.fillStyle = s.in_pits ? "rgba(147,160,174,0.5)" : classColor(s.vehicle_class);
      ctx.fill();
      ctx.lineWidth = 1.2;
      ctx.strokeStyle = COLORS.bg;
      ctx.stroke();
    }
    const me = frame.standings.find((s) => s.is_player);
    const mePos = player
      ? [player.motion.pos[0], player.motion.pos[2]]
      : me
        ? [me.pos[0], me.pos[2]]
        : null;
    if (mePos) {
      const x = px(mePos[0] as number);
      const y = py(mePos[1] as number);
      ctx.beginPath();
      ctx.arc(x, y, r * 2.4, 0, Math.PI * 2);
      ctx.fillStyle = "rgba(255, 180, 84, 0.22)";
      ctx.fill();
      ctx.beginPath();
      ctx.arc(x, y, r * 1.35, 0, Math.PI * 2);
      ctx.fillStyle = COLORS.accent;
      ctx.fill();
      ctx.lineWidth = 2;
      ctx.strokeStyle = COLORS.bg;
      ctx.stroke();
    }
  });

  const learning = progress < 1;
  return (
    <div className="map" ref={hostRef}>
      <canvas ref={canvasRef} className="map-canvas" />
      <div className="map-overlay">
        {learning ? (
          <span className="dim">
            {t("map.learning")} <span className="num">{Math.round(progress * 100)} %</span>
          </span>
        ) : sector > 0 ? (
          <span className="dim">
            {t("map.sector")} <span className="num">{sector}</span>
          </span>
        ) : null}
      </div>
    </div>
  );
}

export const mapWidget = registerWidget({
  id: "map",
  titleKey: "widget.map",
  minW: 2,
  minH: 3,
  defaultSize: { w: 3, h: 6 },
  mobileSize: { w: 4, h: 5 },
  component: MapWidget,
});
