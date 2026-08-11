#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 5 ]]; then
  echo "usage: run-participant.sh IMAGE_ID REQUEST CODEX_AUTH ANSWER AUDIT" >&2
  exit 64
fi

image_id=$1
request=$2
codex_auth=$3
answer=$4
audit=$5
deadline_seconds=1200

if [[ ! $image_id =~ ^sha256:[0-9a-f]{64}$ ]]; then
  echo "run-participant: image must be an exact sha256 image ID" >&2
  exit 64
fi
if [[ -e $answer || -L $answer || -e $audit || -L $audit ]]; then
  echo "run-participant: retained outputs must be new nonsymlink paths" >&2
  exit 64
fi
if [[ ! -f $request || -L $request || ! -f $codex_auth || -L $codex_auth ]]; then
  echo "run-participant: inputs must be regular nonsymlink files" >&2
  exit 64
fi

runtime_uid=$(id -u)
runtime_gid=$(id -g)
if [[ $runtime_uid -eq 0 ]]; then
  echo "run-participant: root execution is forbidden" >&2
  exit 64
fi

python3 - "$request" "$codex_auth" "$answer" "$audit" "$runtime_uid" <<'PY'
import os, stat, subprocess, sys
from pathlib import Path

request, source_auth, answer, audit = map(Path, sys.argv[1:5])
runtime_uid = int(sys.argv[5])
for path, mode, private, label in (
    (request, 0o444, False, "request"),
    (source_auth, None, True, "source auth"),
):
    try:
        meta = path.lstat()
    except OSError:
        raise SystemExit(f"run-participant: {label} cannot be inspected")
    if not stat.S_ISREG(meta.st_mode) or stat.S_ISLNK(meta.st_mode):
        raise SystemExit(f"run-participant: {label} must be regular")
    if mode is not None and stat.S_IMODE(meta.st_mode) != mode:
        raise SystemExit(f"run-participant: {label} mode is invalid")
    if private and stat.S_IMODE(meta.st_mode) & 0o077:
        raise SystemExit(f"run-participant: {label} must be private")

def reject_git(parent: Path) -> None:
    current = parent
    while True:
        if (current / ".git").exists() or (current / ".git").is_symlink():
            raise SystemExit("run-participant: retained outputs must be outside Git")
        if (current / "HEAD").is_file() and (current / "objects").is_dir():
            raise SystemExit("run-participant: retained outputs must be outside bare Git")
        if current == current.parent:
            break
        current = current.parent
    environment = {
        "PATH": os.environ.get("PATH", ""),
        "GIT_CONFIG_NOSYSTEM": "1",
        "GIT_NO_LAZY_FETCH": "1",
        "GIT_OPTIONAL_LOCKS": "0",
        "GIT_TERMINAL_PROMPT": "0",
    }
    for predicate in ("--is-inside-work-tree", "--is-inside-git-dir"):
        probe = subprocess.run(
            ["git", "-C", str(parent), "rev-parse", predicate],
            capture_output=True,
            text=True,
            env=environment,
        )
        if probe.returncode == 0 and probe.stdout.strip() == "true":
            raise SystemExit("run-participant: retained outputs must be outside Git")

for output in (answer, audit):
    if os.path.lexists(output):
        raise SystemExit("run-participant: retained outputs must be new")
    try:
        parent = output.parent.resolve(strict=True)
        meta = parent.lstat()
    except OSError:
        raise SystemExit("run-participant: retained output parent cannot be inspected")
    if not stat.S_ISDIR(meta.st_mode) or stat.S_ISLNK(meta.st_mode):
        raise SystemExit("run-participant: retained output parent must be a real directory")
    if meta.st_uid != runtime_uid or stat.S_IMODE(meta.st_mode) & 0o077:
        raise SystemExit("run-participant: retained output parent must be private and owned")
    if parent != output.parent.absolute():
        raise SystemExit("run-participant: retained output parent must be canonical")
    reject_git(parent)
PY

