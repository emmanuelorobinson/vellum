// Vellum's synchronous data-only bridge. All references are captured before running user code.
(call, deny, random, names, byteLimit) => {
  const { freeze, getPrototypeOf, getOwnPropertyDescriptors, defineProperty, create } = Object;
  const ownKeys = Reflect.ownKeys;
  const stringify = JSON.stringify,
    parse = JSON.parse;
  const array = Array.isArray;
  const finite = Number.isFinite;
  const SetType = Set;
  const ErrorType = Error;
  const syntaxPrototype = SyntaxError.prototype;
  const objectPrototype = Object.prototype,
    arrayPrototype = Array.prototype;
  const forbidden = (what) => {
    deny(what);
    throw new ErrorType("Forbidden capability");
  };
  const validate = (value) => {
    let count = 0,
      bytes = 0;
    const seen = new SetType();
    function visit(v, depth) {
      if (++count > 65536 || depth > 32) forbidden("boundary depth or size");
      if (v == null || v === undefined) return null;
      if (typeof v === "string") {
        bytes += v.length * 3;
        if (bytes > byteLimit) forbidden("memory");
        return v;
      }
      if (typeof v === "boolean") return v;
      if (typeof v === "number" && finite(v)) return v;
      if (typeof v !== "object") forbidden("non-JSON value");
      if (seen.has(v)) forbidden("cyclic data");
      seen.add(v);
      const isArray = array(v),
        proto = getPrototypeOf(v);
      if (isArray ? proto !== arrayPrototype : proto !== objectPrototype && proto !== null)
        forbidden("non-plain data");
      const copy = isArray ? [] : create(null);
      const descriptors = getOwnPropertyDescriptors(v);
      if (
        isArray &&
        (descriptors.length.value > 65536 ||
          ownKeys(descriptors).length !== descriptors.length.value + 1)
      )
        forbidden("sparse or extended array");
      for (const key of ownKeys(descriptors)) {
        if (isArray && key === "length") continue;
        if (isArray && (key >>> 0 >= descriptors.length.value || `${key >>> 0}` !== key))
          forbidden("extended array");
        if (
          typeof key !== "string" ||
          key === "__proto__" ||
          key === "constructor" ||
          key === "prototype"
        )
          forbidden("prototype data");
        const d = descriptors[key];
        if (d.get || d.set) forbidden("accessor data");
        if (d.enumerable || isArray) copy[key] = visit(d.value, depth + 1);
      }
      seen.delete(v);
      return copy;
    }
    return stringify(visit(value, 0));
  };
  const doc = create(null);
  for (const name of names) {
    doc[name] = (...args) => parse(call(name, validate(args)));
  }
  defineProperty(globalThis, "doc", { value: freeze(doc), writable: false, configurable: false });
  defineProperty(Math, "random", { value: random, writable: false, configurable: false });
  const noCode = () => forbidden("dynamic code");
  // Literal async/generator functions have distinct constructor prototypes.
  for (const fn of [() => {}, async () => {}, function* () {}, async function* () {}])
    defineProperty(getPrototypeOf(fn), "constructor", {
      value: noCode,
      writable: false,
      configurable: false,
    });
  defineProperty(globalThis, "eval", { value: noCode, writable: false, configurable: false });
  defineProperty(globalThis, "Function", { value: noCode, writable: false, configurable: false });
  // No Promise entry point; the runtime's promise hook also catches literal async/import syntax.
  defineProperty(globalThis, "Promise", { value: undefined, writable: false, configurable: false });
  const frozen = new Set();
  function deepFreeze(v) {
    if (v === null || (typeof v !== "object" && typeof v !== "function") || frozen.has(v)) return;
    frozen.add(v);
    const descriptors = getOwnPropertyDescriptors(v);
    for (const key of ownKeys(descriptors)) {
      const d = descriptors[key];
      if ("value" in d) deepFreeze(d.value);
      if (d.get) deepFreeze(d.get);
      if (d.set) deepFreeze(d.set);
    }
    const proto = getPrototypeOf(v);
    if (proto) deepFreeze(proto);
    freeze(v);
  }
  for (const key of ownKeys(globalThis)) {
    const d = getOwnPropertyDescriptors(globalThis)[key];
    if ("value" in d && d.value !== globalThis) deepFreeze(d.value);
  }
  // Return a closed-over encoder, never an extra global host capability.
  const describe = (error) => {
    const d = getOwnPropertyDescriptors(error);
    const message =
      typeof d.message?.value === "string"
        ? d.message.value.slice(0, 1024)
        : "Script threw an error";
    const stack = typeof d.stack?.value === "string" ? d.stack.value.slice(0, 4096) : "";
    return stringify([
      getPrototypeOf(error) === syntaxPrototype,
      `${message}${stack ? `\n${stack}` : ""}`,
    ]);
  };
  return { encode: validate, describe };
};
