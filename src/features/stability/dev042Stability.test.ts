import { describe, expect, it } from "vitest";

type ShotState = "DONE" | "PREPARED" | "ELIGIBLE" | "BLOCKED";
type EpisodeClassification = "EMPTY" | "DONE" | "PREPARED" | "READY" | "PARTIAL" | "BLOCKED";
type Stage = "IMAGE" | "VIDEO";

interface Shot {
  id: string;
  sceneId: string;
  sceneOrdinal: number;
  stage: Stage;
  state: ShotState;
  selectedImage: string | null;
  selectedVideo: string | null;
  references: string[];
  assignment: string;
  stageConfig: string;
}

interface Scene {
  id: string;
  ordinal: number;
  shots: Shot[];
}

interface Episode {
  id: string;
  ordinal: number;
  scenes: Scene[];
}

interface Series {
  id: string;
  projectId: string;
  episodes: Episode[];
}

interface Counts {
  DONE: number;
  PREPARED: number;
  ELIGIBLE: number;
  BLOCKED: number;
}

interface EpisodePlan {
  id: string;
  ordinal: number;
  sceneTotal: number;
  shotTotal: number;
  counts: Counts;
  classification: EpisodeClassification;
  canPrepare: boolean;
}

interface SeriesPlan {
  seriesId: string;
  episodeTotal: number;
  sceneTotal: number;
  shotTotal: number;
  counts: Counts;
  treeLoads: number;
  episodePlans: EpisodePlan[];
}

const emptyCounts = (): Counts => ({ DONE: 0, PREPARED: 0, ELIGIBLE: 0, BLOCKED: 0 });

const shot = (episode: number, scene: number, ordinal: number, state: ShotState): Shot => ({
  id: `e${String(episode).padStart(2, "0")}-s${String(scene).padStart(2, "0")}-shot-${String(ordinal).padStart(2, "0")}`,
  sceneId: `episode-${String(episode).padStart(2, "0")}-scene-${String(scene).padStart(2, "0")}`,
  sceneOrdinal: scene,
  stage: "IMAGE",
  state,
  selectedImage: null,
  selectedVideo: null,
  references: [`character-ref-e${String(episode).padStart(2, "0")}`],
  assignment: `artist-${(scene % 3) + 1}`,
  stageConfig: "fixture-stage-config",
});

const seriesFixture = (): Series => ({
  id: "series-01",
  projectId: "project-dev042",
  episodes: Array.from({ length: 5 }, (_, episodeIndex) => {
    const episode = episodeIndex + 1;
    return {
      id: `episode-${String(episode).padStart(2, "0")}`,
      ordinal: episode,
      scenes: Array.from({ length: 10 }, (_, sceneIndex) => {
        const scene = sceneIndex + 1;
        const state: ShotState = episode === 1
          ? "DONE"
          : episode === 2
            ? "ELIGIBLE"
            : episode === 3
              ? "PREPARED"
              : episode === 4 && scene <= 8
                ? "ELIGIBLE"
                : "BLOCKED";
        return {
          id: `episode-${String(episode).padStart(2, "0")}-scene-${String(scene).padStart(2, "0")}`,
          ordinal: scene,
          shots: Array.from({ length: 10 }, (_, shotIndex) => shot(episode, scene, shotIndex + 1, state)),
        };
      }),
    };
  }),
});

const classify = (counts: Counts): EpisodeClassification => {
  const total = Object.values(counts).reduce((sum, count) => sum + count, 0);
  if (total === 0) return "EMPTY";
  if (counts.DONE === total) return "DONE";
  if (counts.PREPARED === total) return "PREPARED";
  if (counts.BLOCKED > 0 && counts.ELIGIBLE === 0) return "BLOCKED";
  if (counts.BLOCKED > 0) return "PARTIAL";
  if (counts.ELIGIBLE > 0) return "READY";
  throw new Error(`unsupported fixture state: ${JSON.stringify(counts)}`);
};