umask 077
retention_parent=$(cd "$(dirname "$answer")" && pwd -P)
run_directory=$(mktemp -d "$retention_parent/.vela-pi-sensitive.XXXXXXXX")
evidence_directory=$(mktemp -d "$retention_parent/.vela-pi-evidence.XXXXXXXX")
chmod 0700 "$run_directory" "$evidence_directory"
derived_auth="$run_directory/auth.json"
broker_socket="$run_directory/inference.sock"
broker_audit="$evidence_directory/broker-audit.jsonl"
broker_cidfile="$run_directory/broker.cid"
derive_cidfile="$run_directory/derive.cid"
probe_cidfile="$run_directory/probe.cid"
participant_cidfile="$run_directory/participant.cid"
auth_report="$evidence_directory/auth-preflight.json"
probe_report="$evidence_directory/container-probe.json"
participant_answer="$evidence_directory/answer.raw"
participant_audit="$evidence_directory/participant-audit.jsonl"
closed_audit="$evidence_directory/closed-audit.jsonl"
broker_pid=
command_pid=
finished=0
deadline_epoch=$((SECONDS + deadline_seconds))

stop_container_file() {
  local cidfile=$1
  if [[ -s $cidfile ]]; then
    local cid
    cid=$(<"$cidfile")
    if [[ $cid =~ ^[0-9a-f]{12,64}$ ]]; then
      docker stop --time 1 "$cid" >/dev/null 2>&1 || true
    fi
  fi
}

cleanup_best_effort() {
  if [[ -n ${command_pid:-} ]]; then
    kill "$command_pid" >/dev/null 2>&1 || true
    wait "$command_pid" >/dev/null 2>&1 || true
  fi
  for cidfile in "$participant_cidfile" "$probe_cidfile" "$derive_cidfile" "$broker_cidfile"; do
    stop_container_file "$cidfile"
  done
  if [[ -n ${broker_pid:-} ]]; then
    kill "$broker_pid" >/dev/null 2>&1 || true
    wait "$broker_pid" >/dev/null 2>&1 || true
  fi
  if [[ -d ${run_directory:-} ]]; then
    find "$run_directory" -mindepth 1 -maxdepth 1 -type s -delete 2>/dev/null || true
    find "$run_directory" -mindepth 1 -maxdepth 1 -type f -delete 2>/dev/null || true
    rmdir "$run_directory" 2>/dev/null || true
  fi
}

on_exit() {
  local status=$?
  if [[ $finished -ne 1 ]]; then
    cleanup_best_effort
    sensitive_cleanup_complete=false
    if [[ ! -e ${derived_auth:-/nonexistent} && ! -L ${derived_auth:-/nonexistent} && ! -e ${broker_socket:-/nonexistent} && ! -L ${broker_socket:-/nonexistent} && ! -e ${run_directory:-/nonexistent} ]]; then
      sensitive_cleanup_complete=true
    else
      echo "run-participant: WARNING sensitive runtime cleanup is incomplete" >&2
    fi
    if [[ -d ${evidence_directory:-} ]]; then
      chmod 0700 "$evidence_directory" 2>/dev/null || true
      python3 - "$evidence_directory" "$status" "$request" "$image_id" "$sensitive_cleanup_complete" <<'PY' || true
import hashlib, json, os, sys
from pathlib import Path
root = Path(sys.argv[1])
request_bytes = Path(sys.argv[3]).read_bytes()
request = json.loads(request_bytes)
allowed = {
    "auth-preflight.json", "container-probe.json", "answer.raw",
    "participant-audit.jsonl", "broker-audit.jsonl", "closed-audit.jsonl",
}
actual = {path.name for path in root.iterdir()}
unexpected = actual - allowed
if unexpected:
    raise SystemExit("unexpected failure-evidence entry")
files = []
for path in sorted(root.iterdir()):
    metadata = path.lstat()
    if not path.is_file() or path.is_symlink():
        raise SystemExit("nonregular failure-evidence entry")
    data = path.read_bytes()
    files.append({
        "path": path.name,
        "mode": f"{metadata.st_mode & 0o777:04o}",
        "bytes": len(data),
        "raw_root": "sha256:" + hashlib.sha256(data).hexdigest(),
    })
record = {
    "schema": "vela.claim-dependency-pi-failure-custody.v1",
    "execution_status": "failed",
    "exit_status": int(sys.argv[2]),
    "image_id": sys.argv[4],
    "sensitive_cleanup_complete": sys.argv[5] == "true",
    "request_raw_root": "sha256:" + hashlib.sha256(request_bytes).hexdigest(),
    "run_id": request.get("run_id"),
    "arm": request.get("arm"),
    "session_id": request.get("session_id"),
    "files": files,
    "answer_usable_status": "not_determined",
    "scientific_disposition": "uninterpreted_failure_evidence_retained",
    "retry_authorized": False,
    "authority_effect": "none",
    "claim_credit": False,
}
path = root / "failure-custody.json"
flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_NOFOLLOW", 0)
descriptor = os.open(path, flags, 0o600)
try:
    os.write(descriptor, json.dumps(record, separators=(",", ":")).encode() + b"\n")
    os.fsync(descriptor)
finally:
    os.close(descriptor)
PY
      echo "run-participant: nonsecret failure evidence retained at $evidence_directory" >&2
    fi
  fi
  exit "$status"
}
trap on_exit EXIT
trap 'exit 124' HUP INT TERM

