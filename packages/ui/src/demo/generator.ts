import type {
  Aero,
  Damage,
  Electronics,
  Energy,
  Engine,
  GameEvent,
  Gaps,
  Inputs,
  LapRecord,
  LapTiming,
  Motion,
  PlayerTelemetry,
  SessionInfo,
  SourceStatus,
  Standing,
  TelemetryFrame,
  VehicleClass,
  Weather,
  Wheel,
} from "@corde/protocol";
import { PROTOCOL_VERSION } from "@corde/protocol";
import { buildTrack, type SyntheticTrack } from "./track";

/** Deterministic PRNG so tests and screenshots are reproducible. */
export function mulberry32(seed: number): () => number {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

const clamp = (v: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, v));
const lerp = (a: number, b: number, t: number) => a + (b - a) * t;

interface Entry {
  driver: string;
  vehicle: string;
  cls: VehicleClass;
  className: string;
  /** Lap time multiplier versus the reference profile (1 = player's pace). */
  pace: number;
  /** Pit every N laps, on lap offset. */
  pitEvery: number;
  pitOffset: number;
}

const OPPONENTS: Entry[] = [
  {
    driver: "M. Vautrin",
    vehicle: "Porsche 963 #6",
    cls: "hypercar",
    className: "Hyper",
    pace: 0.994,
    pitEvery: 30,
    pitOffset: 11,
  },
  {
    driver: "K. Oyelaran",
    vehicle: "Ferrari 499P #51",
    cls: "hypercar",
    className: "Hyper",
    pace: 0.997,
    pitEvery: 30,
    pitOffset: 5,
  },
  {
    driver: "L. Brandt",
    vehicle: "Toyota GR010 #7",
    cls: "hypercar",
    className: "Hyper",
    pace: 1.001,
    pitEvery: 30,
    pitOffset: 17,
  },
  {
    driver: "S. Ikeda",
    vehicle: "Cadillac V-Series.R #2",
    cls: "hypercar",
    className: "Hyper",
    pace: 1.004,
    pitEvery: 30,
    pitOffset: 21,
  },
  {
    driver: "A. Ferreira",
    vehicle: "Alpine A424 #36",
    cls: "hypercar",
    className: "Hyper",
    pace: 1.008,
    pitEvery: 30,
    pitOffset: 9,
  },
  {
    driver: "J. Nowak",
    vehicle: "BMW M Hybrid V8 #20",
    cls: "hypercar",
    className: "Hyper",
    pace: 1.011,
    pitEvery: 30,
    pitOffset: 26,
  },
  {
    driver: "R. Castellanos",
    vehicle: "Peugeot 9X8 #93",
    cls: "hypercar",
    className: "Hyper",
    pace: 1.016,
    pitEvery: 30,
    pitOffset: 3,
  },
  {
    driver: "T. Lindqvist",
    vehicle: "Oreca 07 #22",
    cls: "lmp2",
    className: "LMP2",
    pace: 1.058,
    pitEvery: 26,
    pitOffset: 8,
  },
  {
    driver: "P. Marchetti",
    vehicle: "Oreca 07 #28",
    cls: "lmp2",
    className: "LMP2",
    pace: 1.062,
    pitEvery: 26,
    pitOffset: 14,
  },
  {
    driver: "D. Haugen",
    vehicle: "Oreca 07 #34",
    cls: "lmp2",
    className: "LMP2",
    pace: 1.066,
    pitEvery: 26,
    pitOffset: 20,
  },
  {
    driver: "E. Sørlie",
    vehicle: "Oreca 07 #41",
    cls: "lmp2",
    className: "LMP2",
    pace: 1.071,
    pitEvery: 26,
    pitOffset: 2,
  },
  {
    driver: "Y. Benali",
    vehicle: "Oreca 07 #65",
    cls: "lmp2",
    className: "LMP2",
    pace: 1.078,
    pitEvery: 26,
    pitOffset: 23,
  },
  {
    driver: "C. Whitaker",
    vehicle: "Porsche 911 GT3 R #91",
    cls: "gt3",
    className: "LMGT3",
    pace: 1.162,
    pitEvery: 22,
    pitOffset: 6,
  },
  {
    driver: "N. Petrov",
    vehicle: "Ferrari 296 GT3 #54",
    cls: "gt3",
    className: "LMGT3",
    pace: 1.166,
    pitEvery: 22,
    pitOffset: 12,
  },
  {
    driver: "G. Aubert",
    vehicle: "Corvette Z06 GT3.R #81",
    cls: "gt3",
    className: "LMGT3",
    pace: 1.169,
    pitEvery: 22,
    pitOffset: 18,
  },
  {
    driver: "H. Tanaka",
    vehicle: "Lexus RC F GT3 #78",
    cls: "gt3",
    className: "LMGT3",
    pace: 1.173,
    pitEvery: 22,
    pitOffset: 1,
  },
  {
    driver: "O. Mbeki",
    vehicle: "McLaren 720S GT3 Evo #59",
    cls: "gt3",
    className: "LMGT3",
    pace: 1.178,
    pitEvery: 22,
    pitOffset: 9,
  },
  {
    driver: "F. Delacroix",
    vehicle: "Aston Martin Vantage GT3 #27",
    cls: "gt3",
    className: "LMGT3",
    pace: 1.184,
    pitEvery: 22,
    pitOffset: 15,
  },
  {
    driver: "B. Kowalczyk",
    vehicle: "Ford Mustang GT3 #88",
    cls: "gt3",
    className: "LMGT3",
    pace: 1.191,
    pitEvery: 22,
    pitOffset: 4,
  },
];

