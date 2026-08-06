import type { TaskHistoryItem } from "../../types/history";

export function mergeTaskHistoryItems(
  current: TaskHistoryItem[],
  incoming: TaskHistoryItem[],
  reset: boolean,
): TaskHistoryItem[] {
  if (reset) return incoming;
  const byId = new Map(current.map((item) => [item.id, item]));
  incoming.forEach((item) => byId.set(item.id, item));
  return [...byId.values()];
}
