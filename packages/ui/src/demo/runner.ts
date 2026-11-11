import type { GameEvent, LapRecord, SourceStatus, TelemetryFrame } from "@corde/protocol";
import { RateMeter } from "../net/rate";
import { type DemoOptions, DemoRace } from "./generator";

export interface DemoSink {
  onFrame(frame: TelemetryFrame): void;
  onLap(lap: LapRecord): void;
  onEvent(event: GameEvent): void;
  onStatus(status: SourceStatus): void;
}

const STEP_S = 0.05; // 20 Hz
const MAX_CATCHUP_STEPS = 4;

/**
 * Drives a `DemoRace` on a wall-clock timer and feeds a sink (the telemetry store).
 * The race is warmed up silently for two full laps first so that the dashboard opens with a
 * best lap, a live delta and some lap history instead of empty widgets.
 */
export class DemoRunner {
  readonly race: DemoRace;
  private timer: ReturnType<typeof setInterval> | null = null;
  private last = 0;
  private readonly meter = new RateMeter(2000);

  constructor(
    private readonly sink: DemoSink,
    opts: DemoOptions = {},
  ) {
    this.race = new DemoRace(opts);
  }

  get hz(): number {
    return this.meter.hz();
  }

  get running(): boolean {
    return this.timer !== null;
  }

  start(warmupLaps = 2): void {
    if (this.timer) return;
    const warm = this.warmUp(warmupLaps);
    this.race.rebaseWall();
    this.sink.onStatus(this.race.status());
    for (const lap of warm) this.sink.onLap(lap);
    this.last = performance.now();
    this.meter.reset();
    this.timer = setInterval(() => this.tick(), STEP_S * 1000);
    this.tick();
  }

  stop(): void {
    if (this.timer) clearInterval(this.timer);
    this.timer = null;
  }

  /** Runs the simulation without emitting frames until `laps` complete laps have been recorded. */
  private warmUp(laps: number): LapRecord[] {
    const out: LapRecord[] = [];
    let first = true;
    // hard cap: 4 laps worth of steps, whatever happens
    for (let i = 0; i < (4 * 120) / STEP_S && out.length < laps; i += 1) {
      const res = this.race.step(STEP_S, false);
      for (const lap of res.laps) {
        // the first lap started mid-way: its fuel/tyre averages are partial, drop it
        if (first) {
          first = false;
          continue;
        }
        out.push(lap);
      }
    }
    return out;
  }

  private tick(): void {
    const now = performance.now();
    let steps = Math.floor((now - this.last) / (STEP_S * 1000));
    if (steps <= 0) return;
    if (steps > MAX_CATCHUP_STEPS) {
      // tab was hidden: don't replay minutes of race, just resume
      steps = 1;
      this.last = now;
    } else {
      this.last += steps * STEP_S * 1000;
    }
    for (let i = 0; i < steps; i += 1) {
      const res = this.race.step(STEP_S, i === steps - 1);
      for (const lap of res.laps) this.sink.onLap(lap);
      for (const ev of res.events) this.sink.onEvent(ev);
      if (res.frame) {
        this.meter.tick(now);
        this.sink.onFrame(res.frame);
      }
    }
  }
}