const PLAYER_ENTRY: Entry = {
  driver: "Alex Martin",
  vehicle: "Porsche 963 #38",
  cls: "hypercar",
  className: "Hyper",
  pace: 1,
  pitEvery: 999,
  pitOffset: 0,
};

const PIT_STOP_S = 28;
const PIT_SPEED = 60 / 3.6;

interface CarState {
  id: number;
  entry: Entry;
  s: number;
  laps: number;
  lapStart: number;
  lastLap: number | null;
  bestLap: number | null;
  lastS1: number | null;
  lastS2: number | null;
  bestS1: number | null;
  bestS2: number | null;
  curS1: number | null;
  curS2: number | null;
  sector: number;
  speed: number;
  pitStops: number;
  pitTimer: number;
  pitPhase: "none" | "entering" | "stopped" | "exiting";
  pitLap: boolean;
  /** Per-lap smooth pace noise. */
  lapNoise: number;
  lapPhase: number;
  x: number;
  z: number;
  y: number;
}

export interface DemoOutput {
  frame?: TelemetryFrame;
  laps: LapRecord[];
  events: GameEvent[];
}

export interface DemoOptions {
  seed?: number;
  /** Session elapsed time at start, seconds. */
  sessionElapsed?: number;
  /** Session length, seconds. */
  sessionLength?: number;
  trackName?: string;
}

/**
 * Simulates a 20-car race on the synthetic track and produces complete `TelemetryFrame`s,
 * `LapRecord`s and a few `GameEvent`s. Call `step(dt)` at a fixed rate (20 Hz by default).
 */
export class DemoRace {
  readonly track: SyntheticTrack;
  readonly rand: () => number;
  private t = 0;
  private seq = 0;
  private wallBase: number;
  private readonly sessionStart: number;
  private readonly sessionLength: number;
  private readonly cars: CarState[] = [];
  private readonly player: CarState;
  private pendingEvents: GameEvent[] = [];

  // player physical state
  private tyreT: [number, number, number][] = [
    [82, 84, 85],
    [85, 84, 82],
    [80, 82, 83],
    [83, 82, 80],
  ];
  private carcass = [84, 84, 82, 82];
  private wear = [0.985, 0.984, 0.978, 0.979];
  private brakeT = [380, 380, 320, 320];
  private fuel = 61.4;
  private readonly fuelCapacity = 90;
  private virtualEnergy = 0.874;
  private soc = 0.62;
  private gear = 1;
  private prevSpeed = 0;
  private waterC = 88;
  private oilC = 104;
  private lapInvalidated = false;
  private trackLimits = 0;
  // per-lap accumulators for LapRecord
  private lapFuelStart = 61.4;
  private lapVeStart = 0.874;
  private lapTopSpeed = 0;
  private lapTyreSum = [0, 0, 0, 0];
  private lapPressSum = [0, 0, 0, 0];
  private lapBrakeMax = [0, 0, 0, 0];
  private lapSamples = 0;
  // delta to best lap by distance (5 m resolution)
  private readonly deltaRes = 5;
  private curTimes: Float32Array;
  private bestTimes: Float32Array | null = null;
  private bestLapS1: number | null = null;
  private bestLapS2: number | null = null;
  private lastS3: number | null = null;
  private bestS3: number | null = null;
  private weatherPhase: number;
  private statusTick = 0;

  constructor(opts: DemoOptions = {}) {
    this.rand = mulberry32(opts.seed ?? 7);
    this.track = buildTrack(opts.trackName);
    this.sessionStart = opts.sessionElapsed ?? 1380;
    this.sessionLength = opts.sessionLength ?? 3600;
    this.wallBase = Date.now();
    this.weatherPhase = this.rand() * Math.PI * 2;
    this.curTimes = new Float32Array(Math.ceil(this.track.length / this.deltaRes) + 1);

    const L = this.track.length;
    // player starts mid-pack; spread the field around the track by class
    this.player = this.makeCar(0, PLAYER_ENTRY, 0.62 * L, 11);
    this.cars.push(this.player);
    OPPONENTS.forEach((entry, i) => {
      const spread =
        entry.cls === "hypercar"
          ? 0.55 + i * 0.045
          : entry.cls === "lmp2"
            ? 0.2 + (i - 7) * 0.07
            : 0.75 + (i - 12) * 0.06;
      const laps = entry.cls === "hypercar" ? 11 : entry.cls === "lmp2" ? 10 : 9;
      this.cars.push(this.makeCar(i + 1, entry, (((spread % 1) + 1) % 1) * L, laps));
    });
    this.pendingEvents.push({ kind: "session_started", session: this.session() });
    this.pendingEvents.push({ kind: "entered_realtime" });
  }

