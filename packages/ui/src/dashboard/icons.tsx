/** Minimal inline icons for the chrome (stroke based, inherit `currentColor`). */

const base = {
  viewBox: "0 0 24 24",
  fill: "none",
  stroke: "currentColor",
  strokeWidth: 2,
  strokeLinecap: "round" as const,
  strokeLinejoin: "round" as const,
};

export const IconEdit = () => (
  <svg {...base} aria-hidden="true">
    <path d="M4 20h4l10.5-10.5a2.1 2.1 0 0 0-4-4L4 16v4z" />
    <path d="M13.5 6.5l4 4" />
  </svg>
);

export const IconCheck = () => (
  <svg {...base} aria-hidden="true">
    <path d="M5 12.5l4.5 4.5L19 7.5" />
  </svg>
);

export const IconFullscreen = () => (
  <svg {...base} aria-hidden="true">
    <path d="M4 9V4h5M15 4h5v5M20 15v5h-5M9 20H4v-5" />
  </svg>
);

export const IconExitFullscreen = () => (
  <svg {...base} aria-hidden="true">
    <path d="M9 4v5H4M20 9h-5V4M15 20v-5h5M4 15h5v5" />
  </svg>
);

export const IconSettings = () => (
  <svg {...base} aria-hidden="true">
    <circle cx="12" cy="12" r="3" />
    <path d="M19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z" />
  </svg>
);

export const IconPlus = () => (
  <svg {...base} aria-hidden="true">
    <path d="M12 5v14M5 12h14" />
  </svg>
);

export const IconChevron = () => (
  <svg {...base} aria-hidden="true">
    <path d="M6 9l6 6 6-6" />
  </svg>
);

export const IconDemo = () => (
  <svg {...base} aria-hidden="true">
    <path d="M8 5.5v13l10-6.5z" />
  </svg>
);

/** Home (the first tab). */
export const IconHome = () => (
  <svg {...base} aria-hidden="true">
    <path d="M4 11l8-7 8 7" />
    <path d="M6 9.5V20h4.5v-5.5h3V20H18V9.5" />
  </svg>
);

/** Live telemetry (the dashboard tab). */
export const IconLive = () => (
  <svg {...base} aria-hidden="true">
    <circle cx="12" cy="12" r="2.5" fill="currentColor" stroke="none" />
    <path d="M7.5 7.5a6.5 6.5 0 0 0 0 9M16.5 7.5a6.5 6.5 0 0 1 0 9" />
    <path d="M4.5 4.5a10.5 10.5 0 0 0 0 15M19.5 4.5a10.5 10.5 0 0 1 0 15" />
  </svg>
);

/** Recorded sessions (the sessions tab). */
export const IconSessions = () => (
  <svg {...base} aria-hidden="true">
    <path d="M4 7h16M4 12h16M4 17h10" />
  </svg>
);

export const IconBack = () => (
  <svg {...base} aria-hidden="true">
    <path d="M15 5l-7 7 7 7" />
  </svg>
);

export const IconTrash = () => (
  <svg {...base} aria-hidden="true">
    <path d="M4 7h16M10 11v6M14 11v6M6 7l1 13h10l1-13M9 7V4h6v3" />
  </svg>
);

export const IconDownload = () => (
  <svg {...base} aria-hidden="true">
    <path d="M12 4v11M7 10l5 5 5-5M5 20h14" />
  </svg>
);

export const IconRecord = () => (
  <svg {...base} aria-hidden="true">
    <circle cx="12" cy="12" r="6" fill="currentColor" stroke="none" />
  </svg>
);

export const IconStop = () => (
  <svg {...base} aria-hidden="true">
    <rect x="6" y="6" width="12" height="12" rx="1.5" fill="currentColor" stroke="none" />
  </svg>
);

export const IconZoomOut = () => (
  <svg {...base} aria-hidden="true">
    <circle cx="11" cy="11" r="6.5" />
    <path d="M16 16l4 4M8 11h6" />
  </svg>
);

export const IconClose = () => (
  <svg {...base} aria-hidden="true">
    <path d="M6 6l12 12M18 6L6 18" />
  </svg>
);

/** Online races (a chequered flag on its pole). */
export const IconFlag = () => (
  <svg {...base} aria-hidden="true">
    <path d="M5 21V4" />
    <path d="M5 4h13l-2.5 4.5L18 13H5" />
    <path d="M9 4v9M13 4v9M5 8.5h11.5" strokeWidth="1.2" />
  </svg>
);

/** Driver profile (imported results). */
export const IconProfile = () => (
  <svg {...base} aria-hidden="true">
    <path d="M7 4h10v5a5 5 0 0 1-10 0z" />
    <path d="M7 6H4.5a2.5 2.5 0 0 0 2.5 3M17 6h2.5A2.5 2.5 0 0 1 17 9" />
    <path d="M12 14v3M8.5 20h7M10 17h4" />
  </svg>
);

/** A phone (access from the local network). */
export const IconPhone = () => (
  <svg {...base} aria-hidden="true">
    <rect x="7" y="3" width="10" height="18" rx="2" />
    <path d="M11 17.5h2" />
  </svg>
);

export const IconRefresh = () => (
  <svg {...base} aria-hidden="true">
    <path d="M20 11a8 8 0 0 0-14.5-4.5L4 8" />
    <path d="M4 3v5h5" />
    <path d="M4 13a8 8 0 0 0 14.5 4.5L20 16" />
    <path d="M20 21v-5h-5" />
  </svg>
);

/** The race engineer (headset): session analysis and setup plans. */
export const IconEngineer = () => (
  <svg {...base} aria-hidden="true">
    <path d="M4 14v-2a8 8 0 0 1 16 0v2" />
    <rect x="3" y="13" width="4" height="6" rx="1.5" />
    <rect x="17" y="13" width="4" height="6" rx="1.5" />
    <path d="M19 19v1a2 2 0 0 1-2 2h-4" />
  </svg>
);
