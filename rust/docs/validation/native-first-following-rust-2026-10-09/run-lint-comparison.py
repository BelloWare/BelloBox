from pathlib import Path
import datetime,hashlib,json,os,subprocess,time
E=Path(__file__).resolve().parent
setup=json.loads((E/"setup.json").read_text())
config=json.loads((E/"commands.json").read_text())
env=os.environ.copy()
for k in list(env):
 if k.startswith(("BELLOBOX_AI_","BELLO_AGENT_","OPENAI_","ANTHROPIC_")):env.pop(k)
env.update({"CARGO_TARGET_DIR":config["target"],"CARGO_BUILD_JOBS":"2","MACOSX_DEPLOYMENT_TARGET":"14.0","RUST_BACKTRACE":"1","BELLOBOX_CONFIG_DIR":str(Path(setup["cache"])/"isolated-config")})
command=config["steps"][-1]["command"]
for name,source,args in [("base-strict-clippy",str(Path(setup["cache"])/"base-source"),command),("scoped-clippy",setup["repo"],[*command[:2],"--no-deps",*command[2:]]),("base-scoped-clippy",str(Path(setup["cache"])/"base-source"),[*command[:2],"--no-deps",*command[2:]]),("platform-scoped-clippy",setup["repo"],[command[0],"clippy","--no-deps","--offline","--locked","-p","bello-platform","--all-targets","--features","movie-fixtures","--","-D","warnings"])]:
 if (E/(name+"-result.json")).exists():continue
 log=E/(name+".log");assert not log.exists()
 started=datetime.datetime.now(datetime.timezone.utc).isoformat();start=time.monotonic();print(json.dumps({"started":name,"command":args,"source":source}),flush=True)
 with log.open("xb") as f:result=subprocess.run(args,cwd=Path(source)/"rust",env=env,stdout=f,stderr=subprocess.STDOUT,timeout=1800)
 receipt={"name":name,"command":args,"cwd":str(Path(source)/"rust"),"started_at":started,"ended_at":datetime.datetime.now(datetime.timezone.utc).isoformat(),"elapsed_seconds":time.monotonic()-start,"exit_code":result.returncode,"log":str(log),"log_sha256":hashlib.sha256(log.read_bytes()).hexdigest()}
 (E/(name+"-result.json")).write_text(json.dumps(receipt,indent=2)+"\n")
 print(json.dumps(receipt),flush=True)
