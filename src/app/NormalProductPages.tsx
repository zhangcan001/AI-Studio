import { lazy } from "react";
import type { AppRoute } from "./routes/types";
import type { ProjectView } from "../types/project";
import type { ComfyStatus } from "../types/comfy";
import { normalCreate } from "../features/create/createModel";
import { normalLibrary } from "../features/library/libraryModel";
import { normalRuns } from "../features/runs/runsModel";
import { useStudioStore } from "../stores/studioStore";

const CreatePage = lazy(() => import("../features/create/CreatePage").then(({ CreatePage }) => ({ default: CreatePage })));
const StoryboardWorkbench = lazy(() => import("../features/create/StoryboardWorkbench").then(({ StoryboardWorkbench }) => ({ default: StoryboardWorkbench })));
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
  // A pending Library intent still needs the single-shot slot chooser, so it bypasses the workbench.
  const pendingLibraryIntent = useStudioStore(state => state.pendingLibraryIntent);
  const storyboard = route.kind === "create" && route.stage === "video" && !route.shotId && !pendingLibraryIntent;
  return <>
    {activeProject && normalCreate(route) && route.kind === "create" && storyboard && <StoryboardWorkbench key={activeProject.id} route={route} navigate={navigate} />}
    {activeProject && (normalCreate(route) || (route.kind === "create" && route.stage === "image")) && route.kind === "create" && !storyboard && <CreatePage key={activeProject.id} route={route} runtime={runtime} navigate={navigate} onDirtyChange={onDirtyChange} />}
    {activeProject && normalLibrary(route) && route.kind === "library" && <LibraryPage key={activeProject.id} route={route} navigate={navigate} />}
    {activeProject && normalRuns(route) && route.kind === "runs" && <RunsPage key={activeProject.id} route={route} navigate={navigate} />}
  </>;
}
