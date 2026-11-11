import type {
  AnalysisReport,
  Feedback,
  FeedbackInterpretation,
  NewFeedback,
  NewSetupFiles,
  PlanIteration,
  SetupFilesCreated,
  SetupLibrary,
  SetupPhilosophy,
  SetupPreview,
} from "@corde/protocol";
import { request } from "./sessions-api";

/**
 * REST access to the race engineer (`crates/corde-engineer`, served by corde-server):
 *
 *   GET  /api/sessions/{id}/analysis             AnalysisReport — computed on demand (a few
 *                                                seconds the first time), then cached;
 *                                                404 unknown session, 422 not enough clean laps
 *                                                (`error` is a French sentence)
 *   POST /api/sessions/{id}/analysis/recompute   drop the cache, compute again → AnalysisReport
 *   GET  /api/sessions/{id}/feedback             Feedback[] (oldest first, interpretation joined)
 *   POST /api/sessions/{id}/feedback             NewFeedback → 201 {feedback, interpretation,
 *                                                iteration} · 400 empty / > 2000 chars
 *   GET  /api/sessions/{id}/iterations?plan=     PlanIteration[] (oldest first)
 *   GET  /api/setups?session=                    SetupLibrary — the driver's .svm files for the
 *                                                session's car and the base proposed
 *   GET  /api/sessions/{id}/setup-preview?base=&plan=   SetupPreview (nothing written)
 *   POST /api/sessions/{id}/setup-files          NewSetupFiles → 201 SetupFilesCreated (writes
 *                                                Corde_<Plan>_<base>.svm in the game's folder)
 *
 * `base` (`Folder/File.svm`) picks the setup the plans are resolved against; without it the
 * server uses the base it proposes.
 *
 * The analysis can take a while on a long session: the timeout is generous. Posting a feedback
 * may compute the analysis too, so it shares that timeout.
 */

const ANALYSIS_TIMEOUT_MS = 120_000;

/** `?base=…` (or `&base=…`) when a base was chosen, `""` otherwise. */
export function baseQuery(base: string | null | undefined, sep: "?" | "&" = "?"): string {
  return base ? `${sep}base=${encodeURIComponent(base)}` : "";
}

export function fetchAnalysis(id: string, signal?: AbortSignal, base?: string | null) {
  return request<AnalysisReport>(
    `/api/sessions/${encodeURIComponent(id)}/analysis${baseQuery(base)}`,
    { signal },
    ANALYSIS_TIMEOUT_MS,
  );
}

export function recomputeAnalysis(id: string, signal?: AbortSignal, base?: string | null) {
  return request<AnalysisReport>(
    `/api/sessions/${encodeURIComponent(id)}/analysis/recompute${baseQuery(base)}`,
    { method: "POST", signal },
    ANALYSIS_TIMEOUT_MS,
  );
}

export function fetchFeedback(id: string, signal?: AbortSignal) {
  return request<Feedback[]>(`/api/sessions/${encodeURIComponent(id)}/feedback`, { signal });
}

/** What `POST /api/sessions/{id}/feedback` answers. */
export interface FeedbackResponse {
  feedback: Feedback;
  interpretation: FeedbackInterpretation;
  iteration: PlanIteration;
}

export function postFeedback(id: string, body: NewFeedback) {
  return request<FeedbackResponse>(
    `/api/sessions/${encodeURIComponent(id)}/feedback`,
    {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(body),
    },
    ANALYSIS_TIMEOUT_MS,
  );
}

export function fetchIterations(
  id: string,
  plan?: SetupPhilosophy,
  signal?: AbortSignal,
  base?: string | null,
) {
  const q = plan ? `?plan=${plan}${baseQuery(base, "&")}` : baseQuery(base);
  return request<PlanIteration[]>(`/api/sessions/${encodeURIComponent(id)}/iterations${q}`, {
    signal,
  });
}

export function fetchSetups(sessionId: string, signal?: AbortSignal) {
  return request<SetupLibrary>(`/api/setups?session=${encodeURIComponent(sessionId)}`, {
    signal,
  });
}

export function fetchSetupPreview(
  id: string,
  plan: string,
  base?: string | null,
  signal?: AbortSignal,
) {
  return request<SetupPreview>(
    `/api/sessions/${encodeURIComponent(id)}/setup-preview?plan=${encodeURIComponent(plan)}${baseQuery(base, "&")}`,
    { signal },
    ANALYSIS_TIMEOUT_MS,
  );
}

export function createSetupFiles(id: string, body: NewSetupFiles) {
  return request<SetupFilesCreated>(
    `/api/sessions/${encodeURIComponent(id)}/setup-files`,
    {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(body),
    },
    ANALYSIS_TIMEOUT_MS,
  );
}
