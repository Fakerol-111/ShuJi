import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import ApprovalBanner from './ApprovalBanner';
import type { ApprovalGateContext } from '../utils/approvalGate';

const context: ApprovalGateContext = {
  active: true,
  docId: 'revw_001',
  docType: 'revw',
  stepId: 'gate',
  stepLabel: '等待朱批',
  stepAction: 'approval_gate',
  nextStepLabel: '尚书令执行',
  planSummary: 'Test',
};

describe('ApprovalBanner', () => {
  it('renders nothing when inactive', () => {
    const { container } = render(
      <ApprovalBanner
        context={{ ...context, active: false, docId: null }}
        onView={() => {}}
        onApprove={async () => {}}
      />
    );
    expect(container.firstChild).toBeNull();
  });

  it('shows doc id and approve button', async () => {
    const onApprove = vi.fn().mockResolvedValue(undefined);
    const user = userEvent.setup();
    render(<ApprovalBanner context={context} onView={() => {}} onApprove={onApprove} />);
    expect(screen.getByText('流程等待朱批')).toBeTruthy();
    expect(screen.getByText('revw_001')).toBeTruthy();
    await user.click(screen.getByText('准奏'));
    expect(onApprove).toHaveBeenCalled();
  });
});
