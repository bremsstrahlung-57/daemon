import { invoke } from "@tauri-apps/api/core";

export type SubmitTurnRequest = {
  content: string;
  conversationId?: string;
};

export type NoteRecord = {
  id: string;
  content: string;
  source_message_id: string;
  created_at: number;
  deleted_at: number | null;
};

export type NoteReceipt = {
  note: NoteRecord;
  duplicate: boolean;
};

export type TurnResult = {
  conversation_id: string;
  user_message_id: string;
  assistant_message_id: string;
  assistant_text: string;
};

export type Provider = {
  id: string;
  name: string;
  base_url: string;
  model: string;
  is_active: boolean;
  created_at: number;
  updated_at: number;
  api_key_configured: boolean;
};

export type SaveProviderRequest = {
  id?: string;
  name: string;
  baseUrl: string;
  model: string;
  apiKey?: string;
  makeActive?: boolean;
};

export type MessageReadyPayload = {
  message_id: string;
  conversation_id: string;
  content: string;
};

export type ProposalRecord = {
  id: string;
  conversation_id: string;
  tool_name: string;
  arguments_json: string;
  arguments_hash: string;
  preview: string;
  approval_policy: string;
  status: string;
  provider_context_json: string | null;
  created_at: number;
  expires_at: number | null;
  resolved_at: number | null;
};

export type ProposalApproval = {
  proposal: ProposalRecord;
  duplicate: boolean;
};

export type JobRecord = {
  id: string;
  proposal_id: string;
  kind: string;
  status: string;
  workspace_path: string | null;
  started_at: number | null;
  completed_at: number | null;
  result_json: string | null;
  error_message: string | null;
};

export const submitConversationTurn = (request: SubmitTurnRequest) =>
  invoke<TurnResult>("submit_conversation_turn", {
    request: {
      content: request.content,
      conversation_id: request.conversationId,
    },
  });

export const listProviders = () => invoke<Provider[]>("list_providers");

export const saveProvider = (request: SaveProviderRequest) =>
  invoke<Provider>("save_provider", {
    request: {
      id: request.id,
      name: request.name,
      base_url: request.baseUrl,
      model: request.model,
      api_key: request.apiKey,
      make_active: request.makeActive ?? true,
    },
  });

export const selectProvider = (providerId: string) =>
  invoke<Provider>("select_provider", { request: { provider_id: providerId } });

export const deleteProviderKey = (providerId: string) =>
  invoke<void>("delete_provider_key", { request: { provider_id: providerId } });

export const deleteProvider = (providerId: string) =>
  invoke<boolean>("delete_provider", { request: { provider_id: providerId } });

export const showToolboxMenu = () => invoke<void>("show_toolbox_menu");

export const undoNote = (noteId: string) =>
  invoke<boolean>("undo_note", { request: { note_id: noteId } });

export const pendingProposals = () =>
  invoke<ProposalRecord[]>("pending_proposals");

export const approveProposal = (proposal: ProposalRecord) =>
  invoke<ProposalApproval>("approve_proposal", {
    request: {
      proposal_id: proposal.id,
      arguments_hash: proposal.arguments_hash,
    },
  });

export const denyProposal = (proposal: ProposalRecord) =>
  invoke<ProposalRecord>("deny_proposal", {
    request: {
      proposal_id: proposal.id,
      arguments_hash: proposal.arguments_hash,
    },
  });
