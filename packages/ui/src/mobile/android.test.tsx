import { act, type ReactNode } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, describe, expect, it } from "vitest";
import { en } from "../i18n/en";
import { fr } from "../i18n/fr";
import type { RacePick } from "../net/races-api";
import { useSettings } from "../store/settings";
import {
  AndroidReminders,
  backAction,
  DEBUG_REMINDER_ID,
  debugReminder,
  parseDebugRequest,
} from "./android";
import { AppController, setController, useApp } from "./app-state";
import { phoneOs, phoneTextKey, traitsOf } from "./device";
import { FakeAndroidNotifications, FakeHttp, FakeNotifications, FakePrefs, page } from "./doubles";
import { KEYS } from "./kv";
import { PhoneRaces } from "./phone-races";
import { RaceBook } from "./race-book";
import {
  ANDROID_SOUND,
  CHANNEL,
  cancelAllReminders,
  isReminderExtra,
  notificationOf,
  planReminders,
  SMALL_ICON,
  SOUND,
  syncReminders,
} from "./reminders";
import { PhoneSchedule } from "./schedule";
import { ServerBook } from "./servers";
import { NeedServer, PickAction, ReminderStatus, RemindersExtra } from "./ui";

const NOW = 1_762_891_800_000;
const MIN = 60_000;

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

/** The markup of `node`, rendered in the page (the stores' current state, not the initial one). */
function renderToStaticMarkup(node: ReactNode): string {
  const div = document.createElement("div");
  const root = createRoot(div);
  act(() => root.render(node));
  const html = div.innerHTML;
  act(() => root.unmount());
  return html;
}

function pick(race: string, start: number): RacePick {
  return {
    id: `${race}@${start}`,
    race_id: race,
    cat: "daily",
    src: "rc",
    name: "GT3 Sprint",
    level: "intermediate",
    track: "Spa",
    duration: "30 min",
    image: null,
    start_ms: start,
    eve_ms: null,
  };
}

function androidReminders(confirmAnswer = true) {
  const port = new FakeAndroidNotifications();
  const prefs = new FakePrefs();
  const confirms: string[] = [];
  const exacts: string[] = [];
  let lang: "fr" | "en" = "fr";
  const android = new AndroidReminders({
    port,
    prefs,
    lang: () => lang,
    confirm: (text) => {
      confirms.push(text);
      return confirmAnswer;
    },
    onExact: (v) => exacts.push(v),
  });
  return { port, prefs, android, confirms, exacts, setLang: (l: "fr" | "en") => (lang = l) };
}

describe("the phone the application runs on", () => {
  it("is the iPhone unless Capacitor says Android", () => {
    expect(phoneOs("android")).toBe("android");
    expect(phoneOs("ios")).toBe("ios");
    expect(phoneOs("web")).toBe("ios");
    const ios = traitsOf("ios");
    expect(ios).toMatchObject({ widgets: true, channels: false, localNetworkPrompt: true });
    expect(ios.backButton).toBe(false);
    const android = traitsOf("android");
    // CordeNative (widgets, Live Activity, pictures, diagnosis) only exists on the iPhone.
    expect(android).toMatchObject({ widgets: false, channels: true, localNetworkPrompt: false });
    expect(android).toMatchObject({ backButton: true, deviceName: "Android" });
  });

  it("names the phone's own settings: Android versions of the iPhone sentences", () => {
    for (const key of [
      "races.reminders.hintPhone",
      "mobile.notif.denied",
      "mobile.notif.prompt",
    ] as const) {
      expect(phoneTextKey(key, "ios")).toBe(key);
      const a = phoneTextKey(key, "android");
      expect(a).toBe(`${key}.android`);
      expect(fr[a]).not.toMatch(/iPhone|iOS/);
      expect(en[a]).not.toMatch(/iPhone|iOS/);
    }
    expect(phoneTextKey("mobile.notif.granted", "android")).toBe("mobile.notif.granted");
  });
});

