/**
 * The mark of Corde, as the design system gives it for small sizes (top bar, door of the home
 * server): a rev counter — a grey ring that turns amber in the red zone, a bold amber « C » at
 * its centre, a white needle with a dark cap — on its own dark rounded tile. Same drawing as
 * `public/favicon.svg` and `apps/desktop/assets/corde-mark.svg`: never redrawn, recoloured or
 * rotated, never put on an amber or light ground.
 */
export function CordeMark({ className }: { className?: string }) {
  return (
    <svg className={className} viewBox="0 0 128 128" aria-hidden="true">
      <rect width="128" height="128" rx="28" fill="#0b0e12" />
      <rect
        x="1"
        y="1"
        width="126"
        height="126"
        rx="27"
        fill="none"
        stroke="#2b3440"
        strokeWidth="2"
      />
      <path
        d="M101.48 101.48A53 53 0 1 1 45.87 14.20"
        fill="none"
        stroke="#44526a"
        strokeWidth="4"
      />
      <path
        d="M56.62 11.52A53 53 0 0 1 101.48 26.52"
        fill="none"
        stroke="#ffb454"
        strokeWidth="4"
      />
      <path
        d="M75.24 91.82A30 30 0 1 1 75.24 36.18"
        fill="none"
        stroke="#ffb454"
        strokeWidth="11"
      />
      <path
        d="M91.23 24.45L89.05 23.04L61.15 62.15L54.32 70.83L61.70 75.62L66.85 65.85Z"
        fill="#e9edf2"
      />
      <circle cx="64" cy="64" r="7.5" fill="#0b0e12" stroke="#44526a" strokeWidth="1.3" />
    </svg>
  );
}
