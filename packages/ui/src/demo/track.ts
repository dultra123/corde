/**
 * Synthetic closed circuit in the spirit of Paul Ricard: a long straight (Mistral), a very fast
 * right-hander (Signes), a double-apex right (Beausset), a chicane and a slow final complex.
 * Everything is in metres on the (x, z) ground plane; y is elevation.
 */

export interface TrackSample {
  /** Distance from the start line along the centre line, metres. */
  s: number;
  x: number;
  z: number;
  /** Elevation, metres. */
  y: number;
  /** Heading of travel, radians (atan2(dz, dx)). */
  heading: number;
  /** Signed curvature, 1/m. Positive = turning right. */
  curvature: number;
  /** Target speed from the simplified vehicle model, m/s. */
  speed: number;
}

export interface SyntheticTrack {
  name: string;
  length: number;
  samples: TrackSample[];
  /** Lap fraction where sector 2 and sector 3 start. */
  sectorFractions: [number, number];
  /** Lap time of the reference speed profile, seconds. */
  referenceLapTime: number;
  /** Point lookup by distance (wraps). */
  at(s: number): TrackSample;
  /** Interpolated target speed at distance s. */
  speedAt(s: number): number;
}

// Control points, clockwise. The start/finish line is on the pit straight, heading west.
const CONTROL_POINTS: Array<[number, number]> = [
  [620, 0],
  [150, 0],
  [-200, 20],
  [-420, 140],
  [-470, 330],
  [-360, 470],
  [-430, 620],
  [-380, 800],
  [-200, 900],
  [40, 960],
  [700, 1010],
  [1400, 1030],
  [1900, 990],
  [2110, 820],
  [2090, 560],
  [1900, 400],
  [1660, 390],
  [1470, 300],
  [1340, 160],
  [1160, 210],
  [980, 160],
  [820, 60],
];

const TARGET_LENGTH_M = 5800;
const TARGET_LAP_S = 115;
const SAMPLE_STEP_M = 2;

const G = 9.81;
const A_LAT_MAX = 2.25 * G;
const A_BRAKE = 1.9 * G;
const V_MIN = 80 / 3.6;
const V_MAX = 300 / 3.6;

function catmullRom(p0: number, p1: number, p2: number, p3: number, t: number): number {
  const t2 = t * t;
  const t3 = t2 * t;
  return (
    0.5 *
    (2 * p1 +
      (-p0 + p2) * t +
      (2 * p0 - 5 * p1 + 4 * p2 - p3) * t2 +
      (-p0 + 3 * p1 - 3 * p2 + p3) * t3)
  );
}

/** Power-limited acceleration, m/s², for a ~1 030 kg / 500 kW prototype. */
function accelLimit(v: number): number {
  return Math.min(7.5, 480_000 / (1030 * Math.max(v, 10)));
}

function buildPolyline(points: Array<[number, number]>, scale: number): Array<[number, number]> {
  const n = points.length;
  const out: Array<[number, number]> = [];
  for (let i = 0; i < n; i += 1) {
    const p0 = points[(i - 1 + n) % n] as [number, number];
    const p1 = points[i] as [number, number];
    const p2 = points[(i + 1) % n] as [number, number];
    const p3 = points[(i + 2) % n] as [number, number];
    const segLen = Math.hypot(p2[0] - p1[0], p2[1] - p1[1]) * scale;
    const steps = Math.max(4, Math.ceil(segLen / SAMPLE_STEP_M));
    for (let k = 0; k < steps; k += 1) {
      const t = k / steps;
      out.push([
        catmullRom(p0[0], p1[0], p2[0], p3[0], t) * scale,
        catmullRom(p0[1], p1[1], p2[1], p3[1], t) * scale,
      ]);
    }
  }
  return out;
}

function polylineLength(pts: Array<[number, number]>): number {
  let len = 0;
  for (let i = 0; i < pts.length; i += 1) {
    const a = pts[i] as [number, number];
    const b = pts[(i + 1) % pts.length] as [number, number];
    len += Math.hypot(b[0] - a[0], b[1] - a[1]);
  }
  return len;
}

