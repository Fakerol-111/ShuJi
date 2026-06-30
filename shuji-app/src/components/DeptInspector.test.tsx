import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import DeptInspector from './DeptInspector';
import type { DeptStepEntry } from '../types';

const thinkingStep: DeptStepEntry = {
  dept: '工部尚书',
  ts: '2026-06-30T15:00:00',
  kind: { type: 'thinking', content: 'thinking through the implementation' },
};

vi.mock('../hooks/useDeptEvents', () => ({
  useDeptEvents: () => ({
    deptSteps: new Map([['工部尚书', [thinkingStep]]]),
  }),
}));

describe('DeptInspector', () => {
  it('can switch from summary to technical steps when a thinking step is present', async () => {
    const user = userEvent.setup();

    render(<DeptInspector dept="工部尚书" mode="single" entries={[]} active onBack={() => {}} />);

    await user.click(screen.getByRole('button', { name: /技术|technical/i }));

    expect(screen.getByText(/思考|thinking/i)).toBeTruthy();
  });
});