  private makeCar(id: number, entry: Entry, s: number, laps: number): CarState {
    const refLap = this.track.referenceLapTime * entry.pace;
    const timeIntoLap = (s / this.track.length) * refLap;
    return {
      id,
      entry,
      s,
      laps,
      // the car is already `timeIntoLap` seconds into its lap when the session picks up
      lapStart: this.sessionStart - timeIntoLap,
      lastLap: refLap * (1 + (this.rand() - 0.5) * 0.01),
      // the player's best must come from a lap we actually timed (it carries the delta trace)
      bestLap: entry === PLAYER_ENTRY ? null : refLap * (1 - this.rand() * 0.004),
      lastS1: null,
      lastS2: null,
      bestS1: null,
      bestS2: null,
      curS1: null,
      curS2: null,
      sector: 1,
      speed: this.track.speedAt(s) / entry.pace,
      pitStops: entry.cls === "gt3" ? 1 : 0,
      pitTimer: 0,
      pitPhase: "none",
      pitLap: false,
      lapNoise: 1 + (this.rand() - 0.5) * 0.006,
      lapPhase: this.rand() * Math.PI * 2,
      x: 0,
      z: 0,
      y: 0,
    };
  }

  /** Session elapsed time, seconds. */
  get elapsed(): number {
    return this.sessionStart + this.t;
  }

  get simTime(): number {
    return this.t;
  }

  /** Current wall clock of the simulation, ms. */
  get wallMs(): number {
    return this.wallBase + this.t * 1000;
  }

  /** Re-aligns the simulated wall clock on the real one (after a silent warm-up). */
  rebaseWall(now: number = Date.now()): void {
    this.wallBase = now - this.t * 1000;
  }

  status(): SourceStatus {
    return {
      connected: true,
      game: "sim",
      source: "demo",
      input_rate_hz: 100,
      output_rate_hz: 20,
      in_realtime: true,
      game_version: "demo",
      last_error: null,
      clients: 1,
      recording: false,
    };
  }

  /** Advances the world by `dt` seconds and returns the new frame plus completed laps / events. */
  step(dt: number, emitFrame = true): DemoOutput {
    this.t += dt;
    const laps: LapRecord[] = [];
    for (const car of this.cars) {
      const completed = this.advanceCar(car, dt);
      if (completed && car === this.player) laps.push(completed);
    }
    this.updatePlayerPhysics(dt);
    const events = this.pendingEvents;
    this.pendingEvents = [];
    this.statusTick += dt;
    const out: DemoOutput = { laps, events };
    if (emitFrame) out.frame = this.frame();
    return out;
  }

  // ---------------------------------------------------------------- cars

  private advanceCar(car: CarState, dt: number): LapRecord | null {
    const L = this.track.length;
    const prevS = car.s;
    const isPlayer = car === this.player;

    // pit logic (opponents only)
    if (!isPlayer) {
      const lapNo = car.laps + 1;
      car.pitLap = lapNo % car.entry.pitEvery === car.entry.pitOffset;
      if (car.pitPhase === "none" && car.pitLap && car.s > L - 420) car.pitPhase = "entering";
      if (car.pitPhase === "entering" && car.s > L - 160) {
        car.pitPhase = "stopped";
        car.pitTimer = PIT_STOP_S;
        car.pitStops += 1;
      }
      if (car.pitPhase === "stopped") {
        car.pitTimer -= dt;
        car.speed = 0;
        if (car.pitTimer <= 0) car.pitPhase = "exiting";
        return null;
      }
      if (car.pitPhase === "exiting" && car.s > 380 && car.s < L / 2) car.pitPhase = "none";
    }

    const inPitLane = car.pitPhase !== "none";
    const base = this.track.speedAt(car.s) / car.entry.pace;
    const noise = car.lapNoise + 0.006 * Math.sin(car.s / 260 + car.lapPhase);
    let target = inPitLane ? PIT_SPEED : base / noise;
    if (isPlayer) target = this.playerTargetSpeed(base);
    // smooth speed changes a little (first-order lag) so g-forces stay continuous
    car.speed = inPitLane ? target : lerp(car.speed, target, Math.min(1, dt * 12));
    car.s += car.speed * dt;

    let record: LapRecord | null = null;
    const [f2, f3] = this.track.sectorFractions;
    const now = this.elapsed;
    const timeInto = now - car.lapStart;
    if (prevS < f2 * L && car.s >= f2 * L) {
      car.curS1 = timeInto;
      car.sector = 2;
    } else if (prevS < f3 * L && car.s >= f3 * L) {
      car.curS2 = car.curS1 !== null ? timeInto - car.curS1 : null;
      car.sector = 3;
    }
    if (car.s >= L) {
      car.s -= L;
      const lapTime = timeInto;
      const s1 = car.curS1;
      const s2 = car.curS2;
      const s3 = s1 !== null && s2 !== null ? lapTime - s1 - s2 : null;
      car.lastLap = lapTime;
      car.lastS1 = s1;
      car.lastS2 = s2;
      const valid = isPlayer ? !this.lapInvalidated : true;
      if (valid && (car.bestLap === null || lapTime < car.bestLap)) {
        car.bestLap = lapTime;
        if (isPlayer) {
          this.bestLapS1 = s1;
          this.bestLapS2 = s2;
          this.bestTimes = this.curTimes.slice();
        }
      }
      if (s1 !== null && (car.bestS1 === null || s1 < car.bestS1)) car.bestS1 = s1;
      if (s2 !== null && (car.bestS2 === null || s2 < car.bestS2)) car.bestS2 = s2;
      if (isPlayer) {
        this.lastS3 = s3;
        if (s3 !== null && (this.bestS3 === null || s3 < this.bestS3)) this.bestS3 = s3;
        // 0-based, like the game: the number of laps completed before it.
        record = this.lapRecord(car.laps, lapTime, s1, s2, s3, valid);
      }
      car.laps += 1;
      car.lapStart = now - car.s / Math.max(car.speed, 1);
      car.curS1 = null;
      car.curS2 = null;
      car.sector = 1;
      car.lapNoise = 1 + (this.rand() - 0.5) * (isPlayer ? 0.008 : 0.006);
      car.lapPhase = this.rand() * Math.PI * 2;
      if (isPlayer) this.startPlayerLap();
    }
    const p = this.track.at(car.s);
    car.x = p.x;
    car.z = p.z;
    car.y = p.y;
    if (isPlayer) {
      const idx = Math.min(this.curTimes.length - 1, Math.floor(car.s / this.deltaRes));
      this.curTimes[idx] = now - car.lapStart;
      // fill gaps left by large steps
      for (let i = idx - 1; i >= 0 && this.curTimes[i] === 0; i -= 1)
        this.curTimes[i] = now - car.lapStart;
    }
    return record;
  }

