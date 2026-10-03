import { useCallback, useEffect, useState } from "react";
import { toUserMessage } from "../../i18n/errorMessages";
import type { ShotStage } from "../../types/shot";
import type { ConsistencyBindingPack, ConsistencyContextPreview, ConsistencyBindingReplaceInput, ConsistencyCostumeOption, ConsistencyProfileOption, ConsistencyReferenceSetOption, ConsistencyScopeRef } from "../../types/consistencyBindings";

export interface ShotConsistencyServices {
  listConsistencyProfiles: (projectId: string) => Promise<ConsistencyProfileOption[]>;
  listReferenceSets: (projectId: string) => Promise<ConsistencyReferenceSetOption[]>;
  listCostumeVariants: (projectId: string, characterId: string) => Promise<ConsistencyCostumeOption[]>;
  getShotConsistencyBinding: (projectId: string, shotId: string) => Promise<ConsistencyBindingPack>;
  getConsistencyScopeBinding: (projectId: string, scopeType: ConsistencyScopeRef["scopeType"], scopeId: string) => Promise<ConsistencyBindingPack>;
  getShotContextDraft: (projectId: string, shotId: string, stage: ShotStage) => Promise<ConsistencyContextPreview>;
  replaceShotConsistencyBinding: (input: ConsistencyBindingReplaceInput) => Promise<void>;
  replaceConsistencyScopeBinding: (input: ConsistencyBindingReplaceInput) => Promise<void>;
}

/** Project-scoped options and binding commands; no second persisted shot authority. */
export function useShotConsistencyController(activeProjectId: string | undefined, services: ShotConsistencyServices) {
  const { getConsistencyScopeBinding, getShotConsistencyBinding, getShotContextDraft, listConsistencyProfiles, listCostumeVariants, listReferenceSets, replaceConsistencyScopeBinding, replaceShotConsistencyBinding } = services;
  const [consistencyProfiles, setConsistencyProfiles] = useState<ConsistencyProfileOption[]>([]);
  const [consistencyReferenceSets, setConsistencyReferenceSets] = useState<ConsistencyReferenceSetOption[]>([]);
  const [consistencyCostumes, setConsistencyCostumes] = useState<Record<string, ConsistencyCostumeOption[]>>({});
  const [consistencyLoading, setConsistencyLoading] = useState(false);
  const [consistencyError, setConsistencyError] = useState<string>();
  useEffect(() => {
    if (!activeProjectId) {
      setConsistencyProfiles([]);
      setConsistencyReferenceSets([]);
      setConsistencyCostumes({});
      setConsistencyError(undefined);
      setConsistencyLoading(false);
      return;
    }
    const requestedProjectId = activeProjectId;
    let cancelled = false;
    setConsistencyLoading(true);
    setConsistencyError(undefined);
    void Promise.all([
      listConsistencyProfiles(requestedProjectId),
      listReferenceSets(requestedProjectId),
    ])
      .then(async ([profiles, referenceSets]) => {
        const characterProfiles = profiles.filter((profile) => profile.profileType === "CHARACTER");
        const costumeEntries = await Promise.all(
          characterProfiles.map(async (profile) => [
            profile.id,
            await listCostumeVariants(requestedProjectId, profile.id).catch(() => []),
          ] as const),
        );
        if (cancelled) return;
        setConsistencyProfiles(profiles.map((profile) => ({
          id: profile.id,
          projectId: profile.projectId,
          profileType: profile.profileType,
          name: profile.name,
          description: profile.description,
        })));
        setConsistencyReferenceSets(referenceSets.map((referenceSet) => ({
          id: referenceSet.id,
          projectId: referenceSet.projectId,
          name: referenceSet.name,
          purpose: referenceSet.purpose,
          itemCount: referenceSet.itemCount,
          imageCount: referenceSet.imageCount,
        })));
        setConsistencyCostumes(Object.fromEntries(costumeEntries.map(([profileId, costumes]) => [
          profileId,
          costumes.map((costume) => ({
            id: costume.id,
            characterProfileId: costume.characterProfileId,
            name: costume.name,
            promptFragment: costume.promptFragment,
            referenceSetId: costume.referenceSetId,
            isDefault: costume.isDefault,
          })),
        ])));
      })
      .catch((loadError: unknown) => {
        if (!cancelled) setConsistencyError(toUserMessage(loadError));
      })
      .finally(() => {
        if (!cancelled) setConsistencyLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [activeProjectId, listConsistencyProfiles, listReferenceSets, listCostumeVariants]);

  const loadConsistencyBindingPack = useCallback(async (scope: ConsistencyScopeRef) => {
    if (scope.scopeType === "SHOT") {
      return getShotConsistencyBinding(activeProjectId ?? scope.scopeId, scope.scopeId);
    }
    return getConsistencyScopeBinding(activeProjectId ?? scope.scopeId, scope.scopeType, scope.scopeId);
  }, [activeProjectId, getShotConsistencyBinding, getConsistencyScopeBinding]);

  const saveConsistencyBindingPack = useCallback(async (input: ConsistencyBindingReplaceInput) => {
    if (input.scopeType === "SHOT") {
      await replaceShotConsistencyBinding(input);
    } else {
      await replaceConsistencyScopeBinding(input);
    }
  }, [replaceShotConsistencyBinding, replaceConsistencyScopeBinding]);

  const loadConsistencyContext = useCallback(async (scope: ConsistencyScopeRef, stage: ShotStage): Promise<ConsistencyContextPreview | null> => {
    if (scope.scopeType !== "SHOT") {
      return null;
    }
    return getShotContextDraft(activeProjectId ?? scope.scopeId, scope.scopeId, stage);
  }, [activeProjectId, getShotContextDraft]);

  return { consistencyProfiles, consistencyReferenceSets, consistencyCostumes, consistencyLoading, consistencyError, loadConsistencyBindingPack, saveConsistencyBindingPack, loadConsistencyContext };
}
