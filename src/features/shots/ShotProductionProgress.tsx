import type { ShotProductionReadModel, ShotProductionStep, ShotProductionStepId, ShotProductionStepStatus } from "./shotProductionState";
import "./ShotCreationWorkspace.css";

export interface ShotProductionProgressProps {
  model: ShotProductionReadModel;
  onNavigate: (stepId: ShotProductionStepId) => void;
}
const statusLabels: Record<ShotProductionStepStatus, string> = {
  COMPLETE: "已完成",
  ACTIVE: "进行中",
  READY: "待处理",
  BLOCKED: "受阻",
  PENDING: "等待",
  FAILED: "失败",
};

function StepContent({ step }: { step: ShotProductionStep }) {
  return (
    <span className="shot-production-progress-step-content">
      <strong>{step.label}</strong>
      <small>{statusLabels[step.status]}</small>
    </span>
  );
}

export function ShotProductionProgress({ model, onNavigate }: ShotProductionProgressProps) {
  return (
    <section className="shot-production-progress" aria-label="镜头生产流程">
      <nav aria-label="镜头生产步骤">
        <ol className="shot-production-progress-list">
          {model.steps.map((step) => (
            <li key={step.id} className={`shot-production-progress-item shot-production-progress-item-${step.status.toLowerCase()}`}>
              {step.actionable ? (
                <button
                  type="button"
                  className="shot-production-progress-step"
                  aria-current={model.nextAction.stepId === step.id ? "step" : undefined}
                  aria-label={`${step.label}：${statusLabels[step.status]}`}
                  onClick={() => onNavigate(step.id)}
                >
                  <StepContent step={step} />
                </button>
              ) : (
                <span className="shot-production-progress-step" aria-current={model.nextAction.stepId === step.id ? "step" : undefined}>
                  <StepContent step={step} />
                </span>
              )}
            </li>
          ))}
        </ol>
      </nav>
      <div className="shot-production-next-action" aria-label="下一步">
        <div>
          <span className="shot-production-next-action-label">下一步</span>
          <strong>{model.nextAction.label}</strong>
          {model.nextAction.detail && <small>{model.nextAction.detail}</small>}
        </div>
        {model.nextAction.actionable && (
          <button type="button" className="shot-production-next-action-button" onClick={() => onNavigate(model.nextAction.stepId)}>
            继续
          </button>
        )}
      </div>
    </section>
  );
}
