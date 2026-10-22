// TEST DATA: a frame with a label, for the live-engine browser tests (e2e/engine.spec.ts).
export const ENGINE_DOCUMENT = {
  meta: { formatVersion: 1, name: "TEST DATA", createdWith: "TEST DATA" },
  pages: [
    {
      id: "page000001",
      name: "TEST DATA",
      children: [
        {
          id: "frame00001",
          type: "frame",
          name: "Frame",
          width: 400,
          height: 300,
          position: { type: "absolute", x: 500, y: 200 },
          fills: [{ type: "solid", color: "#FFFFFF" }],
          children: [
            {
              id: "text000001",
              type: "text",
              name: "Label",
              width: "hug",
              height: "hug",
              position: { type: "absolute", x: 20, y: 20 },
              content: [{ text: "TEST DATA", font: "Inter", size: 24 }],
              fills: [{ type: "solid", color: "#000000" }],
            },
          ],
        },
      ],
    },
  ],
};
