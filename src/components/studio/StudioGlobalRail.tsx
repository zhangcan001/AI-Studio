import type { Workspace } from "../../types/workspaceResume";
import "./StudioGlobalRail.css";

export type StudioRailItemId = "project" | "creation" | "assets" | "production" | "review" | "analysis" | "settings";
export type StudioRailIconName = "analysis" | "assets" | "creation" | "production" | "project" | "review" | "settings";

export interface StudioRailItem {
  id: StudioRailItemId;
  label: string;
  destination: Workspace;
  icon: StudioRailIconName;
  hint?: string;
}

export const defaultStudioRailItems: readonly StudioRailItem[] = [
  { id: "project", label: "项目", destination: "command-center", icon: "project", hint: "项目指挥中心" },
  { id: "creation", label: "创作", destination: "shots", icon: "creation", hint: "镜头创作工作区" },
  { id: "assets", label: "资产", destination: "assets", icon: "assets", hint: "资产库" },
  { id: "production", label: "生产", destination: "shots", icon: "production", hint: "生产队列与批量运行" },
  { id: "review", label: "审核", destination: "shots", icon: "review", hint: "镜头审核与任务" },
  { id: "analysis", label: "分析", destination: "command-center", icon: "analysis", hint: "项目进度与生产概览" },
  { id: "settings", label: "设置", destination: "settings", icon: "settings", hint: "运行时与应用设置" },
];

export interface StudioGlobalRailProps {
  activeItem?: StudioRailItemId;
  items?: readonly StudioRailItem[];
  onNavigate: (destination: Workspace, item: StudioRailItem) => void;
  className?: string;
}

export function StudioGlobalRail({ activeItem, items = defaultStudioRailItems, onNavigate, className }: StudioGlobalRailProps) {
  const rootClassName = ["studio-global-rail", className].filter(Boolean).join(" ");

  return (
    <aside className={rootClassName} aria-label="全局工作区导航">
      <div className="studio-global-rail__brand" aria-label="AI Studio">
        <span className="studio-global-rail__brand-mark" aria-hidden="true">AI</span>
      </div>
      <nav className="studio-global-rail__nav" aria-label="主要工作区">
        {items.map((item) => {
          const active = activeItem === item.id;
          return (
            <button
              type="button"
              key={item.id}
              className={active ? "studio-global-rail__item studio-global-rail__item--active" : "studio-global-rail__item"}
              onClick={() => onNavigate(item.destination, item)}
              aria-current={active ? "page" : undefined}
              aria-label={item.hint ? `${item.label}：${item.hint}` : item.label}
              title={item.hint ?? item.label}
            >
              <StudioGlobalRailIcon name={item.icon} />
              <span>{item.label}</span>
            </button>
          );
        })}
      </nav>
      <div className="studio-global-rail__footer" aria-label="本地工作区">
        <span>本地</span>
      </div>
    </aside>
  );
}

function StudioGlobalRailIcon({ name }: { name: StudioRailIconName }) {
  const iconPaths: Record<StudioRailIconName, string> = {
    analysis: "M4 19V5 M4 19h16 M8 16v-4 M12 16V8 M16 16v-7",
    assets: "M3.5 7h6l2 2h9v10h-17z M3.5 7V5.5A1.5 1.5 0 0 1 5 4h5l2 2h7.5A1.5 1.5 0 0 1 21 7.5V9",
    creation: "M12 3v4 M12 17v4 M3 12h4 M17 12h4 M5.6 5.6l2.8 2.8 M15.6 15.6l2.8 2.8 M18.4 5.6l-2.8 2.8 M8.4 15.6l-2.8 2.8 M12 8.5a3.5 3.5 0 1 0 0 7 3.5 3.5 0 0 0 0-7z",
    production: "M5 5h14v14H5z M9 8.5v7l6-3.5z",
    project: "M4 5h16v14H4z M8 9h3 M13 9h3 M8 13h3 M13 13h3 M8 17h8",
    review: "M5 4h14v16H5z M8.5 9.5 11 12l4.5-5 M8 16h8",
    settings: "M12 8.5a3.5 3.5 0 1 0 0 7 3.5 3.5 0 0 0 0-7z M19.4 15a1.7 1.7 0 0 0 .3 1.9l.1.1-1.8 1.8-.1-.1a1.7 1.7 0 0 0-1.9-.3 1.7 1.7 0 0 0-1 1.6v.2h-2.6v-.2a1.7 1.7 0 0 0-1-1.6 1.7 1.7 0 0 0-1.9.3l-.1.1-1.8-1.8.1-.1a1.7 1.7 0 0 0 .3-1.9 1.7 1.7 0 0 0-1.6-1H6v-2.6h.2a1.7 1.7 0 0 0 1.6-1 1.7 1.7 0 0 0-.3-1.9l-.1-.1 1.8-1.8.1.1a1.7 1.7 0 0 0 1.9.3 1.7 1.7 0 0 0 1-1.6V4.8h2.6V5a1.7 1.7 0 0 0 1 1.6 1.7 1.7 0 0 0 1.9-.3l.1-.1 1.8 1.8-.1.1a1.7 1.7 0 0 0-.3 1.9 1.7 1.7 0 0 0 1.6 1h.2v2.6h-.2a1.7 1.7 0 0 0-1.6 1z",
  };

  return (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <path d={iconPaths[name]} />
    </svg>
  );
}
