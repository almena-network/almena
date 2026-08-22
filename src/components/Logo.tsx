/**
 * The Almena mark, drawn rather than loaded.
 *
 * It is the same three-node graph as the application icon, as a path instead of a bitmap, so
 * that it stays sharp at any size and takes its colour from whatever draws it. That is the
 * whole reason it is a component: the icon file cannot follow the text colour of a disabled
 * row or a hovered button, and this can.
 *
 * The geometry is measured from the icon and must not drift from it. If the icon changes,
 * this changes with it.
 */

/** How the mark is drawn. */
interface LogoProps {
  /** Width and height in pixels. The mark is square. */
  size?: number;
  /**
   * Colour of the mark. Anything CSS accepts, including `currentColor`, which is the point
   * of drawing it rather than loading it.
   */
  color?: string;
}

/** The Almena mark, square, coloured by its caller. */
function Logo({ size = 24, color = "currentColor" }: LogoProps) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 1024 1024"
      fill="none"
      // The mark carries no meaning of its own on screen: every place it appears, the
      // product's name is already written beside it.
      aria-hidden="true"
      focusable="false"
    >
      <g
        stroke={color}
        strokeWidth="60"
        strokeLinecap="round"
        strokeLinejoin="round"
      >
        <path d="M511 237 237 784" />
        <path d="M511 237 784 784" />
        <path d="M237 784 648 511" />
      </g>
      <circle cx="511" cy="237" r="94" fill={color} />
      <circle cx="237" cy="784" r="94" fill={color} />
      <circle cx="784" cy="784" r="94" fill={color} />
    </svg>
  );
}

export default Logo;
