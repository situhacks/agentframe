import { pathToFileURL } from "node:url";
import { requestDemoRecorderStop } from "./demo-recorder-lifecycle.js";

async function main() {
  const controlFile = await requestDemoRecorderStop();
  process.stdout.write(`${JSON.stringify({
    requested: true,
    runDir: controlFile.runDir,
    workflowId: controlFile.workflowId,
  }, null, 2)}\n`);
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main().catch((error) => {
    process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
    process.exitCode = 1;
  });
}
