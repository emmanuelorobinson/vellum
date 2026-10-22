import { createLocale } from "@vellum/i18n";
import { createContext, useContext } from "react";

export const LocaleContext = createContext(createLocale("en"));
export const useLocale = () => useContext(LocaleContext);