run_bounded() {
  "$@" &
  command_pid=$!
  while kill -0 "$command_pid" 2>/dev/null; do
    if (( SECONDS >= deadline_epoch )); then
      kill "$command_pid" >/dev/null 2>&1 || true
      wait "$command_pid" >/dev/null 2>&1 || true
      command_pid=
      echo "run-participant: absolute wall deadline exceeded" >&2
      return 124
    fi
    sleep 0.2
  done
  local status=0
  wait "$command_pid" || status=$?
  command_pid=
  return "$status"
}

run_bounded docker run --rm \
  --cidfile "$derive_cidfile" \
  --network none \
  --read-only \
  --cap-drop ALL \
  --security-opt no-new-privileges \
  --user "$runtime_uid:$runtime_gid" \
  --workdir /workspace \
  --mount "type=bind,src=$codex_auth,dst=/source/codex-auth.json,readonly" \
  --mount "type=bind,src=$run_directory,dst=/auth-out" \
  --entrypoint node \
  "$image_id" \
  /opt/participant/auth-preflight.mjs derive \
  --codex-auth /source/codex-auth.json \
  --output /auth-out/auth.json >"$auth_report"

python3 - "$derived_auth" "$runtime_uid" "$auth_report" <<'PY'
import json, os, stat, sys
path, uid, report_path = sys.argv[1], int(sys.argv[2]), sys.argv[3]
meta = os.lstat(path)
if not stat.S_ISREG(meta.st_mode) or stat.S_IMODE(meta.st_mode) != 0o400 or meta.st_uid != uid:
    raise SystemExit("run-participant: derived auth owner or mode is invalid")
with open(report_path, "rb") as source:
    raw = source.read()
report = json.loads(raw)
expected = {
    "provider": "openai-codex",
    "credential_type": "oauth",
    "validity_window": "at_least_6h",
    "output_mode_0400": True,
    "refresh_forbidden": True,
    "real_refresh_copied": False,
    "mutation_refused": True,
}
if report != expected or raw != json.dumps(report, separators=(",", ":")).encode() + b"\n":
    raise SystemExit("run-participant: auth preflight report drifted")
PY

run_bounded docker run --rm \
  --cidfile "$probe_cidfile" \
  --network none \
  --read-only \
  --cap-drop ALL \
  --security-opt no-new-privileges \
  --user "$runtime_uid:$runtime_gid" \
  --workdir /workspace \
  --mount "type=bind,src=$request,dst=/run/request.json,readonly" \
  --mount "type=bind,src=$derived_auth,dst=/run/auth.json,readonly" \
  --entrypoint node \
  "$image_id" \
  -e 'const fs=require("fs"); fs.readFileSync("/run/request.json"); fs.readFileSync("/run/auth.json"); let refused=0; for (const p of ["/run/request.json","/run/auth.json"]) { try { fs.appendFileSync(p,"x"); } catch { refused++; } } if (process.getuid()===0 || refused!==2) process.exit(1); process.stdout.write(JSON.stringify({schema:"vela.claim-dependency-pi-container-probe.v1",nonroot:true,request_read:true,auth_read:true,request_write_refused:true,auth_write_refused:true})+"\n");' >"$probe_report"

