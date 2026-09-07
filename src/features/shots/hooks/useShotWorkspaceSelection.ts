import { useCallback, useEffect, useRef, useState } from "react";
import type { WorkspaceSelection } from "../../../types/workspaceSelection";

export interface UseShotWorkspaceSelectionOptions {
  projectId: string;
  initialSelectedShotId?: string;
  onShotSelected?: (shotId?: string) => void;
}

export interface UseShotWorkspaceSelectionResult {
  selectedShotId?: string;
  workspaceSelection: WorkspaceSelection;
  selectWorkspaceSelection: (selection: WorkspaceSelection) => void;
  selectShot: (shotId: string) => void;
  reconcileSelectedShot: (shotIds: string[]) => string | undefined;
}

export function useShotWorkspaceSelection({
  projectId,
  initialSelectedShotId,
  onShotSelected,
}: UseShotWorkspaceSelectionOptions): UseShotWorkspaceSelectionResult {
  const initialSelection = initialSelectedShotId
    ? { type: "shot" as const, shotId: initialSelectedShotId }
    : { type: "project" as const, projectId };
  const [selectedShotId, setSelectedShotId] = useState<string | undefined>(initialSelectedShotId);
  const [workspaceSelection, setWorkspaceSelection] = useState<WorkspaceSelection>(initialSelection);
  const selectedShotIdRef = useRef(selectedShotId);
  const workspaceSelectionRef = useRef(workspaceSelection);
  const initialSelectedShotIdRef = useRef(initialSelectedShotId);
  const onShotSelectedRef = useRef(onShotSelected);

  useEffect(() => {
    onShotSelectedRef.current = onShotSelected;
  }, [onShotSelected]);

  useEffect(() => {
    initialSelectedShotIdRef.current = initialSelectedShotId;
  }, [initialSelectedShotId]);

  useEffect(() => {
    if (workspaceSelectionRef.current.type === "shot") {
      return;
    }
    const nextSelection = initialSelectedShotId
      ? { type: "shot" as const, shotId: initialSelectedShotId }
      : { type: "project" as const, projectId };
    selectedShotIdRef.current = initialSelectedShotId;
    workspaceSelectionRef.current = nextSelection;
    setSelectedShotId(initialSelectedShotId);
    setWorkspaceSelection(nextSelection);
  }, [initialSelectedShotId, projectId]);

  const selectWorkspaceSelection = useCallback((selection: WorkspaceSelection) => {
    workspaceSelectionRef.current = selection;
    setWorkspaceSelection(selection);
    if (selection.type !== "shot") return;
    selectedShotIdRef.current = selection.shotId;
    setSelectedShotId(selection.shotId);
    onShotSelectedRef.current?.(selection.shotId);
  }, []);

  const selectShot = useCallback((shotId: string) => {
    selectWorkspaceSelection({ type: "shot", shotId });
  }, [selectWorkspaceSelection]);

  const reconcileSelectedShot = useCallback((shotIds: string[]) => {
    const current = selectedShotIdRef.current;
    const nextSelectedShotId = current && shotIds.includes(current)
      ? current
      : initialSelectedShotIdRef.current && shotIds.includes(initialSelectedShotIdRef.current)
        ? initialSelectedShotIdRef.current
        : shotIds[0];

    if (nextSelectedShotId !== current) {
      selectedShotIdRef.current = nextSelectedShotId;
      setSelectedShotId(nextSelectedShotId);
      onShotSelectedRef.current?.(nextSelectedShotId);
    }

    const currentSelection = workspaceSelectionRef.current;
    if (currentSelection.type === "shot" && currentSelection.shotId !== nextSelectedShotId) {
      const nextSelection = nextSelectedShotId
        ? { type: "shot" as const, shotId: nextSelectedShotId }
        : { type: "project" as const, projectId };
      workspaceSelectionRef.current = nextSelection;
      setWorkspaceSelection(nextSelection);
    }
    return nextSelectedShotId;
  }, [projectId]);

  return {
    selectedShotId,
    workspaceSelection,
    selectWorkspaceSelection,
    selectShot,
    reconcileSelectedShot,
  };
}