describe("reminders as Android notifications", () => {
  const plan = () => planReminders([pick("a", NOW + 60 * MIN)], [15, 5], NOW);

  it("leaves the iPhone notification exactly as it was", () => {
    const [p] = plan();
    if (!p) throw new Error("no reminder");
    const n = notificationOf(p, "fr");
    expect(Object.keys(n).sort()).toEqual(
      [
        "actionTypeId",
        "body",
        "extra",
        "id",
        "schedule",
        "sound",
        "threadIdentifier",
        "title",
      ].sort(),
    );
    expect(n.sound).toBe(SOUND);
    expect(Object.keys(n.schedule ?? {})).toEqual(["at"]);
    expect(Object.keys(n.extra as object)).not.toContain("exact");
  });

  it("uses the channel, the small icon, the raw sound, and exact alarms only when allowed", () => {
    const [p] = plan();
    if (!p) throw new Error("no reminder");
    const n = notificationOf(p, "fr", { exact: true });
    expect(n).toMatchObject({
      channelId: CHANNEL,
      smallIcon: SMALL_ICON,
      sound: ANDROID_SOUND,
      isExactNotification: true,
      schedule: { allowWhileIdle: true },
    });
    expect(n.threadIdentifier).toBeUndefined();
    expect(isReminderExtra(n.extra)).toBe(true);
    // Never « exact » without the permission: the plugin would open the settings by itself.
    expect(notificationOf(p, "fr", { exact: false }).isExactNotification).toBe(false);
    // A raw resource name has no `-`.
    expect(ANDROID_SOUND).toMatch(/^[a-z0-9_]+\.wav$/);
  });

  it("schedules again when exact alarms are allowed or withdrawn", async () => {
    const n = new FakeNotifications();
    n.permission = "granted";
    let r = await syncReminders(n, plan(), "fr", undefined, {
      android: { exact: false },
      now: NOW,
    });
    expect(r).toMatchObject({ scheduled: 2, cancelled: 0 });
    r = await syncReminders(n, plan(), "fr", undefined, { android: { exact: false }, now: NOW });
    expect(r).toMatchObject({ scheduled: 0, kept: 2 });
    r = await syncReminders(n, plan(), "fr", undefined, { android: { exact: true }, now: NOW });
    expect(r).toMatchObject({ scheduled: 2, cancelled: 2, kept: 0 });
    expect(n.pending.every((p) => p.isExactNotification === true)).toBe(true);
  });

  it("leaves the reminders already delivered alone (Android keeps them in its list)", async () => {
    const n = new FakeNotifications();
    n.permission = "granted";
    const old = planReminders([pick("old", NOW - 10 * MIN)], [5], NOW - 30 * MIN);
    await syncReminders(n, old, "fr", undefined, { android: { exact: true }, now: NOW - 30 * MIN });
    expect(n.pending).toHaveLength(1);
    const r = await syncReminders(n, plan(), "fr", undefined, {
      android: { exact: true },
      now: NOW,
    });
    expect(r).toMatchObject({ scheduled: 2, cancelled: 0 });
    expect(n.cancelled).toHaveLength(0);
  });
});

