import { describe, expect, it } from "vitest";
import { askQuestions, answeredInput } from "./ask";

const input = {
  questions: [
    { question: "Which DB?", header: "DB", multiSelect: false, options: [{ label: "Postgres", description: "SQL" }, { label: "Mongo" }] },
    { question: "Which extras?", header: "Extras", multiSelect: true, options: [{ label: "Redis" }, { label: "Kafka" }] },
  ],
};

describe("askQuestions", () => {
  it("reads questions and options from AskUserQuestion's input", () => {
    const q = askQuestions(input);
    expect(q.map((x) => x.question)).toEqual(["Which DB?", "Which extras?"]);
    expect(q[0].options.map((o) => o.label)).toEqual(["Postgres", "Mongo"]);
    expect(q[1].multiSelect).toBe(true);
  });

  it("is empty for anything malformed", () => {
    expect(askQuestions({})).toEqual([]);
    expect(askQuestions({ questions: "x" })).toEqual([]);
  });
});

describe("answeredInput", () => {
  it("keeps the original input and adds answers keyed by question", () => {
    const out = answeredInput(input, { "Which DB?": ["Mongo"], "Which extras?": ["Redis", "Kafka"] });
    expect(out?.questions).toBe(input.questions);
    expect(out?.answers).toEqual({ "Which DB?": "Mongo", "Which extras?": "Redis, Kafka" });
  });

  it("refuses until every question has an answer", () => {
    expect(answeredInput(input, { "Which DB?": ["Mongo"] })).toBeNull();
    expect(answeredInput(input, { "Which DB?": [], "Which extras?": ["Redis"] })).toBeNull();
  });
});
