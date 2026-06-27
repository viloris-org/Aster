export function safeJsonStringify(value: unknown, space?: string | number): string {
  const seen = new WeakSet<object>();

  return JSON.stringify(value, (_key, item) => {
    if (typeof item !== 'object' || item === null) return item;
    if (seen.has(item)) return '[Circular]';
    seen.add(item);
    return item;
  }, space) ?? 'null';
}

export function safeJsonValue(value: unknown): unknown {
  return JSON.parse(safeJsonStringify(value));
}
