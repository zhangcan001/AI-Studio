export interface ReferenceAnchorApplySuccess {
  ok: true;
  assetIds: string[];
}

export interface ReferenceAnchorApplyFailure {
  ok: false;
  assetIds: string[];
  error: string;
}

export type ReferenceAnchorApplyResult = ReferenceAnchorApplySuccess | ReferenceAnchorApplyFailure;

const DEFAULT_REFERENCE_LIMIT = 20;

export function appendAnchorReferences(
  currentAssetIds: string[],
  anchorAssetIds: string[],
  maxItems = DEFAULT_REFERENCE_LIMIT,
): ReferenceAnchorApplyResult {
  return finalizeAnchorReferences([...currentAssetIds, ...anchorAssetIds], maxItems);
}

export function replaceWithAnchorReferences(
  anchorAssetIds: string[],
  maxItems = DEFAULT_REFERENCE_LIMIT,
): ReferenceAnchorApplyResult {
  return finalizeAnchorReferences(anchorAssetIds, maxItems);
}

function finalizeAnchorReferences(assetIds: string[], maxItems: number): ReferenceAnchorApplyResult {
  const uniqueAssetIds = [...new Set(assetIds)];
  if (uniqueAssetIds.length > maxItems) {
    return {
      ok: false,
      assetIds: uniqueAssetIds,
      error: `参考图最多允许 ${maxItems} 张，未保存任何变更。`,
    };
  }
  return { ok: true, assetIds: uniqueAssetIds };
}