describe("the reminders' channel and exact alarms", () => {
  it("creates the channel with Corde's sound, in the language of the interface", async () => {
    const { port, android, setLang } = androidReminders();
    await android.createChannel();
    expect(port.channels).toEqual([
      expect.objectContaining({
        id: CHANNEL,
        name: "Rappels des courses",
        sound: ANDROID_SOUND,
        importance: 4,
        vibration: true,
      }),
    ]);
    setLang("en");
    await android.createChannel();
    expect(port.channels).toHaveLength(1);
    expect(port.channels[0]?.name).toBe("Race reminders");
  });

  it("reads exact alarms, and reports only the changes", async () => {
    const { port, android, exacts } = androidReminders();
    expect(await android.style()).toEqual({ exact: false });
    expect(await android.exact()).toBe("denied");
    port.exact = "granted";
    expect(await android.style()).toEqual({ exact: true });
    expect(exacts).toEqual(["denied", "granted"]);
    // Unknown (an old plugin, an error): inexact, never the settings screen.
    port.failCheck = true;
    expect(await android.exact()).toBe("denied");
  });

  it("offers « Alarmes et rappels » once, after the notifications are allowed", async () => {
    const { port, prefs, android, confirms } = androidReminders(true);
    await android.offerExact("denied");
    await android.offerExact("prompt");
    expect(confirms).toHaveLength(0);
    await android.offerExact("granted");
    expect(confirms).toHaveLength(1);
    expect(confirms[0]).toContain("Alarmes et rappels");
    expect(port.screens).toBe(1);
    expect(prefs.data.get(KEYS.exactAsked)).toBe("true");
    port.exact = "denied";
    await android.offerExact("granted");
    expect(confirms).toHaveLength(1);
    expect(port.screens).toBe(1);
  });

  it("does not open the screen when the driver says no, nor when already allowed", async () => {
    const no = androidReminders(false);
    await no.android.offerExact("granted");
    expect(no.confirms).toHaveLength(1);
    expect(no.port.screens).toBe(0);
    const yes = androidReminders(true);
    yes.port.exact = "granted";
    await yes.android.offerExact("granted");
    expect(yes.confirms).toHaveLength(0);
  });
});

describe("the phone's races on Android", () => {
  function setup(android: AndroidReminders | null) {
    const prefs = new FakePrefs();
    const notifications = new FakeNotifications();
    const after: string[] = [];
    const races = new PhoneRaces({
      prefs,
      schedule: new PhoneSchedule({
        http: new FakeHttp(() => page("")),
        userAgent: "Corde/test",
        now: () => NOW,
      }),
      book: new RaceBook(prefs, () => NOW),
      notifications,
      lang: () => "fr",
      now: () => NOW,
      ...(android
        ? {
            androidStyle: () => android.style(),
            afterPermission: async (p) => {
              after.push(p);
              await android.offerExact(p);
            },
          }
        : {}),
    });
    return { races, notifications, after };
  }

  it("plans the reminders on the channel, and offers exact alarms after the first pick", async () => {
    const a = androidReminders(false);
    const { races, notifications, after } = setup(a.android);
    await races.savePick(pick("a", NOW + 60 * MIN));
    await races.replan();
    expect(after).toEqual(["granted"]);
    expect(a.confirms).toHaveLength(1);
    expect(notifications.pending.length).toBeGreaterThan(0);
    expect(notifications.pending.every((n) => n.channelId === CHANNEL)).toBe(true);
    expect(notifications.pending.every((n) => n.isExactNotification === false)).toBe(true);
    // Allowed later (Paramètres, then back to Corde): planned again as exact alarms.
    a.port.exact = "granted";
    await races.replan();
    expect(notifications.pending.every((n) => n.isExactNotification === true)).toBe(true);
  });

  it("changes nothing on the iPhone", async () => {
    const { races, notifications, after } = setup(null);
    await races.savePick(pick("a", NOW + 60 * MIN));
    await races.replan();
    expect(after).toEqual([]);
    expect(notifications.pending.length).toBeGreaterThan(0);
    expect(notifications.pending.every((n) => n.channelId === undefined)).toBe(true);
    expect(notifications.pending.every((n) => n.sound === SOUND)).toBe(true);
  });
});

