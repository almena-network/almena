/**
 * The interface's icon set, in one place.
 *
 * Every icon is a single path on the same 24-pixel grid, stroked in the current text colour,
 * so that they line up with each other and with the text beside them without anyone tuning a
 * size per icon. Keeping them together is what makes that true — a second icon file would
 * drift in weight and grid within a release.
 *
 * The set holds what the interface uses and nothing else. An icon nobody draws is deleted, and
 * the set grows with the screens that need one.
 */

/** Every icon this interface can draw. */
export type IconName = "home" | "network" | "settings";

/** The outline of each icon, on a 24 by 24 grid. */
const OUTLINES: Record<IconName, string> = {
  home: "M3.6 10.8 12 3.8l8.4 7M6 9.6V20h12V9.6M10 20v-5.4h4V20",
  // A globe with its meridian: a network is somewhere else as much as it is here.
  network:
    "M12 3a9 9 0 100 18 9 9 0 000-18zM3.6 9h16.8M3.6 15h16.8M12 3c2.4 2.4 3.6 5.4 3.6 9S14.4 18.6 12 21c-2.4-2.4-3.6-5.4-3.6-9S9.6 5.4 12 3z",
  settings: "M4 7h16M4 12h16M4 17h16",
};

/** What an icon is drawn from. */
interface IconProps {
  /** Which icon. */
  name: IconName;
  /** Its side, in pixels. Square, always. */
  size?: number;
}

/**
 * One icon, in the colour of whatever draws it.
 *
 * It is decoration beside a name that says the same thing, so it is hidden from a screen
 * reader rather than given a label of its own to read twice.
 */
function Icon({ name, size = 20 }: IconProps) {
  return (
    <svg
      className="icon"
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.6"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      focusable="false"
    >
      <path d={OUTLINES[name]} />
    </svg>
  );
}

export default Icon;
