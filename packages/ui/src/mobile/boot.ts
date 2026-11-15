import "../styles/app.css";
import { pageIndex } from "../dash/model";
import { useDashPage } from "../dash/store";
import { type TranslationKey, t } from "../i18n";
import { parseRoute } from "../nav/routes";
import { setTokenProvider } from "../net/auth";
import { connection } from "../net/connection";
import { setRaceBackend } from "../races/backend";
import { useRaces } from "../store/races";
import { useSettings } from "../store/settings";
import { useTelemetry } from "../store/telemetry";
import {
  AndroidReminders,
  backAction,
  type DebugRequest,
  debugReminder,
  parseDebugRequest,
} from "./android";
import { AppController, setController, useApp } from "./app-state";
import { phoneTextKey } from "./device";
import { userAgent } from "./env";
import { hashOfAppLink } from "./links";
import { PhoneRaces } from "./phone-races";
import { PicksSync } from "./picks-sync";
import {
  App,
  androidNotifications,
  cookies,
  http,
  Keyboard,
  KeyboardResize,
  KeyboardStyle,
  LocalNotifications,
  log,
  native,
  notifications,
  prefs,
  ScreenOrientation,
  scanQrCode,
  traits,
} from "./platform";
import { RaceBook } from "./race-book";
import { CATEGORY, isReminderExtra } from "./reminders";
import { ServersPage } from "./ServersPage";
import { PhoneSchedule } from "./schedule";
import { ServerBook } from "./servers";
import { appSlots } from "./slots";
import {
  BottomTabs,
  HomeServerCard,
  PickAction,
  PicksShare,
  ReminderStatus,
  RemindersExtra,
  startDemo,
  useServerGate,
  useServerOnline,
} from "./ui";
import { WidgetSync } from "./widget-sync";

/** « Mes courses » are exchanged with the servers this often while the application is shown. */
const PICKS_SYNC_EVERY_MS = 60_000;

/** Errors of the native side never stop the interface: they are written to the log. */
function quiet(p: Promise<unknown>, what: string): void {
  p.catch((e: unknown) => log(`${what}: ${e instanceof Error ? e.message : String(e)}`));
}

/** Portrait everywhere; the live dashboard may turn (a phone on its side next to the wheel). */
function followOrientation(): void {
  let last: boolean | null = null;
  const apply = () => {
    const live = parseRoute(window.location.hash).page === "dashboard";
    if (live === last) return;
    last = live;
    quiet(
      live ? ScreenOrientation.unlock() : ScreenOrientation.lock({ orientation: "portrait" }),
      "orientation",
    );
  };
  window.addEventListener("hashchange", apply);
  apply();
}

/**
 * The test entries of the Android Debug build (`android.ts`, `DebugHooks.java`): each one only
 * writes what happened to the log, read by the CI's smoke test.
 */
async function runDebug(r: DebugRequest, android: AndroidReminders | null): Promise<void> {
  switch (r.action) {
    case "demo": {
      if (useTelemetry.getState().demo !== "manual") startDemo();
      else window.location.hash = "#/";
      useDashPage.getState().show(pageIndex(r.page));
      if (r.orientation) {
        // After the hash change (`followOrientation` unlocks the dashboard first).
        await new Promise((ok) => setTimeout(ok, 300));
        await ScreenOrientation.lock({ orientation: r.orientation });
      }
      log(`debug: demo ${r.page}${r.orientation ? ` ${r.orientation}` : ""}`);
      return;
    }
    case "reminder": {
      if (!android) {
        log("debug: reminder (Android only)");
        return;
      }
      const style = await android.style();
      const n = debugReminder(Date.now(), r.delaySec, useSettings.getState().lang, style);
      await notifications.schedule({ notifications: [n] });
      log(
        `debug: reminder in ${r.delaySec} s (${style.exact ? "exact" : "inexact"}, channel ${n.channelId})`,
      );
      return;
    }
    case "fetch": {
      const res = await window.fetch(r.url, { headers: { Accept: "application/json" } });
      const body = (await res.text()).slice(0, 160).replace(/\s+/g, " ");
      log(`debug: fetch ${res.status} ${body}`);
      return;
    }
    case "scan": {
      const text = await scanQrCode(t("servers.add.scan"));
      log(`debug: scan ${text === null ? "cancelled" : "read"}`);
      return;
    }
  }
}

