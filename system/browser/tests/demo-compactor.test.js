import assert from "node:assert/strict";
import test from "node:test";
import { compactRecordedEvents, sanitizeRecordedEvent, sanitizeRecordedUrl } from "../src/demo-compactor.js";

test("sanitizeRecordedEvent removes query details and redacts long free text", () => {
  const event = sanitizeRecordedEvent(
    {
      eventType: "input",
      target: {
        name: "Message body",
        role: "textbox",
        selector: "div[aria-label='Message body']",
        tagName: "div",
      },
      timestamp: "2026-05-02T20:00:00.000Z",
      title: "Outlook",
      url: "https://outlook.office.com/mail/?token=abc",
      value: {
        redacted: false,
        valueLength: 140,
        valuePreview: "This is a long body that should not become deterministic script material.",
      },
    },
    4,
  );

  assert.equal(event.url, "https://outlook.office.com/mail/");
  assert.equal(event.value.valueClass, "free_text");
  assert.equal(event.value.valuePlaceholder, "[redacted-free-text]");
});

test("compactRecordedEvents merges repeated fill events on the same target", () => {
  const actions = compactRecordedEvents([
    {
      eventIndex: 0,
      eventType: "click",
      target: { name: "New mail", role: "button", tagName: "button" },
      title: "Outlook",
      url: "https://outlook.office.com/mail/",
    },
    {
      eventIndex: 1,
      eventType: "input",
      target: { name: "Subject", role: "input", selector: "#subject", tagName: "input" },
      title: "Outlook",
      url: "https://outlook.office.com/mail/",
      value: { valueClass: "safe_text", valueLength: 1, valuePlaceholder: "H" },
    },
    {
      eventIndex: 2,
      eventType: "input",
      target: { name: "Subject", role: "input", selector: "#subject", tagName: "input" },
      title: "Outlook",
      url: "https://outlook.office.com/mail/",
      value: { valueClass: "safe_text", valueLength: 5, valuePlaceholder: "Hello" },
    },
  ]);

  assert.deepEqual(
    actions.map((action) => ({
      actionType: action.actionType,
      sourceEventCount: action.sourceEventCount,
      step: action.step,
      valuePlaceholder: action.valuePlaceholder,
    })),
    [
      { actionType: "click", sourceEventCount: 1, step: 1, valuePlaceholder: undefined },
      { actionType: "fill", sourceEventCount: 2, step: 2, valuePlaceholder: "Hello" },
    ],
  );
});

test("sanitizeRecordedUrl drops query strings entirely for compacted actions", () => {
  assert.equal(sanitizeRecordedUrl("https://example.com/path?x=1#frag"), "https://example.com/path");
});
