import sys
from exporter import stream_records

items = list(range(250))
chunks = list(stream_records(items, chunk_size=100))
assert len(chunks) == 3, f"Expected 3 chunks, got {len(chunks)}"
assert len(chunks[0]) == 100, f"Expected chunk 0 to be 100, got {len(chunks[0])}"
assert len(chunks[1]) == 100, f"Expected chunk 1 to be 100, got {len(chunks[1])}"
assert len(chunks[2]) == 50, f"Expected chunk 2 to be 50, got {len(chunks[2])}"
print("Export streaming test passed")
