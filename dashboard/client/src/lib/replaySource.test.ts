import { describe, expect, it } from "vitest";
import { DEMO3_FUNCTIONAL_REPLAY_URL, replaySourceUrl } from "./replaySource";

describe("replaySourceUrl", () => {
  it("uses the real demo 3 functional sidecar when no report API replay is selected", () => {
    expect(replaySourceUrl("https://api.example", null)).toBe(DEMO3_FUNCTIONAL_REPLAY_URL);
    expect(replaySourceUrl("https://api.example", "report-7")).toBe("https://api.example/v1/replays/report-7");
  });
});
