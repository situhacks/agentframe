import assert from "node:assert/strict";
import test from "node:test";
import {
  buildEdgeArgs,
  ensureWorkBrowser,
  findEdgeExecutable,
  markerMatchesWorkBrowser,
  normalizeEndpoint,
  toSafeWorkBrowserEvidence,
} from "../src/work-browser.js";

test("findEdgeExecutable honors explicit Edge path override", () => {
  const edgePath = "C:\\Tools\\Edge\\msedge.exe";
  assert.equal(
    findEdgeExecutable({
      env: { AGENTFRAME_EDGE_PATH: edgePath },
      exists: (candidate) => candidate === edgePath,
      platform: "win32",
    }),
    edgePath,
  );
});

test("buildEdgeArgs opens a controlled Edge profile with remote debugging", () => {
  const args = buildEdgeArgs({
    port: 9222,
    profilePath: "C:\\agentframe-profile",
    startUrl: "https://outlook.office.com/mail/",
  });

  assert.match(args.join("\n"), /--remote-debugging-port=9222/);
  assert.match(args.join("\n"), /--user-data-dir=C:\\agentframe-profile/);
  assert.equal(args.at(-1), "https://outlook.office.com/mail/");
});

test("ensureWorkBrowser reuses an owned existing endpoint without launching Edge", async () => {
  let launched = false;
  const result = await ensureWorkBrowser({
    edgePath: "C:\\Edge\\msedge.exe",
    endpoint: "http://127.0.0.1:9222",
    markerPath: "C:\\runtime\\work-browser-session.json",
    profilePath: "C:\\agentframe-profile",
    probeEndpoint: async () => ({
      browser: "Edg/147.0.3912.98",
      endpoint: "http://127.0.0.1:9222",
      ok: true,
      webSocketDebuggerUrl: "ws://127.0.0.1:9222/devtools/browser/test",
    }),
    readMarker: async () => ({
      browser: "edge",
      endpoint: "http://127.0.0.1:9222",
      pid: 4321,
      profilePath: "C:\\agentframe-profile",
      timestamp: "2026-05-02T20:00:00.000Z",
    }),
    spawnBrowser: () => {
      launched = true;
      return { pid: 1234 };
    },
  });

  assert.equal(result.status, "already_running");
  assert.equal(result.webSocketDebuggerUrl, "ws://127.0.0.1:9222/devtools/browser/test");
  assert.equal(launched, false);
});

test("ensureWorkBrowser refuses to reuse an unowned reachable endpoint", async () => {
  let launched = false;
  const result = await ensureWorkBrowser({
    edgePath: "C:\\Edge\\msedge.exe",
    endpoint: "http://127.0.0.1:9222",
    markerPath: "C:\\runtime\\work-browser-session.json",
    profilePath: "C:\\agentframe-profile",
    probeEndpoint: async () => ({
      browser: "Chrome/147.0.3912.98",
      endpoint: "http://127.0.0.1:9222",
      ok: true,
      webSocketDebuggerUrl: "ws://127.0.0.1:9222/devtools/browser/external",
    }),
    readMarker: async () => undefined,
    spawnBrowser: () => {
      launched = true;
      return { pid: 1234 };
    },
  });

  assert.equal(result.status, "externally_managed");
  assert.equal(result.webSocketDebuggerUrl, undefined);
  assert.match(result.warning, /not owned by Work Browser Mode/);
  assert.equal(launched, false);
});

test("marker and evidence helpers keep endpoint identity and hide websocket URLs", () => {
  assert.equal(normalizeEndpoint(9222), "http://127.0.0.1:9222");
  assert.equal(
    markerMatchesWorkBrowser(
      {
        browser: "edge",
        endpoint: "http://127.0.0.1:9222",
        profilePath: "C:\\agentframe-profile",
      },
      "http://127.0.0.1:9222",
      "C:\\agentframe-profile",
    ),
    true,
  );

  const safe = toSafeWorkBrowserEvidence({
    browser: "edge",
    debuggingEndpoint: "http://127.0.0.1:9222",
    profilePath: "C:\\agentframe-profile",
    status: "already_running",
    webSocketDebuggerUrl: "ws://127.0.0.1:9222/devtools/browser/test",
  });

  assert.equal("webSocketDebuggerUrl" in safe, false);
  assert.equal(safe.hasDebuggingEndpoint, true);
});
