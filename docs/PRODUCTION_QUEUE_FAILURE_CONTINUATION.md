# Production Queue Failure Continuation

## Default behavior

New production batches continue to the next independent item when an item
reaches a known terminal failure or cancellation. The failed item remains
`FAILED` (or `CANCELLED`) in the batch and can be retried later as a new queue
attempt; the original task and error evidence are preserved.

When all items are terminal, a batch is `COMPLETED` even when its failure count
is non-zero. A completed batch with failures is therefore not a sequential
execution blocker.

## Safety pauses

The existing queue remains the only execution authority. A batch still pauses
when continuing could create an unknown or unsafe ComfyUI state:

- `COMFY_OFFLINE`
- `COMFY_STREAM_DISCONNECTED`
- `SUBMISSION_STATE_UNCERTAIN`
- `QUEUE_DISPATCH_UNCERTAIN`
- `EXECUTION_ADMISSION_UNAVAILABLE`

These states require explicit operator handling before later work is started.
An ordinary per-item `COMFY_TIMEOUT`, including an input upload timeout, is a
known terminal failure and does not by itself block independent items.

## Compatibility

The persisted `continue_on_failure` flag remains supported. New UI and service
entry points default it to enabled; existing batches that were explicitly
created with it disabled retain strict behavior. No queue, task, generation,
result, or database model was added.

## Sequential UI behavior

Explicitly armed batches advance after a prior batch reaches `COMPLETED` with
all items terminal, regardless of that batch's success/failure mix. A genuinely
`PAUSED` batch continues to require the existing explicit resume/handling step.