  private playerTargetSpeed(base: number): number {
    const car = this.player;
    const wearPenalty = 1 + (1 - Math.min(...this.wear)) * 0.08;
    const noise =
      car.lapNoise * wearPenalty +
      0.005 * Math.sin(car.s / 310 + car.lapPhase) +
      0.003 * Math.sin(car.s / 73);
    return base / noise;
  }

  private startPlayerLap(): void {
    this.curTimes.fill(0);
    this.lapFuelStart = this.fuel;
    this.lapVeStart = this.virtualEnergy;
    this.lapTopSpeed = 0;
    this.lapTyreSum = [0, 0, 0, 0];
    this.lapPressSum = [0, 0, 0, 0];
    this.lapBrakeMax = [0, 0, 0, 0];
    this.lapSamples = 0;
    this.lapInvalidated = false;
    // an occasional track-limits warning somewhere in the lap
    if (this.rand() < 0.18) this.trackLimits += 1;
  }

  private lapRecord(
    lapNumber: number,
    lapTime: number,
    s1: number | null,
    s2: number | null,
    s3: number | null,
    valid: boolean,
  ): LapRecord {
    const n = Math.max(1, this.lapSamples);
    const w = this.weather();
    return {
      lap_number: lapNumber,
      lap_time_s: lapTime,
      s1,
      s2,
      s3,
      valid,
      in_lap: false,
      out_lap: false,
      fuel_used_l: Math.max(0, this.lapFuelStart - this.fuel),
      virtual_energy_used: Math.max(0, this.lapVeStart - this.virtualEnergy),
      top_speed_kmh: this.lapTopSpeed,
      front_compound: "Medium",
      rear_compound: "Medium",
      avg_tyre_temp_c: [
        (this.lapTyreSum[0] as number) / n,
        (this.lapTyreSum[1] as number) / n,
        (this.lapTyreSum[2] as number) / n,
        (this.lapTyreSum[3] as number) / n,
      ],
      avg_tyre_pressure_kpa: [
        (this.lapPressSum[0] as number) / n,
        (this.lapPressSum[1] as number) / n,
        (this.lapPressSum[2] as number) / n,
        (this.lapPressSum[3] as number) / n,
      ],
      max_brake_temp_c: [
        this.lapBrakeMax[0] as number,
        this.lapBrakeMax[1] as number,
        this.lapBrakeMax[2] as number,
        this.lapBrakeMax[3] as number,
      ],
      wear_remaining_end: [
        this.wear[0] as number,
        this.wear[1] as number,
        this.wear[2] as number,
        this.wear[3] as number,
      ],
      track_c: w.track_c,
      ambient_c: w.ambient_c,
      rain: w.rain,
      session_elapsed_end_s: this.elapsed,
      timed_by_game: valid,
      note: valid ? null : "not_timed",
    };
  }

  // ---------------------------------------------------------- player physics

  private gLat = 0;
  private gLong = 0;
  private throttle = 0;
  private brake = 0;
  private steering = 0;
  private curvature = 0;
  private rpm = 6000;
  private tcActive = false;
  private regenKw = 0;

