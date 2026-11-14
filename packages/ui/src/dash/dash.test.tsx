import type { LapRecord, TelemetryFrame } from "@corde/protocol";
import { act, type ReactNode } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { DemoRace } from "../demo/generator";
import { parseRoute, routeHash } from "../nav/routes";
import { useSettings } from "../store/settings";
import { useTelemetry } from "../store/telemetry";
import { CoursePage } from "./CoursePage";
import { DashView } from "./DashView";
import { EnergyPage } from "./EnergyPage";
import { EMPTY_PAGE, type PageProps } from "./parts";
import { useDashPage } from "./store";
import { TimingPage } from "./TimingPage";
import { TyresPage } from "./TyresPage";
import { browserSetter, KeepAwake } from "./wake";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

function render(node: ReactNode): { div: HTMLDivElement; unmount: () => void } {
  const div = document.createElement("div");
  document.body.append(div);
  const root = createRoot(div);
  act(() => root.render(node));
  return {
    div,
    unmount: () => {
      act(() => root.unmount());
      div.remove();
    },
  };
}

/** A few minutes of the demo race: frames and completed laps, nothing invented here. */
function demo(): { frame: TelemetryFrame; laps: LapRecord[] } {
  const race = new DemoRace({ seed: 7 });
  const laps: LapRecord[] = [];
  let frame: TelemetryFrame | null = null;
  for (let i = 0; i < 2.3 * 115 * 20; i += 1) {
    const out = race.step(0.05, i % 20 === 0);
    laps.push(...out.laps);
    if (out.frame) frame = out.frame;
  }
  if (!frame) throw new Error("no frame");
  return { frame, laps };
}

const DEMO = demo();

function props(over: Partial<PageProps> = {}): PageProps {
  return {
    ...EMPTY_PAGE,
    frame: DEMO.frame,
    laps: DEMO.laps,
    standings: DEMO.frame.standings,
    ...over,
  };
}

beforeEach(() => {
  useSettings.setState({ lang: "fr", keepAwake: false });
});

afterEach(() => {
  useTelemetry.getState().resetData();
});

describe("pages without data", () => {
  it("show « — », never a made-up value", () => {
    for (const Page of [CoursePage, TyresPage, EnergyPage, TimingPage]) {
      const { div, unmount } = render(<Page {...EMPTY_PAGE} />);
      expect(div.textContent).toContain("—");
      // no digit but the sector names (S1…) and the page's own labels
      expect(div.textContent?.replace(/S[123]/g, "")).not.toMatch(/\d/);
      unmount();
    }
  });
});

describe("Course", () => {
  it("shows the position, the laps, the class and only the car's settings", () => {
    const { div, unmount } = render(<CoursePage {...props()} />);
    const p = DEMO.frame.player;
    expect(div.querySelector(".dc-pos b")?.textContent).toBe(`P${p?.place}`);
    expect(div.querySelectorAll(".drev i")).toHaveLength(15);
    // the demo car has no ABS: its cell is left out; TC and the map are there
    const labels = [...div.querySelectorAll(".dcell .dlab")].map((n) => n.textContent);
    expect(labels).toContain("TC");
    expect(labels).not.toContain("ABS");
    expect(labels).toContain("Carto");
    expect(labels).toContain("Énergie");
    unmount();
  });

  it("writes the fast values outside React and lights the shift lights", async () => {
    const frame = structuredClone(DEMO.frame);
    if (!frame.player) throw new Error("no player");
    frame.player.engine.rpm = frame.player.engine.max_rpm * 0.99;
    frame.player.engine.gear = 5;
    frame.player.motion.speed_kmh = 247.4;
    act(() => useTelemetry.getState().pushFrame(frame));
    const { div, unmount } = render(<CoursePage {...props({ frame })} />);
    await act(() => new Promise((r) => requestAnimationFrame(() => r(null))));
    expect(div.querySelector(".dgear-n")?.textContent).toBe("5");
    expect(div.querySelector(".dspeed b")?.textContent).toBe("247");
    expect(div.querySelector<HTMLElement>(".drev")?.dataset.state).toBe("shift");
    frame.player.engine.speed_limiter = true;
    act(() => useTelemetry.getState().pushFrame(structuredClone(frame)));
    await act(() => new Promise((r) => requestAnimationFrame(() => r(null))));
    expect(div.querySelector<HTMLElement>(".drev")?.dataset.state).toBe("limiter");
    expect(div.querySelector<HTMLElement>(".dgear")?.dataset.limiter).toBe("1");
    unmount();
  });

  it("colours the frame of the flags", () => {
    const { div, unmount } = render(
      <CoursePage {...props({ flags: { yellow: true, chequered: false, blue: false } })} />,
    );
    expect(div.querySelector<HTMLElement>(".dflags")?.dataset.tone).toBe("yellow");
    unmount();
  });
});

