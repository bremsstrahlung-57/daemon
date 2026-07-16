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

export const onMessageReady = (
  handler: (payload: MessageReadyPayload) => void,
): Promise<UnlistenFn> =>
  listen<MessageReadyPayload>("daemon://message-ready", (event) => {
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
