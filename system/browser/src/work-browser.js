import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
import {
  mkdir as defaultMkdir,
  readFile as defaultReadFile,
  writeFile as defaultWriteFile,
} from "node:fs/promises";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

export const DEFAULT_DEBUGGING_PORT = 9222;
export const DEFAULT_START_URL = "about:blank";

export function normalizeEndpoint(portOrEndpoint) {
  if (typeof portOrEndpoint === "number") {
    return `http://127.0.0.1:${portOrEndpoint}`;
  }

  return portOrEndpoint.replace(/\/$/, "");
}

function getBrowserRoot() {
  return path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
}

export function getDefaultProfilePath() {
  return path.join(getBrowserRoot(), "local", "agentframe-work-profile");
}

export function getDefaultMarkerPath() {
  return path.join(getBrowserRoot(), "local", "runtime", "work-browser-session.json");
}

function normalizePathForMarker(candidate) {
  return path.resolve(candidate).toLowerCase();
}

export function markerMatchesWorkBrowser(marker, endpoint, profilePath) {
  return Boolean(
    marker &&
      marker.browser === "edge" &&
      normalizeEndpoint(marker.endpoint) === normalizeEndpoint(endpoint) &&
      normalizePathForMarker(marker.profilePath) === normalizePathForMarker(profilePath),
  );
}

export async function readWorkBrowserMarker(markerPath = getDefaultMarkerPath()) {
  try {
    return JSON.parse(await defaultReadFile(markerPath, "utf8"));
  } catch {
    return undefined;
  }
}

export async function writeWorkBrowserMarker(markerPath, marker) {
  await defaultMkdir(path.dirname(markerPath), { recursive: true });
  await defaultWriteFile(markerPath, `${JSON.stringify(marker, null, 2)}\n`, "utf8");
}

export function toSafeWorkBrowserEvidence(result) {
  const {
    hasDebuggingWebSocket: _hasDebuggingWebSocket,
    webSocketDebuggerUrl: _webSocketDebuggerUrl,
    ...safe
  } = result;

  return {
    ...safe,
    hasDebuggingEndpoint: Boolean(result.webSocketDebuggerUrl ?? result.hasDebuggingWebSocket),
  };
}

export function findEdgeExecutable(options = {}) {
  const env = options.env ?? process.env;
  const platform = options.platform ?? process.platform;
  const exists = options.exists ?? existsSync;
  const candidates = [
    env.AGENTFRAME_EDGE_PATH,
    platform === "win32" && env.ProgramFiles
      ? path.join(env.ProgramFiles, "Microsoft", "Edge", "Application", "msedge.exe")
      : undefined,
    platform === "win32" && env["ProgramFiles(x86)"]
      ? path.join(env["ProgramFiles(x86)"], "Microsoft", "Edge", "Application", "msedge.exe")
      : undefined,
    platform === "win32" && env.LOCALAPPDATA
      ? path.join(env.LOCALAPPDATA, "Microsoft", "Edge", "Application", "msedge.exe")
      : undefined,
  ];

  return candidates.find((candidate) => Boolean(candidate && exists(candidate)));
}

export function buildEdgeArgs(options) {
  const args = [
    `--remote-debugging-port=${options.port}`,
    `--user-data-dir=${options.profilePath}`,
    "--remote-allow-origins=*",
    "--no-first-run",
    "--no-default-browser-check",
    "--new-window",
  ];

  if (options.startUrl) {
    args.push(options.startUrl);
  }

  return args;
}

export async function probeDevtoolsEndpoint(endpoint, timeoutMs = 2_000) {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), timeoutMs);

  try {
    const response = await fetch(`${normalizeEndpoint(endpoint)}/json/version`, {
      signal: controller.signal,
    });

    if (!response.ok) {
      return {
        endpoint: normalizeEndpoint(endpoint),
        error: `HTTP ${response.status}`,
        ok: false,
      };
    }

    const body = await response.json();
    return {
      browser: body.Browser,
      endpoint: normalizeEndpoint(endpoint),
      ok: Boolean(body.webSocketDebuggerUrl),
      webSocketDebuggerUrl: body.webSocketDebuggerUrl,
    };
  } catch (error) {
    return {
      endpoint: normalizeEndpoint(endpoint),
      error: error instanceof Error ? error.message : String(error),
      ok: false,
    };
  } finally {
    clearTimeout(timeout);
  }
}

