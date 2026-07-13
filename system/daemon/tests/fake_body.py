import argparse
import json
import time
from pathlib import Path


parser = argparse.ArgumentParser()
parser.add_argument("--task-file", required=True)
parser.add_argument("--result-file", required=True)
args = parser.parse_args()

task = json.loads(Path(args.task_file).read_text(encoding="utf-8"))
mode = task.get("mode", "done")
if mode == "timeout":
    time.sleep(10)
elif mode == "no-receipt":
    print("body exited without receipt")
else:
    payload = {
        "schema_version": 1,
        "task_id": task["id"],
        "status": mode if mode in {"done", "blocked", "failed"} else "failed",
        "summary": f"fake body returned {mode}",
        "outputs": [],
        "operator_action": "review" if mode == "blocked" else None,
    }
    Path(args.result_file).write_text(json.dumps(payload), encoding="utf-8")
