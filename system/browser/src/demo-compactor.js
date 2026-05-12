import { access, readdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { getRunsRoot } from "./run-evidence.js";

const SECRET_SHAPE_PATTERN = /\b(password|passwd|token|secret|credential|authorization|cookie|session|bearer|api[-_ ]?key|websocketdebuggerurl)\b/i;
const EMAIL_PATTERN = /\b[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}\b/i;
const EMAIL_REPLACE_PATTERN = /\b[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}\b/gi;
const LONG_FREE_TEXT_LENGTH = 80;

export function sanitizeRecordedUrl(value) {
  try {
    const url = new URL(value);
    url.hash = "";
    url.search = "";
    return url.toString();
  } catch {
    return value.split(/[?#]/, 1)[0] || value;
  }
}

function normalizeText(value) {
  return (value ?? "").trim().replace(/\s+/g, " ");
}

function sanitizeVisibleText(value) {
  const normalized = normalizeText(value);
  if (SECRET_SHAPE_PATTERN.test(normalized)) {
    return "[redacted-secret-label]";
  }
  if (EMAIL_PATTERN.test(normalized)) {
    return normalized.replace(EMAIL_REPLACE_PATTERN, "[email]");
  }
  return normalized;
}

function sanitizeTarget(target) {
  const sanitized = {
    name: sanitizeVisibleText(target.name),
    role: sanitizeVisibleText(target.role).toLowerCase() || "control",
    tagName: sanitizeVisibleText(target.tagName).toLowerCase() || "unknown",
  };
  if (target.inputType) {
    sanitized.inputType = sanitizeVisibleText(target.inputType).toLowerCase();
  }
  if (target.selector && !SECRET_SHAPE_PATTERN.test(target.selector)) {
    sanitized.selector = sanitizeVisibleText(target.selector);
  }
  return sanitized;
}

function classifyValue(input) {
  const value = input.value;
  if (!value) {
    return undefined;
  }

  const preview = normalizeText(value.valuePreview);
  const targetHint = [input.target.inputType, input.target.name, input.target.role, input.target.selector, input.target.tagName].join(" ");

  if (value.redacted || SECRET_SHAPE_PATTERN.test(targetHint) || SECRET_SHAPE_PATTERN.test(preview)) {
    return {
      redacted: true,
      valueClass: "redacted",
      valueLength: value.valueLength,
      valuePlaceholder: "[redacted]",
    };
  }
  if (value.valueLength === 0) {
    return {
      redacted: false,
      valueClass: "empty",
      valueLength: 0,
      valuePlaceholder: "",
    };
  }
  if (EMAIL_PATTERN.test(preview)) {
    return {
      redacted: true,
      valueClass: "email",
      valueLength: value.valueLength,
      valuePlaceholder: "[email]",
    };
  }
  if (value.valueLength > LONG_FREE_TEXT_LENGTH || input.target.role === "textbox") {
    return {
      redacted: true,
      valueClass: "free_text",
      valueLength: value.valueLength,
      valuePlaceholder: "[redacted-free-text]",
    };
  }

  return {
    redacted: false,
    valueClass: "safe_text",
    valueLength: value.valueLength,
    valuePlaceholder: sanitizeVisibleText(preview),
  };
}

export function sanitizeRecordedEvent(event, eventIndex) {
  const target = sanitizeTarget(event.target);
  const value = classifyValue({ target: event.target, value: event.value });
  const sanitized = {
    eventIndex,
    eventType: event.eventType,
    target,
    timestamp: event.timestamp,
    title: sanitizeVisibleText(event.title),
    url: sanitizeRecordedUrl(event.url),
  };
  if (value) {
    sanitized.value = value;
  }
  if (event.risk) {
    sanitized.risk = event.risk;
  }
  return sanitized;
}

function actionTypeFor(event) {
  if (event.eventType === "navigation") {
    return "navigate";
  }
  if (event.eventType === "submit") {
    return "submit";
  }
  if (event.eventType === "input" || event.eventType === "change") {
    return "fill";
  }
  return "click";
}

function targetKey(event) {
  return [event.target.selector, event.target.name, event.target.role, event.target.tagName].join("|");
}

function shouldMergeFill(previous, next) {
  return actionTypeFor(previous) === "fill" && actionTypeFor(next) === "fill" && targetKey(previous) === targetKey(next);
}

function buildAction(events, step) {
  const first = events[0];
  const last = events.at(-1);
  if (!first || !last) {
    throw new Error("Cannot compact an empty event group.");
  }

  const action = {
    actionType: actionTypeFor(last),
    sourceEventCount: events.length,
    sourceEventRange: {
      end: last.eventIndex,
      start: first.eventIndex,
    },
    step,
    target: last.target,
    title: last.title,
    url: last.url,
  };
  if (last.value) {
    action.valueClass = last.value.valueClass;
    action.valueLength = last.value.valueLength;
    action.valuePlaceholder = last.value.valuePlaceholder;
  }
  if (last.risk) {
    action.risk = last.risk;
  }
  return action;
}

export function compactRecordedEvents(events) {
  const groups = [];
  for (const event of events) {
    const previousGroup = groups.at(-1);
    const previousEvent = previousGroup?.at(-1);
    if (previousGroup && previousEvent && shouldMergeFill(previousEvent, event)) {
      previousGroup.push(event);
      continue;
    }
    groups.push([event]);
  }
  return groups.map((group, index) => buildAction(group, index + 1));
}

async function pathExists(filePath) {
  return access(filePath).then(() => true, () => false);
}

export async function resolveRunDirectory(inputRunDir) {
  if (inputRunDir) {
    return path.resolve(inputRunDir);
  }

  const runsRoot = getRunsRoot();
  const entries = await readdir(runsRoot, { withFileTypes: true });
  const candidates = entries
    .filter((entry) => entry.isDirectory())
    .map((entry) => path.join(runsRoot, entry.name))
    .sort((a, b) => b.localeCompare(a));
  const latestWithEvents = await Promise.all(
    candidates.map(async (candidate) => ({
      candidate,
      hasEvents: await pathExists(path.join(candidate, "events.json")),
    })),
  );
  const latest = latestWithEvents.find((candidate) => candidate.hasEvents)?.candidate;
  if (!latest) {
    throw new Error(`No recorder run with events.json found under ${runsRoot}`);
  }
  return latest;
}

function inferWorkflowHint(runDir) {
  const basename = path.basename(runDir);
  const separatorIndex = basename.indexOf("_");
  return separatorIndex >= 0 ? basename.slice(separatorIndex + 1) : null;
}

export async function compactRun(runDirInput) {
  const runDir = await resolveRunDirectory(runDirInput);
  const rawEventsPath = path.join(runDir, "events.json");
  const actionsPath = path.join(runDir, "actions.json");
  const rawEvents = JSON.parse(await readFile(rawEventsPath, "utf8"));
  const sanitizedEvents = rawEvents.map((event, index) => sanitizeRecordedEvent(event, index));
  const actions = compactRecordedEvents(sanitizedEvents);
  const summary = {
    actions: actions.length,
    actionsPath,
    runDir,
    sourceEventCount: rawEvents.length,
    workflowHint: inferWorkflowHint(runDir),
  };

  await writeFile(actionsPath, `${JSON.stringify(actions, null, 2)}\n`, "utf8");

  return {
    actions,
    actionsPath,
    runDir,
    sanitizedEvents,
    summary,
  };
}

async function main() {
  const runDirArg = process.argv.slice(2).find((arg) => !arg.startsWith("--"));
  const result = await compactRun(runDirArg);
  process.stdout.write(`${JSON.stringify(result.summary, null, 2)}\n`);
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main().catch((error) => {
    process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
    process.exitCode = 1;
  });
}
