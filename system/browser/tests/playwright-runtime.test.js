import assert from "node:assert/strict";
import test from "node:test";
import {
  buildCandidateControlSummary,
  buildObservationSnapshot,
  requireOwnedDebuggingWebSocket,
} from "../src/playwright-runtime.js";

test("requireOwnedDebuggingWebSocket accepts only owned browser sessions", () => {
  assert.equal(
    requireOwnedDebuggingWebSocket({
      status: "already_running",
      webSocketDebuggerUrl: "ws://127.0.0.1:9222/devtools/browser/test",
    }),
    "ws://127.0.0.1:9222/devtools/browser/test",
  );

  assert.throws(
    () =>
      requireOwnedDebuggingWebSocket({
        status: "externally_managed",
        webSocketDebuggerUrl: "ws://127.0.0.1:9222/devtools/browser/external",
      }),
    /owned Work Browser/,
  );
});

test("buildCandidateControlSummary prioritizes editable controls and normalizes labels", () => {
  const controls = buildCandidateControlSummary([
    { name: "  Generic Link  ", role: "link", selector: "a:nth-of-type(1)" },
    { name: "  Message   body ", role: "TEXTBOX", selector: "div:nth-of-type(3)" },
    { name: "", role: "button", selector: "button:nth-of-type(2)" },
  ]);

  assert.deepEqual(controls, [
    { name: "Message body", role: "textbox", selector: "div:nth-of-type(3)" },
    { name: "Generic Link", role: "link", selector: "a:nth-of-type(1)" },
  ]);
});

test("buildObservationSnapshot returns a compact Cursor-readable snapshot", () => {
  assert.deepEqual(
    buildObservationSnapshot({
      bodyTextSnippet: "Inbox\nNew mail",
      candidateControls: [{ name: "New mail", role: "button", selector: "button:nth-of-type(1)" }],
      screenshotPath: "C:\\runs\\initial.png",
      title: "Outlook",
      url: "https://outlook.office.com/mail/",
    }),
    {
      bodyTextSnippet: "Inbox\nNew mail",
      candidateControls: [{ name: "New mail", role: "button", selector: "button:nth-of-type(1)" }],
      screenshotPath: "C:\\runs\\initial.png",
      title: "Outlook",
      url: "https://outlook.office.com/mail/",
    },
  );
});