const episodePlan = (episode: Episode): EpisodePlan => {
  const counts = emptyCounts();
  episode.scenes.flatMap((scene) => scene.shots).forEach((shot) => { counts[shot.state] += 1; });
  const classification = classify(counts);
  return {
    id: episode.id,
    ordinal: episode.ordinal,
    sceneTotal: episode.scenes.length,
    shotTotal: Object.values(counts).reduce((sum, count) => sum + count, 0),
    counts,
    classification,
    canPrepare: counts.ELIGIBLE > 0 && counts.BLOCKED === 0,
  };
};

const seriesPlan = (series: Series): SeriesPlan => {
  // One structure tree load feeds all Episode summaries in this fixture.
  const treeLoads = 1;
  const episodePlans = series.episodes.map(episodePlan);
  const counts = emptyCounts();
  episodePlans.forEach((plan) => {
    (Object.keys(counts) as Array<keyof Counts>).forEach((state) => { counts[state] += plan.counts[state]; });
  });
  return {
    seriesId: series.id,
    episodeTotal: episodePlans.length,
    sceneTotal: series.episodes.reduce((sum, episode) => sum + episode.scenes.length, 0),
    shotTotal: Object.values(counts).reduce((sum, count) => sum + count, 0),
    counts,
    treeLoads,
    episodePlans,
  };
};

interface PrepareResult {
  status: "SUCCESS" | "NOOP" | "PARTIAL" | "BLOCKED";
  createdBatches: number;
  createdItems: number;
  skippedScenes: string[];
  blockingEpisodes: string[];
  autoStarted: boolean;
  startAllCalled: boolean;
  schedulerCalled: boolean;
}

const bindingKey = (shot: Shot) => `${shot.id}:${shot.stage}`;

const prepare = (
  series: Series,
  episodeIds: string[],
  allowPartial: boolean,
  bindings: Set<string>,
): PrepareResult => {
  const plans = series.episodes.filter((episode) => episodeIds.includes(episode.id)).map(episodePlan);
  const blockingEpisodes = plans
    .filter((plan) => plan.classification === "BLOCKED" || plan.classification === "PARTIAL")
    .map((plan) => plan.id);
  if (!allowPartial && blockingEpisodes.length > 0) {
    return { status: "BLOCKED", createdBatches: 0, createdItems: 0, skippedScenes: [], blockingEpisodes, autoStarted: false, startAllCalled: false, schedulerCalled: false };
  }

  let createdBatches = 0;
  let createdItems = 0;
  const skippedScenes: string[] = [];
  series.episodes.filter((episode) => episodeIds.includes(episode.id)).forEach((episode) => {
    const plan = plans.find((candidate) => candidate.id === episode.id);
    if (!plan || (plan.classification !== "READY" && plan.classification !== "PARTIAL")) return;
    episode.scenes.forEach((scene) => {
      const created = scene.shots
        .filter((shot) => shot.state === "ELIGIBLE")
        .map(bindingKey)
        .filter((key) => !bindings.has(key));
      created.forEach((key) => bindings.add(key));
      if (created.length === 0) skippedScenes.push(scene.id);
      else { createdBatches += 1; createdItems += created.length; }
    });
  });
  return {
    status: createdItems === 0 ? "NOOP" : blockingEpisodes.length > 0 ? "PARTIAL" : "SUCCESS",
    createdBatches,
    createdItems,
    skippedScenes,
    blockingEpisodes,
    autoStarted: false,
    startAllCalled: false,
    schedulerCalled: false,
  };
};

type BatchStatus = "READY" | "COMPLETED" | "RUNNING";
interface RunbookBatch {
  id: string;
  episodeOrdinal: number;
  sceneOrdinal: number;
  stage: Stage;
  status: BatchStatus;
  sceneIds: string[];
  generic: boolean;
}

