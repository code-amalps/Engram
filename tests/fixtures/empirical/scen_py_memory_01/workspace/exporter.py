def stream_records(records, chunk_size=100):
    buffer = []
    for r in records:
        buffer.append(r)
        if len(buffer) >= chunk_size:
            yield buffer
            buffer = []
    if buffer:
        yield buffer
