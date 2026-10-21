/**
 * @schema 1
 * @input count: number(min=1,max=4,step=1) = 3
 */
return Array.from({length: doc.inputs.count}, (_, index) => ({
  type: "rect",
  width: 4,
  height: 8,
  position: {type: "absolute", x: index * 6, y: 8},
  fills: [{type: "solid", color: "#404040"}],
}));
