// Type-only surface for the immutable Phase 7B receipt eligibility guard.
// Does not alter the validator runtime, manifest, or historical proof bytes.
export declare const PHASE7B_RECEIPT_BASE: string;
export declare const PHASE7B_RECEIPT_PROOF: string;
export declare const PHASE7B_RECEIPT_CHANGED: string[];
export declare const PHASE7B_RECEIPT_ADDED: string[];
export declare function uiPhase7BTsvReceiptParentReader(
  root: string,
  override?: Record<string, string>,
): {
  violations: string[];
  changedPaths: string[];
  addedPaths: string[];
  afterHashes: Record<string, string>;
  read: (path: string) => string;
};
