// AskUserQuestion, answered from the island. The question arrives as a
// permission request; Claude Code's own dialog answers it by allowing the tool
// with `answers` added to its input, keyed by question text — and so do we.

export interface AskOption {
  label: string;
  description?: string;
}

export interface AskQuestion {
  question: string;
  header?: string;
  multiSelect: boolean;
  options: AskOption[];
}

export function askQuestions(input: Record<string, unknown>): AskQuestion[] {
  const raw = input.questions;
  if (!Array.isArray(raw)) return [];
  return raw
    .filter((q): q is Record<string, unknown> => !!q && typeof q === "object" && typeof q.question === "string")
    .map((q) => ({
      question: q.question as string,
      header: typeof q.header === "string" ? q.header : undefined,
      multiSelect: q.multiSelect === true,
      options: (Array.isArray(q.options) ? q.options : [])
        .filter((o): o is Record<string, unknown> => !!o && typeof o === "object" && typeof o.label === "string")
        .map((o) => ({ label: o.label as string, description: typeof o.description === "string" ? o.description : undefined })),
    }));
}

/** The tool input with every question answered, or null while one is still open. */
export function answeredInput(
  input: Record<string, unknown>,
  picks: Record<string, string[]>,
): (Record<string, unknown> & { answers: Record<string, string> }) | null {
  const questions = askQuestions(input);
  const answers: Record<string, string> = {};
  for (const q of questions) {
    const chosen = picks[q.question] ?? [];
    if (chosen.length === 0) return null;
    answers[q.question] = chosen.join(", ");
  }
  return questions.length ? { ...input, answers } : null;
}
