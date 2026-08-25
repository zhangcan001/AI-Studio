import type { ReactNode } from "react";
import type { ComfyStatus } from "../../types/comfy";
import { comfyStatusLabel } from "../../i18n/statusLabels";
import "./StudioTopBar.css";

export interface StudioBreadcrumbItem {
  label: string;
  href?: string;
  onClick?: () => void;
  current?: boolean;
}

export interface StudioTopBarProps {
  breadcrumbs: readonly StudioBreadcrumbItem[];
  project?: { id: string; name: string } | null;
  projectSelector?: ReactNode;
  comfyStatus?: ComfyStatus | null;
  comfyLoading?: boolean;
  onSearch?: () => void;
  searchLabel?: string;
  searchShortcut?: string;
  onNotifications?: () => void;
  notificationCount?: number;
  onHelp?: () => void;
  onSettings?: () => void;
  onBrandClick?: () => void;
  className?: string;
}

export function StudioTopBar({
  breadcrumbs,
  project,
  projectSelector,
  comfyStatus,
  comfyLoading = false,
  onSearch,
  searchLabel = "搜索当前项目",
  searchShortcut = "Ctrl K",
  onNotifications,
  notificationCount = 0,
  onHelp,
  onSettings,
  onBrandClick,
  className,
}: StudioTopBarProps) {
  const status = comfyStatus?.status;
  const statusTone = status?.toLowerCase() ?? "offline";
  const statusLabel = comfyLoading ? "检查中" : comfyStatusLabel(status);
  const statusDetail = comfyLoading
    ? "正在读取运行状态"
    : comfyStatus?.endpoint ?? "未配置运行端点";
  const hasProjectSelector = projectSelector !== undefined && projectSelector !== null && projectSelector !== false;
  const rootClassName = ["studio-topbar", className].filter(Boolean).join(" ");

  const brand = (
    <span className="studio-topbar__brand-lockup">
      <span className="studio-topbar__brand-mark" aria-hidden="true">AI</span>
      <span className="studio-topbar__brand-copy">
        <strong>AI Studio</strong>
        <small>创作工作台</small>
      </span>
    </span>
  );

  return (
    <header className={rootClassName}>
      <div className="studio-topbar__identity">
        {onBrandClick ? (
          <button type="button" className="studio-topbar__brand-button" onClick={onBrandClick} aria-label="返回 AI Studio 项目工作台">
            {brand}
          </button>
        ) : (
          brand
        )}
      </div>

      <nav className="studio-topbar__breadcrumbs" aria-label="当前项目层级">
        {breadcrumbs.length ? breadcrumbs.map((item, index) => {
          const current = item.current ?? index === breadcrumbs.length - 1;
          return (
            <span className="studio-topbar__breadcrumb-group" key={`${item.label}-${index}`}>
              {index > 0 && <StudioTopBarIcon name="chevron" className="studio-topbar__breadcrumb-chevron" />}
              {item.onClick && !current ? (
                <button type="button" className="studio-topbar__breadcrumb-button" onClick={item.onClick}>{item.label}</button>
              ) : item.href && !current ? (
                <a href={item.href} className="studio-topbar__breadcrumb-link">{item.label}</a>
              ) : (
                <span className={current ? "studio-topbar__breadcrumb-current" : "studio-topbar__breadcrumb-label"} aria-current={current ? "page" : undefined}>
                  {item.label}
                </span>
              )}
            </span>
          );
        }) : (
          <span className="studio-topbar__breadcrumb-empty">未选择项目</span>
        )}
      </nav>

      <div className="studio-topbar__actions">
        {(hasProjectSelector || project) && (
          <div className="studio-topbar__project-context" title={project?.id}>
            {hasProjectSelector ? projectSelector : <span>{project?.name}</span>}
          </div>
        )}
        <div className={`studio-topbar__comfy-status studio-topbar__comfy-status--${statusTone}`} title={statusDetail}>
          <span className="studio-topbar__status-dot" aria-hidden="true" />
          <span className="studio-topbar__comfy-copy">
            <strong>ComfyUI {statusLabel}</strong>
            <small>{statusDetail}</small>
          </span>
        </div>

        {onSearch && (
          <button
            type="button"
            className="studio-topbar__search-trigger"
            onClick={onSearch}
            aria-keyshortcuts={searchShortcut}
          >
            <StudioTopBarIcon name="search" />
            <span>{searchLabel}</span>
            {searchShortcut && <kbd>{searchShortcut}</kbd>}
          </button>
        )}

        <div className="studio-topbar__system-actions" aria-label="系统操作">
          {onNotifications && (
            <button type="button" className="studio-topbar__icon-button" onClick={onNotifications} aria-label="通知">
              <StudioTopBarIcon name="bell" />
              {notificationCount > 0 && <span className="studio-topbar__notification-badge" aria-label={`${notificationCount} 条通知`}>{notificationCount > 9 ? "9+" : notificationCount}</span>}
            </button>
          )}
          {onHelp && (
            <button type="button" className="studio-topbar__icon-button" onClick={onHelp} aria-label="帮助">
              <StudioTopBarIcon name="help" />
            </button>
          )}
          {onSettings && (
            <button type="button" className="studio-topbar__icon-button" onClick={onSettings} aria-label="设置">
              <StudioTopBarIcon name="settings" />
            </button>
          )}
        </div>
      </div>
    </header>
  );
}

function StudioTopBarIcon({ name, className }: { name: "bell" | "chevron" | "help" | "search" | "settings"; className?: string }) {
  const iconPaths: Record<typeof name, string> = {
    bell: "M18 8a6 6 0 0 0-12 0c0 7-3 7-3 9h18c0-2-3-2-3-9 M10 21h4",
    chevron: "m9 18 6-6-6-6",
    help: "M9.5 9a2.7 2.7 0 1 1 4.4 2.1c-1.2.9-1.9 1.4-1.9 3 M12 17.5v.1 M12 4a8 8 0 1 0 0 16 8 8 0 0 0 0-16z",
    search: "m20 20-4.4-4.4 M10.8 17a6.2 6.2 0 1 0 0-12.4 6.2 6.2 0 0 0 0 12.4z",
    settings: "M12 8.5a3.5 3.5 0 1 0 0 7 3.5 3.5 0 0 0 0-7z M19.4 15a1.7 1.7 0 0 0 .3 1.9l.1.1-1.8 1.8-.1-.1a1.7 1.7 0 0 0-1.9-.3 1.7 1.7 0 0 0-1 1.6v.2h-2.6v-.2a1.7 1.7 0 0 0-1-1.6 1.7 1.7 0 0 0-1.9.3l-.1.1-1.8-1.8.1-.1a1.7 1.7 0 0 0 .3-1.9 1.7 1.7 0 0 0-1.6-1H6v-2.6h.2a1.7 1.7 0 0 0 1.6-1 1.7 1.7 0 0 0-.3-1.9l-.1-.1 1.8-1.8.1.1a1.7 1.7 0 0 0 1.9.3 1.7 1.7 0 0 0 1-1.6V4.8h2.6V5a1.7 1.7 0 0 0 1 1.6 1.7 1.7 0 0 0 1.9-.3l.1-.1 1.8 1.8-.1.1a1.7 1.7 0 0 0-.3 1.9 1.7 1.7 0 0 0 1.6 1h.2v2.6h-.2a1.7 1.7 0 0 0-1.6 1z",
  };

  return (
    <svg className={className} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <path d={iconPaths[name]} />
    </svg>
  );
}