/**
 * The test entries of the Debug builds (`cordeDebug` events, sent by `DebugHooks.java` on
 * Android and by `WidgetShots.swift` on the iPhone; nothing sends them in a Release build).
 */
function installDebug(android: AndroidReminders | null): void {
  window.addEventListener("cordeDebug", (ev) => {
    const r = parseDebugRequest(ev);
    if (!r) return;
    runDebug(r, android).catch((e: unknown) =>
      log(`debug: ${r.action} failed (${e instanceof Error ? e.message : String(e)})`),
    );
  });
}

/** Android: the back button. */
function installAndroid(): void {
  quiet(
    App.addListener("backButton", ({ canGoBack }) => {
      const action = backAction(canGoBack);
      log(`back: ${action}`);
      if (action === "back") window.history.back();
      else quiet(App.exitApp(), "exit");
    }),
    "back button",
  );
}

/**
 * Start of the phone application (`main.tsx` imports this module only in the `mobile` build):
 * installs the phone's races, « Mes serveurs », the notifications and the native chrome, then
 * returns once the first render can happen. The shared connection starts after it.
 *
 * The same code runs on the iPhone and on Android; what only one of them has is decided by
 * `traits` (`device.ts`): the widgets and the Live Activity on the iPhone, the notification
 * channel, exact alarms and the back button on Android.
 */