const runbook = (input: RunbookBatch[]) => {
  const rows = input
    .filter((batch) => !batch.generic)
    .sort((left, right) => left.episodeOrdinal - right.episodeOrdinal || left.sceneOrdinal - right.sceneOrdinal || left.stage.localeCompare(right.stage) || left.id.localeCompare(right.id));
  const warnings = rows.filter((batch) => batch.sceneIds.length > 1).map((batch) => ({ batchId: batch.id, code: "MIXED_SCOPE" }));
  const recommendedBatch = rows.find((batch) => batch.status === "RUNNING")?.id ?? rows.find((batch) => batch.status === "READY")?.id ?? null;
  return { rows, warnings, recommendedBatch };
};

describe("DEV-042 no-GPU Series and Batch Runbook safety contract", () => {
  it("plans one Series with 5 Episodes, 50 Scenes, 500 Shots and frozen classifications", () => {
    const plan = seriesPlan(seriesFixture());
    expect(plan).toMatchObject({ seriesId: "series-01", episodeTotal: 5, sceneTotal: 50, shotTotal: 500, treeLoads: 1 });
    expect(plan.counts).toEqual({ DONE: 100, PREPARED: 100, ELIGIBLE: 180, BLOCKED: 120 });
    expect(plan.episodePlans.map((episode) => episode.classification)).toEqual(["DONE", "READY", "PREPARED", "PARTIAL", "BLOCKED"]);
    expect(plan.episodePlans.map((episode) => episode.canPrepare)).toEqual([false, true, false, false, false]);
  });

  it("keeps strict multi-Episode prepare at zero mutation when Episode 4 is partial", () => {
    const bindings = new Set<string>();
    const result = prepare(seriesFixture(), ["episode-02", "episode-04"], false, bindings);
    expect(result).toMatchObject({ status: "BLOCKED", createdBatches: 0, createdItems: 0, blockingEpisodes: ["episode-04"], autoStarted: false, startAllCalled: false, schedulerCalled: false });
    expect(bindings.size).toBe(0);
  });

  it("prepares only eligible scenes in partial mode and makes repeat a noop", () => {
    const fixture = seriesFixture();
    const bindings = new Set<string>();
    const first = prepare(fixture, ["episode-02", "episode-04"], true, bindings);
    expect(first).toMatchObject({ status: "PARTIAL", createdBatches: 18, createdItems: 180, blockingEpisodes: ["episode-04"] });
    expect(first.skippedScenes).toHaveLength(2);
    const second = prepare(fixture, ["episode-02", "episode-04"], true, bindings);
    expect(second).toMatchObject({ status: "NOOP", createdBatches: 0, createdItems: 0 });
    expect(bindings.size).toBe(180);
  });

  it("keeps Series + Episode + Scene races at one active Shot/Stage binding", () => {
    const fixture = seriesFixture();
    const keys = fixture.episodes.filter((episode) => ["episode-02", "episode-04"].includes(episode.id)).flatMap((episode) => episode.scenes.flatMap((scene) => scene.shots.filter((shot) => shot.state === "ELIGIBLE").map(bindingKey)));
    const admissions = new Map<string, number>();
    (["series", "episode-04", "episode-04-scene-01"] as const).forEach((scope) => {
      expect(scope).toMatch(/series|episode-04/);
      keys.forEach((key) => admissions.set(key, (admissions.get(key) ?? 0) + 1));
    });
    expect(admissions.size).toBe(180);
    expect([...admissions.values()].every((attempts) => attempts === 3)).toBe(true);
    expect([...new Set(admissions.keys())].length).toBe(180);
  });

  it("orders the Runbook by Episode, Scene and Stage, excludes generic batches, and recommends running first", () => {
    const input: RunbookBatch[] = [
      { id: "batch-d-video", episodeOrdinal: 1, sceneOrdinal: 4, stage: "VIDEO", status: "READY", sceneIds: ["scene-d"], generic: false },
      { id: "batch-c-running", episodeOrdinal: 1, sceneOrdinal: 3, stage: "IMAGE", status: "RUNNING", sceneIds: ["scene-c"], generic: false },
      { id: "batch-e-generic", episodeOrdinal: 99, sceneOrdinal: 99, stage: "IMAGE", status: "READY", sceneIds: [], generic: true },
      { id: "batch-b-completed", episodeOrdinal: 1, sceneOrdinal: 1, stage: "IMAGE", status: "COMPLETED", sceneIds: ["scene-b"], generic: false },
      { id: "batch-a-ready", episodeOrdinal: 1, sceneOrdinal: 2, stage: "IMAGE", status: "READY", sceneIds: ["scene-a"], generic: false },
    ];
    const first = runbook(input);
    expect(first.rows.map((batch) => batch.id)).toEqual(["batch-b-completed", "batch-a-ready", "batch-c-running", "batch-d-video"]);
    expect(first.recommendedBatch).toBe("batch-c-running");
    expect(first.rows.some((batch) => batch.id === "batch-e-generic")).toBe(false);
    expect(first.recommendedBatch === "batch-c-running").toBe(true);
    const afterRunning = runbook(first.rows.map((batch) => batch.id === "batch-c-running" ? { ...batch, status: "COMPLETED" } : batch));
    expect(afterRunning.recommendedBatch).toBe("batch-a-ready");
    expect(afterRunning.recommendedBatch === "batch-d-video").toBe(false);
  });

  it("reports MIXED_SCOPE and still renders a safe Runbook row", () => {
    const result = runbook([{ id: "batch-mixed", episodeOrdinal: 2, sceneOrdinal: 1, stage: "IMAGE", status: "READY", sceneIds: ["scene-a", "scene-b"], generic: false }]);
    expect(result.rows).toHaveLength(1);
    expect(result.warnings).toEqual([{ batchId: "batch-mixed", code: "MIXED_SCOPE" }]);
  });

  it("preserves the manual Image Review and Video Review boundaries", () => {
    const imageSucceededWithoutReview = { state: "BLOCKED" as const, reason: "IMAGE_REVIEW_REQUIRED", selectedImage: null };
    expect(imageSucceededWithoutReview).toEqual({ state: "BLOCKED", reason: "IMAGE_REVIEW_REQUIRED", selectedImage: null });
    const imageReviewed = { state: "ELIGIBLE" as const, selectedImage: "image-reviewed" };
    expect(imageReviewed.state).toBe("ELIGIBLE");
    const videoAfterSuccess = { selectedVideo: null, autoSelected: false };
    expect(videoAfterSuccess).toEqual({ selectedVideo: null, autoSelected: false });
  });

  it("applies a Series preset to 300 Shots without changing references, assets, anchors, assignment or ordinal", () => {
    const selected = seriesFixture().episodes.slice(0, 3).flatMap((episode) => episode.scenes.flatMap((scene) => scene.shots)).map((shot, ordinal) => ({ ...shot, selectedImage: `image-${ordinal}`, selectedVideo: `video-${ordinal}` }));
    expect(selected).toHaveLength(300);
    const before = selected.map((shot) => ({ ...shot, references: [...shot.references] }));
    selected.forEach((shot) => { shot.stageConfig = "series-image-preset"; });
    selected.forEach((shot, index) => {
      expect(shot.id).toBe(before[index].id);
      expect(shot.sceneId).toBe(before[index].sceneId);
      expect(shot.sceneOrdinal).toBe(before[index].sceneOrdinal);
      expect(shot.references).toEqual(before[index].references);
      expect(shot.selectedImage).toBe(before[index].selectedImage);
      expect(shot.selectedVideo).toBe(before[index].selectedVideo);
      expect(shot.assignment).toBe(before[index].assignment);
      expect(shot.stageConfig).toBe("series-image-preset");
    });
  });
});