export function buildTrack(name = "Circuit du Castellet (démo)"): SyntheticTrack {
  const rawLen = polylineLength(buildPolyline(CONTROL_POINTS, 1));
  const scale = TARGET_LENGTH_M / rawLen;
  const pts = buildPolyline(CONTROL_POINTS, scale);
  const n = pts.length;

  // distances and headings
  const s = new Float64Array(n);
  const heading = new Float64Array(n);
  let acc = 0;
  for (let i = 0; i < n; i += 1) {
    const a = pts[i] as [number, number];
    const b = pts[(i + 1) % n] as [number, number];
    s[i] = acc;
    heading[i] = Math.atan2(b[1] - a[1], b[0] - a[0]);
    acc += Math.hypot(b[0] - a[0], b[1] - a[1]);
  }
  const length = acc;

  // curvature = d(heading)/ds, smoothed over ±8 samples (~16 m)
  const curv = new Float64Array(n);
  for (let i = 0; i < n; i += 1) {
    const h0 = heading[(i - 1 + n) % n] as number;
    const h1 = heading[(i + 1) % n] as number;
    let dh = h1 - h0;
    while (dh > Math.PI) dh -= 2 * Math.PI;
    while (dh < -Math.PI) dh += 2 * Math.PI;
    const ds = Math.max(1e-3, segmentLength(pts, i - 1) + segmentLength(pts, i));
    // heading increases counter-clockwise (left); positive curvature = right turn
    curv[i] = -dh / ds;
  }
  const smooth = new Float64Array(n);
  const W = 8;
  for (let i = 0; i < n; i += 1) {
    let sum = 0;
    for (let k = -W; k <= W; k += 1) sum += curv[(i + k + n) % n] as number;
    smooth[i] = sum / (2 * W + 1);
  }

  // speed profile: cornering limit, then forward (power) and backward (braking) passes.
  // A single "grip" multiplier scales the lateral, braking and power limits and is bisected
  // until the reference lap lands on TARGET_LAP_S while keeping the 80–300 km/h envelope.
  const v = new Float64Array(n);
  const computeProfile = (grip: number): number => {
    for (let i = 0; i < n; i += 1) {
      const k = Math.abs(smooth[i] as number);
      v[i] = k < 1e-6 ? V_MAX : Math.min(V_MAX, Math.max(V_MIN, Math.sqrt((A_LAT_MAX * grip) / k)));
    }
    for (let pass = 0; pass < 3; pass += 1) {
      for (let i = 0; i < n; i += 1) {
        const j = (i + 1) % n;
        const ds = segmentLength(pts, i);
        const vi = v[i] as number;
        v[j] = Math.min(v[j] as number, Math.sqrt(vi * vi + 2 * accelLimit(vi) * grip * ds));
      }
      for (let i = n - 1; i >= 0; i -= 1) {
        const j = (i + 1) % n;
        const ds = segmentLength(pts, i);
        const vj = v[j] as number;
        v[i] = Math.min(v[i] as number, Math.sqrt(vj * vj + 2 * A_BRAKE * grip * ds));
      }
    }
    let t = 0;
    for (let i = 0; i < n; i += 1) t += segmentLength(pts, i) / (v[i] as number);
    return t;
  };
  let lo = 0.2;
  let hi = 1.5;
  let lapTime = computeProfile(1);
  for (let iter = 0; iter < 24 && Math.abs(lapTime - TARGET_LAP_S) > 0.01; iter += 1) {
    const mid = (lo + hi) / 2;
    lapTime = computeProfile(mid);
    if (lapTime > TARGET_LAP_S) lo = mid;
    else hi = mid;
  }

  const samples: TrackSample[] = [];
  for (let i = 0; i < n; i += 1) {
    const p = pts[i] as [number, number];
    samples.push({
      s: s[i] as number,
      x: p[0],
      z: p[1],
      y:
        12 +
        6 * Math.sin(((s[i] as number) / length) * Math.PI * 2) +
        3 * Math.sin(((s[i] as number) / length) * Math.PI * 6),
      heading: heading[i] as number,
      curvature: smooth[i] as number,
      speed: v[i] as number,
    });
  }

  const wrap = (d: number) => ((d % length) + length) % length;
  const indexAt = (d: number) => {
    const w = wrap(d);
    // samples are ~uniform: binary search on s
    let lo = 0;
    let hi = n - 1;
    while (lo < hi) {
      const mid = (lo + hi + 1) >> 1;
      if ((samples[mid] as TrackSample).s <= w) lo = mid;
      else hi = mid - 1;
    }
    return lo;
  };

  return {
    name,
    length,
    samples,
    sectorFractions: [0.34, 0.69],
    referenceLapTime: lapTime,
    at(d) {
      return samples[indexAt(d)] as TrackSample;
    },
    speedAt(d) {
      const i = indexAt(d);
      const a = samples[i] as TrackSample;
      const b = samples[(i + 1) % n] as TrackSample;
      const segLen = i === n - 1 ? length - a.s : b.s - a.s;
      const t = segLen > 0 ? (wrap(d) - a.s) / segLen : 0;
      return a.speed + (b.speed - a.speed) * Math.min(1, Math.max(0, t));
    },
  };

  function segmentLength(p: Array<[number, number]>, i: number): number {
    const a = p[((i % p.length) + p.length) % p.length] as [number, number];
    const b = p[(((i + 1) % p.length) + p.length) % p.length] as [number, number];
    return Math.hypot(b[0] - a[0], b[1] - a[1]);
  }
}
