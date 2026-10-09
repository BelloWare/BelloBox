from pathlib import Path
import datetime, hashlib, json, os, re, signal, subprocess, sys, time
ROOT=Path(__file__).resolve().parent.parent
SOURCE=Path("/Users/admin/projects/BelloBox-rust-first-following")
EVIDENCE=ROOT/"evidence"
CONFIG=json.loads((EVIDENCE/"commands.json").read_text())
ENV=os.environ.copy()
removed=[k for k in ENV if k.startswith(("BELLOBOX_AI_","BELLO_AGENT_","OPENAI_","ANTHROPIC_")) or k in ("AZURE_OPENAI_API_KEY","GEMINI_API_KEY","GOOGLE_API_KEY","LITELLM_API_KEY")]
for k in removed: ENV.pop(k)
config_dir=ROOT/"isolated-config";config_dir.mkdir(exist_ok=True)
ENV.update({"CARGO_TARGET_DIR":CONFIG["target"],"CARGO_BUILD_JOBS":"2","MACOSX_DEPLOYMENT_TARGET":"14.0","RUST_BACKTRACE":"1","BELLOBOX_CONFIG_DIR":str(config_dir)})
def sha(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
def stamp():return datetime.datetime.now(datetime.timezone.utc).isoformat()
host={"checked_at":stamp(),"commands":[],"removed_environment_variable_names":sorted(removed),"required_unset":["BELLOBOX_AI_PROVIDER","BELLOBOX_AI_ENDPOINT","BELLOBOX_AI_MODEL","BELLOBOX_AI_KEY"],"build_environment":{k:ENV.get(k) for k in ["CARGO_TARGET_DIR","CARGO_BUILD_JOBS","MACOSX_DEPLOYMENT_TARGET","RUST_BACKTRACE","RUSTC","RUSTFLAGS","CARGO_ENCODED_RUSTFLAGS","DEVELOPER_DIR","BELLOBOX_CONFIG_DIR"]}}
assert all(k not in ENV for k in host["required_unset"])
for command in [["/usr/bin/sw_vers"],["/usr/bin/uname","-m"],["/opt/homebrew/bin/rustc","-vV"],["/opt/homebrew/bin/cargo","-V"],["/usr/bin/xcrun","swiftc","--version"],["/usr/bin/xcrun","--sdk","macosx","--show-sdk-version"]]:
 p=subprocess.run(command,capture_output=True,text=True,env=ENV,check=True);host["commands"].append({"command":command,"stdout":p.stdout,"stderr":p.stderr,"exit_code":p.returncode})
(EVIDENCE/"host-toolchain.json").write_text(json.dumps(host,indent=2)+"\n")
selected=set(sys.argv[1:])
for step in CONFIG["steps"]:
 name=step["name"];command=step["command"]
 if selected and name not in selected:continue
 resultpath=EVIDENCE/(name+"-result.json")
 if resultpath.exists():raise RuntimeError("result already exists: "+name)
 log=EVIDENCE/(name+".log")
 current={**step,"started_at":stamp(),"cwd":str(SOURCE/"rust"),"log":str(log)}
 (EVIDENCE/"status.json").write_text(json.dumps({**current,"state":"running"},indent=2)+"\n")
 print(json.dumps({"started":name,"command":command}),flush=True);start=time.monotonic()
 with log.open("xb") as stream:
  child=subprocess.Popen(command,cwd=SOURCE/"rust",env=ENV,stdout=stream,stderr=subprocess.STDOUT,start_new_session=True)
  try:rc=child.wait(timeout=1800)
  except subprocess.TimeoutExpired:os.killpg(child.pid,signal.SIGKILL);child.wait();rc=124
 content=log.read_text(errors="replace")
 counts=[dict(zip(["passed","failed","ignored","measured","filtered_out"],map(int,m))) for m in re.findall(r"test result: \w+\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out",content)]
 binaries=[]
 for rel in re.findall(r"Running unittests .*?\(([^()]+)\)",content):
  p=Path(rel) if Path(rel).is_absolute() else SOURCE/"rust"/rel
  if p.exists():binaries.append({"path":str(p),"sha256":sha(p)})
 result={**current,"ended_at":stamp(),"elapsed_seconds":time.monotonic()-start,"exit_code":rc,"log_sha256":sha(log),"counts":counts,"test_binaries":binaries}
 resultpath.write_text(json.dumps(result,indent=2)+"\n")
 (EVIDENCE/"status.json").write_text(json.dumps({**result,"state":"passed" if rc==0 else "failed"},indent=2)+"\n")
 print(json.dumps({"finished":name,"exit_code":rc,"counts":counts,"elapsed_seconds":result["elapsed_seconds"]}),flush=True)
 if rc:sys.exit(rc)
 assert counts and counts[-1]["passed"]==step["expected_passed"] and counts[-1]["ignored"]==step["expected_ignored"],(name,counts)
(EVIDENCE/"status.json").write_text(json.dumps({"state":"all-selected-commands-completed","checked_at":stamp()},indent=2)+"\n")
