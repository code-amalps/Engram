# ADR-042: Bounded Memory Streaming Exporters

## Context
Unbounded in-memory buffers cause OOM spikes when exporting large datasets.

## Decision
All export routines must yield fixed-size chunks (default 100) using Python generators.
In-memory list accumulation across batches is strictly prohibited.

## Status
Ratified
