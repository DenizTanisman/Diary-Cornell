import { describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/react';
import { SaveIndicator } from '../../src/components/common/SaveIndicator';

describe('<SaveIndicator>', () => {
  it('renders saved state', () => {
    const { container } = render(<SaveIndicator isSaving={false} isDirty={false} />);
    expect(screen.getByText('Kaydedildi')).toBeInTheDocument();
    expect(container.querySelector('.save-indicator--saved')).not.toBeNull();
  });

  it('renders saving state', () => {
    const { container } = render(<SaveIndicator isSaving={true} isDirty={true} />);
    expect(screen.getByText('Kaydediliyor…')).toBeInTheDocument();
    expect(container.querySelector('.save-indicator--saving')).not.toBeNull();
  });

  it('renders dirty state', () => {
    const { container } = render(<SaveIndicator isSaving={false} isDirty={true} />);
    expect(screen.getByText('Kaydedilmedi')).toBeInTheDocument();
    expect(container.querySelector('.save-indicator--dirty')).not.toBeNull();
  });
});
