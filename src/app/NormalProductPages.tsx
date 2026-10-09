import { lazy } from "react";
import type { AppRoute } from "./routes/types";
import type { ProjectView } from "../types/project";
import type { ComfyStatus } from "../types/comfy";
import { normalCreate } from "../features/create/createModel";
import { normalLibrary } from "../features/library/libraryModel";
import { normalRuns } from "../features/runs/runsModel";

const CreatePage = lazy(() => import("../features/create/CreatePage").then(({ CreatePage }) => ({ default: CreatePage })));
const LibraryPage = lazy(() => import("../features/library/LibraryPage").then(({ LibraryPage }) => ({ default: LibraryPage })));
const RunsPage = lazy(() => import("../features/runs/RunsPage").then(({ RunsPage }) => ({ default: RunsPage })));

interface Props {
  project?: ProjectView;
  runtime?: ComfyStatus;
  route: AppRoute;
  navigate: (route: AppRoute) => unknown;
  onDirtyChange: (dirty: boolean) => void;
}

/** Route composition only. The outer application Suspense and DOM host stay unchanged. */
export function NormalProductPages({ project: activeProject, runtime, route, navigate, onDirtyChange }: Props) {
  return <>
    {activeProject && (normalCreate(route) || (route.kind === "create" && route.stage === "image")) && route.kind === "create" && <>
      <CreatePage key={activeProject.id} route={route} runtime={runtime} navigate={navigate} onDirtyChange={onDirtyChange} />
      {route.stage === "video" && <details><summary>高级创作</summary><button type="button" onClick={() => void navigate({ ...route, surface: "batch" })}>批量创作</button></details>}
    </>}
    {activeProject && normalLibrary(route) && route.kind === "library" && <LibraryPage key={activeProject.id} route={route} navigate={navigate} />}
    {activeProject && normalRuns(route) && route.kind === "runs" && <RunsPage key={activeProject.id} route={route} navigate={navigate} />}
  </>;
}