docker run --rm \
  --cidfile "$broker_cidfile" \
  --network bridge \
  --read-only \
  --cap-drop ALL \
  --security-opt no-new-privileges \
  --pids-limit 64 \
  --memory 512m \
  --cpus 1 \
  --user "$runtime_uid:$runtime_gid" \
  --workdir /workspace \
  --mount "type=bind,src=$request,dst=/run/request.json,readonly" \
  --mount "type=bind,src=$derived_auth,dst=/run/auth.json,readonly" \
  --mount "type=bind,src=$run_directory,dst=/broker" \
  --entrypoint node \
  "$image_id" \
  /opt/participant/egress-broker.mjs \
  --socket /broker/inference.sock \
  --request /run/request.json \
  --auth /run/auth.json >"$broker_audit" 2>&1 &
broker_pid=$!

while [[ ! -S $broker_socket ]]; do
  if ! kill -0 "$broker_pid" 2>/dev/null; then
    echo "run-participant: bounded egress broker failed before readiness" >&2
    exit 1
  fi
  if (( SECONDS >= deadline_epoch )); then
    echo "run-participant: absolute wall deadline exceeded before broker readiness" >&2
    exit 124
  fi
  sleep 0.05
done

run_bounded docker run --rm \
  --cidfile "$participant_cidfile" \
  --network none \
  --read-only \
  --cap-drop ALL \
  --security-opt no-new-privileges \
  --pids-limit 128 \
  --memory 4g \
  --cpus 2 \
  --user "$runtime_uid:$runtime_gid" \
  --workdir /workspace \
  --env VELA_PI_BROKER_SOCKET=/broker/inference.sock \
  --mount "type=bind,src=$request,dst=/run/request.json,readonly" \
  --mount "type=bind,src=$derived_auth,dst=/run/auth.json,readonly" \
  --mount "type=bind,src=$run_directory,dst=/broker,readonly" \
  "$image_id" \
  --request /run/request.json \
  --auth /run/auth.json >"$participant_answer" 2>"$participant_audit"

while kill -0 "$broker_pid" 2>/dev/null; do
  if (( SECONDS >= deadline_epoch )); then
    echo "run-participant: absolute wall deadline exceeded while closing broker" >&2
    exit 124
  fi
  sleep 0.05
done
wait "$broker_pid"
broker_pid=

VELA_IMAGE_ID="$image_id" python3 - "$request" "$auth_report" "$probe_report" "$participant_answer" "$participant_audit" "$broker_audit" "$closed_audit" <<'PY'
import hashlib, json, os, stat, sys
from pathlib import Path

request_path, auth_path, probe_path, answer_path, participant_path, broker_path, output_path = map(Path, sys.argv[1:])
def root(data: bytes) -> str:
    return "sha256:" + hashlib.sha256(data).hexdigest()
def one(path: Path):
    raw = path.read_bytes()
    value = json.loads(raw)
    if raw != json.dumps(value, separators=(",", ":")).encode() + b"\n":
        raise SystemExit("run-participant: compact JSON custody record drifted")
    return value
def lines(path: Path):
    raw = path.read_bytes()
    if not raw.endswith(b"\n"):
        raise SystemExit("run-participant: custody JSONL lacks terminal newline")
    result = [json.loads(line) for line in raw.splitlines()]
    if raw != b"".join(json.dumps(value, separators=(",", ":")).encode() + b"\n" for value in result):
        raise SystemExit("run-participant: custody JSONL is not compact exact JSON")
    return result
def exact(value, keys, label):
    if not isinstance(value, dict) or set(value) != set(keys):
        raise SystemExit(f"run-participant: {label} key set drifted")

request = one(request_path)
auth = one(auth_path)
probe = one(probe_path)
expected_auth = {
    "provider": "openai-codex", "credential_type": "oauth", "validity_window": "at_least_6h",
    "output_mode_0400": True, "refresh_forbidden": True, "real_refresh_copied": False, "mutation_refused": True,
}
expected_probe = {
    "schema": "vela.claim-dependency-pi-container-probe.v1", "nonroot": True,
    "request_read": True, "auth_read": True, "request_write_refused": True, "auth_write_refused": True,
}
if auth != expected_auth or probe != expected_probe:
    raise SystemExit("run-participant: preflight custody record drifted")

participant = lines(participant_path)
if len(participant) < 3 or participant[0].get("kind") != "start" or participant[-1].get("kind") != "final":
    raise SystemExit("run-participant: participant audit envelope drifted")
