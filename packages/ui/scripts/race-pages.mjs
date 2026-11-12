/**
 * Sample schedule pages for the screenshot scripts (screenshot-races.mjs, screenshot-home.mjs):
 * this machine cannot reach racecontrol.gg nor lmu.tools, so `GET /api/races/sources` is answered
 * in the browser with pages written here, in the format the parser reads (racecontrol.gg: the
 * format seen on the real page on 11 Nov 2026, in UTC−4; lmu.tools: the format assumed from the
 * owner's script, in UTC+1), with start times computed from the current time.
 */

const MIN = 60e3;
const HOUR = 3600e3;
const DAY = 864e5;

const MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
const DAYS = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
const pad = (n) => String(n).padStart(2, "0");
const ceilTo = (ms, step) => Math.ceil(ms / step) * step;

/** racecontrol.gg writes « 11 Nov at 4:25pm » in its own zone (UTC−4 on 11 Nov 2026). */
function rcTime(ms) {
  const d = new Date(ms - 4 * HOUR);
  const h = d.getUTCHours();
  const h12 = h % 12 === 0 ? 12 : h % 12;
  return `${d.getUTCDate()} ${MONTHS[d.getUTCMonth()]} at ${h12}:${pad(d.getUTCMinutes())}${h < 12 ? "am" : "pm"}`;
}

/**
 * `images`: give some cards a picture (`https://img.example/rc/daily/<name>.webp`, to be served
 * by the script); the Courses page pictures are left as they were.
 */
export function raceControlPage(now, { images = false } = {}) {
  const pic = (name) =>
    images ? `<img src="https://img.example/rc/daily/${name}.webp?w=800" alt="">` : "";
  const card = (level, name, duration, track, first, every, n, img = "") => {
    const times = Array.from({ length: n }, (_, i) => `<li>${rcTime(first + i * every)}</li>`);
    return `<article class="card">${img}<span>${level}</span><h3>${name}</h3>
      <div><span>Duration</span><span>${duration}</span></div>
      <div><span>Track</span><span>${track}</span></div><ul>${times.join("")}</ul></article>`;
  };
  // 9 to 14 min ahead: no 15 min reminder fires while the script runs
  const t0 = ceilTo(now + 9 * MIN, 5 * MIN);
  return `<!doctype html><html><body><section><h2>Upcoming Daily Races</h2>
    <img src="/images/lmu_logo.svg" alt="Le Mans Ultimate"><div>
    ${card("Advanced", "One Stint Sprint", "55 minutes", "Monza", t0 + 10 * MIN, 90 * MIN, 6, pic("one-stint-sprint-monza"))}
    ${card("Beginner", "LMP3 Fixed", "32 minutes", "Monza", t0, 45 * MIN, 10, pic("lmp3-fixed-monza"))}
    ${card("Intermediate", "Prototype Fixed", "44 minutes", "Circuit De Barcelona Catalunya", t0 + 5 * MIN, HOUR, 8, pic("missing-picture"))}
    ${card("Advanced", "ELMS Super 60", "1h 13 minutes", "Le Mans", t0 + 35 * MIN, 90 * MIN, 5)}
    </div></section><section><h2>Featured Community Events</h2></section></body></html>`;
}

/** lmu.tools writes « Fri 19:00 » in UTC+1 and « in 2d 21h 50m ». */
function ltWhen(ms, now) {
  const d = new Date(ms + HOUR);
  const left = Math.floor((ms - now) / MIN);
  const dd = Math.floor(left / 1440);
  const hh = Math.floor((left % 1440) / 60);
  const mm = left % 60;
  const delta = `${dd ? `${dd}d ` : ""}${hh || dd ? `${hh}h ` : ""}${mm}m`;
  return `<li><b>${DAYS[d.getUTCDay()]} ${pad(d.getUTCHours())}:${pad(d.getUTCMinutes())}</b> <span>in ${delta}</span></li>`;
}

