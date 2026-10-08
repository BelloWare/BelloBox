from pathlib import Path
import datetime,hashlib,json,os,re,signal,subprocess,sys,time
e=Path(__file__).resolve().parent
name=sys.argv[1];command=sys.argv[2:]
env=os.environ.copy()
removed=[k for k in env if k.startswith(("BELLOBOX_AI_","BELLO_AGENT_","OPENAI_","ANTHROPIC_")) or k in ("AZURE_OPENAI_API_KEY","GEMINI_API_KEY","GOOGLE_API_KEY","LITELLM_API_KEY")]
for k in removed:env.pop(k)
config=e.parent/"isolated-config";config.mkdir(exist_ok=True)
env.update(CARGO_TARGET_DIR="/Users/admin/Library/Caches/BelloRustWork/box-target",CARGO_BUILD_JOBS="2",MACOSX_DEPLOYMENT_TARGET="14.0",RUST_BACKTRACE="1",BELLOBOX_CONFIG_DIR=str(config))
log=e/(name+".log");result=e/(name+".json")
assert not log.exists() and not result.exists()
start=time.monotonic();stamp=lambda:datetime.datetime.now(datetime.timezone.utc).isoformat()
receipt={"command":command,"cwd":"/Users/admin/projects/BelloBox-rust-gif-strict-readback/rust","started_at":stamp(),"environment":{k:env.get(k) for k in ["CARGO_TARGET_DIR","CARGO_BUILD_JOBS","MACOSX_DEPLOYMENT_TARGET","RUST_BACKTRACE","RUSTC","RUSTFLAGS","CARGO_ENCODED_RUSTFLAGS","DEVELOPER_DIR","BELLOBOX_CONFIG_DIR"]},"removed_environment_variable_names":removed}
with log.open("xb") as stream:
 p=subprocess.Popen(command,cwd=receipt["cwd"],env=env,stdout=stream,stderr=subprocess.STDOUT,start_new_session=True)
 try:rc=p.wait(timeout=1800)
 except subprocess.TimeoutExpired:os.killpg(p.pid,signal.SIGKILL);p.wait();rc=124
content=log.read_text(errors="replace")
counts=[dict(zip(["passed","failed","ignored","measured","filtered_out"],map(int,m))) for m in re.findall(r"test result: \w+\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out",content)]
binaries=[]
for value in re.findall(r"Running unittests .*?\(([^()]+)\)",content):
 b=Path(value);b=b if b.is_absolute() else Path(receipt["cwd"])/b
 if b.exists():binaries.append({"path":str(b),"sha256":hashlib.sha256(b.read_bytes()).hexdigest()})
receipt.update(exit_code=rc,ended_at=stamp(),elapsed_seconds=time.monotonic()-start,log_sha256=hashlib.sha256(log.read_bytes()).hexdigest(),counts=counts,test_binaries=binaries)
result.write_text(json.dumps(receipt,indent=2)+"\n")
print(json.dumps(receipt))
sys.exit(rc)

