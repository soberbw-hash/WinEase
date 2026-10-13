export function CheckIcon({kind}: {kind: string}) {
  const paths: Record<string, string> = {
    cleaning: "M6 4h12M9 4V2h6v2M5 7l1 14h12l1-14M9 10v7M15 10v7",
    files: "M4 3h11l5 5v13H4zM15 3v6h5M8 13h8M8 17h5",
    health: "M12 2l9 4v6c0 5-6 9-9 10-3-1-9-5-9-10V6zM7 12h3l2-4 2 8 2-4h2",
    network: "M3 9c5-5 13-5 18 0M6 12c3-3 9-3 12 0M9 15c2-2 4-2 6 0M12 19h.01",
    display: "M3 3h18v14H3zM8 21h8M12 17v4",
    power: "M13 2L4 14h7l-1 8 10-13h-7z",
    game: "M6 7h12l3 10c1 4-3 4-5 0H8c-2 4-6 4-5 0zM6 12h4M8 10v4M16 11h.01M18 13h.01",
    notifications: "M5 17h14l-2-3V9a5 5 0 0 0-10 0v5zM10 21h4",
    repair: "M14 3a6 6 0 0 0-7 7L2 17l5 5 7-7a6 6 0 0 0 7-7l-4 4-5-5z",
    apps: "M3 3h7v7H3zM14 3h7v7h-7zM3 14h7v7H3zM14 14h7v7h-7z",
    preferences: "M3 6h18M3 12h18M3 18h18M8 3v6M16 9v6M10 15v6",
    performance: "M3 20V4M3 20h18M7 16v-5M12 16V7M17 16v-8",
    maintenance: "M14 3a6 6 0 0 0-7 7L2 17l5 5 7-7a6 6 0 0 0 7-7l-4 4-5-5z",
    startup: "M12 2v10M6 5a9 9 0 1 0 12 0",
  };
  return <span className="check-icon" aria-hidden="true"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round"><path d={paths[kind] ?? "M5 3h10l4 4v14H5zM15 3v5h4M8 12h8M8 16h6"}/></svg></span>;
}
