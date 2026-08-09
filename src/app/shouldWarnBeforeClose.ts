export interface CloseActivity {
  activeTaskCount: number;
  productionBusy: boolean;
}

export function shouldWarnBeforeClose(
  activity: CloseActivity | null | undefined,
  queryFailed = false,
): boolean {
  if (queryFailed || !activity) return true;
  return activity.activeTaskCount > 0 || activity.productionBusy;
}