  private updatePlayerPhysics(dt: number): void {
    const car = this.player;
    const sample = this.track.at(car.s);
    const v = car.speed;
    const a = (v - this.prevSpeed) / dt;
    this.prevSpeed = v;
    const k = sample.curvature;
    this.curvature = k;
    this.gLat = (v * v * k) / 9.81;
    this.gLong = a / 9.81;

    // pedals from the longitudinal acceleration demand
    const dragComp = 0.55 * (v / 83.3) ** 2;
    if (a >= -1.2) {
      this.throttle = clamp(0.18 + a / 7.5 + dragComp + 0.02 * Math.sin(this.t * 7), 0, 1);
      this.brake = 0;
    } else {
      this.brake = clamp(-a / 19 + 0.03 * Math.sin(this.t * 11), 0, 1);
      this.throttle = 0;
    }
    // steering: Ackermann + understeer growing with lateral load
    const angle = k * 3.1 * (1 + 0.35 * Math.abs(this.gLat)) * 57.3 * 15;
    this.steering = clamp(angle / 270, -1, 1);

    // gearbox
    const kmh = v * 3.6;
    const up = [0, 92, 132, 172, 212, 252, 288, 312];
    while (this.gear < 7 && kmh > (up[this.gear] as number)) this.gear += 1;
    while (this.gear > 1 && kmh < (up[this.gear - 1] as number) * 0.78) this.gear -= 1;
    const gearTop = up[this.gear] as number;
    this.rpm = clamp(
      (kmh / gearTop) * 8400 + 300 * this.throttle * (this.gear <= 2 ? 1 : 0.2),
      2200,
      8600,
    );
    this.tcActive =
      this.throttle > 0.75 &&
      this.gear <= 3 &&
      Math.abs(this.gLat) > 0.9 &&
      Math.sin(this.t * 23) > 0.2;

    // tyres
    const loadL = Math.max(0, this.gLat); // turning right loads the left side
    const loadR = Math.max(0, -this.gLat);
    const brakeHeat = this.brake * 6;
    const spinHeat = this.tcActive ? 5 : 0;
    const speedHeat = 4 * (v / 83.3);
    for (let i = 0; i < 4; i += 1) {
      const isLeft = i === 0 || i === 2;
      const isFront = i < 2;
      const load = isLeft ? loadL : loadR;
      const base = isFront ? 86 : 83;
      const centre = base + 13 * load + (isFront ? brakeHeat : spinHeat) + speedHeat;
      // inner edge runs hotter (camber); the loaded tyre heats its outer shoulder
      const inner = centre + 4 + 2 * load;
      const outer = centre - 3 + 7 * load;
      // zones are left / centre / right of the tyre: inner = right zone on left tyres
      const target: [number, number, number] = isLeft
        ? [outer, centre, inner]
        : [inner, centre, outer];
      const cur = this.tyreT[i] as [number, number, number];
      for (let z = 0; z < 3; z += 1) {
        const tz = target[z] as number;
        cur[z] = lerp(cur[z] as number, tz, Math.min(1, dt / 2.8));
      }
      const carcass = this.carcass[i] as number;
      this.carcass[i] = lerp(carcass, (cur[1] as number) - 2, Math.min(1, dt / 18));
      const slide =
        0.6 * load + 0.8 * this.brake * (isFront ? 1 : 0.6) + (this.tcActive && !isFront ? 1 : 0);
      this.wear[i] = Math.max(
        0,
        (this.wear[i] as number) - ((0.0042 / this.track.length) * v * dt * (0.5 + slide)) / 1.2,
      );
    }

    // brakes
    for (let i = 0; i < 4; i += 1) {
      const isFront = i < 2;
      const heat = this.brake * v * v * (isFront ? 0.075 : 0.05);
      const cool = ((this.brakeT[i] as number) - 150) * (0.006 + 0.0005 * v);
      this.brakeT[i] = clamp((this.brakeT[i] as number) + (heat - cool) * dt, 150, 780);
    }

    // energy
    const fuelRate = this.throttle * (0.55 + 0.45 * (this.rpm / 8600)) * 0.0575;
    this.fuel = Math.max(0, this.fuel - fuelRate * dt);
    this.virtualEnergy = Math.max(0, this.virtualEnergy - fuelRate * dt * 0.0062);
    const deploying = this.throttle > 0.5 && kmh > 190 && this.soc > 0.12;
    const regen = this.brake > 0.1;
    this.regenKw = regen ? this.brake * 190 : 0;
    this.soc = clamp(
      this.soc + (regen ? 0.0032 * this.brake : 0) * dt - (deploying ? 0.0048 : 0) * dt,
      0.08,
      0.96,
    );

    // fluids
    this.waterC = lerp(
      this.waterC,
      86 + 6 * this.throttle + 2 * (1 - v / 83.3),
      Math.min(1, dt / 20),
    );
    this.oilC = lerp(this.oilC, 101 + 9 * this.throttle, Math.min(1, dt / 30));

    // lap accumulators
    this.lapTopSpeed = Math.max(this.lapTopSpeed, kmh);
    for (let i = 0; i < 4; i += 1) {
      const z = this.tyreT[i] as [number, number, number];
      this.lapTyreSum[i] = (this.lapTyreSum[i] as number) + (z[0] + z[1] + z[2]) / 3;
      this.lapPressSum[i] = (this.lapPressSum[i] as number) + this.pressure(i);
      this.lapBrakeMax[i] = Math.max(this.lapBrakeMax[i] as number, this.brakeT[i] as number);
    }
    this.lapSamples += 1;

    // track limits: invalidate the lap once in a while, in a fast corner
    if (
      !this.lapInvalidated &&
      this.trackLimits > 0 &&
      this.rand() < 0.0004 &&
      Math.abs(this.gLat) > 1.6
    ) {
      this.lapInvalidated = true;
      this.trackLimits = 0;
    }
  }

  private pressure(i: number): number {
    return 150 + 0.4 * (this.carcass[i] as number) + (i < 2 ? 2 : 0);
  }

  // ------------------------------------------------------------- builders

