import assert from "node:assert/strict";
import test from "node:test";
import { buildRunDirectoryName, redactEvidence, summarizeError } from "../src/run-evidence.js";

test("buildRunDirectoryName creates filesystem-safe names", () => {
  assert.equal(
    buildRunDirectoryName("Outlook Draft Email", new Date("2026-05-02T20:03:38.704Z")),
    "2026-05-02T20-03-38-704Z_outlook-draft-email",
  );
});

test("redactEvidence hides credential-shaped keys and URL query strings", () => {
  const redacted = redactEvidence({
    nested: {
      webSocketDebuggerUrl: "ws://127.0.0.1:9222/devtools/browser/test",
    },
    url: "https://login.microsoftonline.com/oauth2/authorize?state=abc#token=def",
  });

  assert.equal(redacted.nested.webSocketDebuggerUrl, "[redacted]");
  assert.equal(redacted.url, "https://login.microsoftonline.com/oauth2/authorize?[query-redacted]");
});

test("summarizeError keeps only stable error fields", () => {
  assert.deepEqual(summarizeError(new TypeError("boom")), {
    message: "boom",
    name: "TypeError",
  });
});
