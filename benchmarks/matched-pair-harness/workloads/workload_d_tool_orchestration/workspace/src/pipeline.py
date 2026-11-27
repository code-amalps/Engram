import json
from pathlib import Path

def load_pipeline_config():
    p = Path(__file__).resolve().parent.parent / 'config.json'
    return json.loads(p.read_text(encoding='utf-8'))
