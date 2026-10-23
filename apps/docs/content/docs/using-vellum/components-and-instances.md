---
title: "Components and instances"
description: "Make a component from layers, place instances, override props, use variants and slots, and share components through libraries."
---

A component is a layer tree you reuse. Change the component and every instance that does not override the change follows
it. That is how a design system stays consistent across screens.

## Make a component

Select the layers and make them a component. The layers become the component's definition, and the layer you selected
becomes its first instance. From an agent, use `create_component` with the layer ids.

Give a component a name, and keep its layers named too. Agents read the component list with `list_components`, which shows
each component's id, name, props, variants, slots and the number of instances that use it.

## Instances

An instance points at a component and can change some of its props. A prop is a named value the component exposes, such
as a label or a boolean that shows an icon. An instance can also override a layer inside it, for one screen only.

Use `create_instance` to place instances, with their props and overrides, in one call.

- **Detach** turns an instance into ordinary layers. It then stops following the component. The layers keep their place.
- **Replace inside an instance** swaps one layer inside an instance for a different layer, for that instance only. The
  component is not changed.
- **Turn into a layer**, for an instance that is its component's only instance, turns it back into layers and removes the
  component's definition. A component that other instances still use is not removed.

Changing the component itself is a separate operation: `manage_components` renames a component, and replaces its props or
variants. The layers inside a component are changed with the layer tools, on the component's definition.

## Variants

A variant is a named set of overrides on one component, such as a size or a state. An instance chooses a variant instead
of overriding each layer. Variants keep a button's sizes and states in one place.

## Slots

A slot is an empty frame inside a component that each instance fills with its own content. A card component with a slot
for its image lets each card show a different picture without a new component. Mark a frame as a slot with `make_slot`,
and suggest the components that usually go there. The suggestions guide an agent, and they do not limit what goes in.

## Libraries

A library is a document of components and variables. Another document imports it by path and uses its components.

- An instance of a library component keeps a link to the library. When the library changes, the instance follows.
- A local override still wins over a library change.
- Pasting a library component into a document that does not import the library asks whether to link to it, or to paste
  an independent copy.

The [documents and pages](/docs/using-vellum/documents-and-pages) page explains the file side of a library.

## Components and agents

Agents read the components a document has before they design, so a clear name and a short description help them choose the
right one. The [skills and styles](/docs/agents/skills-and-styles) page explains how a design system's guidelines reach an
agent.
