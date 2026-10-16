# Build a design system

Read format.md, execute.md, existing guidelines, variables, theme axes and components. Agree on the scope
before introducing a system. Preserve names and semantic roles already in use. List missing decisions
about typography, palette or component behavior; a visual style is only a starting recipe.

Create semantic variables before repeated components. Give each component a clear name, minimal props and
variants for actual states; use slots for replaceable content. Reuse nested instances instead of duplicating
their source trees. Exercise each theme and state with layout checks and screenshots. Check contrast and
font availability explicitly. A palette called high-contrast alone does not establish accessibility.

Small construction example on an existing Screens page:

```javascript
const page = doc.page("Screens");
doc.setVariables({spaceUnit01:{type:"number", name:"space/unit", values:[{value:8}]}});
const button = doc.insert(page, {type:"frame", name:"Action", width:120, height:40,
  layout:"row", gap:{$var:"spaceUnit01"}, padding:8});
doc.insert(button, {type:"text", name:"Label", width:"hug", height:"hug",
  content:[{text:"Continue", font:"Inter", size:14}]});
const component = doc.createComponent(button);
const instance = doc.instance(page, component, {name:"Action preview"});
return {component:component, instance:instance};
```

Deliver a library and guidelines describing roles, states, allowed combinations and exceptions. Any writing
of guidelines.md or other files outside the canvas requires the person's authorized file action through the
host's PermissionBroker; execute cannot write files. Report those follow-up files if that capability is
unavailable. Do not claim the library, guidelines or theme checks exist until verified.
