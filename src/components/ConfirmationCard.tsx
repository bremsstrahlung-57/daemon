import type { ProposalRecord } from "../lib/daemon";

type ConfirmationCardProps = {
  proposal: ProposalRecord;
  busy: boolean;
  onApprove: () => void;
  onDeny: () => void;
};

function ConfirmationCard({
  proposal,
  busy,
  onApprove,
  onDeny,
}: ConfirmationCardProps) {
  return (
    <section className="confirmation-card" aria-label="Confirm proposed action">
      <p>{proposal.preview}</p>
      <dl>
        <div>
          <dt>Approval</dt>
          <dd>Every task</dd>
        </div>
        <div>
          <dt>Exposure</dt>
          <dd>Approved repository content only</dd>
        </div>
      </dl>
      <details>
        <summary>Exact arguments</summary>
        <pre>{proposal.arguments_json}</pre>
      </details>
      <div className="confirmation-actions">
        <button type="button" disabled={busy} onClick={onApprove}>
          Do it
        </button>
        <button type="button" disabled={busy} onClick={onDeny}>
          Not now
        </button>
      </div>
    </section>
  );
}

export default ConfirmationCard;