  private weather(): Weather {
    const tod = 16 * 3600 + 38 * 60 + this.elapsed;
    const cloud = 0.25 + 0.15 * Math.sin(this.t / 400 + this.weatherPhase);
    return {
      ambient_c: 23.4 - 0.0006 * this.t,
      track_c: 35.2 - 0.0012 * this.t + 0.6 * Math.sin(this.t / 90),
      rain: 0,
      wetness_min: 0,
      wetness_max: 0,
      wetness_avg: 0,
      cloud_darkness: cloud,
      cloud_coverage: cloud * 10,
      track_grip_level: 3,
      wind_ms: [0, 0, 0],
      time_of_day_s: tod,
    };
  }

  private session(): SessionInfo {
    const elapsed = this.elapsed;
    return {
      game: "sim",
      track_name: this.track.name,
      track_length_m: this.track.length,
      session_type: "race",
      session_index: 10,
      phase: "green",
      yellow_flag: "none",
      sector_flags: [0, 0, 0],
      elapsed_s: elapsed,
      end_s: this.sessionLength,
      time_remaining_s: Math.max(0, this.sessionLength - elapsed),
      max_laps: 0,
      num_vehicles: this.cars.length,
      in_realtime: true,
      is_fixed_setup: false,
      player_name: PLAYER_ENTRY.driver,
      server_name: "Corde — course de démonstration",
      is_multiplayer: false,
      weather: this.weather(),
    };
  }

  private standings(): { list: Standing[]; byId: Map<number, Standing> } {
    const L = this.track.length;
    const ordered = [...this.cars].sort((a, b) => b.laps + b.s / L - (a.laps + a.s / L));
    const leader = ordered[0] as CarState;
    const classCount = new Map<string, number>();
    const list: Standing[] = [];
    const byId = new Map<number, Standing>();
    const progress = (c: CarState) => c.laps + c.s / L;
    ordered.forEach((car, idx) => {
      const refLap = this.track.referenceLapTime * car.entry.pace;
      const next = idx > 0 ? (ordered[idx - 1] as CarState) : null;
      const dNext = next ? progress(next) - progress(car) : 0;
      const dLeader = progress(leader) - progress(car);
      const classPlace = (classCount.get(car.entry.className) ?? 0) + 1;
      classCount.set(car.entry.className, classPlace);
      const isPlayer = car === this.player;
      const timeInto = this.elapsed - car.lapStart;
      // blue flag: a faster-class car less than 2 s behind on track
      const behind = this.cars.find(
        (o) =>
          o !== car &&
          o.entry.pace < car.entry.pace - 0.03 &&
          (car.s - o.s + L) % L < car.speed * 2,
      );
      const st: Standing = {
        id: car.id,
        driver_name: car.entry.driver,
        vehicle_name: car.entry.vehicle,
        class_name: car.entry.className,
        vehicle_class: car.entry.cls,
        is_player: isPlayer,
        control: isPlayer ? "local_player" : "local_ai",
        place: idx + 1,
        class_place: classPlace,
        laps_completed: car.laps,
        sector: car.sector,
        finish_status: "none",
        lap_dist_m: car.s,
        lap_fraction: car.s / L,
        path_lateral_m: 1.2 * Math.sin(car.s / 90 + car.id),
        in_pits: car.pitPhase !== "none",
        pit_state: car.pitPhase === "none" ? "none" : car.pitPhase,
        in_garage: false,
        num_pitstops: car.pitStops,
        num_penalties: 0,
        time_behind_next_s: (dNext % 1) * refLap,
        laps_behind_next: Math.floor(dNext),
        time_behind_leader_s: (dLeader % 1) * refLap,
        laps_behind_leader: Math.floor(dLeader),
        best_lap_s: car.bestLap,
        last_lap_s: car.lastLap,
        best_s1: car.bestS1,
        best_s2: car.bestS2,
        last_s1: car.lastS1,
        last_s2: car.lastS2,
        cur_s1: car.curS1,
        cur_s2: car.curS2,
        time_into_lap_s: timeInto,
        estimated_lap_s: car.bestLap ?? refLap,
        pos: [car.x, car.y, car.z],
        speed_kmh: car.speed * 3.6,
        fuel_fraction: isPlayer ? this.fuel / this.fuelCapacity : null,
        flag_blue: Boolean(behind) && !isPlayer,
        under_yellow: false,
        headlights: true,
        drs: false,
        qualification: car.id + 1,
        pit_lap_dist_m: L - 160,
      };
      list.push(st);
      byId.set(car.id, st);
    });
    return { list, byId };
  }

