import { existsSync } from "node:fs";
import { mkdir, readFile, rm, writeFile } from "node:fs/promises";
import path from "node:path";
import { getBrowserRoot } from "./run-evidence.js";

export function getDefaultDemoRecorderRuntimeDir() {
  return path.join(getBrowserRoot(), "local", "runtime");
}

export function getDefaultDemoRecorderCurrentPath() {
  return path.join(getDefaultDemoRecorderRuntimeDir(), "demo-recorder-current.json");
}

export function buildDemoRecorderControlFile(options) {
  const runtimeDir = options.runtimeDir ?? getDefaultDemoRecorderRuntimeDir();
  return {
    pid: options.pid,
    runDir: options.runDir,
    startedAt: options.startedAt,
    stopFile: path.join(runtimeDir, `demo-recorder-stop-${options.pid}.json`),
    workflowId: options.workflowId,
  };
}

async function writeJsonFile(filePath, value) {
  await mkdir(path.dirname(filePath), { recursive: true });
  await writeFile(filePath, `${JSON.stringify(value, null, 2)}\n`, "utf8");
}

export async function readDemoRecorderCurrentRun(currentRunPath = getDefaultDemoRecorderCurrentPath()) {
  try {
    return JSON.parse(await readFile(currentRunPath, "utf8"));
  } catch {
    return undefined;
  }
}

export async function writeDemoRecorderControlFile(controlFile, currentRunPath = getDefaultDemoRecorderCurrentPath()) {
  await writeJsonFile(currentRunPath, controlFile);
}

export async function clearDemoRecorderControlFiles(controlFile, currentRunPath = getDefaultDemoRecorderCurrentPath()) {
  await rm(currentRunPath, { force: true }).catch(() => {});
  if (controlFile?.stopFile) {
    await rm(controlFile.stopFile, { force: true }).catch(() => {});
  }
}

export async function requestDemoRecorderStop(options = {}) {
  const currentRunPath = options.currentRunPath ?? getDefaultDemoRecorderCurrentPath();
  const readCurrentRun = options.readCurrentRun ?? readDemoRecorderCurrentRun;
  const writeJson = options.writeJson ?? writeJsonFile;
  const controlFile = await readCurrentRun(currentRunPath);

  if (!controlFile) {
    throw new Error(`No active demo recorder control file found at ${currentRunPath}`);
  }

  await writeJson(controlFile.stopFile, {
    requestedAt: (options.now ?? (() => new Date().toISOString()))(),
    reason: "agent_stop",
    runDir: controlFile.runDir,
    workflowId: controlFile.workflowId,
  });

  return controlFile;
}

export function hasDemoRecorderStopFile(stopFile) {
  return existsSync(stopFile);
}

export function buildRecorderStatus(options) {
  if (options.error) {
    return "failed";
  }
  if (options.stopReason === "timeout" && options.eventsCaptured === 0) {
    return "timeout_no_events";
  }
  return "recorded";
}

export function buildDemoRecorderEvidencePaths(evidence) {
  return {
    events: evidence.eventsPath,
    finalScreenshot: evidence.finalObservation?.screenshotPath ?? null,
    initialScreenshot: evidence.initialObservation?.screenshotPath ?? null,
    result: evidence.resultPath,
  };
}

export function finalizeDemoRecorderEvidence(options) {
  let finalizePromise = null;

  return async () => {
    finalizePromise ??= (async () => {
      const eventsPath = path.join(options.evidence.runDir, "events.json");
      const resultPath = path.join(options.evidence.runDir, "result.json");
      const stopReason = options.getStopReason ? options.getStopReason() : options.stopReason ?? null;
      const terminalError = options.getTerminalError ? options.getTerminalError() : options.terminalError ?? null;
      const writeJson = options.writeJson;

      options.evidence.completedAt = (options.now ?? (() => new Date().toISOString()))();
      options.evidence.eventsCaptured = options.events.length;
      options.evidence.eventsPath = eventsPath;
      options.evidence.resultPath = resultPath;
      options.evidence.stopReason = stopReason;
      options.evidence.status = buildRecorderStatus({
        error: terminalError,
        eventsCaptured: options.events.length,
        stopReason,
      });
      options.evidence.evidencePaths = buildDemoRecorderEvidencePaths(options.evidence);

      await writeJson(eventsPath, options.events);
      await writeJson(resultPath, options.evidence);
      return options.evidence;
    })();

    return finalizePromise;
  };
}
