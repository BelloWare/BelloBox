from pathlib import Path
import datetime,hashlib,json,os,subprocess
root=Path(__file__).resolve().parent.parent;s=Path("/Users/admin/projects/BelloBox-rust-gif-strict-readback");e=root/"evidence"
env=os.environ.copy()
for key in list(env):
 if key.startswith(("BELLOBOX_AI_","BELLO_AGENT_","OPENAI_","ANTHROPIC_")) or key in ("AZURE_OPENAI_API_KEY","GEMINI_API_KEY","GOOGLE_API_KEY","LITELLM_API_KEY"):env.pop(key)
env.update(CARGO_TARGET_DIR="/Users/admin/Library/Caches/BelloRustWork/box-target",CARGO_BUILD_JOBS="2",MACOSX_DEPLOYMENT_TARGET="14.0",RUST_BACKTRACE="1",BELLOBOX_CONFIG_DIR=str(root/"isolated-config"))
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
receipt={"started_at":datetime.datetime.now(datetime.timezone.utc).isoformat(),"commands":[],"source_sha256":sha(e/"native-export-probe.rs")}
def run(command,name,cwd):
 out=e/(name+".stdout");err=e/(name+".stderr")
 with out.open("xb") as o,err.open("xb") as er:p=subprocess.run(command,cwd=cwd,env=env,stdout=o,stderr=er,timeout=1800)
 receipt["commands"].append({"command":command,"cwd":str(cwd),"exit_code":p.returncode,"stdout_sha256":sha(out),"stderr_sha256":sha(err)})
 (e/"native-export-probe-receipt-v2.json").write_text(json.dumps(receipt,indent=2)+"\n")
 p.check_returncode();return out.read_text()
content=run(["/opt/homebrew/bin/cargo","build","--offline","--locked","-p","bello-platform","-p","bellobox-core","--features","movie-fixtures","--lib","--message-format=json"],"probe-build-v2",s/"rust")
libs={}
for line in content.splitlines():
 data=json.loads(line)
 if data.get("reason")=="compiler-artifact" and data["target"]["name"] in ["bello_platform","bellobox_core","gif"]:
  for filename in data["filenames"]:
   if filename.endswith(".rlib"):libs[data["target"]["name"]]=filename
assert set(libs)=={"bello_platform","bellobox_core","gif"},libs
receipt["libraries"]={k:{"path":v,"sha256":sha(v)} for k,v in libs.items()}
command=["/opt/homebrew/bin/rustc","--edition=2024",str(e/"native-export-probe.rs"),"-L","dependency="+env["CARGO_TARGET_DIR"]+"/debug/deps"]
for k,v in libs.items():command+=["--extern",k+"="+v]
command+=["-o",str(e/"native-export-probe")]
run(command,"probe-rustc",e)
receipt["binary_sha256"]=sha(e/"native-export-probe")
run([str(e/"native-export-probe"),str(e/"probe-fixtures")],"probe-run",e)
receipt["completed_at"]=datetime.datetime.now(datetime.timezone.utc).isoformat()
(e/"native-export-probe-receipt-v2.json").write_text(json.dumps(receipt,indent=2)+"\n")
print(json.dumps(receipt,indent=2))