if any(row.get("schema") != "vela.claim-dependency-pi-participant-audit.v1" for row in participant):
    raise SystemExit("run-participant: participant audit schema drifted")
events = participant[1:-1]
if any(row.get("kind") != "pi_event" or row.get("sequence") != index for index, row in enumerate(events, 1)):
    raise SystemExit("run-participant: participant event sequence drifted")
start, final = participant[0], participant[-1]
exact(start, {
    "schema", "kind", "run_id", "arm", "session_id", "system_prompt_raw_root", "user_message_raw_root",
    "sanitized_environment_names", "model_visible_filesystem_inputs", "active_tools", "transport_boundary",
}, "participant start")
for row in events:
    exact(row, {"schema", "kind", "sequence", "event_type"}, "participant event")
exact(final, {
    "schema", "kind", "run_id", "arm", "session_id", "message_roles", "session_counts", "usage", "active_tools",
    "retry_attempt", "compacting", "pending_messages", "effective_system_prompt_raw_root", "user_message_raw_root",
    "answer_raw_root", "event_count", "event_types", "sanitized_environment_names",
}, "participant final")
exact(final.get("session_counts"), {"user_messages", "assistant_messages", "tool_calls", "tool_results", "total_messages"}, "participant counts")
exact(final.get("usage"), {"input", "output", "cache_read", "cache_write", "total"}, "participant usage")
for key in ("run_id", "arm", "session_id"):
    if start.get(key) != request[key] or final.get(key) != request[key]:
        raise SystemExit("run-participant: participant run binding drifted")
if start.get("system_prompt_raw_root") != request["system_prompt_raw_root"] or start.get("user_message_raw_root") != request["user_message_raw_root"]:
    raise SystemExit("run-participant: participant prompt binding drifted")
if start.get("model_visible_filesystem_inputs") != 0 or start.get("active_tools") != [] or start.get("transport_boundary") != "one_request_unix_socket_broker":
    raise SystemExit("run-participant: participant start boundary drifted")
event_types = [row.get("event_type") for row in events]
if final.get("event_count") != len(events) or final.get("event_types") != event_types:
    raise SystemExit("run-participant: participant final event binding drifted")
if final.get("message_roles") != ["user", "assistant"] or final.get("active_tools") != []:
    raise SystemExit("run-participant: participant message/tool boundary drifted")
counts = final.get("session_counts", {})
if counts.get("user_messages") != 1 or counts.get("assistant_messages") != 1 or counts.get("tool_calls") != 0 or counts.get("tool_results") != 0 or counts.get("total_messages") != 2:
    raise SystemExit("run-participant: participant session counts drifted")
if final.get("retry_attempt") != 0 or final.get("compacting") is not False or final.get("pending_messages") != 0:
    raise SystemExit("run-participant: participant continuation state drifted")
answer = answer_path.read_bytes()
if not answer or final.get("answer_raw_root") != root(answer):
    raise SystemExit("run-participant: retained answer root drifted")

broker = lines(broker_path)
if [row.get("kind") for row in broker] != ["ready", "validated_request", "completed"]:
    raise SystemExit("run-participant: broker audit sequence drifted")
if any(row.get("schema") != "vela.claim-dependency-pi-egress-broker-audit.v1" for row in broker):
    raise SystemExit("run-participant: broker audit schema drifted")
exact(broker[0], {"schema", "kind", "request_count", "target"}, "broker ready")
exact(broker[1], {"schema", "kind", "request_count", "target", "encoded_request_raw_root", "decoded_request_raw_root", "header_names"}, "broker validated request")
exact(broker[2], {"schema", "kind", "request_count", "status", "response_bytes", "response_raw_root", "additional_requests"}, "broker completed")
target = "https://chatgpt.com/backend-api/codex/responses"
if broker[0].get("request_count") != 0 or broker[0].get("target") != target:
    raise SystemExit("run-participant: broker readiness drifted")
if broker[1].get("request_count") != 1 or broker[1].get("target") != target:
    raise SystemExit("run-participant: broker request binding drifted")
if broker[2].get("request_count") != 1 or broker[2].get("additional_requests") != 0 or not isinstance(broker[2].get("status"), int):
    raise SystemExit("run-participant: broker completion drifted")

