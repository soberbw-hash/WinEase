import { useEffect, useRef, useState, type ReactNode } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";

const cache = new Map<string, Promise<string | null>>();
const queue: Array<() => void> = [];
let active = 0;

function loadIcon(target: string, command: boolean, file: boolean) {
  const key = `${file}:${command}:${target.toLowerCase()}`;
  const previous = cache.get(key);
  if (previous) return previous;
  const promise = new Promise<string | null>((resolve) => {
    queue.push(() => {
      active++;
      void invoke<string | null>(file ? "get_file_icon" : "get_application_icon", { target, command })
        .then(resolve, () => resolve(null))
        .finally(() => {
          active--;
          drain();
        });
    });
  });
  if (cache.size >= 256) cache.delete(cache.keys().next().value!);
  cache.set(key, promise);
  drain();
  return promise;
}
function drain() {
  while (active < 4 && queue.length) queue.shift()!();
}

export function ApplicationIcon({
  target,
  command = false,
  file = false,
  fallback,
}: {
  target: string;
  command?: boolean;
  file?: boolean;
  fallback?: ReactNode;
}) {
  const host = useRef<HTMLSpanElement>(null);
  const [icon, setIcon] = useState<{
    target: string;
    image: string | null;
  } | null>(null);
  useEffect(() => {
    if (!target || !isTauri() || !host.current) return;
    let disposed = false;
    const observer = new IntersectionObserver(
      (entries) => {
        if (!entries.some((entry) => entry.isIntersecting)) return;
        observer.disconnect();
        void loadIcon(target, command, file).then((image) => {
          if (!disposed) setIcon({ target, image });
        });
      },
      { rootMargin: "100px" },
    );
    observer.observe(host.current);
    return () => {
      disposed = true;
      observer.disconnect();
    };
  }, [target, command, file]);
  const source = icon?.target === target ? icon.image : null;
  return (
    <span ref={host} className="application-icon" aria-hidden="true">
      {source ? (
        <img src={source} alt="" />
      ) : fallback ?? (
        <svg
          viewBox="0 0 24 24"
          width="26"
          height="26"
          fill="none"
          stroke="currentColor"
          strokeWidth="1.5"
        >
          <rect x="3" y="3" width="18" height="18" rx="3" />
          <path d="M3 8h18M7 5.5h.01M10 5.5h.01" />
        </svg>
      )}
    </span>
  );
}
