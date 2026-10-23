---
title: "Clean room and licensing"
description: "What the project may copy, port and link, the rules for each reference product, how the clean-room check works, and the licence of each part."
---

{{brand}} is built to compete with products that already exist. It may learn from them, and it may not copy them. These are the rules that keep the
project's code, text, prompts, skills and interface its own.

## The clean-room rule

Invariant 14 of the build guide says the product's code, text, prompts, skills, API wording and assets are the project's own. No code, text, prompt,
skill, API wording or asset is copied from another product. Generated HTML from another product's export is not pasted into the product, and the
project's interface does not reproduce another product's interface.

Capabilities are a different matter. Matching what a product's public documentation says it does is allowed and expected. The rule is about the
expression of those capabilities, not about the capabilities themselves. Descriptive names for common actions, such as `get_variables`, are allowed
to match, because they are the natural names for those actions.

## The rules for each reference

| Reference | Rule |
| --- | --- |
| A design tool that the project uses to draw its own screens | The tool is used as a design tool. Nothing from its application, its extensions, its MCP server's internals, its prompts, its skills text or its interface is read for code, copied or decompiled. Its public documentation is the only source for capabilities |
| A vector-editor project under a copyleft licence | Read for ideas. Never copied, and never ported |
| A commercial whiteboard project | Its public documentation may be read for ideas. Its code is never copied, and it is never a dependency |
| Open-source projects under permissive licences | Read and ported with attribution, when the port is needed for a hard problem |

The [reference repositories](/docs/development/reference-repositories) page lists each reference and its rule.

## The attribution rule

A ported file starts with a header that names the source repository, the path in it and the commit. The project's third-party notices list every
project that a file was ported from, with its licence. The clean-room check fails when a ported file has no header, or when its source has no entry in
the notices.

## The clean-room check

The repository runs a script that scans every file for the strings on a denied list, and that checks the attribution headers. It reports the number of
files it checked. A denied string anywhere in the repository fails the check. The denied list is itself reviewed, because a list that is too broad
would block ordinary words.

```sh
pnpm clean-room
```

## Licence

The VS Code extension is licensed under Apache-2.0, as the owner selected. Its package carries the repository's licence file, and each bundled
dependency keeps its own licence text. The architecture proposes Apache-2.0 for the core, the command line, the file format and the ports, and
AGPL for the sync server. That proposal is open. See [open questions](/docs/architecture/open-questions).

## Dependencies

A new dependency is checked against the dependency rules and the advisory policy before it is added. A dependency with a licence the policy does not
allow fails `pnpm cargo:deny` and the dependency checks. A prerelease version is not used in a lockfile.
