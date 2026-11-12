/**
 * « This device is not (or no longer) paired with the home server »: any API call answered 401
 * says so (`request` in `sessions-api`), and the home-server store (`store/hub`) shows the
 * pairing screen. Kept apart so that the network layer needs no store.
 */
type Listener = () => void;

const listeners = new Set<Listener>();

export function onUnauthorized(cb: Listener): () => void {
  listeners.add(cb);
  return () => listeners.delete(cb);
}

export function notifyUnauthorized(): void {
  for (const cb of listeners) cb();
}
