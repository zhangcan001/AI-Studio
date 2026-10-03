import { lazy } from "react";
import type { AppRoute } from "./routes/types";
import type { ProjectView } from "../types/project";
import { normalCreate } from "../features/create/createModel";
import { normalLibrary } from "../features/library/libraryModel";
import { normalRuns } from "../features/runs/runsModel";

const CreatePage = lazy(() => import("../features/create/CreatePage").then(({ CreatePage }) => ({ default: CreatePage })));
const LibraryPage = lazy(() => import("../features/library/LibraryPage").then(({ LibraryPage }) => ({ default: LibraryPage })));
const RunsPage = lazy(() => import("../features/runs/RunsPage").then(({ RunsPage }) => ({ default: RunsPage })));

interface Props {
  project?: ProjectView;
  route: AppRoute;
  navigate: (route: AppRoute) => unknown;
  onDirtyChange: (dirty: boolean) => void;
}

/** Route composition only. The outer application Suspense and DOM host stay unchanged. */
export function NormalProductPages({ project: activeProject, route, navigate, onDirtyChange }: Props) {
  return <>
    {activeProject && normalCreate(route) && route.kind === "create" && <>
      <CreatePage key={activeProject.id} route={route} navigate={navigate} onDirtyChange={onDirtyChange} />
      <details><summary>高级创作</summary><button type="button" onClick={() => void navigate({ ...route, surface: "batch" })}>批量创作</button></details>
    </>}
    {activeProject && normalLibrary(route) && route.kind === "library" && <LibraryPage key={activeProject.id} route={route} navigate={navigate} />}
    {activeProject && normalRuns(route) && route.kind === "runs" && <RunsPage key={activeProject.id} route={route} navigate={navigate} />}
  </>;
}
