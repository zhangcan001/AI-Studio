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
  const initialTargetActiveRef = useRef(Boolean(initialSelectedShotId));
  const projectIdRef = useRef(projectId);
  const onShotSelectedRef = useRef(onShotSelected);

  useEffect(() => {
    onShotSelectedRef.current = onShotSelected;
  }, [onShotSelected]);

  useEffect(() => {
    const projectChanged = projectIdRef.current !== projectId;
    const initialTargetChanged = initialSelectedShotIdRef.current !== initialSelectedShotId;
    const targetAlreadySelected = initialTargetChanged && initialSelectedShotId === selectedShotIdRef.current;
    projectIdRef.current = projectId;
    initialSelectedShotIdRef.current = initialSelectedShotId;
    if (projectChanged || (initialTargetChanged && !targetAlreadySelected)) initialTargetActiveRef.current = Boolean(initialSelectedShotId);
    if (!projectChanged && !initialTargetChanged && workspaceSelectionRef.current.type === "shot") {
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
    if (selection.type !== "shot") {
      initialTargetActiveRef.current = false;
      return;
    }
    if (selection.shotId !== initialSelectedShotIdRef.current) initialTargetActiveRef.current = false;
    selectedShotIdRef.current = selection.shotId;
    setSelectedShotId(selection.shotId);
    onShotSelectedRef.current?.(selection.shotId);
  }, []);

  const selectShot = useCallback((shotId: string) => {
    selectWorkspaceSelection({ type: "shot", shotId });
  }, [selectWorkspaceSelection]);

  const reconcileSelectedShot = useCallback((shotIds: string[]) => {
    const current = selectedShotIdRef.current;
    const requestedShotId = initialSelectedShotIdRef.current;
    const nextSelectedShotId = current && shotIds.includes(current)
      ? current
      : requestedShotId && initialTargetActiveRef.current
        ? shotIds.includes(requestedShotId) ? requestedShotId : undefined
        : shotIds[0];

    if (nextSelectedShotId !== current) {
      selectedShotIdRef.current = nextSelectedShotId;
      setSelectedShotId(nextSelectedShotId);
      if (nextSelectedShotId || !requestedShotId || !initialTargetActiveRef.current) onShotSelectedRef.current?.(nextSelectedShotId);
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
