/**
 * Fixed-capacity circular buffer of (time, value) samples backed by typed arrays.
 * Writers push at telemetry rate; readers ask for the last N seconds as plain arrays
 * (what uPlot wants). Not reactive on purpose — widgets poll it from requestAnimationFrame.
 */
export class RingBuffer {
  private readonly t: Float64Array;
  private readonly v: Float32Array;
  private head = 0; // next write index
  private count = 0;

  constructor(public readonly capacity: number) {
    this.t = new Float64Array(capacity);
    this.v = new Float32Array(capacity);
  }

  get length(): number {
    return this.count;
  }

  push(time: number, value: number): void {
    this.t[this.head] = time;
    this.v[this.head] = value;
    this.head = (this.head + 1) % this.capacity;
    if (this.count < this.capacity) this.count += 1;
  }

  clear(): void {
    this.head = 0;
    this.count = 0;
  }

  /** Most recent sample or undefined. */
  last(): { t: number; v: number } | undefined {
    if (this.count === 0) return undefined;
    const i = (this.head - 1 + this.capacity) % this.capacity;
    return { t: this.t[i] as number, v: this.v[i] as number };
  }

  /** Samples newer than `since` (absolute time), oldest first. */
  since(since: number): { t: number[]; v: number[] } {
    const t: number[] = [];
    const v: number[] = [];
    const start = (this.head - this.count + this.capacity) % this.capacity;
    for (let k = 0; k < this.count; k += 1) {
      const i = (start + k) % this.capacity;
      const ti = this.t[i] as number;
      if (ti >= since) {
        t.push(ti);
        v.push(this.v[i] as number);
      }
    }
    return { t, v };
  }

  /** All samples, oldest first. */
  toArrays(): { t: number[]; v: number[] } {
    return this.since(Number.NEGATIVE_INFINITY);
  }
}

/** Sample buffers kept for the trace widgets (30 s at up to 60 Hz). */
export const TRACE_CAPACITY = 30 * 60;

export type TraceChannel =
  | "speed"
  | "throttle"
  | "brake"
  | "steering"
  | "gLat"
  | "gLong"
  | "rpm"
  | "tyreFL"
  | "tyreFR"
  | "tyreRL"
  | "tyreRR";

export const TRACE_CHANNELS: readonly TraceChannel[] = [
  "speed",
  "throttle",
  "brake",
  "steering",
  "gLat",
  "gLong",
  "rpm",
  "tyreFL",
  "tyreFR",
  "tyreRL",
  "tyreRR",
];

export type TraceBuffers = Record<TraceChannel, RingBuffer>;

export function createTraceBuffers(capacity = TRACE_CAPACITY): TraceBuffers {
  const out = {} as TraceBuffers;
  for (const ch of TRACE_CHANNELS) out[ch] = new RingBuffer(capacity);
  return out;
}
