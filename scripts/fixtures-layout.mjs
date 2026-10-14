// TEST DATA: Prompt 18 layout behavior matrix. Writes inputs only; Chrome writes every expectation.
// Regenerate these inputs with: node scripts/fixtures-layout.mjs
import { writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const dir = fileURLToPath(new URL("../fixtures/layout-oracle/", import.meta.url));
const div = (props, children = []) =>
  `<div ${Object.entries(props)
    .map(([k, v]) => `data-${k}="${v}"`)
    .join(" ")}>${children.join("")}</div>`;
function fixture(name, title, tree) {
  writeFileSync(
    `${dir}${name}.html`,
    `<!doctype html>\n<!-- TEST DATA: layout oracle matrix -->\n<title>${title}</title>\n${tree}\n`,
  );
}
for (const mode of ["row", "column"]) {
  for (const justify of ["start", "center", "end", "space-between", "space-around"]) {
    for (const align of ["start", "center", "end"]) {
      for (const fill of [false, true]) {
        const dims = (main, cross) =>
          mode === "row" ? { width: main, height: cross } : { width: cross, height: main };
        fixture(
          `matrix_${mode}_${justify.replaceAll("-", "")}_${align}_${fill ? "fill" : "fixed"}`,
          `${mode} ${justify} ${align}, fractional padding and ${fill ? "mixed fill" : "fixed"} children`,
          div(
            {
              layout: mode,
              ...dims(321.5, 111.25),
              gap: 7.25,
              padding: "3.5 5.25 9.75 11.5",
              justify,
              align,
            },
            [
              div(dims(31.5, 21.25)),
              div(dims(fill ? "fill" : 62.25, fill ? "fill" : 32.5)),
              div(dims(fill ? "fill" : 41.75, 14.75)),
            ],
          ),
        );
      }
    }
  }
}
for (const columns of [1, 2, 3, 4]) {
  for (const justify of ["start", "center", "end"]) {
    for (const align of ["start", "center", "end"]) {
      fixture(
        `matrix_grid_${columns}_${justify}_${align}`,
        `Grid with ${columns} equal columns, ${justify}/${align}, fills and overflowing items`,
        div(
          {
            layout: "grid",
            columns,
            rows: 2,
            width: 311.75,
            height: 137.5,
            gap: 8.25,
            padding: "4.5 7.25",
            justify,
            align,
          },
          [
            div({ width: 17.25, height: 23.5 }),
            div({ width: "fill", height: "fill" }),
            div({ width: 101.75, height: 31.25 }),
            div({ width: 15.5, height: "fill" }),
            div({ width: "fill", height: 18.5 }),
          ],
        ),
      );
    }
  }
}
for (const mode of ["row", "column", "grid"]) {
  for (const strokeAlign of ["inside", "center", "outside"]) {
    for (const includes of [false, true]) {
      fixture(
        `matrix_stroke_${mode}_${strokeAlign}_${includes ? "included" : "ignored"}`,
        `${mode} own ${strokeAlign} stroke ${includes ? "included" : "ignored"}, absolute origin and hidden subtree`,
        div(
          {
            layout: mode,
            ...(mode === "grid" ? { columns: 2 } : {}),
            width: 213.5,
            height: 121.25,
            gap: 7.5,
            padding: "2.5 4.25",
            "layout-includes-stroke": includes,
            stroke: "2.5 4.75 6.25 8.5",
            "stroke-align": strokeAlign,
          },
          [
            div({ width: "fill", height: "fill" }),
            div({ width: 27.25, height: 19.5 }),
            div({ position: "absolute", x: -7.25, y: 3.5, width: 14.75, height: 9.25 }),
            div({ layout: "row", visible: false, width: 999, height: 99 }, [
              div({ width: 30, height: 30 }),
            ]),
          ],
        ),
      );
    }
  }
}
for (const mode of ["row", "column"]) {
  for (const childMode of ["row", "column", "grid"]) {
    for (const hugging of [false, true]) {
      fixture(
        `matrix_nested_${mode}_${childMode}_${hugging ? "hug" : "fixed"}`,
        `${mode} with ${childMode}, ${hugging ? "hug and fallback" : "fill and overflow"}, nested padding and hidden child`,
        div(
          {
            layout: mode,
            width: hugging ? "hug" : 301.25,
            height: hugging ? "hug" : 171.5,
            padding: "1.5 3.75",
            gap: 9.25,
          },
          [
            div(
              {
                layout: childMode,
                ...(childMode === "grid" ? { columns: 2 } : {}),
                width: hugging ? "hug" : "fill",
                height: "hug",
                gap: 4.25,
                padding: "2.75 4.5",
              },
              [div({ width: 81.25, height: 12.5 }), div({ width: 44.75, height: 21.25 })],
            ),
            div({
              width: hugging && mode === "row" ? "fill" : 17.5,
              ...(hugging && mode === "row" ? { "width-fallback": 17.5 } : {}),
              height: 19.25,
            }),
          ],
        ),
      );
    }
  }
}

for (const mode of ["row", "column", "grid"]) {
  for (const content of ["empty", "hidden", "populated"]) {
    fixture(
      `matrix_hug_fallback_${mode}_${content}`,
      `${mode} hug fallback with ${content} content and inward stroke`,
      div(
        {
          layout: mode,
          ...(mode === "grid" ? { columns: 2 } : {}),
          width: "hug",
          "width-fallback": 73.5,
          height: "hug",
          "height-fallback": 27.25,
          padding: "2.75 4.5",
          "layout-includes-stroke": true,
          stroke: "2.5",
          "stroke-align": "center",
        },
        content === "empty"
          ? []
          : [
              div({
                width: 31.25,
                height: 21.5,
                ...(content === "hidden" ? { visible: false } : {}),
              }),
            ],
      ),
    );
  }
}
