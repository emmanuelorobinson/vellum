---
title: "Architecture"
description: "How {{brand}} is built: the document engine, the daemon, the editor and its hosts, the cloud services, the quality gates, and the open questions."
---

These pages describe how {{brand}} is put together and why. They summarise the repository's architecture document, its decision
records and its runbooks, and they link to the sources for the detail. Read them when you are changing the product, or when you want
to know why a part behaves the way it does.

## Reading order

1. [The big picture](/docs/architecture/big-picture): the parts and the rules that hold them together
2. [Principles](/docs/architecture/principles): the invariants every change must keep
3. [The document model](/docs/architecture/document-model): what a design is, and how it is stored and merged
4. [The core modules](/docs/architecture/core-modules): the parts of the engine and what each one owns
5. [Clients and hosts](/docs/architecture/clients-and-hosts): the editor, and the programs that run it
6. [The daemon](/docs/architecture/daemon): the process that owns documents, and how it talks to clients
7. [AI and agents](/docs/architecture/ai-and-agents): how models and agents fit around the tools
8. [Cloud](/docs/architecture/cloud): the optional services for Pro features and live collaboration
9. [Quality gates](/docs/architecture/quality-gates): the checks that protect each promise
10. [Performance](/docs/architecture/performance): the budgets the product is measured against
11. [Monorepo](/docs/architecture/monorepo): the workspace layout
12. [Threat model](/docs/architecture/threat-model): the trust boundaries and what each one defends against
13. [Open questions](/docs/architecture/open-questions): what was settled, and what is still open

## Where the decisions are

Each architectural decision has a numbered record. The [decision records](/docs/reference/decisions) index lists them with their status.
A record is the place to look for why a choice was made. These pages describe the result.