  private gaps(standing: Standing, byId: Map<number, Standing>, list: Standing[]): Gaps {
    const L = this.track.length;
    const me = this.player;
    const refLap = this.track.referenceLapTime;
    // nearest cars on track regardless of class
    let aheadDist = Number.POSITIVE_INFINITY;
    let behindDist = Number.POSITIVE_INFINITY;
    for (const c of this.cars) {
      if (c === me || c.pitPhase === "stopped") continue;
      const dAhead = (c.s - me.s + L) % L;
      const dBehind = (me.s - c.s + L) % L;
      aheadDist = Math.min(aheadDist, dAhead);
      behindDist = Math.min(behindDist, dBehind);
    }
    const toTime = (m: number) => (Number.isFinite(m) ? (m / L) * refLap : null);
    const placeAhead = list.find((s) => s.place === standing.place - 1);
    const placeBehind = list.find((s) => s.place === standing.place + 1);
    const placeGap = (other: Standing | undefined) => {
      if (!other) return null;
      const o = byId.get(other.id);
      if (!o) return null;
      const d = Math.abs(
        o.laps_behind_leader +
          o.time_behind_leader_s / refLap -
          (standing.laps_behind_leader + standing.time_behind_leader_s / refLap),
      );
      return d * refLap;
    };
    return {
      car_ahead_s: toTime(aheadDist),
      car_behind_s: toTime(behindDist),
      place_ahead_s: placeGap(placeAhead),
      place_behind_s: placeGap(placeBehind),
    };
  }

  private wheel(i: number, v: number): Wheel {
    const temps = this.tyreT[i] as [number, number, number];
    const isFront = i < 2;
    const isLeft = i === 0 || i === 2;
    const load = isLeft ? Math.max(0, this.gLat) : Math.max(0, -this.gLat);
    const radius = isFront ? 0.345 : 0.365;
    const staticLoad = isFront ? 2600 : 2900;
    const downforce = 9000 * (v / 83.3) ** 2;
    const loadN =
      staticLoad +
      downforce / 4 +
      2400 * load -
      1200 * (isLeft ? Math.max(0, -this.gLat) : Math.max(0, this.gLat));
    const slipRatio =
      this.brake > 0.7
        ? -0.04 - 0.05 * this.brake * (isFront ? 1 : 0.4)
        : this.tcActive && !isFront
          ? 0.08
          : 0.01 * this.throttle;
    return {
      susp_deflection_m: 0.018 + 0.012 * (loadN / 6000),
      ride_height_m: (isFront ? 0.044 : 0.052) - 0.014 * (v / 83.3) ** 2,
      susp_force_n: loadN * 0.92,
      brake_temp_c: this.brakeT[i] as number,
      brake_pressure: this.brake * (isFront ? 0.56 : 0.44),
      rotation_rads: v / radius,
      camber_deg: isFront ? -3.1 : -2.0,
      toe_deg: isFront ? -0.08 : 0.12,
      lateral_patch_vel: 0.3 * load,
      longitudinal_patch_vel: slipRatio * v,
      lateral_ground_vel: this.gLat * 0.4,
      longitudinal_ground_vel: v,
      lateral_force_n: loadN * clamp(this.gLat, -2.5, 2.5) * 0.9,
      longitudinal_force_n: loadN * clamp(this.gLong, -2.5, 2.5) * 0.6,
      load_n: loadN,
      grip_fract: clamp(1 - 0.35 * load - 0.4 * this.brake, 0.3, 1),
      pressure_kpa: this.pressure(i),
      temp_c: [temps[0], temps[1], temps[2]],
      temp_inner_c: [temps[0] * 0.92 + 8, temps[1] * 0.92 + 8, temps[2] * 0.92 + 8],
      carcass_c: this.carcass[i] as number,
      optimal_temp_c: 90,
      wear_remaining: this.wear[i] as number,
      surface: "dry",
      terrain: "asphalt",
      flat: false,
      detached: false,
      compound_index: 1,
      compound_type: "medium",
      radius_m: radius,
      vertical_deflection_m: 0.009 + 0.004 * (loadN / 6000),
      slip_ratio: slipRatio,
      slip_angle_deg: this.gLat * (isFront ? 3.2 : 2.4),
    };
  }

