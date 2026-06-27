import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { GlossaryTerm } from './GlossaryTerm';

describe('GlossaryTerm', () => {
  it('renders children with title from glossary', () => {
    render(<GlossaryTerm term="artifact">架阁</GlossaryTerm>);
    const el = screen.getByText('架阁');
    expect(el.getAttribute('title')).toContain('文档');
  });
});