records = [{
    "schema": "vela.claim-dependency-pi-runner-audit.v1", "kind": "preflight",
    "absolute_wall_deadline_seconds": 1200, "image_id": os.environ["VELA_IMAGE_ID"],
    "authority_effect": "none", "claim_credit": False,
}, auth, probe, *participant, *broker]
with open(output_path, "xb") as sink:
    for record in records:
        sink.write(json.dumps(record, separators=(",", ":")).encode() + b"\n")
    sink.flush()
    os.fsync(sink.fileno())
os.chmod(output_path, 0o600)
PY

for cidfile in "$participant_cidfile" "$probe_cidfile" "$derive_cidfile" "$broker_cidfile"; do
  if [[ -s $cidfile ]]; then
    cid=$(<"$cidfile")
    if docker inspect -f '{{.State.Running}}' "$cid" 2>/dev/null | grep -qx true; then
      echo "run-participant: container remained active after completion" >&2
      exit 1
    fi
  fi
done

find "$run_directory" -mindepth 1 -maxdepth 1 -type s -delete
find "$run_directory" -mindepth 1 -maxdepth 1 -type f -delete
rmdir "$run_directory"
if [[ -e $derived_auth || -L $derived_auth || -e $broker_socket || -L $broker_socket || -e $run_directory ]]; then
  echo "run-participant: sensitive runtime cleanup failed" >&2
  exit 1
fi

python3 - "$closed_audit" <<'PY'
import json, os, stat, sys
path = sys.argv[1]
record = {
    "schema": "vela.claim-dependency-pi-runner-audit.v1", "kind": "sensitive_cleanup",
    "derived_auth_absent": True, "broker_socket_absent": True, "sensitive_directory_absent": True,
}
flags = os.O_WRONLY | os.O_APPEND | getattr(os, "O_NOFOLLOW", 0)
descriptor = os.open(path, flags)
try:
    before = os.fstat(descriptor)
    if not stat.S_ISREG(before.st_mode) or stat.S_IMODE(before.st_mode) != 0o600:
        raise SystemExit("run-participant: closed audit custody drifted")
    os.write(descriptor, json.dumps(record, separators=(",", ":")).encode() + b"\n")
    os.fsync(descriptor)
    after = os.fstat(descriptor)
    current = os.lstat(path)
    if (before.st_dev, before.st_ino) != (after.st_dev, after.st_ino) or (before.st_dev, before.st_ino) != (current.st_dev, current.st_ino):
        raise SystemExit("run-participant: closed audit identity changed")
finally:
    os.close(descriptor)
PY

python3 - "$participant_answer" "$closed_audit" "$answer" "$audit" <<'PY'
import os, stat, sys
from pathlib import Path

sources = [Path(sys.argv[1]), Path(sys.argv[2])]
targets = [Path(sys.argv[3]), Path(sys.argv[4])]
created = []
try:
    for source, target in zip(sources, targets):
        data = source.read_bytes()
        if os.path.lexists(target):
            raise OSError("retained output path appeared during execution")
        descriptor = os.open(target, os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_NOFOLLOW", 0), 0o600)
        try:
            view = memoryview(data)
            while view:
                view = view[os.write(descriptor, view):]
            os.fchmod(descriptor, 0o600)
            os.fsync(descriptor)
            meta = os.fstat(descriptor)
        finally:
            os.close(descriptor)
        created.append((target, meta.st_dev, meta.st_ino))
except Exception:
    for path, device, inode in created:
        try:
            current = path.lstat()
            if (current.st_dev, current.st_ino) == (device, inode):
                path.unlink()
        except OSError:
            pass
    raise SystemExit("run-participant: retained output publication failed")
PY

find "$evidence_directory" -mindepth 1 -maxdepth 1 -type f -delete
rmdir "$evidence_directory"
if [[ -e $evidence_directory ]]; then
  echo "run-participant: evidence staging cleanup failed" >&2
  exit 1
fi

python3 - "$answer" "$audit" <<'PY'
import os, stat, sys
for path in sys.argv[1:]:
    meta = os.lstat(path)
    if not stat.S_ISREG(meta.st_mode) or stat.S_ISLNK(meta.st_mode) or stat.S_IMODE(meta.st_mode) != 0o600:
        raise SystemExit("run-participant: retained output custody is invalid")
PY

finished=1
trap - EXIT HUP INT TERM
