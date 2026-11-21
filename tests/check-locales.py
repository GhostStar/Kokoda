"""Validate shared translations and generated native resources."""
from pathlib import Path
import subprocess,sys
root=Path(__file__).resolve().parents[1]
subprocess.run([sys.executable,str(root/'scripts/generate-locales.py'),'--check'],check=True)
if sys.platform=='darwin':
 for path in (root/'apps/macos/Resources').rglob('*.strings'):
  subprocess.run(['plutil','-lint',str(path)],check=True)
