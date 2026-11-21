/**
 * Learns the shape of the circuit from car positions: each sample lands in a bin indexed by lap
 * fraction. The player's samples drive the lap detection (first complete lap closes the loop);
 * the other cars' samples (`observe`) fill the bins faster, so the map appears within seconds
 * when there is a field on track, and the loop also closes once every bin has been seen.
 * Positions recorded in the pit lane are ignored.
 *
 * A rally stage is not a loop (`open`): it goes from a start line to a finish line. The
 * same bins are learnt; the two ends are simply not joined, and the place « 1 » is the
 * finish line, not the start line again.
 */
export class TrackLearner {
  readonly bins: number;
  private readonly xs: Float64Array;
  private readonly zs: Float64Array;
  private readonly filled: Uint8Array;
  private prevFraction: number | null = null;
  private seenFractions = 0;
  closed = false;
  /** A line from a start to a finish, not a loop. */
  open = false;
  /** Lap fraction where sector 2 and 3 start, when observed. */
  sectorStarts: [number | null, number | null] = [null, null];
  private prevSector = 0;

  constructor(bins = 480) {
    this.bins = bins;
    this.xs = new Float64Array(bins);
    this.zs = new Float64Array(bins);
    this.filled = new Uint8Array(bins);
  }

  reset(): void {
    this.filled.fill(0);
    this.prevFraction = null;
    this.seenFractions = 0;
    this.closed = false;
    this.sectorStarts = [null, null];
    this.prevSector = 0;
  }

  /** Tells whether the track is a line (a rally stage); what was learnt as the other is forgotten. */
  setOpen(open: boolean): void {
    if (open === this.open) return;
    this.open = open;
    this.reset();
  }

  /** A lap fraction brought into 0..1: around the loop, or held at the ends of a line. */
  private norm(fraction: number): number {
    if (this.open) return Math.min(Math.max(fraction, 0), 1 - 1e-9);
    return ((fraction % 1) + 1) % 1;
  }

  /** Fraction of the lap already seen, 0..1. */
  get progress(): number {
    return this.closed ? 1 : this.seenFractions / this.bins;
  }

  /** The player's own sample: fills a bin and detects the end of the first complete lap. */
  learn(x: number, z: number, fraction: number, sector: number, inPits: boolean): void {
    if (!Number.isFinite(fraction)) return;
    const f = this.norm(fraction);
    if (sector !== this.prevSector) {
      if (sector === 2 && this.sectorStarts[0] === null) this.sectorStarts[0] = f;
      if (sector === 3 && this.sectorStarts[1] === null) this.sectorStarts[1] = f;
      this.prevSector = sector;
    }
    // A stage is known once a run is over (the car is then out of the run, « in the pits »).
    if (this.open && inPits && !this.closed && this.seenFractions >= this.bins * 0.9) this.close();
    if (this.closed || inPits) {
      this.prevFraction = f;
      return;
    }
    this.fill(x, z, f);
    const wrapped = this.prevFraction !== null && this.prevFraction > 0.8 && f < 0.2;
    this.prevFraction = f;
    if (wrapped && this.seenFractions >= this.bins * 0.9) this.close();
  }

  /** Another car's sample: only fills bins. */
  observe(x: number, z: number, fraction: number, inPits: boolean): void {
    if (this.closed || inPits || !Number.isFinite(fraction)) return;
    this.fill(x, z, this.norm(fraction));
    if (this.seenFractions >= this.bins * 0.985) this.close();
  }

  private fill(x: number, z: number, f: number): void {
    const bin = Math.min(this.bins - 1, Math.floor(f * this.bins));
    if (!this.filled[bin]) {
      this.filled[bin] = 1;
      this.seenFractions += 1;
    }
    this.xs[bin] = x;
    this.zs[bin] = z;
  }

  /** Points of the learnt loop (interpolated over missing bins once closed), in lap order. */
  path(): Array<[number, number]> {
    const out: Array<[number, number]> = [];
    for (let i = 0; i < this.bins; i += 1) {
      if (this.filled[i]) out.push([this.xs[i] as number, this.zs[i] as number]);
    }
    return out;
  }

  /** Runs of consecutive known bins, so a partial track is drawn without chords across gaps. */
  segments(): Array<Array<[number, number]>> {
    if (this.closed) return [this.path()];
    const runs: Array<Array<[number, number]>> = [];
    let cur: Array<[number, number]> = [];
    for (let i = 0; i < this.bins; i += 1) {
      if (this.filled[i]) cur.push([this.xs[i] as number, this.zs[i] as number]);
      else if (cur.length) {
        runs.push(cur);
        cur = [];
      }
    }
    if (cur.length) {
      // wrap around the start line (a loop only)
      const first = runs[0];
      if (first && this.filled[0] && !this.open) runs[0] = [...cur, ...first];
      else runs.push(cur);
    }
    return runs;
  }

  /** Position on the learnt loop at a lap fraction (linear between bins). */
  at(fraction: number): [number, number] | null {
    if (!this.closed) return null;
    const f = this.norm(fraction);
    const pos = f * this.bins;
    const a = Math.floor(pos) % this.bins;
    const b = this.open ? Math.min(a + 1, this.bins - 1) : (a + 1) % this.bins;
    const t = pos - Math.floor(pos);
    return [
      (this.xs[a] as number) + ((this.xs[b] as number) - (this.xs[a] as number)) * t,
      (this.zs[a] as number) + ((this.zs[b] as number) - (this.zs[a] as number)) * t,
    ];
  }

  private close(): void {
    // fill gaps by linear interpolation between the nearest filled neighbours
    const n = this.bins;
    for (let i = 0; i < n; i += 1) {
      if (this.filled[i]) continue;
      let before = i;
      let after = i;
      let db = 0;
      let da = 0;
      while (!this.filled[before] && db < n) {
        before = (before - 1 + n) % n;
        db += 1;
      }
      while (!this.filled[after] && da < n) {
        after = (after + 1) % n;
        da += 1;
      }
      if (this.open) {
        // A line: a gap at one end takes the nearest known point, never the other end.
        if (before > i) before = after;
        if (after < i) after = before;
      }
      const t = before === after ? 0 : db / (db + da);
      this.xs[i] =
        (this.xs[before] as number) +
        ((this.xs[after] as number) - (this.xs[before] as number)) * t;
      this.zs[i] =
        (this.zs[before] as number) +
        ((this.zs[after] as number) - (this.zs[before] as number)) * t;
    }
    this.filled.fill(1);
    this.seenFractions = n;
    this.closed = true;
  }
}

/**
 * Does the game give coordinates for this car? Some games only publish those of the player's
 * car: the others then come with a position of zero.
 */
export function hasCoordinates(pos: readonly number[]): boolean {
  return (pos[0] ?? 0) !== 0 || (pos[2] ?? 0) !== 0;
}

/**
 * Where to draw a car: its own coordinates when the game gives them; otherwise the point of
 * the learnt loop where it is on the lap — nowhere while the loop is not known yet.
 */
export function carPoint(
  learner: TrackLearner,
  pos: readonly number[],
  fraction: number,
): [number, number] | null {
  if (hasCoordinates(pos)) return [pos[0] as number, pos[2] as number];
  return Number.isFinite(fraction) ? learner.at(fraction) : null;
}