export async function bootApp(): Promise<void> {
  document.documentElement.classList.add("is-app");
  if (traits.os === "android") document.documentElement.classList.add("is-android");
  useApp.setState({ os: traits.os });

  const lang = () => useSettings.getState().lang;
  const android = traits.channels
    ? new AndroidReminders({
        port: androidNotifications,
        prefs,
        lang,
        log,
        confirm: (text) => window.confirm(text),
        onExact: (v) => useApp.setState({ exact: v }),
      })
    : null;
  const book = new RaceBook(prefs);
  const schedule = new PhoneSchedule({ http, cookies, userAgent: userAgent(), log });
  // Widgets and Live Activity follow every change of the races (created just below).
  let widgets: WidgetSync | null = null;
  // « Mes courses » are exchanged with the servers after every change (created below).
  let picksSync: PicksSync | null = null;
  const races = new PhoneRaces({
    prefs,
    schedule,
    book,
    notifications,
    lang,
    log,
    onPermission: (p) => useApp.setState({ permission: p }),
    onChange: (what) => {
      void widgets?.update();
      if (what === "picks") {
        void widgets?.activity({ kind: "picks" });
        void picksSync?.sync();
      }
    },
    ...(android
      ? {
          androidStyle: () => android.style(),
          afterPermission: (p) => android.offerExact(p),
        }
      : {}),
  });
  if (traits.widgets) {
    widgets = new WidgetSync({
      prefs,
      native,
      picks: () => book.list(),
      settings: () => book.getSettings(),
      enabled: () => races.fetchEnabled(),
      sources: () => races.lastSources,
      lang,
      log,
      onActivity: (a) => useApp.setState({ activity: a }),
    });
  }
  const servers = new ServerBook({
    prefs,
    fetch: (u, i) => window.fetch(u, i),
    log,
    deviceName: traits.deviceName,
  });
  picksSync = new PicksSync({
    servers,
    book,
    log,
    // Picks made on another device: the pages, the reminders and the widgets follow (not
    // through `onChange` above, which would start another exchange).
    onMerged: () => {
      useRaces.getState().bumpPicks();
      void races.replan();
      void widgets?.update();
      void widgets?.activity({ kind: "picks" });
    },
    onView: (view) => useApp.setState({ picksShare: view }),
  });
  const controller = new AppController(servers, races, () => scanQrCode(t("servers.add.scan")));
  controller.widgets = widgets;
  controller.android = android;
  controller.picks = picksSync;
  setController(controller);

  // The shared interface, wired to the phone.
  setRaceBackend(races);
  useRaces.setState({ serverReminders: false });
  setTokenProvider(() => servers.activeToken());
  connection.setEndpointSource(() => controller.endpoint());
  Object.assign(appSlots, {
    BottomTabs,
    ServersPage,
    useServerGate,
    HomeServerCard,
    useServerOnline,
    ReminderStatus,
    RemindersExtra,
    PickAction,
    PicksShare,
    phoneText: (key: TranslationKey) => phoneTextKey(key, traits.os),
    // The phone dashboard keeps the screen on (both systems, no permission).
    keepAwake: async (on: boolean) => {
      try {
        await native.setKeepAwake({ on });
        log(`screen kept on: ${on}`);
      } catch (e) {
        log(`screen kept on: ${on} refused (${e instanceof Error ? e.message : String(e)})`);
      }
    },
  });

  // A reminder rung while the application is open: the banner of the interface too (the phone
  // already played its sound). A tap on a notification opens the Courses page.
  quiet(
    LocalNotifications.registerActionTypes({
      types: [
        {
          id: CATEGORY,
          actions: [{ id: "open", title: t("races.banner.view"), foreground: true }],
        },
      ],
    }),
    "notification category",
  );
  quiet(
    LocalNotifications.addListener("localNotificationReceived", (n) => {
      if (isReminderExtra(n.extra)) useRaces.getState().pushReminder(n.extra.reminder, true);
    }),
    "notifications",
  );
  quiet(
    LocalNotifications.addListener("localNotificationActionPerformed", (a) => {
      if (isReminderExtra(a.notification.extra)) window.location.hash = "#/races";
    }),
    "notifications",
  );

  // `corde://races` and friends (see `links.ts`), at launch and while running: the widgets and
  // the Live Activity open `corde://races`.
  const openLink = (url: string | undefined) => {
    const hash = url ? hashOfAppLink(url) : null;
    if (hash) {
      log(`link: ${hash}`);
      if (window.location.hash !== hash) window.location.hash = hash;
    }
  };
  quiet(
    App.getLaunchUrl().then((r) => openLink(r?.url)),
    "launch url",
  );
  quiet(
    App.addListener("appUrlOpen", (e) => openLink(e.url)),
    "app url",
  );
  quiet(
    App.addListener("appStateChange", (s) => {
      if (s.isActive) void controller.resume();
    }),
    "app state",
  );

  // Keyboard: dark, and the tab bar steps aside while it is open. (Style and resize mode are
  // iOS settings; on Android the window is resized by the insets of the keyboard.)
  if (traits.os === "ios") {
    quiet(Keyboard.setStyle({ style: KeyboardStyle.Dark }), "keyboard");
    quiet(Keyboard.setResizeMode({ mode: KeyboardResize.Native }), "keyboard");
  }
  quiet(
    Keyboard.addListener("keyboardWillShow", () => useApp.setState({ keyboard: true })),
    "keyboard",
  );
  quiet(
    Keyboard.addListener("keyboardWillHide", () => useApp.setState({ keyboard: false })),
    "keyboard",
  );
  followOrientation();
  installDebug(android);
  if (android) {
    installAndroid();
    // The channel of the reminders exists before anything is planned on it.
    await android.createChannel().catch((e: unknown) => log(`notification channel: ${String(e)}`));
  }

  // Servers: read the list now; the test of the active one runs in the background.
  await controller.load();
  void controller.start().catch((e: unknown) => log(`servers: ${String(e)}`));
  // Notifications: never asked here (at the first pick); what is pending is planned again.
  void races
    .permission()
    .then((p) => useApp.setState({ permission: p }))
    .catch(() => {});
  void races.replan();
  // « Mes courses » of the other devices: now, then every minute while the application is
  // shown (it also runs when the application comes back, `AppController.resume`).
  void picksSync.sync();
  setInterval(() => {
    if (document.visibilityState !== "hidden") void picksSync?.sync();
  }, PICKS_SYNC_EVERY_MS);
  // Widgets (iPhone): the block is written once at start (the schedule follows when the
  // Courses page or the home carousel reads it); the Live Activity is checked against the picks.
  void widgets?.update(true);
  void widgets?.activity({ kind: "foreground" });
  useSettings.subscribe((s, prev) => {
    if (s.lang === prev.lang) return;
    void widgets?.update();
    if (android) quiet(android.createChannel(), "notification channel");
  });
  log(`application started (${traits.os})`);
}
