import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
import {
  mkdir as defaultMkdir,
  readFile as defaultReadFile,
  writeFile as defaultWriteFile,
} from "node:fs/promises";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

export const DEFAULT_BROWSER = "edge";
export const DEFAULT_DEBUGGING_PORT = 9222;
export const DEFAULT_START_URL = "about:blank";

// One controlled profile per browser, each on its own port, so the work (Edge) and
// home (Chrome) browsers can run at the same time without fighting over DevTools.
export const BROWSERS = {
  edge: {
    defaultPort: 9222,
    executableEnvVar: "AGENTFRAME_EDGE_PATH",
    executableName: "msedge.exe",
    label: "Microsoft Edge",
    markerFile: "work-browser-session.json",
    posixCandidates: ["/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge", "/usr/bin/microsoft-edge"],
    profileDir: "agentframe-work-profile",
    profileEnvVar: "AGENTFRAME_WORK_PROFILE",
    vendorPath: ["Microsoft", "Edge", "Application"],
  },
  chrome: {
    defaultPort: 9223,
    executableEnvVar: "AGENTFRAME_CHROME_PATH",
    executableName: "chrome.exe",
    label: "Google Chrome",
    markerFile: "home-browser-session.json",
    posixCandidates: ["/Applications/Google Chrome.app/Contents/MacOS/Google Chrome", "/usr/bin/google-chrome"],
    profileDir: "agentframe-home-profile",
    profileEnvVar: "AGENTFRAME_HOME_PROFILE",
    vendorPath: ["Google", "Chrome", "Application"],
  },
};

export function resolveBrowser(name = DEFAULT_BROWSER) {
  const key = String(name).toLowerCase();
  if (!BROWSERS[key]) {
    throw new Error(`Unknown browser "${name}". Supported: ${Object.keys(BROWSERS).join(", ")}.`);
  }

  return key;
}

export function normalizeEndpoint(portOrEndpoint) {
  if (typeof portOrEndpoint === "number") {
    return `http://127.0.0.1:${portOrEndpoint}`;
  }

  return portOrEndpoint.replace(/\/$/, "");
}

function getBrowserRoot() {
  return path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
}

export function getDefaultProfilePath(browser = DEFAULT_BROWSER) {
  return path.join(getBrowserRoot(), "local", BROWSERS[resolveBrowser(browser)].profileDir);
}

export function getDefaultMarkerPath(browser = DEFAULT_BROWSER) {
  return path.join(getBrowserRoot(), "local", "runtime", BROWSERS[resolveBrowser(browser)].markerFile);
}

function normalizePathForMarker(candidate) {
  return path.resolve(candidate).toLowerCase();
}

