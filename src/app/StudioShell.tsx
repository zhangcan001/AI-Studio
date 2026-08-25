import type { ReactNode } from "react";
import type { ComfyStatus } from "../types/comfy";
import type { Workspace } from "../types/workspaceResume";
import {
  StudioGlobalRail,
  type StudioGlobalRailProps,
  type StudioRailItem,
  type StudioRailItemId,
} from "../components/studio/StudioGlobalRail";
import { StudioTopBar, type StudioBreadcrumbItem } from "../components/studio/StudioTopBar";
import "./StudioShell.css";

export interface StudioShellProps {
  children: ReactNode;
  workspace?: Workspace;
  currentSection?: StudioRailItemId;
  onNavigate: StudioGlobalRailProps["onNavigate"];
  project?: { id: string; name: string } | null;
  projectSelector?: ReactNode;
  comfyStatus?: ComfyStatus | null;
  comfyLoading?: boolean;
  breadcrumbs?: readonly StudioBreadcrumbItem[];
  onSearch?: () => void;
  searchLabel?: string;
  searchShortcut?: string;
  onNotifications?: () => void;
  notificationCount?: number;
  onHelp?: () => void;
  onSettings?: () => void;
  onBrandClick?: () => void;
  railItems?: readonly StudioRailItem[];
  projectStructure?: ReactNode;
  inspector?: ReactNode;
  queueSlot?: ReactNode;
  inspectorCollapsed?: boolean;
  queueCollapsed?: boolean;
  projectStructureLabel?: string;
  inspectorLabel?: string;
  queueLabel?: string;
  mainLabel?: string;
  className?: string;
}

export function StudioShell({
  children,
  workspace,
  currentSection,
  onNavigate,
  project,
  projectSelector,
  comfyStatus,
  comfyLoading = false,
  breadcrumbs = [],
  onSearch,
  searchLabel,
  searchShortcut,
  onNotifications,
  notificationCount,
  onHelp,
  onSettings,
  onBrandClick,
  railItems,
  projectStructure,
  inspector,
  queueSlot,
  inspectorCollapsed = false,
  queueCollapsed = true,
  projectStructureLabel = "项目结构",
  inspectorLabel = "检查器",
  queueLabel = "生产队列",
  mainLabel = "当前工作区",
  className,
}: StudioShellProps) {
  const resolvedSection = currentSection ?? (workspace ? workspaceToRailSection[workspace] : undefined);
  const hasStructure = projectStructure !== undefined && projectStructure !== null && projectStructure !== false;
  const hasInspector = inspector !== undefined && inspector !== null && inspector !== false && !inspectorCollapsed;
  const hasQueue = queueSlot !== undefined && queueSlot !== null && queueSlot !== false;
  const shellClassName = [
    "studio-shell",
    !hasStructure && "studio-shell--without-structure",
    !hasInspector && "studio-shell--without-inspector",
    hasQueue && !queueCollapsed && "studio-shell--queue-expanded",
    className,
  ].filter(Boolean).join(" ");

  return (
    <div className={shellClassName}>
      <a className="studio-shell__skip-link" href="#studio-shell-main">跳到当前工作区</a>
      <StudioTopBar
        breadcrumbs={breadcrumbs}
        project={project}
        projectSelector={projectSelector}
        comfyStatus={comfyStatus}
        comfyLoading={comfyLoading}
        onSearch={onSearch}
        searchLabel={searchLabel}
        searchShortcut={searchShortcut}
        onNotifications={onNotifications}
        notificationCount={notificationCount}
        onHelp={onHelp}
        onSettings={onSettings}
        onBrandClick={onBrandClick}
      />
      <div className="studio-shell__body">
        <StudioGlobalRail activeItem={resolvedSection} items={railItems} onNavigate={onNavigate} />
        {hasStructure && (
          <aside className="studio-shell__structure" aria-label={projectStructureLabel}>
            {projectStructure}
          </aside>
        )}
        <main id="studio-shell-main" className="studio-shell__main" aria-label={mainLabel} tabIndex={-1}>
          {children}
        </main>
        {hasInspector && (
          <aside className="studio-shell__inspector" aria-label={inspectorLabel}>
            {inspector}
          </aside>
        )}
      </div>
      {hasQueue && (
        <section className="studio-shell__queue" aria-label={queueLabel}>
          {queueSlot}
        </section>
      )}
    </div>
  );
}

const workspaceToRailSection: Partial<Record<Workspace, StudioRailItemId>> = {
  "command-center": "project",
  studio: "creation",
  video: "production",
  assets: "assets",
  shots: "creation",
  tasks: "review",
  projects: "project",
  workflows: "creation",
  settings: "settings",
};
