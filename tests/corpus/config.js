import { readFile } from "node:fs/promises";

export async function loadConfig(path) {
  const text = await readFile(path, "utf8");
  const config = JSON.parse(text);
  if (!config.name) {
    throw new Error(`missing name in ${path}`);
  }
  return { ...config, loadedAt: Date.now() };
}

export function merge(a, b) {
  return Object.fromEntries(
    [...Object.keys(a), ...Object.keys(b)].map((k) => [k, b[k] ?? a[k]]),
  );
}
