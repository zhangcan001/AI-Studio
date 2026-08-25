import type { PromptTemplateAnalysis } from "../../types/promptTemplate";

export const PROMPT_TEMPLATE_VARIABLE_GROUPS = [
  {
    label: "项目",
    variables: ["project.id", "project.name", "project.description"],
  },
  {
    label: "系列",
    variables: ["series.id", "series.name", "series.description", "series.number"],
  },
  {
    label: "集",
    variables: ["episode.id", "episode.name", "episode.description", "episode.number"],
  },
  {
    label: "场景",
    variables: ["scene.id", "scene.name", "scene.description", "scene.number"],
  },
  {
    label: "镜头",
    variables: ["shot.id", "shot.name", "shot.number", "shot.basePrompt"],
  },
  {
    label: "锚点",
    variables: [
      "anchors.character.names",
      "anchors.character.context",
      "anchors.scene.names",
      "anchors.scene.context",
      "anchors.prop.names",
      "anchors.prop.context",
      "anchors.style.names",
      "anchors.style.context",
      "anchors.all.names",
      "anchors.all.context",
    ],
  },
  { label: "自定义", variables: ["custom.camera", "custom.mood"] },
] as const;

const BUILTIN_VARIABLES: ReadonlySet<string> = new Set(
  PROMPT_TEMPLATE_VARIABLE_GROUPS.flatMap((group) => group.variables).filter((variable) => !variable.startsWith("custom.")),
);
const VARIABLE_PATTERN = /\{\{\s*([A-Za-z0-9_.-]+)\s*\}\}/g;

export function isPromptTemplateText(text: string): boolean {
  return text.includes("{{");
}

export function extractPromptTemplateVariables(text: string): string[] {
  const variables: string[] = [];
  for (const match of text.matchAll(VARIABLE_PATTERN)) {
    const variable = match[1];
    if (!variables.includes(variable)) variables.push(variable);
  }
  return variables;
}

export function customPromptVariableNames(variables: readonly string[]): string[] {
  return variables
    .filter((variable) => variable.startsWith("custom."))
    .map((variable) => variable.slice("custom.".length))
    .filter(Boolean);
}

export function analyzePromptTemplateText(text: string): PromptTemplateAnalysis {
  const variables = extractPromptTemplateVariables(text);
  const builtinVariables = variables.filter((variable) => BUILTIN_VARIABLES.has(variable));
  const customVariables = variables.filter((variable) => variable.startsWith("custom."));
  return {
    isTemplate: isPromptTemplateText(text),
    variables,
    builtinVariables,
    customVariables,
    requiresStructure: variables.some((variable) => /^(series|episode|scene)\./.test(variable)),
  };
}

export function toggleOrderedPromptSelection(current: readonly string[], id: string, checked: boolean): string[] {
  if (checked) return current.includes(id) ? [...current] : [...current, id];
  return current.filter((item) => item !== id);
}

export function promptTemplateVariableLabel(variable: string): string {
  return `{{${variable}}}`;
}
