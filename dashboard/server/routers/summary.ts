import { invokeLLM, listLLMModels } from "../_core/llm";
import { adminProcedure, router } from "../_core/trpc";
import { z } from "zod";

const factSchema = z.object({
  id: z.string().min(1).max(120),
  text: z.string().min(1).max(500),
});

const inputSchema = z.object({
  reportId: z.string().min(1).max(120),
  map: z.string().min(1).max(80),
  overallRisk: z.number().min(0).max(1),
  facts: z.array(factSchema).min(1).max(80),
});

const outputSchema = z.object({
  overview: z.string().min(1).max(700),
  observations: z.array(z.object({
    factId: z.string().min(1).max(120),
    text: z.string().min(1).max(400),
  })).max(8),
  limitations: z.string().min(1).max(400),
});

export function validateSummaryFacts(
  summary: z.infer<typeof outputSchema>,
  facts: z.infer<typeof factSchema>[],
) {
  const allowed = new Set(facts.map(fact => fact.id));
  if (summary.observations.some(observation => !allowed.has(observation.factId))) {
    throw new Error("AI summary referenced a fact outside the supplied evidence schema");
  }
  return summary;
}

export const summaryRouter = router({
  generate: adminProcedure.input(inputSchema).mutation(async ({ input }) => {
    const { data: models } = await listLLMModels();
    const model = models.find(model => model.id === "gpt-5-mini")?.id ?? models[0]?.id;
    if (!model) throw new Error("No built-in language model is available");

    const response = await invokeLLM({
      model,
      maxTokens: 700,
      messages: [
        {
          role: "system",
          content: "You summarize a CS2 Sentinel report using only supplied facts. Do not infer intent, cheating, hidden actions, causality, or player state. Each observation must cite exactly one supplied factId. If evidence is incomplete, state that in limitations. Output JSON only.",
        },
        {
          role: "user",
          content: JSON.stringify(input),
        },
      ],
      response_format: {
        type: "json_schema",
        json_schema: {
          name: "sentinel_facts_only_summary",
          strict: true,
          schema: {
            type: "object",
            properties: {
              overview: { type: "string" },
              observations: {
                type: "array",
                items: {
                  type: "object",
                  properties: {
                    factId: { type: "string" },
                    text: { type: "string" },
                  },
                  required: ["factId", "text"],
                  additionalProperties: false,
                },
              },
              limitations: { type: "string" },
            },
            required: ["overview", "observations", "limitations"],
            additionalProperties: false,
          },
        },
      },
    });
    const content = response.choices[0]?.message.content;
    if (!content || typeof content !== "string") {
      throw new Error("AI summary returned no textual content");
    }
    return validateSummaryFacts(outputSchema.parse(JSON.parse(content)), input.facts);
  }),
});
