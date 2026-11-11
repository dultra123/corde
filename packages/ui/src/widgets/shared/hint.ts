import { OVERLAY_AT_BOOT } from "../../overlay/boot";

/** A tooltip (`title`) for a widget, left out on an OBS overlay page: no tooltip on a stream. */
export function hint(text: string): string | undefined {
  return OVERLAY_AT_BOOT ? undefined : text;
}