describe("the back button and the smoke test's entry", () => {
  it("goes back in the interface, and leaves from the first page", () => {
    expect(backAction(true)).toBe("back");
    expect(backAction(false)).toBe("exit");
  });

  it("reads only the three requests of the Debug build", () => {
    expect(parseDebugRequest({ action: "reminder", delaySec: 60 })).toEqual({
      action: "reminder",
      delaySec: 60,
    });
    expect(parseDebugRequest({ detail: { action: "reminder", delaySec: 2 } })).toEqual({
      action: "reminder",
      delaySec: 10,
    });
    expect(parseDebugRequest({ action: "reminder", delaySec: 99999 })).toMatchObject({
      delaySec: 600,
    });
    expect(parseDebugRequest({ action: "fetch", url: "http://10.0.2.2:8471/api/health" })).toEqual({
      action: "fetch",
      url: "http://10.0.2.2:8471/api/health",
    });
    expect(parseDebugRequest({ action: "fetch", url: "https://example.com/" })).toBeNull();
    expect(parseDebugRequest({ action: "fetch", url: "javascript:alert(1)" })).toBeNull();
    expect(parseDebugRequest({ action: "scan" })).toEqual({ action: "scan" });
    expect(parseDebugRequest({ action: "pick" })).toBeNull();
    expect(parseDebugRequest(null)).toBeNull();
  });

  it("builds a test reminder the planning never cancels", async () => {
    const n = debugReminder(NOW, 60, "fr", { exact: true });
    expect(n).toMatchObject({
      id: DEBUG_REMINDER_ID,
      channelId: CHANNEL,
      isExactNotification: true,
      title: "C'est l'heure du départ",
    });
    expect(n.schedule?.at?.getTime()).toBe(NOW + 60_000);
    expect(isReminderExtra(n.extra)).toBe(false);
    const notifications = new FakeNotifications();
    notifications.permission = "granted";
    await notifications.schedule({ notifications: [n] });
    expect(await cancelAllReminders(notifications)).toBe(0);
    expect(notifications.pending).toHaveLength(1);
  });
});

describe("the application's screens on Android", () => {
  afterEach(() => {
    useApp.setState({ os: "ios", exact: null, permission: null, activity: null });
  });

  function controller(android: AndroidReminders | null) {
    const prefs = new FakePrefs();
    const races = new PhoneRaces({
      prefs,
      schedule: new PhoneSchedule({ http: new FakeHttp(() => page("")), userAgent: "Corde/test" }),
      book: new RaceBook(prefs),
      notifications: new FakeNotifications(),
      lang: () => "fr",
    });
    const c = new AppController(new ServerBook({ prefs, fetch: fetch }), races, null);
    c.android = android;
    // `widgets` stays null, as boot.ts leaves it on Android.
    setController(c);
    return c;
  }

  it("shows no widget setting, no countdown button, no widget diagnosis", () => {
    useSettings.setState({ lang: "fr" });
    controller(androidReminders().android);
    useApp.setState({
      os: "android",
      activity: { allowed: true, pickId: null, enabled: true },
    });
    expect(renderToStaticMarkup(<RemindersExtra />)).toBe("");
    expect(
      renderToStaticMarkup(<PickAction pick={pick("a", Date.now() + MIN)} now={Date.now()} />),
    ).toBe("");
  });

  it("explains the notifications and exact alarms with Android's words", () => {
    useSettings.setState({ lang: "fr" });
    controller(androidReminders().android);
    useApp.setState({ os: "android", permission: "denied", exact: null });
    let html = renderToStaticMarkup(<ReminderStatus />);
    expect(html).toContain("Paramètres du téléphone");
    expect(html).not.toContain("iPhone");
    useApp.setState({ permission: "granted", exact: "denied" });
    html = renderToStaticMarkup(<ReminderStatus />);
    expect(html).toContain("Alarmes et rappels");
    expect(html).toContain("Autoriser");
    useApp.setState({ exact: "granted" });
    expect(renderToStaticMarkup(<ReminderStatus />)).toContain("minute près");
  });

  it("keeps the iPhone's words and its local-network advice on the iPhone", () => {
    useSettings.setState({ lang: "fr" });
    controller(null);
    useApp.setState({ os: "ios", permission: "denied", exact: null });
    const html = renderToStaticMarkup(<ReminderStatus />);
    expect(html).toContain("Réglages de l'iPhone");
    expect(html).not.toContain("Alarmes et rappels");
    expect(renderToStaticMarkup(<NeedServer status="unreachable" />)).toContain("Réseau local");
    useApp.setState({ os: "android" });
    expect(renderToStaticMarkup(<NeedServer status="unreachable" />)).not.toContain("Réseau local");
  });
});
