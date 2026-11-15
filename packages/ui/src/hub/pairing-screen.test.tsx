import { act, type ReactNode } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, describe, expect, it } from "vitest";
import { pairCommand } from "../net/hub-api";
import { useHub } from "../store/hub";
import { useSettings } from "../store/settings";
import { PairingScreen } from "./PairingScreen";

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

describe("the door of the home server", () => {
  afterEach(() => {
    useHub.setState({ container: false });
  });

  it("names the command of a server set up by the installer", () => {
    useSettings.setState({ lang: "fr" });
    useHub.setState({ role: "hub", paired: false, container: false });
    const { div, unmount } = render(<PairingScreen />);
    expect(div.querySelector(".hub-cmd")?.textContent).toBe("sudo corde-server pair");
    expect(div.textContent).toContain("Sur le serveur, lance :");
    expect(div.textContent).toContain("« sudo corde-server revoke » sur le serveur");
    unmount();
  });

  it("names the command of a server in a container: its terminal, no sudo", () => {
    useSettings.setState({ lang: "fr" });
    useHub.setState({ role: "hub", paired: false, container: true });
    const { div, unmount } = render(<PairingScreen />);
    expect(div.querySelector(".hub-cmd")?.textContent).toBe("corde-server pair");
    expect(div.textContent).toContain("dans le terminal du conteneur");
    expect(div.textContent).not.toContain("sudo");
    unmount();
  });

  it("keeps sudo for a server that does not say where it runs (before 1.5.0)", () => {
    expect(pairCommand(false)).toBe("sudo corde-server pair");
    expect(pairCommand(true)).toBe("corde-server pair");
  });
});
