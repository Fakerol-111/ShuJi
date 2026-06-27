import { describe, expect, it, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import WorkflowTimeline from './WorkflowTimeline';
import type { TimelineNode } from '../types';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => key,
    i18n: { language: 'zh' },
  }),
}));

const nodes: TimelineNode[] = [
  {
    id: 's1',
    label: 'Design',
    status: 'done',
    docId: 'dsgn_1',
    kind: 'pipeline',
  },
  {
    id: 's2',
    label: 'Review',
    status: 'waiting',
    dept: '门下侍中',
    kind: 'pipeline',
  },
];

describe('WorkflowTimeline', () => {
  it('renders step nodes', () => {
    render(<WorkflowTimeline nodes={nodes} />);
    expect(screen.getByText('Design')).toBeInTheDocument();
    expect(screen.getByText('Review')).toBeInTheDocument();
  });

  it('calls onNodeClick with doc or dept', () => {
    const onNodeClick = vi.fn();
    render(<WorkflowTimeline nodes={nodes} onNodeClick={onNodeClick} />);
    fireEvent.click(screen.getByText('Design'));
    expect(onNodeClick).toHaveBeenCalledWith(expect.objectContaining({ id: 's1' }));
    fireEvent.click(screen.getByText('Review'));
    expect(onNodeClick).toHaveBeenCalledWith(expect.objectContaining({ id: 's2' }));
  });
});
