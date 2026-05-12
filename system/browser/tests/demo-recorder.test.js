import assert from "node:assert/strict";
import test from "node:test";
import {
  buildRecordedEvent,
  buildRecorderStatus,
  buildTargetSummary,
  installRecorderListenersInContext,
  installRecorderListeners,
  redactRecordedValue,
  sanitizeRecordedUrl,
} from "../src/demo-recorder.js";

test("redactRecordedValue redacts password, token, and cookie-shaped values", () => {
  assert.deepEqual(redactRecordedValue("super-secret", { inputType: "password" }), {
    redacted: true,
    valueLength: 12,
    valuePreview: "[redacted]",
  });
  assert.deepEqual(redactRecordedValue("abc123", { secretHint: "session_token" }), {
    redacted: true,
    valueLength: 6,
    valuePreview: "[redacted]",
  });
});

test("buildTargetSummary keeps a bounded actionable control descriptor", () => {
  assert.deepEqual(
    buildTargetSummary({
      ariaLabel: "New mail",
      id: "id-with spaces",
      role: "Button",
      tagName: "BUTTON",
      text: "ignored visible label",
    }),
    {
      name: "New mail",
      role: "button",
      selector: "button#id-with\\ spaces",
      tagName: "button",
    },
  );
});

test("buildRecordedEvent redacts URL queries and labels consequence-like actions", () => {
  const event = buildRecordedEvent({
    eventType: "click",
    pageTitle: "Outlook",
    pageUrl: "https://outlook.office.com/mail/?login_hint=person@example.com&token=abc",
    target: {
      ariaLabel: "Send",
      role: "button",
      tagName: "BUTTON",
      text: "Send",
    },
    timestamp: "2026-05-02T20:00:00.000Z",
  });

  assert.equal(event.url, "https://outlook.office.com/mail/?[query-redacted]");
  assert.equal(event.risk, "external_consequence");
});

test("sanitizeRecordedUrl strips fragments and query details", () => {
  assert.equal(
    sanitizeRecordedUrl("https://login.microsoftonline.com/oauth2/authorize?state=abc#token=def"),
    "https://login.microsoftonline.com/oauth2/authorize?[query-redacted]",
  );
});

test("buildRecorderStatus distinguishes enter stop from timeout", () => {
  assert.equal(buildRecorderStatus({ error: null, eventsCaptured: 4, stopReason: "terminal_enter" }), "recorded");
  assert.equal(buildRecorderStatus({ error: null, eventsCaptured: 0, stopReason: "timeout" }), "timeout_no_events");
  assert.equal(buildRecorderStatus({ error: new Error("boom"), eventsCaptured: 0 }), "failed");
});

test("installRecorderListeners installs on active frames and registers reinstall hooks", async () => {
  const calls = [];
  const listeners = new Map();
  const mainFrame = {
    evaluate: async (fn) => {
      assert.equal(typeof fn, "function");
      calls.push("main");
    },
  };
  const childFrame = {
    evaluate: async (fn) => {
      assert.equal(typeof fn, "function");
      calls.push("child");
    },
  };
  const page = {
    addInitScript: async (script) => {
      assert.match(script.content ?? "", /agentframeDemoRecorderInstalled/);
      calls.push("init");
    },
    exposeFunction: async (name, callback) => {
      assert.equal(name, "__agentframeRecordBrowserEvent");
      assert.equal(typeof callback, "function");
      calls.push("expose");
    },
    frames: () => [mainFrame, childFrame],
    on: (eventName, callback) => {
      listeners.set(eventName, callback);
      return page;
    },
  };

  await installRecorderListeners(page, () => {});

  assert.deepEqual(calls, ["expose", "init", "main", "child"]);
  assert.equal(typeof listeners.get("domcontentloaded"), "function");
  await listeners.get("framenavigated")?.(childFrame);
  assert.deepEqual(calls, ["expose", "init", "main", "child", "child"]);
});

test("installRecorderListenersInContext installs on existing and future pages", async () => {
  function makePage(name) {
    const calls = [];
    const listeners = new Map();
    const page = {
      calls,
      exposed: new Map(),
      addInitScript: async () => {
        calls.push(`${name}:init`);
      },
      exposeFunction: async (functionName, callback) => {
        calls.push(`${name}:expose:${functionName}`);
        page.exposed.set(functionName, callback);
      },
      frames: () => [
        {
          evaluate: async () => {
            calls.push(`${name}:frame`);
          },
        },
      ],
      on: (eventName, callback) => {
        listeners.set(eventName, callback);
        return page;
      },
      url: () => `https://example.com/${name}`,
    };
    return page;
  }

  const pageOne = makePage("one");
  const pageTwo = makePage("two");
  const contextListeners = new Map();
  const events = [];
  const context = {
    on: (eventName, callback) => {
      contextListeners.set(eventName, callback);
      return context;
    },
    pages: () => [pageOne],
  };

  const tracker = await installRecorderListenersInContext(context, (rawEvent) => {
    events.push(rawEvent);
  });

  assert.deepEqual(pageOne.calls, ["one:expose:__agentframeRecordBrowserEvent", "one:init", "one:frame"]);
  assert.equal(typeof contextListeners.get("page"), "function");

  await contextListeners.get("page")?.(pageTwo);
  await contextListeners.get("page")?.(pageTwo);

  assert.deepEqual(pageTwo.calls, ["two:expose:__agentframeRecordBrowserEvent", "two:init", "two:frame"]);

  await pageTwo.exposed.get("__agentframeRecordBrowserEvent")?.({
    eventType: "click",
    pageUrl: "https://workfront.example/new-tab",
  });

  assert.equal(events.length, 1);
  assert.equal(events[0].pageUrl, "https://workfront.example/new-tab");
  assert.equal(tracker.getActivePage(), pageTwo);
  assert.deepEqual(tracker.getInstalledPages(), [pageOne, pageTwo]);
});
