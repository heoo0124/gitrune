/** Settings › Editor pages: General, Font and Color Scheme. */
import type { PageSpec } from "../spec";
import { GENERAL_PAGES } from "./general";
import { FONT_PAGE } from "./font";
import { COLOR_SCHEME_PAGES } from "./colorScheme";

export const EDITOR_PAGES: PageSpec[] = [...GENERAL_PAGES, FONT_PAGE, ...COLOR_SCHEME_PAGES];
