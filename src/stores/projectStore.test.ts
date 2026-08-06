import { beforeEach, describe, expect, it } from "vitest";
import {
  ACTIVE_PROJECT_STORAGE_KEY,
  DEFAULT_PROJECT_ID,
  resolveActiveProjectId,
  useProjectStore,
} from "./projectStore";
import type { ProjectView } from "../types/project";

const projects: ProjectView[] = [
  { id: DEFAULT_PROJECT_ID, name: "Default", description: null, createdAt: "1", updatedAt: "1" },
  { id: "prj_other", name: "Other", description: "", createdAt: "2", updatedAt: "2" },
];

describe("project store context selection", () => {
  beforeEach(() => {
    if (typeof globalThis.localStorage === "undefined") {
      const values = new Map<string, string>();
      Object.defineProperty(globalThis, "localStorage", {
        configurable: true,
        value: {
          getItem: (key: string) => values.get(key) ?? null,
          setItem: (key: string, value: string) => values.set(key, value),
          removeItem: (key: string) => values.delete(key),
          clear: () => values.clear(),
        },
      });
    }
    globalThis.localStorage.clear();
    useProjectStore.setState({ projects: [], activeProjectId: undefined, loading: true, error: undefined });
  });

  it("prefers a saved existing project, then default, then first project", () => {
    expect(resolveActiveProjectId(projects, "prj_other")).toBe("prj_other");
    expect(resolveActiveProjectId(projects, "missing")).toBe(DEFAULT_PROJECT_ID);
    expect(resolveActiveProjectId(projects.slice(1), "missing")).toBe("prj_other");
  });

  it("persists the active project and exposes the active project getter", () => {
    useProjectStore.getState().setProjects(projects);
    useProjectStore.getState().setActiveProject("prj_other");
    expect(globalThis.localStorage.getItem(ACTIVE_PROJECT_STORAGE_KEY)).toBe("prj_other");
    expect(useProjectStore.getState().activeProject()?.name).toBe("Other");
  });
});
