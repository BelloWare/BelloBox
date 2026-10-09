#!/usr/bin/env python3
"""Test final source-binding rejection from text-only copies; read-only Git."""
import argparse,json,shutil,subprocess,sys,tempfile
from pathlib import Path
P=Path(__file__).resolve().parent
p=argparse.ArgumentParser();p.add_argument('--repo',required=True);a=p.parse_args();results=[]
files=['verify.py','verify_from_git.py','verify_publication.py','ledger.json','source-manifest.json','final-manifest.json','editor-manifest.json','popup-baseline-ledger.json','publication-binding.json']
for control in ['wrong-final-tree','wrong-parent','incorrect-cumulative-support']:
    with tempfile.TemporaryDirectory(prefix='loc-binding-control-') as t:
        root=Path(t)
        for n in files:shutil.copyfile(P/n,root/n)
        proof=json.loads((root/'publication-binding.json').read_text())
        if control=='wrong-final-tree':proof['source_tree']='0'*40
        elif control=='wrong-parent':proof['parent_commit']='0'*40
        else:proof['cumulative_counts']['support']+=1
        (root/'publication-binding.json').write_text(json.dumps(proof))
        result=subprocess.run([sys.executable,str(root/'verify_publication.py'),'--repo',a.repo],capture_output=True,text=True)
        if result.returncode==0:raise RuntimeError(control+' unexpectedly passed')
        results.append(dict(control=control,rejected=True,reason=result.stderr.strip().splitlines()[-1]))
print(json.dumps(results,indent=2))
