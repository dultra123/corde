/** Measures how many events per second arrive, over a sliding window. */
export class RateMeter {
  private stamps: number[] = [];

  constructor(private readonly windowMs = 2000) {}

  tick(now: number = performance.now()): void {
    this.stamps.push(now);
    this.prune(now);
  }

  /** Events per second over the window; 0 when nothing arrived recently. */
  hz(now: number = performance.now()): number {
    this.prune(now);
    const n = this.stamps.length;
    if (n < 2) return n === 1 && now - (this.stamps[0] as number) < 1000 ? 1 : 0;
    const first = this.stamps[0] as number;
    const span = now - first;
    if (span <= 0) return 0;
    return ((n - 1) * 1000) / span;
  }

  reset(): void {
    this.stamps = [];
  }

  private prune(now: number): void {
    const cutoff = now - this.windowMs;
    let drop = 0;
    while (drop < this.stamps.length && (this.stamps[drop] as number) < cutoff) drop += 1;
    if (drop > 0) this.stamps.splice(0, drop);
  }
}
