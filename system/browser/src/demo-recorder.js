import path from "node:path";
import { pathToFileURL } from "node:url";
import "dotenv/config";
import {
  buildDemoRecorderControlFile,
  buildRecorderStatus,
  clearDemoRecorderControlFiles,
  finalizeDemoRecorderEvidence,
  getDefaultDemoRecorderCurrentPath,
  hasDemoRecorderStopFile,
  writeDemoRecorderControlFile,
} from "./demo-recorder-lifecycle.js";
import { connectToOwnedWorkBrowser, observePage } from "./playwright-runtime.js";
import { createRunDirectory, redactEvidence, summarizeError, writeRunJson } from "./run-evidence.js";
import { ensureWorkBrowser, toSafeWorkBrowserEvidence } from "./work-browser.js";

const DEFAULT_WORKFLOW_ID = "browser-demo";
const DEFAULT_START_URL = "about:blank";
const DEFAULT_TIMEOUT_MS = 10 * 60 * 1_000;
const DEFAULT_MAX_EVENTS = 500;
const DEFAULT_VALUE_PREVIEW_LENGTH = 120;

export { buildRecorderStatus };

export function sanitizeRecordedUrl(value) {
  try {
    const url = new URL(value);
    url.hash = "";
    if (url.search) {
      url.search = "?[query-redacted]";
    }
    return url.toString();
  } catch {
    return value.split(/[?#]/, 1)[0] || value;
  }
}

function normalizeLabel(value, limit = 160) {
  const normalized = (value ?? "").trim().replace(/\s+/g, " ");
  return normalized.length <= limit ? normalized : `${normalized.slice(0, limit)}...`;
}

function escapeCssIdentifier(value) {
  return value.replace(/[^a-zA-Z0-9_-]/g, (character) => `\\${character}`);
}

function escapeCssString(value) {
  return value.replace(/["\\]/g, (character) => `\\${character}`);
}

function inferTargetName(target) {
  return (
    normalizeLabel(target.ariaLabel) ||
    normalizeLabel(target.title) ||
    normalizeLabel(target.text) ||
    normalizeLabel(target.placeholder) ||
    normalizeLabel(target.name) ||
    normalizeLabel(target.id) ||
    normalizeLabel(target.tagName) ||
    "unknown"
  );
}

function inferTargetRole(target) {
  return normalizeLabel(target.role || target.inputType || target.tagName || "control", 80).toLowerCase() || "control";
}

function buildSelector(target, tagName) {
  if (target.dataTestId) {
    return `${tagName}[data-testid="${escapeCssString(target.dataTestId)}"]`;
  }
  if (target.id) {
    return `${tagName}#${escapeCssIdentifier(target.id)}`;
  }
  if (target.name) {
    return `${tagName}[name="${escapeCssString(target.name)}"]`;
  }
  if (target.ariaLabel) {
    return `${tagName}[aria-label="${escapeCssString(target.ariaLabel)}"]`;
  }
  return undefined;
}

export function buildTargetSummary(target) {
  const tagName = normalizeLabel(target?.tagName, 40).toLowerCase() || "unknown";
  const summary = {
    name: target ? inferTargetName(target) : "unknown",
    role: target ? inferTargetRole(target) : "control",
    tagName,
  };
  const inputType = normalizeLabel(target?.inputType, 40).toLowerCase();
  if (inputType) {
    summary.inputType = inputType;
  }
  const selector = target ? buildSelector(target, tagName) : undefined;
  if (selector) {
    summary.selector = selector;
  }
  return summary;
}

function hasSecretShape(value) {
  return /(password|passwd|token|secret|credential|authorization|cookie|session|bearer)/i.test(value);
}

export function redactRecordedValue(value, options = {}) {
  if (value === undefined || value === null) {
    return undefined;
  }
  if (/^(password|hidden)$/i.test(options.inputType ?? "") || hasSecretShape(options.secretHint ?? "") || hasSecretShape(value)) {
    return {
      redacted: true,
      valueLength: value.length,
      valuePreview: "[redacted]",
    };
  }

  const maxPreviewLength = options.maxPreviewLength ?? DEFAULT_VALUE_PREVIEW_LENGTH;
  const normalized = value.replace(/\s+/g, " ").trim();
  return {
    redacted: false,
    valueLength: value.length,
    valuePreview: normalized.length > maxPreviewLength ? `${normalized.slice(0, maxPreviewLength)}...` : normalized,
  };
}

function buildSecretHint(target) {
  return [target?.ariaLabel, target?.id, target?.name, target?.placeholder, target?.text, target?.title]
    .filter((value) => Boolean(value))
    .join(" ");
}

function isConsequenceLikeEvent(input, target) {
  if (input.eventType === "submit") {
    return true;
  }
  if (input.eventType !== "click") {
    return false;
  }

  const rawText = [
    input.target?.ariaLabel,
    input.target?.id,
    input.target?.name,
    input.target?.placeholder,
    input.target?.text,
    input.target?.title,
    target.name,
  ]
    .filter((value) => Boolean(value))
    .join(" ");

  return /\b(send|submit|publish|delete|purchase|pay)\b/i.test(rawText);
}

export function buildRecordedEvent(input) {
  const target = buildTargetSummary(input.target);
  const event = {
    eventType: input.eventType,
    target,
    timestamp: input.timestamp,
    title: normalizeLabel(input.pageTitle, 200),
    url: sanitizeRecordedUrl(input.pageUrl),
  };
  const value = redactRecordedValue(input.target?.value, {
    inputType: input.target?.inputType,
    secretHint: buildSecretHint(input.target),
  });
  if (value) {
    event.value = value;
  }
  if (isConsequenceLikeEvent(input, target)) {
    event.risk = "external_consequence";
  }
  return event;
}

export function installRecorderScript() {
  const recorderWindow = window;
  if (recorderWindow.__agentframeDemoRecorderInstalled) {
    return;
  }

  recorderWindow.__agentframeDemoRecorderInstalled = true;

  function summarizeTarget(eventTarget) {
    const rawElement = eventTarget instanceof Element ? eventTarget : null;
    const element =
      rawElement?.closest("button, [role], input, textarea, select, [contenteditable='true'], a, [aria-label], [title], [data-testid]") ??
      rawElement;
    const htmlElement = element instanceof HTMLElement ? element : null;
    const inputElement = element instanceof HTMLInputElement ? element : null;
    const textAreaElement = element instanceof HTMLTextAreaElement ? element : null;
    const selectElement = element instanceof HTMLSelectElement ? element : null;
    const isSensitive = inputElement?.type === "password" || inputElement?.type === "hidden";
    const rawValue =
      inputElement?.value ??
      textAreaElement?.value ??
      selectElement?.value ??
      (htmlElement?.isContentEditable ? htmlElement.innerText : null);

    return {
      ariaLabel: element?.getAttribute("aria-label") ?? null,
      dataTestId: element?.getAttribute("data-testid") ?? null,
      id: element?.id || null,
      inputType: inputElement?.type ?? null,
      name: inputElement?.name || textAreaElement?.name || selectElement?.name || null,
      placeholder: inputElement?.placeholder || textAreaElement?.placeholder || element?.getAttribute("placeholder") || null,
      role: element?.getAttribute("role") ?? null,
      tagName: element?.tagName ?? null,
      text: htmlElement?.innerText?.slice(0, 200) ?? null,
      title: element?.getAttribute("title") ?? null,
      value: isSensitive ? null : rawValue,
    };
  }

  function record(eventType, eventTarget) {
    recorderWindow.__agentframeRecordBrowserEvent?.({
      eventType,
      pageTitle: document.title,
      pageUrl: window.location.href,
      target: summarizeTarget(eventTarget),
      timestamp: new Date().toISOString(),
    });
  }

  const listenerOptions = { capture: true, passive: true };
  document.addEventListener("click", (event) => record("click", event.target), listenerOptions);
  document.addEventListener("input", (event) => record("input", event.target), listenerOptions);
  document.addEventListener("change", (event) => record("change", event.target), listenerOptions);
  document.addEventListener("submit", (event) => record("submit", event.target), listenerOptions);
  window.addEventListener("popstate", () => record("navigation", document.body), listenerOptions);
  window.addEventListener("hashchange", () => record("navigation", document.body), listenerOptions);
}

function buildRecorderInstallScriptContent() {
  return [
    "(function(){",
    "const __name = globalThis.__name || function(target) { return target; };",
    `(${installRecorderScript.toString()})();`,
    "})();",
  ].join("\n");
}

function evaluateRecorderInstallScript(source) {
  (0, eval)(source);
}

async function installRecorderListenersInFrame(frame) {
  const scriptContent = buildRecorderInstallScriptContent();
  if (typeof frame.addScriptTag === "function") {
    const injected = await frame.addScriptTag({ content: scriptContent }).then(() => true, () => false);
    if (injected) {
      return;
    }
  }
  await frame.evaluate(evaluateRecorderInstallScript, scriptContent).catch(() => {});
}

async function installRecorderListenersInAllFrames(page) {
  await Promise.all(page.frames().map((frame) => installRecorderListenersInFrame(frame)));
}

export async function installRecorderListeners(page, onEvent) {
  await page.exposeFunction("__agentframeRecordBrowserEvent", onEvent);
  await page.addInitScript({ content: buildRecorderInstallScriptContent() });
  await installRecorderListenersInAllFrames(page);

  const reinstallAllFrames = () => {
    void installRecorderListenersInAllFrames(page);
  };
  const reinstallFrame = (frame) => {
    void installRecorderListenersInFrame(frame);
  };

  page.on("domcontentloaded", reinstallAllFrames);
  page.on("load", reinstallAllFrames);
  page.on("frameattached", reinstallFrame);
  page.on("framenavigated", reinstallFrame);
}

export async function installRecorderListenersInContext(context, onEvent) {
  const installedPages = new Set();
  let activePage = null;

  const installPage = async (candidatePage) => {
    if (!candidatePage || installedPages.has(candidatePage)) {
      return;
    }
    installedPages.add(candidatePage);
    await installRecorderListeners(candidatePage, (rawEvent) => {
      activePage = candidatePage;
      onEvent(rawEvent);
    });
  };

  await Promise.all(context.pages().map((candidatePage) => installPage(candidatePage)));
  context.on?.("page", (newPage) => installPage(newPage));

  return {
    getActivePage: () => activePage,
    getInstalledPages: () => Array.from(installedPages),
  };
}

function parseCliArgs(argv, env = process.env) {
  const options = {
    maxEvents: Number(env.AGENTFRAME_DEMO_RECORDER_MAX_EVENTS ?? DEFAULT_MAX_EVENTS),
    startUrl: env.AGENTFRAME_DEMO_START_URL || DEFAULT_START_URL,
    timeoutMs: Number(env.AGENTFRAME_DEMO_RECORDER_TIMEOUT_MS ?? DEFAULT_TIMEOUT_MS),
    workflowId: env.AGENTFRAME_DEMO_WORKFLOW_ID || DEFAULT_WORKFLOW_ID,
    workflowPath: env.AGENTFRAME_DEMO_WORKFLOW_PATH,
  };

  for (const arg of argv) {
    if (arg.startsWith("--start-url=")) {
      options.startUrl = arg.slice("--start-url=".length);
    } else if (arg.startsWith("--workflow-id=")) {
      options.workflowId = arg.slice("--workflow-id=".length);
    } else if (arg.startsWith("--workflow-path=")) {
      options.workflowPath = arg.slice("--workflow-path=".length);
    } else if (arg.startsWith("--timeout-ms=")) {
      options.timeoutMs = Number(arg.slice("--timeout-ms=".length));
    } else if (arg.startsWith("--max-events=")) {
      options.maxEvents = Number(arg.slice("--max-events=".length));
    }
  }

  return options;
}

async function waitForStopSignal(options) {
  return new Promise((resolve) => {
    let resolved = false;
    const finish = (reason) => {
      if (resolved) {
        return;
      }
      resolved = true;
      clearTimeout(timeout);
      clearInterval(pollInterval);
      process.stdin.off("data", onData);
      process.stdin.pause();
      resolve(reason);
    };
    const onData = () => finish("terminal_enter");
    const timeout = setTimeout(() => finish("timeout"), options.timeoutMs);
    const pollInterval = setInterval(() => {
      if (hasDemoRecorderStopFile(options.stopFile)) {
        finish("agent_stop");
      }
    }, 500);

    process.stdin.resume();
    process.stdin.once("data", onData);
    if (hasDemoRecorderStopFile(options.stopFile)) {
      finish("agent_stop");
    }
  });
}

async function main() {
  const options = parseCliArgs(process.argv.slice(2));
  const runDir = await createRunDirectory(options.workflowId, new Date(), {
    workflowPath: options.workflowPath,
  });
  const startedAt = new Date().toISOString();
  const events = [];
  let browserConnection = null;
  let page = null;
  let stopReason = null;
  let terminalError = null;

  const evidence = {
    browser: null,
    completedAt: null,
    config: options,
    error: null,
    eventsCaptured: 0,
    eventsPath: null,
    evidencePaths: {},
    finalObservation: null,
    initialObservation: null,
    resultPath: null,
    runDir,
    startedAt,
    status: "running",
    stopReason: null,
    workflowId: options.workflowId,
  };

  const currentRunPath = getDefaultDemoRecorderCurrentPath();
  const controlFile = buildDemoRecorderControlFile({
    pid: process.pid,
    runDir,
    startedAt,
    workflowId: options.workflowId,
  });
  const finalizeEvidence = finalizeDemoRecorderEvidence({
    evidence,
    events,
    getStopReason: () => stopReason,
    getTerminalError: () => terminalError,
    writeJson: writeRunJson,
  });

  await writeDemoRecorderControlFile(controlFile, currentRunPath);

  try {
    const browser = await ensureWorkBrowser({ startUrl: options.startUrl });
    evidence.browser = toSafeWorkBrowserEvidence(browser);
    if (!browser.webSocketDebuggerUrl) {
      throw new Error("Work Browser endpoint is externally managed or unowned; recorder refused to attach.");
    }

    browserConnection = await connectToOwnedWorkBrowser(browser);
    const context = browserConnection.contexts()[0] ?? (await browserConnection.newContext());
    page = context.pages()[0] ?? (await context.newPage());
    if (page.url() !== options.startUrl) {
      await page.goto(options.startUrl, { waitUntil: "domcontentloaded", timeout: 45_000 }).catch(() => {});
    }

    const recorderTracker = await installRecorderListenersInContext(context, (rawEvent) => {
      if (events.length < options.maxEvents) {
        events.push(buildRecordedEvent(rawEvent));
      }
    });
    evidence.initialObservation = await observePage(page, {
      screenshotPath: path.join(runDir, "initial.png"),
    });

    process.stdout.write([
      "",
      "Browser tracker is running in Work Browser Mode.",
      `Run directory: ${runDir}`,
      `Start URL: ${options.startUrl}`,
      "Demonstrate the workflow in the controlled browser.",
      "When finished, the agent should stop the recorder with `npm run record:stop`.",
      "Press Enter in this terminal only as a manual fallback.",
      "",
    ].join("\n"));

    stopReason = await waitForStopSignal({
      stopFile: controlFile.stopFile,
      timeoutMs: options.timeoutMs,
    });

    const finalPage = recorderTracker.getActivePage() ?? page;
    evidence.finalObservation = await observePage(finalPage, {
      screenshotPath: path.join(runDir, "final.png"),
    }).catch(() => null);
  } catch (error) {
    terminalError = error instanceof Error ? error : new Error(String(error));
    evidence.error = summarizeError(error);
    process.exitCode = 1;
  } finally {
    const finalized = await finalizeEvidence();
    await clearDemoRecorderControlFiles(controlFile, currentRunPath);
    await browserConnection?.close().catch(() => {});
    process.stdout.write(`${JSON.stringify(redactEvidence(finalized), null, 2)}\n`);
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main().catch((error) => {
    process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
    process.exitCode = 1;
  });
}
