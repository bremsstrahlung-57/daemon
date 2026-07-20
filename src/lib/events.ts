import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { JobRecord, MessageReadyPayload } from "./daemon";

export type ProposalCreatedPayload = {
  proposal_id: string;
};

export type ProposalResolvedPayload = {
  proposal_id: string;
  status: string;
};

export type JobLifecyclePayload = { job: JobRecord };
export type NoteCreatedPayload = { id: string; content: string };
export type MascotReactionPayload = { reaction: "happy" | "not_happy" };
export type ScreenAwareStatusPayload = {
  status: "capturing" | "ready" | "error" | "model-downloading" | "model-ready";
  message: string;
};

export const onMessageReady = (
  handler: (payload: MessageReadyPayload) => void,
): Promise<UnlistenFn> =>
  listen<MessageReadyPayload>("daemon://message-ready", (event) => {
    handler(event.payload);
  });

export const onNoteCreated = (
  handler: (payload: NoteCreatedPayload) => void,
): Promise<UnlistenFn> => listen<NoteCreatedPayload>("daemon://note-created", (event) => {
  handler(event.payload);
});

export const onMascotReaction = (
  handler: (payload: MascotReactionPayload) => void,
): Promise<UnlistenFn> => listen<MascotReactionPayload>("daemon://mascot-reaction", (event) => {
  handler(event.payload);
});

export const onProposalCreated = (
  handler: (payload: ProposalCreatedPayload) => void,
): Promise<UnlistenFn> =>
  listen<ProposalCreatedPayload>("daemon://proposal-created", (event) => {
    handler(event.payload);
  });

export const onProposalResolved = (
  handler: (payload: ProposalResolvedPayload) => void,
): Promise<UnlistenFn> =>
  listen<ProposalResolvedPayload>("daemon://proposal-resolved", (event) => {
    handler(event.payload);
  });

export const onJobStarted = (
  handler: (payload: JobLifecyclePayload) => void,
): Promise<UnlistenFn> => listen<JobLifecyclePayload>("daemon://job-started", (event) => {
  handler(event.payload);
});

export const onJobCompleted = (
  handler: (payload: JobLifecyclePayload) => void,
): Promise<UnlistenFn> => listen<JobLifecyclePayload>("daemon://job-completed", (event) => {
  handler(event.payload);
});

export const onJobFailed = (
  handler: (payload: JobLifecyclePayload) => void,
): Promise<UnlistenFn> => listen<JobLifecyclePayload>("daemon://job-failed", (event) => {
  handler(event.payload);
});

export const onScreenAwareStatus = (
  handler: (payload: ScreenAwareStatusPayload) => void,
): Promise<UnlistenFn> => listen<ScreenAwareStatusPayload>("daemon://screen-aware-status", (event) => {
  handler(event.payload);
});

export const onScreenResponseStarted = (
  handler: () => void,
): Promise<UnlistenFn> => listen<string>("daemon://screen-response-started", () => {
  handler();
});

export const onScreenResponseFailed = (
  handler: (message: string) => void,
): Promise<UnlistenFn> => listen<string>("daemon://screen-response-failed", (event) => {
  handler(event.payload);
});
