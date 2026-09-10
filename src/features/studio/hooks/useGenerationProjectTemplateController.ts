import { useCallback, useEffect, useRef, useState } from "react";
import { createProjectTemplate } from "../../../services/tauriClient";
import { toUserMessage } from "../../../i18n/errorMessages";
import type { GenerationValues, RecipeViewModel } from "../../../types/generation";

export interface UseGenerationProjectTemplateControllerOptions {
  projectId: string;
  selectedWorkflow?: RecipeViewModel;
  values: GenerationValues;
  onNotice: (message: string | null) => void;
}

export function useGenerationProjectTemplateController({
  projectId,
  selectedWorkflow,
  values,
  onNotice,
}: UseGenerationProjectTemplateControllerOptions) {
  const [templateEditorOpen, setTemplateEditorOpen] = useState(false);
  const [templateName, setTemplateName] = useState("");
  const [templateDescription, setTemplateDescription] = useState("");
  const [templateSaving, setTemplateSaving] = useState(false);
  const [templateError, setTemplateError] = useState<string>();
  const mountedRef = useRef(true);
  const lifecycleRef = useRef(0);
  const savingRef = useRef(false);

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
    };
  }, []);

  useEffect(() => {
    const lifecycle = ++lifecycleRef.current;
    savingRef.current = false;
    setTemplateEditorOpen(false);
    setTemplateName("");
    setTemplateDescription("");
    setTemplateSaving(false);
    setTemplateError(undefined);

    return () => {
      if (lifecycleRef.current === lifecycle) lifecycleRef.current += 1;
    };
  }, [projectId]);

  const openEditor = useCallback(() => {
    setTemplateError(undefined);
    setTemplateEditorOpen(true);
  }, []);

  const closeEditor = useCallback(() => {
    setTemplateEditorOpen(false);
  }, []);

  const save = useCallback(async () => {
    if (!selectedWorkflow || !templateName.trim() || savingRef.current) return;
    const lifecycle = lifecycleRef.current;
    savingRef.current = true;
    setTemplateSaving(true);
    setTemplateError(undefined);
    try {
      await createProjectTemplate({
        name: templateName,
        description: templateDescription.trim() || undefined,
        workflowVersionId: selectedWorkflow.workflowVersionId,
        recipeId: selectedWorkflow.recipeId,
        values,
      });
      if (!mountedRef.current || lifecycleRef.current !== lifecycle) return;
      setTemplateEditorOpen(false);
      setTemplateName("");
      setTemplateDescription("");
      onNotice("项目模板已保存；素材输入不会写入模板。");
    } catch (value: unknown) {
      if (mountedRef.current && lifecycleRef.current === lifecycle) setTemplateError(toUserMessage(value));
    } finally {
      if (lifecycleRef.current === lifecycle) savingRef.current = false;
      if (mountedRef.current && lifecycleRef.current === lifecycle) setTemplateSaving(false);
    }
  }, [onNotice, selectedWorkflow, templateDescription, templateName, values]);

  return {
    templateEditorOpen,
    templateName,
    templateDescription,
    templateSaving,
    templateError,
    openEditor,
    closeEditor,
    setTemplateName,
    setTemplateDescription,
    save,
  };
}
