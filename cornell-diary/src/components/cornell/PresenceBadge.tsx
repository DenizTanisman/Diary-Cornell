/**
 * Tiny indicator that surfaces "X is also editing this entry" while
 * the WS channel reports more than one peer. We deliberately keep it
 * unobtrusive — it sits in the date header next to the word count and
 * only renders when there's actually something to say.
 */
import { useT } from '../../locales';

interface Props {
  /** Subscribed peer ids from `crdt:presence`. Includes us. */
  peers: string[];
  /** The local peer id (so we can subtract ourselves when rendering). */
  localPeerId: string | null;
}

export function PresenceBadge({ peers, localPeerId }: Props) {
  const t = useT();
  const others = peers.filter((p) => p !== localPeerId);
  if (others.length === 0) return null;

  // We display the friendly label but pin the peer ids in the title
  // attribute so the user can hover-confirm who's editing.
  const label =
    others.length === 1
      ? t('crdt.presence.single', { peer: others[0] })
      : t('crdt.presence.many', { count: String(others.length) });

  return (
    <span
      className="presence-badge"
      role="status"
      title={others.join('\n')}
      data-testid="presence-badge"
      style={{
        display: 'inline-flex',
        alignItems: 'center',
        gap: '0.35rem',
        padding: '0.15rem 0.55rem',
        borderRadius: '999px',
        background: '#FFF1C8',
        color: '#5C4400',
        fontSize: '0.75rem',
        fontWeight: 500,
        marginLeft: '0.5rem',
      }}
    >
      <span
        aria-hidden="true"
        style={{
          width: 8,
          height: 8,
          borderRadius: '50%',
          background: '#E0A100',
          boxShadow: '0 0 6px rgba(224, 161, 0, 0.6)',
        }}
      />
      {label}
    </span>
  );
}
