# ForgeBench Task Operations

ForgeBench is one small task-operations product used to evaluate repository-scale coding work.
It contains two TypeScript consoles, shared domain/UI/config packages, a Python task API,
an event worker, and release tooling. The clean baseline must pass `python3 tools/release/verify.py`.

Benchmark scenarios are applied by the eval controller from outside this tree. This repository
contains no case prompts, hidden verifiers, solution notes, or nested version-control metadata.
