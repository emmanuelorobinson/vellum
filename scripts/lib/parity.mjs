// Architecture §7.4 is the source of rows. The manifest links each v1 row to real named Rust tests.
// This is a coverage inventory, not a test runner or a claim that pending behavior works.
export function parityRows(architecture) {
  const lines = architecture.split(/\r?\n/);
  const start = lines.findIndex((line) => /^#{1,3} 7\.4\s/.test(line));
  if (start < 0) throw new Error("Architecture §7.4 is missing");
  const rows = [];
  for (const line of lines.slice(start + 1)) {
    if (/^#{1,3} /.test(line)) break;
    if (!line.startsWith("|")) continue;
    const cells = line
      .split("|")
      .slice(1, -1)
      .map((cell) => cell.trim());
    if (/^v1(?:$|\s|\()/.test(cells[2] ?? "")) {
      rows.push({ capability: cells[0], surface: cells[1], release: cells[2] });
    }
  }
  if (!rows.length) throw new Error("Architecture §7.4 has no v1 rows");
  if (new Set(rows.map((row) => row.capability)).size !== rows.length) {
    throw new Error("Architecture §7.4 has duplicate capability rows");
  }
  return rows;
}

function namedTest(text, name) {
  const matches = [...text.matchAll(/((?:#\[[^\]]*\]\s*)+)(?:pub\s+)?fn\s+(\w+)\s*\(/g)].filter(
    (match) => match[2] === name && /#\[test\]/.test(match[1]),
  );
  if (matches.length !== 1) throw new Error(`${name} is not a unique named Rust test`);
  return { ignored: /#\[ignore(?:\s*=|\])/.test(matches[0][1]) };
}

export function checkCoverage(architecture, manifest, read) {
  const rows = parityRows(architecture);
  const known = new Set(rows.map((row) => row.capability));
  const entries = new Map();
  if (!Array.isArray(manifest)) throw new Error("Parity manifest must be an array");
  for (const entry of manifest) {
    if (entries.has(entry.capability)) throw new Error(`Duplicate mapping: ${entry.capability}`);
    if (!known.has(entry.capability))
      throw new Error(`${entry.capability} is not an architecture v1 row`);
    entries.set(entry.capability, entry);
  }
  const pending = [];
  let covered = 0;
  for (const row of rows) {
    const entry = entries.get(row.capability);
    if (!entry || !Array.isArray(entry.tests) || !entry.tests.length) {
      throw new Error(`${row.capability}: no named test`);
    }
    if (
      entry.pending &&
      (!Number.isInteger(entry.pending.prompt) ||
        entry.pending.prompt < 32 ||
        typeof entry.pending.reason !== "string" ||
        !entry.pending.reason.trim())
    ) {
      throw new Error(`${row.capability}: pending needs a later prompt and a reason`);
    }
    for (const target of entry.tests) {
      if (
        typeof target.file !== "string" ||
        !target.file.endsWith(".rs") ||
        target.file.includes("\\") ||
        target.file.split("/").some((p) => !p || p === "." || p === "..") ||
        !/^[a-zA-Z_]\w*$/.test(target.name)
      ) {
        throw new Error(`${row.capability}: invalid test path or name`);
      }
      const test = namedTest(read(target.file), target.name);
      if (test.ignored && !entry.pending)
        throw new Error(`${row.capability}: ignored test must be pending`);
      if (entry.pending && !test.ignored)
        throw new Error(`${row.capability}: pending test must be ignored`);
    }
    if (entry.pending) pending.push({ ...row, ...entry.pending, tests: entry.tests });
    else covered++;
  }
  return { total: rows.length, covered, pending };
}