  private playerTelemetry(
    standing: Standing,
    byId: Map<number, Standing>,
    list: Standing[],
  ): PlayerTelemetry {
    const car = this.player;
    const v = car.speed;
    const kmh = v * 3.6;
    const L = this.track.length;
    const now = this.elapsed;
    const timeInto = now - car.lapStart;
    const idx = Math.min(this.curTimes.length - 1, Math.floor(car.s / this.deltaRes));
    const bestAtDist = this.bestTimes ? (this.bestTimes[idx] as number) : 0;
    const delta = this.bestTimes && bestAtDist > 0 ? timeInto - bestAtDist : 0;

    const lap: LapTiming = {
      lap_number: car.laps,
      laps_completed: car.laps,
      lap_start_s: car.lapStart,
      current_lap_s: timeInto,
      current_sector: car.sector,
      in_pit_lane: false,
      lap_invalidated: this.lapInvalidated,
      delta_best_s: delta,
      last_lap_s: car.lastLap,
      best_lap_s: car.bestLap,
      last_s1: car.lastS1,
      last_s2: car.lastS2,
      last_s3: this.lastS3,
      best_s1: car.bestS1,
      best_s2: car.bestS2,
      best_s3: this.bestS3,
      cur_s1: car.curS1,
      cur_s2: car.curS2,
      best_lap_s1: this.bestLapS1,
      best_lap_s2: this.bestLapS2,
      estimated_lap_s: (car.bestLap ?? this.track.referenceLapTime) + delta,
      time_into_lap_s: timeInto,
      lap_dist_m: car.s,
      lap_fraction: car.s / L,
      count_lap_flag: this.lapInvalidated ? 1 : 2,
    };

    const sample = this.track.at(car.s);
    const motion: Motion = {
      pos: [car.x, car.y, car.z],
      local_vel: [0, 0, -v],
      local_accel: [-this.gLat * 9.81, 0, -this.gLong * 9.81],
      local_rot: [0, -v * this.curvature, 0],
      speed_ms: v,
      speed_kmh: kmh,
      g_lat: this.gLat,
      g_long: this.gLong,
      g_vert: 1 + 0.03 * Math.sin(this.t * 17) + 0.02 * Math.sin(this.t * 41),
      yaw_rate_dps: v * this.curvature * 57.3,
      heading_rad: sample.heading,
    };

    const engine: Engine = {
      rpm: this.rpm,
      max_rpm: 8600,
      gear: this.gear,
      max_gears: 7,
      water_c: this.waterC,
      oil_c: this.oilC,
      torque_nm: 620 * this.throttle,
      turbo_boost: 1.4 + 0.9 * this.throttle,
      clutch_rpm: this.rpm,
      ignition_starter: 1,
      speed_limiter: false,
      speed_limiter_available: true,
      anti_stall: false,
      overheating: false,
    };

    const inputs: Inputs = {
      throttle: this.tcActive ? this.throttle * 0.82 : this.throttle,
      brake: this.brake,
      steering: this.steering,
      clutch: 0,
      throttle_raw: this.throttle,
      brake_raw: this.brake,
      steering_raw: this.steering,
      clutch_raw: 0,
      steering_torque_nm: 4.5 * this.gLat + 0.3 * Math.sin(this.t * 31),
      steering_angle_deg: this.steering * 270,
      wheel_range_deg: 540,
      ffb_torque: clamp(this.gLat / 2.5, -1, 1),
    };

    const q = (v / 83.3) ** 2;
    const aero: Aero = {
      front_ride_height_mm: 44 - 13 * q,
      rear_ride_height_mm: 52 - 15 * q,
      front_wing_height_mm: 22,
      front_downforce_n: 7400 * q,
      rear_downforce_n: 9800 * q,
      drag_n: 4300 * q,
      third_front_mm: 6 * q,
      third_rear_mm: 8 * q,
      front_flap_active: false,
      rear_flap_active: false,
      rear_flap_legal: 0,
    };

    const energy: Energy = {
      fuel_l: this.fuel,
      fuel_capacity_l: this.fuelCapacity,
      fuel_fraction: this.fuel / this.fuelCapacity,
      virtual_energy: this.virtualEnergy,
      battery_soc: this.soc,
      regen_kw: this.regenKw,
      motor_torque_nm:
        this.regenKw > 0 ? -210 * this.brake : this.throttle > 0.5 && kmh > 190 ? 180 : 0,
      motor_rpm: v * 130,
      motor_temp_c: 62 + 8 * this.soc,
      motor_water_c: 54,
      motor_state:
        this.regenKw > 0
          ? "regeneration"
          : this.throttle > 0.5 && kmh > 190 && this.soc > 0.12
            ? "propulsion"
            : "inactive",
      scheduled_stops: 2,
    };

    const electronics: Electronics = {
      tc: 5,
      tc_max: 11,
      tc_slip: 6,
      tc_slip_max: 11,
      tc_cut: 4,
      tc_cut_max: 11,
      abs: 0,
      abs_max: 0,
      motor_map: 3,
      motor_map_max: 6,
      brake_migration: 3,
      brake_migration_max: 5,
      arb_front: 3,
      arb_front_max: 6,
      arb_rear: 4,
      arb_rear_max: 6,
      brake_bias_rear: 0.442,
      tc_active: this.tcActive,
      abs_active: false,
      lift_and_coast: 0,
      wiper: 0,
      headlights: true,
    };

    const damage: Damage = {
      dents: [0, 0, 0, 0, 0, 0, 0, 0],
      detached_parts: false,
      last_impact_s: 0,
      last_impact_magnitude: 0,
      last_impact_pos: [0, 0, 0],
    };

    return {
      id: car.id,
      vehicle_name: PLAYER_ENTRY.vehicle,
      vehicle_model: "Porsche 963",
      vehicle_class: PLAYER_ENTRY.cls,
      class_name: PLAYER_ENTRY.className,
      championship: "WEC",
      place: standing.place,
      lap,
      motion,
      engine,
      inputs,
      aero,
      energy,
      electronics,
      wheels: [this.wheel(0, v), this.wheel(1, v), this.wheel(2, v), this.wheel(3, v)],
      damage,
      gaps: this.gaps(standing, byId, list),
      tyres: {
        front_compound: "Medium",
        rear_compound: "Medium",
        front_compound_index: 1,
        rear_compound_index: 1,
      },
      track_limits_steps: this.trackLimits,
      in_pits: false,
      pit_state: "none",
      in_garage: false,
      finish_status: "none",
    };
  }

  frame(): TelemetryFrame {
    this.seq += 1;
    const { list, byId } = this.standings();
    const me = byId.get(this.player.id) as Standing;
    return {
      v: PROTOCOL_VERSION,
      seq: this.seq,
      wall_ms: Math.round(this.wallMs),
      source: "demo",
      session: this.session(),
      player: this.playerTelemetry(me, byId, list),
      standings: list,
    };
  }
}
