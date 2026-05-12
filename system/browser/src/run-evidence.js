import { mkdir, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const SECRET_KEY_PATTERN =
  /(password|passwd|cookie|token|secret|credential|authorization|session|webSocketDebuggerUrl)/i;

export function getBrowserRoot() {
  return path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
}

export function getRunsRoot() {
  return path.join(getBrowserRoot(), "runs");
}

export function getWorkflowRunsRoot(workflowPath) {
  const resolvedWorkflowPath = path.resolve(workflowPath);
  const workflowDir = path.basename(resolvedWorkflowPath).toLowerCase() === "recipe.md"
    ? path.dirname(resolvedWorkflowPath)
    : resolvedWorkflowPath;
  return path.join(workflowDir, "runs");
}

export function resolveRunsRoot(options = {}) {
  if (options.runsRoot) {
    return path.resolve(options.runsRoot);
  }
  if (options.workflowPath) {
    return getWorkflowRunsRoot(options.workflowPath);
  }
  return getRunsRoot();
}

export function buildRunDirectoryName(workflowId, date = new Date()) {
  const timestamp = date.toISOString().replace(/[:.]/g, "-");
  const safeWorkflow = workflowId
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-|-$/g, "");
  return `${timestamp}_${safeWorkflow || "browser-run"}`;
}

export function redactEvidence(value) {
  if (Array.isArray(value)) {
    return value.map((item) => redactEvidence(item));
  }

  if (value && typeof value === "object") {
    return Object.fromEntries(
      Object.entries(value).map(([key, nestedValue]) => [
        key,
        SECRET_KEY_PATTERN.test(key) ? "[redacted]" : redactEvidence(nestedValue),
      ]),
    );
  }

  if (typeof value === "string" && /^https?:\/\//i.test(value)) {
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

  return value;
}

export function summarizeError(error) {
  if (error instanceof Error) {
    return {
      message: error.message,
      name: error.name,
    };
  }

  return {
    message: String(error),
    name: "Error",
  };
}

export async function createRunDirectory(workflowId, date = new Date(), options = {}) {
  const runDir = path.join(resolveRunsRoot(options), buildRunDirectoryName(workflowId, date));
  await mkdir(runDir, { recursive: true });
  return runDir;
}

export async function writeRunJson(filePath, evidence) {
  await mkdir(path.dirname(filePath), { recursive: true });
  await writeFile(filePath, `${JSON.stringify(redactEvidence(evidence), null, 2)}\n`, "utf8");
}
