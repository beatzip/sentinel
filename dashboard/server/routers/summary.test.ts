import { describe, expect, it } from "vitest";
import { validateSummaryFacts } from "./summary";

describe("validateSummaryFacts", () => {
  const facts = [{ id: "round-1", text: "Round 1: Terrorist (1-0)" }];

  it("accepts only observations linked to supplied evidence ids", () => {
    expect(validateSummaryFacts({ overview: "Observed round record.", observations: [{ factId: "round-1", text: "The recorded result is shown." }], limitations: "No additional context was supplied." }, facts).observations).toHaveLength(1);
  });

  it("rejects an observation without an evidence reference", () => {
    expect(() => validateSummaryFacts({ overview: "Observed round record.", observations: [{ factId: "invented", text: "Unsupported claim." }], limitations: "No additional context was supplied." }, facts)).toThrow("outside the supplied evidence schema");
  });
});
