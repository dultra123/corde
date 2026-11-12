import "../styles/app.css";
import { t } from "../i18n";
import { parseRoute } from "../nav/routes";
import { setTokenProvider } from "../net/auth";
import { connection } from "../net/connection";
import { setRaceBackend } from "../races/backend";
import { useRaces } from "../store/races";
import { useSettings } from "../store/settings";
import { AppController, setController, useApp } from "./app-state";
import { userAgent } from "./env";
import { hashOfAppLink } from "./links";
import { PhoneRaces } from "./phone-races";
import {
  App,
  cookies,
  http,
  Keyboard,
  KeyboardResize,
  KeyboardStyle,
  LocalNotifications,
  log,
  notifications,
  prefs,
  ScreenOrientation,
  scanQrCode,
} from "./platform";
import { RaceBook } from "./race-book";
import { CATEGORY, isReminderExtra } from "./reminders";
import { ServersPage } from "./ServersPage";
import { PhoneSchedule } from "./schedule";
import { ServerBook } from "./servers";
import { appSlots } from "./slots";
import { BottomTabs, HomeServerCard, ReminderStatus, useServerGate, useServerOnline } from "./ui";

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
 * Start of the iPhone application (`main.tsx` imports this module only in the `mobile` build):
 * installs the phone's races, « Mes serveurs », the notifications and the native chrome, then
 * returns once the first render can happen. The shared connection starts after it.
 */
export async function bootApp(): Promise<void> {
  document.documentElement.classList.add("is-app");

  const book = new RaceBook(prefs);
  const schedule = new PhoneSchedule({ http, cookies, userAgent: userAgent(), log });
  const races = new PhoneRaces({
    prefs,
    schedule,
    book,
    notifications,
    lang: () => useSettings.getState().lang,
    log,
    onPermission: (p) => useApp.setState({ permission: p }),
  });
  const servers = new ServerBook({ prefs, fetch: (u, i) => window.fetch(u, i), log });
  const controller = new AppController(servers, races, () => scanQrCode(t("servers.add.scan")));
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

  // `corde://races` and friends (see `links.ts`), at launch and while running.
  const openLink = (url: string | undefined) => {
    const hash = url ? hashOfAppLink(url) : null;
    if (hash) window.location.hash = hash;
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

  // Keyboard: dark, and the tab bar steps aside while it is open.
  quiet(Keyboard.setStyle({ style: KeyboardStyle.Dark }), "keyboard");
  quiet(Keyboard.setResizeMode({ mode: KeyboardResize.Native }), "keyboard");
  quiet(
    Keyboard.addListener("keyboardWillShow", () => useApp.setState({ keyboard: true })),
    "keyboard",
  );
  quiet(
    Keyboard.addListener("keyboardWillHide", () => useApp.setState({ keyboard: false })),
    "keyboard",
  );
  followOrientation();

  // Servers: read the list now; the test of the active one runs in the background.
  await controller.load();
  void controller.start().catch((e: unknown) => log(`servers: ${String(e)}`));
  // Notifications: never asked here (at the first pick); what is pending is planned again.
  void races
    .permission()
    .then((p) => useApp.setState({ permission: p }))
    .catch(() => {});
  void races.replan();
  log("application started");
}
