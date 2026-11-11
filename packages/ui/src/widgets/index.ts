/**
 * Importing this module registers every built-in widget. The order is the order of the palette.
 * To add a widget: create `src/widgets/MyWidget.tsx` that calls `registerWidget(...)`, import it
 * here and add its title key to the i18n dictionaries.
 */
import "./Laps";
import "./Gaps";
import "./Inputs";
import "./Tyres";
import "./Brakes";
import "./Fuel";
import "./Map";
import "./Standings";
import "./Session";
import "./Electronics";

export type { WidgetDef, WidgetProps } from "./registry";
export { getWidget, listWidgets, registerWidget } from "./registry";
