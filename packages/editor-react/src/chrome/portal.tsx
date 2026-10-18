import { createContext, useContext } from "react";

/**
 * Where menus, selects and popovers are mounted. They must live inside the workspace element so they pick up its
 * appearance (light, dark, high contrast), which a portal into `<body>` would not.
 */
export const PortalContainer = createContext<HTMLElement | null>(null);
export const usePortalContainer = () => useContext(PortalContainer);
