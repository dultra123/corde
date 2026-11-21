import type { GameId } from "@corde/protocol";
import { BUILTIN_ID } from "../dashdef/model";
import { useDashes } from "../dashdef/store";
import { useT } from "../i18n";
import { gameName, PROFILE_GAMES } from "../lib/games";
import { useDashboard } from "../store/dashboard";
import { useGameProfiles } from "../store/game-profiles";
import { gameLayoutId } from "./defaults";

/**
 * « Profils par jeu » — what Corde shows for each game it reads (`store/game-profiles`): the
 * layout of widgets of this kind of screen and the race dashboard. Like « Application », these
 * act at once: they are not part of what « Enregistrer » saves.
 */
export function GameProfilesSection() {
  const { t } = useT();
  const enabled = useGameProfiles((s) => s.enabled);
  const profiles = useGameProfiles((s) => s.profiles);
  const active = useGameProfiles((s) => s.active);
  const { setEnabled, choose, reset } = useGameProfiles.getState();
  const device = useDashboard((s) => s.device);
  const layouts = useDashboard((s) => s.layouts);
  const current = useDashboard((s) => s.current);
  const dashes = useDashes((s) => s.defs);
  const dashSelected = useDashes((s) => s.selected);
  // The layouts of this kind of screen (anything that is neither a phone's nor an overlay is
  // a desktop's, as in the store).
  const mine = layouts.filter(
    (l) => (l.device === "mobile" || l.device === "overlay" ? l.device : "desktop") === device,
  );

  const onReset = (game: GameId) => {
    const made = layouts.some((l) => l.id === gameLayoutId(game, device));
    // Le Mans Ultimate goes back to « Course », which is not rewritten: nothing to lose.
    if (game !== "lmu" && made && !window.confirm(t("profiles.resetConfirm"))) return;
    reset(game);
  };

  return (
    <>
      <div className="sheet-section">{t("profiles.section")}</div>
      <div className="field is-switches">
        <label className="switch is-wrap">
          <input
            className="switch-toggle"
            type="checkbox"
            checked={enabled}
            onChange={(e) => setEnabled(e.target.checked)}
          />
          {t("profiles.enable")}
        </label>
        <span className="hint">{t("profiles.hint")}</span>
      </div>
      {enabled &&
        PROFILE_GAMES.map((game) => {
          const profile = profiles[game];
          // Before a first other game, Le Mans Ultimate's profile is what is shown.
          const wanted = profile?.[device] ?? (game === "lmu" ? current[device] : undefined);
          const layoutId = mine.some((l) => l.id === wanted) ? (wanted ?? "") : "";
          const dashWanted = profile?.dash ?? (game === "lmu" ? dashSelected : BUILTIN_ID);
          const dashId = dashes.some((d) => d.id === dashWanted) ? dashWanted : BUILTIN_ID;
          return (
            <fieldset className="profile-game" key={game}>
              <legend>
                {gameName(game)}
                {active === game && <span className="profile-on">{t("profiles.inPlace")}</span>}
              </legend>
              <div className="profile-fields">
                <div className="field">
                  <label htmlFor={`profile-layout-${game}`}>{t("profiles.layout")}</label>
                  <select
                    id={`profile-layout-${game}`}
                    className="select"
                    value={layoutId}
                    onChange={(e) =>
                      choose(game, { [device]: e.target.value || gameLayoutId(game, device) })
                    }
                  >
                    {layoutId === "" && (
                      <option value="" disabled={game === "lmu"}>
                        {game === "lmu" ? "—" : t("profiles.proposed")}
                      </option>
                    )}
                    {mine.map((l) => (
                      <option key={l.id} value={l.id}>
                        {l.name}
                      </option>
                    ))}
                  </select>
                </div>
                <div className="field">
                  <label htmlFor={`profile-dash-${game}`}>{t("profiles.dash")}</label>
                  <select
                    id={`profile-dash-${game}`}
                    className="select"
                    value={dashId}
                    onChange={(e) => choose(game, { dash: e.target.value })}
                  >
                    <option value={BUILTIN_ID}>{t("dash.corde")}</option>
                    {dashes.map((d) => (
                      <option key={d.id} value={d.id}>
                        {d.name}
                      </option>
                    ))}
                  </select>
                </div>
              </div>
              <button type="button" className="btn profile-reset" onClick={() => onReset(game)}>
                {t("profiles.reset")}
              </button>
            </fieldset>
          );
        })}
    </>
  );
}
