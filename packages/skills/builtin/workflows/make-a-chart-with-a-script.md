# Make a chart with a script

Confirm the supplied series, units, labels, range and intended comparison. Read notes, guidelines,
format.md and execute.md. Represent values accurately and include text labels; color alone cannot encode
the data. Handle an empty series and a zero range. Use deterministic input order and finite coordinates.

This execute example creates editable bars from demonstration values on an existing Screens page:

```javascript
const page = doc.page("Screens");
const values = [12, 24, 18];
const maximum = Math.max(1, ...values);
const chart = doc.insert(page, {type:"frame", name:"Chart", width:320, height:200});
for (let i = 0; i < values.length; i++) {
  const height = values[i] / maximum * 120;
  doc.insert(chart, {type:"rect", name:"Bar " + (i + 1), width:48, height:height,
    fills:[{type:"solid", color:"#2455C6"}],
    position:{type:"absolute", x:24 + i * 88, y:144 - height}});
  doc.insert(chart, {type:"text", name:"Value " + (i + 1), width:72, height:24,
    position:{type:"absolute", x:24 + i * 88, y:152},
    content:[{text:String(values[i]), font:"Inter", size:14}]});
}
return {chart:chart};
```

Use the person's data and bound fills from the document for the actual chart. Check bounds and the
screenshot, including negative values and long labels if the brief allows them.

For a persistent script node, the host first writes an authorized .js source beside the document. Its
initial header declares inputs in leading comments. This standalone source demonstrates derived bars;
replace its sample values and literal color with the requested data and declared inputs:

```vellum-script
// @schema 1
// @input count: number(min=1,max=12) = 3
const bars = [];
for (let i = 0; i < doc.inputs.count; i++) {
  bars.push({type:"rect", name:"Bar " + (i + 1), width:24, height:(i + 1) * 20,
    position:{type:"absolute", x:i * 32, y:120 - (i + 1) * 20},
    fills:[{type:"solid", color:"#2455C6"}]});
}
return bars;
```

The script-node doc exposes apiVersion and readonly inputs and returns a NodeSpec or NodeSpec array;
it cannot mutate the document with doc.insert. Use execute for editable document changes and a script node
for derived output. File creation needs PermissionBroker access; if unavailable, provide the intended
source for review without claiming it was saved. Re-evaluate twice with the same inputs to check determinism.
