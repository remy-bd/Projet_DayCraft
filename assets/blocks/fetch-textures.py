from pathlib import Path
import hashlib,json,urllib.request
root=Path(__file__).resolve().parent
meta=json.loads((root/'SOURCES.json').read_text(encoding='utf-8'))
for entry in meta['files']:
    data=urllib.request.urlopen(entry['url'],timeout=30).read()
    assert hashlib.sha256(data).hexdigest()==entry['sha256'],entry['file']
    (root/entry['file']).write_bytes(data)
print('Textures vérifiées et restaurées.')
