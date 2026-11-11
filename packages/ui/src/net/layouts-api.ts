import type { DashboardLayout } from "@corde/protocol";

/**
 * REST access to the server's layout store.
 *   GET /api/layouts            → DashboardLayout[]
 *   PUT /api/layouts            → body: DashboardLayout[] (full replacement)
 * Both calls fail silently (return null/false) when the server is unreachable: layouts are
 * always kept in localStorage first, the server copy is a convenience to share between devices.
 */
export async function fetchLayouts(
  httpBase: string,
  timeoutMs = 3000,
): Promise<DashboardLayout[] | null> {
  const ctrl = new AbortController();
  const timer = setTimeout(() => ctrl.abort(), timeoutMs);
  try {
    const res = await fetch(`${httpBase}/api/layouts`, {
      signal: ctrl.signal,
      headers: { Accept: "application/json" },
    });
    if (!res.ok) return null;
    const json: unknown = await res.json();
    if (!Array.isArray(json)) return null;
    return json.filter(isLayout);
  } catch {
    return null;
  } finally {
    clearTimeout(timer);
  }
}

export async function pushLayouts(
  httpBase: string,
  layouts: DashboardLayout[],
  timeoutMs = 3000,
): Promise<boolean> {
  const ctrl = new AbortController();
  const timer = setTimeout(() => ctrl.abort(), timeoutMs);
  try {
    const res = await fetch(`${httpBase}/api/layouts`, {
      method: "PUT",
      signal: ctrl.signal,
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(layouts),
    });
    return res.ok;
  } catch {
    return false;
  } finally {
    clearTimeout(timer);
  }
}

export function isLayout(v: unknown): v is DashboardLayout {
  if (typeof v !== "object" || v === null) return false;
  const o = v as Record<string, unknown>;
  return (
    typeof o.id === "string" &&
    typeof o.name === "string" &&
    typeof o.columns === "number" &&
    Array.isArray(o.widgets) &&
    typeof o.updated_ms === "number"
  );
}

/** Newest `updated_ms` wins per id; layouts only known on one side are kept. */
export function mergeLayouts(
  local: DashboardLayout[],
  remote: DashboardLayout[],
): DashboardLayout[] {
  const byId = new Map<string, DashboardLayout>();
  for (const l of local) byId.set(l.id, l);
  for (const r of remote) {
    const cur = byId.get(r.id);
    if (!cur || r.updated_ms > cur.updated_ms) byId.set(r.id, r);
  }
  return [...byId.values()];
}
