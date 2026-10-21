import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/svelte';
import { goto } from '$app/navigation';
import EditPage from '$routes/eggs/[uuid]/[tab=eggtab]/+page.svelte';
import { page } from '$test_helpers/page-state.svelte';
import { adminUser, eggInfo } from '$mocks/e2e/data';

vi.mock('$app/state', async () => import('$test_helpers/page-state.svelte'));

afterEach(cleanup);

beforeEach(() => {
  page.params = { tab: 'general', uuid: 'egg-paper' };
  // Navigating to a tab URL updates the route param like SvelteKit does.
  vi.mocked(goto).mockImplementation(async (url) => {
    const [uuid, tab] = String(url).split('/').slice(2);
    if (tab) {
      page.params = { tab, uuid };
    }
  });
});

const tick = async (ms = 0) => new Promise((r) => setTimeout(r, ms));

const mount = async () => {
  render(EditPage, {
    props: {
      data: {
        eggRes: Promise.resolve({
          data: structuredClone(eggInfo),
          response: new Response()
        }),
        user: Promise.resolve(adminUser),
        uuid: 'egg-paper'
      }
    } as never
  });
  await screen.findByRole('heading', { name: 'Egg: Paper' });
  await tick();
};

const tab = (name: string) =>
  screen.getByRole('tab', { name: new RegExp(`^${name}`) });

const openTab = async (name: string) => {
  await fireEvent.mouseDown(tab(name));
  await fireEvent.click(tab(name));
  await tick();
};

const nameInput = () => screen.getByLabelText('Egg Name');

describe('egg edit page tabs', () => {
  it.each([
    ['general', 'General'],
    ['variables', 'Variables'],
    ['install', 'Install script'],
    ['runtime', 'Runtime'],
    ['config', 'Config']
  ])('opens the /%s param as %s', async (param, name) => {
    page.params = { tab: param, uuid: 'egg-paper' };
    await mount();
    expect(tab(name)).toHaveAttribute('data-state', 'active');
    expect(
      screen.getAllByRole('tab').filter((t) => t.dataset.state === 'active')
    ).toHaveLength(1);
  });

  it('defaults to General without a tab param', async () => {
    page.params = { uuid: 'egg-paper' };
    await mount();
    expect(tab('General')).toHaveAttribute('data-state', 'active');
  });

  it('navigates to the tab URL keeping focus and scroll', async () => {
    await mount();
    await openTab('Install script');
    expect(goto).toHaveBeenCalledExactlyOnceWith('/eggs/egg-paper/install', {
      keepFocus: true,
      noScroll: true
    });
    expect(tab('Install script')).toHaveAttribute('data-state', 'active');
  });

  it('does not navigate when clicking the open tab', async () => {
    await mount();
    await openTab('General');
    expect(goto).not.toHaveBeenCalled();
  });

  it('keeps unsaved edits when switching tabs', async () => {
    await mount();
    await vi.waitFor(() => expect(nameInput()).toHaveValue('Paper'));
    await fireEvent.input(nameInput(), { target: { value: 'Draft' } });
    expect(await screen.findByText('Unsaved changes')).toBeInTheDocument();
    await openTab('Variables');
    await openTab('Config');
    await openTab('General');
    expect(nameInput()).toHaveValue('Draft');
    expect(screen.getByText('Unsaved changes')).toBeInTheDocument();
  });
});