export async function ensureWorkBrowser(options = {}) {
  const endpoint = normalizeEndpoint(options.endpoint ?? DEFAULT_DEBUGGING_PORT);
  const profilePath = options.profilePath ?? getDefaultProfilePath();
  const markerPath = options.markerPath ?? getDefaultMarkerPath();
  const probeEndpoint = options.probeEndpoint ?? probeDevtoolsEndpoint;
  const readMarker = options.readMarker ?? readWorkBrowserMarker;
  const writeMarker = options.writeMarker ?? writeWorkBrowserMarker;
  const connectTimeoutMs = options.connectTimeoutMs ?? 10_000;

  const initialProbe = await probeEndpoint(endpoint, 2_000);
  if (initialProbe.ok && initialProbe.webSocketDebuggerUrl) {
    const marker = await readMarker(markerPath);
    if (!markerMatchesWorkBrowser(marker, endpoint, profilePath)) {
      return {
        browser: "edge",
        debuggingEndpoint: endpoint,
        edgePath: options.edgePath,
        hasDebuggingWebSocket: true,
        profilePath,
        status: "externally_managed",
        warning:
          "DevTools endpoint is reachable but is not owned by Work Browser Mode for the expected profile. Refusing to attach.",
      };
    }

    return {
      browser: "edge",
      debuggingEndpoint: endpoint,
      edgePath: options.edgePath,
      hasDebuggingWebSocket: true,
      profilePath,
      status: "already_running",
      webSocketDebuggerUrl: initialProbe.webSocketDebuggerUrl,
    };
  }

  const edgePath =
    options.edgePath ??
    findEdgeExecutable({
      env: options.env,
      exists: options.exists,
      platform: options.platform,
    });

  if (!edgePath) {
    throw new Error("Microsoft Edge was not found. Set AGENTFRAME_EDGE_PATH to the msedge.exe path.");
  }

  const mkdir = options.mkdir ?? defaultMkdir;
  await mkdir(profilePath, { recursive: true });

  const port = new URL(endpoint).port ? Number(new URL(endpoint).port) : DEFAULT_DEBUGGING_PORT;
  const args = buildEdgeArgs({
    port,
    profilePath,
    startUrl: options.startUrl ?? DEFAULT_START_URL,
  });

  const spawnBrowser =
    options.spawnBrowser ??
    ((browserPath, browserArgs) => {
      const child = spawn(browserPath, browserArgs, {
        detached: true,
        stdio: "ignore",
        windowsHide: false,
      });
      child.unref();
      return { pid: child.pid };
    });

  const launched = spawnBrowser(edgePath, args);
  const deadline = Date.now() + connectTimeoutMs;
  const wait = options.wait ?? ((ms) => new Promise((resolve) => setTimeout(resolve, ms)));

  while (Date.now() <= deadline) {
    const probe = await probeEndpoint(endpoint, 2_000);
    if (probe.ok && probe.webSocketDebuggerUrl) {
      await writeMarker(markerPath, {
        browser: "edge",
        endpoint,
        pid: launched.pid,
        profilePath,
        timestamp: new Date().toISOString(),
      });

      return {
        browser: "edge",
        browserPid: launched.pid,
        debuggingEndpoint: endpoint,
        edgePath,
        hasDebuggingWebSocket: true,
        profilePath,
        status: "launched",
        webSocketDebuggerUrl: probe.webSocketDebuggerUrl,
      };
    }

    await wait(500);
  }

  throw new Error(`Work Browser Mode did not expose DevTools at ${endpoint}`);
}

function parseCliArgs(argv) {
  const options = {};
  for (const arg of argv) {
    if (arg.startsWith("--port=")) {
      options.endpoint = normalizeEndpoint(Number(arg.slice("--port=".length)));
    } else if (arg.startsWith("--profile=")) {
      options.profilePath = arg.slice("--profile=".length);
    } else if (arg.startsWith("--start-url=")) {
      options.startUrl = arg.slice("--start-url=".length);
    } else if (arg.startsWith("--edge-path=")) {
      options.edgePath = arg.slice("--edge-path=".length);
    }
  }

  options.edgePath ??= process.env.AGENTFRAME_EDGE_PATH;
  options.profilePath ??= process.env.AGENTFRAME_WORK_PROFILE;
  options.endpoint ??= normalizeEndpoint(
    process.env.AGENTFRAME_BROWSER_PORT ? Number(process.env.AGENTFRAME_BROWSER_PORT) : DEFAULT_DEBUGGING_PORT,
  );

  return options;
}

async function main() {
  const result = await ensureWorkBrowser(parseCliArgs(process.argv.slice(2)));
  process.stdout.write(`${JSON.stringify(toSafeWorkBrowserEvidence(result), null, 2)}\n`);
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main().catch((error) => {
    process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
    process.exitCode = 1;
  });
}