describe("the other pages", () => {
  it("Tyres: four corners with temperatures, pressure, brake, wear", () => {
    const { div, unmount } = render(<TyresPage {...props()} />);
    expect(div.querySelectorAll(".dt-card")).toHaveLength(4);
    expect(div.querySelectorAll(".dt-tread i")).toHaveLength(12);
    expect(div.textContent).toContain("kPa");
    unmount();
  });

  it("Energy: virtual energy and hybrid only when the car has them", () => {
    const { div, unmount } = render(<EnergyPage {...props()} />);
    expect(div.textContent).toContain("Énergie virtuelle");
    expect(div.textContent).toContain("Batterie");
    unmount();
    const frame = structuredClone(DEMO.frame);
    if (!frame.player) throw new Error("no player");
    frame.player.energy.virtual_energy = null;
    frame.player.energy.battery_soc = null;
    frame.player.energy.motor_state = "unavailable";
    const r = render(<EnergyPage {...props({ frame })} />);
    expect(r.div.textContent).not.toContain("Énergie virtuelle");
    expect(r.div.textContent).not.toContain("Batterie");
    r.unmount();
  });

  it("Timing: two cars ahead, the driver, two behind", () => {
    const { div, unmount } = render(<TimingPage {...props()} />);
    const around = div.querySelectorAll(".dk-around li");
    expect(around).toHaveLength(5);
    expect(around[2]?.classList.contains("is-me")).toBe(true);
    unmount();
  });
});

describe("DashView", () => {
  it("shows the demo label, four dots, and moves with the dots and the keys", () => {
    act(() => {
      useTelemetry.setState({ demo: "manual" });
      useTelemetry.getState().pushFrame(DEMO.frame);
      useDashPage.getState().show(0);
    });
    const { div, unmount } = render(<DashView />);
    expect(div.querySelector(".dash-demo")?.textContent).toBe("Démonstration");
    const dots = div.querySelectorAll<HTMLButtonElement>(".dash-dots button");
    expect(dots).toHaveLength(4);
    act(() => dots[2]?.click());
    expect(useDashPage.getState().index).toBe(2);
    act(() => {
      window.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowRight" }));
    });
    expect(useDashPage.getState().index).toBe(3);
    act(() => {
      window.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowRight" }));
    });
    expect(useDashPage.getState().index).toBe(3);
    act(() => {
      window.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowLeft" }));
    });
    expect(useDashPage.getState().index).toBe(2);
    expect(localStorage.getItem("corde.dash.page")).toBe("energy");
    unmount();
    act(() => useTelemetry.setState({ demo: "off" }));
  });

  it("no demo label on real data", () => {
    act(() => useTelemetry.getState().pushFrame(DEMO.frame));
    const { div, unmount } = render(<DashView />);
    expect(div.querySelector(".dash-demo")).toBeNull();
    unmount();
  });
});

describe("#/dash", () => {
  it("is the live dashboard, forced to the phone dashboard", () => {
    expect(parseRoute("#/dash")).toEqual({ page: "dashboard", dash: true });
    expect(routeHash({ page: "dashboard", dash: true })).toBe("#/dash");
    expect(routeHash({ page: "dashboard" })).toBe("#/");
  });
});

describe("keeping the screen on", () => {
  it("holds while wanted and visible, lets go otherwise, only on changes", async () => {
    const calls: boolean[] = [];
    const keep = new KeepAwake(async (on) => {
      calls.push(on);
    });
    keep.want(true);
    keep.want(true);
    keep.setVisible(false);
    keep.setVisible(true);
    keep.want(false);
    await Promise.resolve();
    expect(calls).toEqual([true, false, true, false]);
  });

  it("asks again after the system refused", async () => {
    let refuse = true;
    const calls: boolean[] = [];
    const keep = new KeepAwake(async (on) => {
      calls.push(on);
      if (refuse) throw new Error("refused");
    });
    keep.want(true);
    await new Promise((r) => setTimeout(r, 0));
    refuse = false;
    keep.setVisible(false);
    keep.setVisible(true);
    expect(calls).toEqual([true, true]);
  });

  it("uses navigator.wakeLock when the browser has it", async () => {
    expect(browserSetter({})).toBeNull();
    const log: string[] = [];
    const setter = browserSetter({
      wakeLock: {
        request: async () => {
          log.push("request");
          return { release: async () => void log.push("release") };
        },
      },
    });
    await setter?.(true);
    await setter?.(false);
    await setter?.(false);
    expect(log).toEqual(["request", "release"]);
  });
});
