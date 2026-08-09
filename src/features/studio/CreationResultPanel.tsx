import type { TaskView } from "../../types/task";
import { ImageOutput } from "./ImageOutput";
import { TaskProgressCard } from "./TaskProgressCard";

interface Props {
  projectId: string;
  task?: TaskView;
  cancelling: boolean;
  onCancel: () => void;
  onOpenTask?: () => void;
}

export function CreationResultPanel({ projectId, task, cancelling, onCancel, onOpenTask }: Props) {
  return (
    <aside className="creation-result-column" aria-label="任务和生成结果">
      {task && <TaskProgressCard task={task} cancelling={cancelling} onCancel={onCancel} />}
      <ImageOutput projectId={projectId} task={task} onOpenTask={onOpenTask} />
    </aside>
  );
}
