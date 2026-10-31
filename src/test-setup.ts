import "@testing-library/jest-dom/vitest";
import { cleanup } from "@testing-library/react";
import { afterEach, beforeEach } from "vitest";
import { setLang } from "./i18n";

// Les tests vérifient les textes en français, la langue source.
beforeEach(() => setLang("fr"));
afterEach(() => cleanup());
