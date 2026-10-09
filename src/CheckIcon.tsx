export function CheckIcon({kind}: {kind: string}) {
  const paths: Record<string, string> = {
    cleaning: "M6 4h12M9 4V2h6v2M5 7l1 14h12l1-14M9 10v7M15 10v7",
    files: "M4 3h11l5 5v13H4zM15 3v6h5M8 13h8M8 17h5",
    health: "M12 2l9 4v6c0 5-6 9-9 10-3-1-9-5-9-10V6zM7 12h3l2-4 2 8 2-4h2",
    network: "M3 9c5-5 13-5 18 0M6 12c3-3 9-3 12 0M9 15c2-2 4-2 6 0M12 19h.01",
    startup: "M12 2v10M6 5a9 9 0 1 0 12 0",
  };
  return <span className="check-icon" aria-hidden="true"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round"><path d={paths[kind] ?? "M5 3h10l4 4v14H5zM15 3v5h4M8 12h8M8 16h6"}/></svg></span>;
}
