import type { LogPort } from "./ports";
import { pickSetFromWire, type RaceBook } from "./race-book";
import type { PicksExchange, ServerBook } from "./servers";

/**
 * « Mes courses » shared with the driver's Corde servers (ADR 0019).
 *
 * The phone keeps its own picks ({@link RaceBook}); this module exchanges them with **every
 * server of « Mes serveurs »** — the PC at home, the home server at home or through the VPN —
 * with `POST /api/races/picks/sync`: the phone sends its copy, the server merges it and
 * answers its own, the phone merges the answer (`races/picks-merge.ts`, the same rule on both
 * sides). A race picked on the PC is therefore found on the phone as soon as one server that
 * knows it can be reached, and the other way round. The PC and the home server exchange their
 * copies by themselves; the phone also passes on what one server taught it to the others.
 *
 * It runs when the application starts, when it comes back to the front, after each change of
 * the picks on the phone and every minute while the application is shown (`boot.ts`). The
 * application does nothing in the background: a race picked on the PC reaches the phone — and
 * its reminders are planned — the next time the application is opened.
 *
 * A server that does not answer, is not paired or is too old is simply left out: the picks
 * stay on the phone and the exchange is tried again later. Nothing here reads a token
 * ({@link ServerBook.exchangePicks} does) and nothing but counts is written to the log.
 */
export type ExchangeOutcome = PicksExchange["kind"];

/** What the last exchange gave, per server (the Courses page shows it). */
export interface PicksSyncView {
  /** When it ended (`Date.now()`). */
  at: number;
  servers: { id: string; name: string; outcome: ExchangeOutcome }[];
}

export interface PicksSyncOptions {
  servers: Pick<ServerBook, "list" | "exchangePicks">;
  book: Pick<RaceBook, "exportSet" | "mergeSet">;
  /** The phone's picks changed because of another device: reminders, widgets, pages follow. */
  onMerged: () => void;
  /** After each exchange. */
  onView?: (view: PicksSyncView) => void;
  log?: LogPort;
  now?: () => number;
}

export class PicksSync {
  private running: Promise<PicksSyncView> | null = null;
  private again = false;
  private lastLine = "";
  private readonly now: () => number;

  constructor(private readonly opts: PicksSyncOptions) {
    this.now = opts.now ?? Date.now;
  }

  /**
   * One exchange with every server. One at a time: a call made during an exchange runs one
   * more after it, with the latest picks, and gets the same answer.
   */
  sync(): Promise<PicksSyncView> {
    if (this.running) {
      this.again = true;
      return this.running;
    }
    const run = async (): Promise<PicksSyncView> => {
      let view: PicksSyncView;
      do {
        this.again = false;
        let pass = await this.pass();
        if (pass.changed) {
          this.opts.onMerged();
          // What one server taught the phone goes to the others at once.
          if (pass.view.servers.filter((s) => s.outcome === "ok").length > 1) {
            pass = await this.pass();
            if (pass.changed) this.opts.onMerged();
          }
        }
        view = pass.view;
      } while (this.again);
      this.report(view);
      return view;
    };
    this.running = run().finally(() => {
      this.running = null;
    });
    return this.running;
  }

  private async pass(): Promise<{ view: PicksSyncView; changed: boolean }> {
    const servers = this.opts.servers.list();
    if (servers.length === 0) return { view: { at: this.now(), servers: [] }, changed: false };
    const mine = await this.opts.book.exportSet();
    const answers = await Promise.all(
      servers.map((s) =>
        this.opts.servers
          .exchangePicks(s.id, mine)
          .catch((): PicksExchange => ({ kind: "unreachable" })),
      ),
    );
    let changed = false;
    const view: PicksSyncView = { at: this.now(), servers: [] };
    for (const [i, s] of servers.entries()) {
      const answer = answers[i] ?? { kind: "unreachable" };
      let outcome: ExchangeOutcome = answer.kind;
      if (answer.kind === "ok") {
        try {
          const merged = await this.opts.book.mergeSet(pickSetFromWire(answer.body));
          if (merged.changed) changed = true;
        } catch {
          outcome = "error";
        }
      }
      view.servers.push({ id: s.id, name: s.name, outcome });
    }
    return { view, changed };
  }

  /** One line when the outcome is not the one of the last exchange (it runs every minute). */
  private report(view: PicksSyncView): void {
    this.opts.onView?.(view);
    const ok = view.servers.filter((s) => s.outcome === "ok").length;
    const line = `picks: shared with ${ok} of ${view.servers.length} server(s)`;
    if (line !== this.lastLine) {
      this.lastLine = line;
      this.opts.log?.(line);
    }
  }
}