/** Next `weekday` at `h:m` in UTC+1, after `now`. */
function nextAt(now, weekday, h, m) {
  for (let k = 0; k < 8; k++) {
    const local = new Date(now + HOUR + k * DAY);
    const t =
      Date.UTC(local.getUTCFullYear(), local.getUTCMonth(), local.getUTCDate(), h, m) - HOUR;
    if (t > now && new Date(t + HOUR).getUTCDay() === weekday) return t;
  }
  throw new Error("no day");
}

function ymd(ms) {
  const d = new Date(ms);
  return `${d.getUTCFullYear()}-${pad(d.getUTCMonth() + 1)}-${pad(d.getUTCDate())}`;
}

export function lmuToolsPage(now) {
  const race = (name, track, classes, fmt, mult, setup, starts) => `<div class="race">
    <h4>${name}</h4><p>${track}</p>${classes.map((c) => `<span>${c}</span>`).join("")}
    <dl><dt>Practice</dt><dd>${fmt[0]} min</dd><dt>Qualifying</dt><dd>${fmt[1]} min</dd><dt>Race</dt><dd>${fmt[2]} min</dd></dl>
    <p>SR ${mult}x</p><p>DR ${mult}x</p><p>${setup} setup</p>
    <ul>${starts.map((s) => ltWhen(s, now)).join("")}</ul>
    <a href="/fuel-calculator">Fuel calculator</a></div>`;
  const sat = nextAt(now + 6 * DAY, 6, 12, 0);
  const sat2 = nextAt(sat + 20 * DAY, 6, 12, 0);
  // 9 to 14 min ahead: no 15 min reminder fires while the script runs
  const t0 = ceilTo(now + 9 * MIN, 5 * MIN);
  return `<!doctype html><html><body><main>
    <h2>Special events</h2>
    <div><span>Next up</span><h3>Sebring International Raceway</h3><p>Sat ${new Date(sat).getUTCDate()} – Sun</p>
      <p>Hypercar · LMP2 · LMGT3</p><a href="/stint-planner?minutes=480">Plan this race</a>
      <a href="/poll/new?title=Sebring+1000+Miles&amp;from=${ymd(sat)}&amp;to=${ymd(sat + DAY)}">Create availability poll</a></div>
    <div><span>6 Hours</span><h3>Track TBA</h3><p>Sat ${new Date(sat2).getUTCDate()} – Sun</p><p>LMGT3</p>
      <a href="/poll/new?from=${ymd(sat2)}&amp;to=${ymd(sat2 + DAY)}">Create availability poll</a></div>
    <h2>Online races</h2>
    <h3>Weekly races</h3>
    ${race(
      "GT3 Sprint Weekly",
      "Algarve International Circuit",
      ["LMGT3"],
      [10, 10, 40],
      1.5,
      "Fixed",
      [nextAt(now, 5, 19, 0), nextAt(now, 6, 19, 0), nextAt(now, 0, 15, 0)].sort((a, b) => a - b),
    )}
    ${race(
      "Hypercar Weekly",
      "Circuit de la Sarthe",
      ["Hypercar"],
      [15, 10, 90],
      1.5,
      "Open",
      [nextAt(now, 6, 20, 0), nextAt(now, 0, 20, 0)].sort((a, b) => a - b),
    )}
    <h3>Daily races</h3>
    ${race("One Stint Sprint", "Autodromo Nazionale Monza", ["Hypercar", "LMP2"], [10, 10, 55], 1.5, "Fixed", [t0 + 10 * MIN])}
    ${race("LMP3 Fixed", "Autodromo Nazionale Monza", ["LMP3"], [5, 5, 32], 1, "Fixed", [t0])}
    <h3>Championships</h3>
    ${race(
      "RC Pro LMGT3 Championship",
      "Circuit de Spa-Francorchamps",
      ["LMGT3"],
      [15, 15, 60],
      2,
      "Open",
      [nextAt(now, 4, 20, 0), nextAt(now, 6, 14, 0)].sort((a, b) => a - b),
    )}
    <p>Source: RaceControl</p></main></body></html>`;
}
