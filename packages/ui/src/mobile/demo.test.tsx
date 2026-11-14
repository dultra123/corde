import { act, type ReactNode } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, describe, expect, it } from "vitest";
import { connection } from "../net/connection";
import { useSettings } from "../store/settings";
import { useTelemetry } from "../store/telemetry";
import { useApp } from "./app-state";
import { DemoOffer, NeedServer, useServerGate } from "./ui";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

function markup(node: ReactNode): string {
  const div = document.createElement("div");
  const root = createRoot(div);
  act(() => root.render(node));
  const html = div.innerHTML;
  act(() => root.unmount());
  return html;
}

function Gate({ page }: { page: "dashboard" | "sessions" }) {
  const gate = useServerGate({ page });
  return <>{gate === null ? "open" : "gated"}</>;
}

afterEach(() => {
  useTelemetry.setState({ demo: "off", socket: "idle" });
  useApp.setState({ status: "none" });
});

describe("the demonstration in the application", () => {
  it("is offered on the « no server » screen", () => {
    useSettings.setState({ lang: "fr" });
    const html = markup(<NeedServer status="none" />);
    expect(html).toContain("Voir une démonstration");
    expect(html).toContain("rien n'est envoyé");
  });

  it("offers to leave once it runs", () => {
    useSettings.setState({ lang: "fr" });
    useTelemetry.setState({ demo: "manual" });
    const html = markup(<DemoOffer />);
    expect(html).toContain("Quitter la démonstration");
    expect(html).not.toContain("Voir une démonstration");
  });

  it("opens the live dashboard without a server, and only it", () => {
    useApp.setState({ status: "none" });
    expect(markup(<Gate page="dashboard" />)).toBe("gated");
    useTelemetry.setState({ demo: "manual" });
    expect(markup(<Gate page="dashboard" />)).toBe("open");
    expect(markup(<Gate page="sessions" />)).toBe("gated");
  });

  it("is never mixed with a server that comes back: no connection while it runs", () => {
    useTelemetry.setState({ demo: "manual", socket: "closed" });
    connection.connect();
    expect(useTelemetry.getState().socket).toBe("closed");
  });
});
