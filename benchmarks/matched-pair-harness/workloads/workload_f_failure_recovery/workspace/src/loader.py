import json
from pathlib import Path

def load_settings():
    # Attempting to load missing file crashes unless fallback is implemented
    base = Path(__file__).resolve().parent.parent
    p = base / 'settings.json'
    if p.exists():
        return json.loads(p.read_text(encoding='utf-8'))
    # Fallback to default_settings.json
    def_p = base / 'default_settings.json'
    return json.loads(def_p.read_text(encoding='utf-8'))
