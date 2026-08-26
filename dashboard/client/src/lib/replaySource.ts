import { loadCurrentTrackApproximateReplay } from "@/lib/currentTrackApproximate";

export const DEMO3_FUNCTIONAL_REPLAY_URL = "/manus-storage/demo3-functional.replay_ddfec7bd.json";

export function replaySourceUrl(apiUrl: string, reportId: string | null) {
  return reportId ? `${apiUrl}/v1/replays/${reportId}` : DEMO3_FUNCTIONAL_REPLAY_URL;
}

export type ReplaySourceMode = "current_track" | "api_report";

export async function loadReplaySource(apiUrl: string, reportId: string | null, mode: ReplaySourceMode) {
  if (mode === "current_track") return loadCurrentTrackApproximateReplay();
  if (!reportId) throw new Error("report source unavailable");
  const response = await fetch(replaySourceUrl(apiUrl, reportId));
  if (!response.ok) throw new Error("replay missing");
  return response.json();
}