export function markerMatchesWorkBrowser(marker, endpoint, profilePath, browser = DEFAULT_BROWSER) {
  return Boolean(
    marker &&
      marker.browser === resolveBrowser(browser) &&
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

export function findBrowserExecutable(browser = DEFAULT_BROWSER, options = {}) {
  const spec = BROWSERS[resolveBrowser(browser)];
  const env = options.env ?? process.env;
  const platform = options.platform ?? process.platform;
  const exists = options.exists ?? existsSync;
  const windowsRoots = [env.ProgramFiles, env["ProgramFiles(x86)"], env.LOCALAPPDATA];
  const candidates = [
    env[spec.executableEnvVar],
    ...(platform === "win32"
      ? windowsRoots.map((root) => (root ? path.join(root, ...spec.vendorPath, spec.executableName) : undefined))
      : spec.posixCandidates),
  ];

  return candidates.find((candidate) => Boolean(candidate && exists(candidate)));
}

export function findEdgeExecutable(options = {}) {
  return findBrowserExecutable("edge", options);
}

export function buildBrowserArgs(options) {
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

export const buildEdgeArgs = buildBrowserArgs;

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
  const browser = resolveBrowser(options.browser ?? DEFAULT_BROWSER);
  const spec = BROWSERS[browser];
  const endpoint = normalizeEndpoint(options.endpoint ?? spec.defaultPort);
  const profilePath = options.profilePath ?? getDefaultProfilePath(browser);
  const markerPath = options.markerPath ?? getDefaultMarkerPath(browser);
  const probeEndpoint = options.probeEndpoint ?? probeDevtoolsEndpoint;
  const readMarker = options.readMarker ?? readWorkBrowserMarker;
  const writeMarker = options.writeMarker ?? writeWorkBrowserMarker;
  const connectTimeoutMs = options.connectTimeoutMs ?? 10_000;

  const initialProbe = await probeEndpoint(endpoint, 2_000);
  if (initialProbe.ok && initialProbe.webSocketDebuggerUrl) {
    const marker = await readMarker(markerPath);
    if (!markerMatchesWorkBrowser(marker, endpoint, profilePath, browser)) {
      return {
        browser,
        debuggingEndpoint: endpoint,
        executablePath: options.executablePath ?? options.edgePath,
        hasDebuggingWebSocket: true,
        profilePath,
        status: "externally_managed",
        warning:
          "DevTools endpoint is reachable but is not owned by Work Browser Mode for the expected profile. Refusing to attach.",
      };
    }

    return {
      browser,
      debuggingEndpoint: endpoint,
      executablePath: options.executablePath ?? options.edgePath,
      hasDebuggingWebSocket: true,
      profilePath,
      status: "already_running",
      webSocketDebuggerUrl: initialProbe.webSocketDebuggerUrl,
    };
  }

  const executablePath =
    options.executablePath ??
    options.edgePath ??
    findBrowserExecutable(browser, {
      env: options.env,
      exists: options.exists,
      platform: options.platform,
    });

  if (!executablePath) {
    throw new Error(`${spec.label} was not found. Set ${spec.executableEnvVar} to the ${spec.executableName} path.`);
  }

  const mkdir = options.mkdir ?? defaultMkdir;
  await mkdir(profilePath, { recursive: true });

  const port = new URL(endpoint).port ? Number(new URL(endpoint).port) : spec.defaultPort;
  const args = buildBrowserArgs({
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

  const launched = spawnBrowser(executablePath, args);
  const deadline = Date.now() + connectTimeoutMs;
  const wait = options.wait ?? ((ms) => new Promise((resolve) => setTimeout(resolve, ms)));

  while (Date.now() <= deadline) {
    const probe = await probeEndpoint(endpoint, 2_000);
    if (probe.ok && probe.webSocketDebuggerUrl) {
      await writeMarker(markerPath, {
        browser,
        endpoint,
        pid: launched.pid,
        profilePath,
        timestamp: new Date().toISOString(),
      });

      return {
        browser,
        browserPid: launched.pid,
        debuggingEndpoint: endpoint,
        executablePath,
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
    if (arg.startsWith("--browser=")) {
      options.browser = arg.slice("--browser=".length);
    } else if (arg.startsWith("--port=")) {
      options.endpoint = normalizeEndpoint(Number(arg.slice("--port=".length)));
    } else if (arg.startsWith("--profile=")) {
      options.profilePath = arg.slice("--profile=".length);
    } else if (arg.startsWith("--start-url=")) {
      options.startUrl = arg.slice("--start-url=".length);
    } else if (arg.startsWith("--edge-path=") || arg.startsWith("--browser-path=")) {
      options.executablePath = arg.slice(arg.indexOf("=") + 1);
    }
  }

  const browser = resolveBrowser(options.browser ?? process.env.AGENTFRAME_BROWSER ?? DEFAULT_BROWSER);
  const spec = BROWSERS[browser];

  options.browser = browser;
  options.executablePath ??= process.env[spec.executableEnvVar];
  options.profilePath ??= process.env[spec.profileEnvVar];
  options.endpoint ??= normalizeEndpoint(
    process.env.AGENTFRAME_BROWSER_PORT ? Number(process.env.AGENTFRAME_BROWSER_PORT) : spec.defaultPort,
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
