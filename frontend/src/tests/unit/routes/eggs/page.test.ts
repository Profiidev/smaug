import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/svelte';
import { goto } from '$app/navigation';
import { toast } from '@profidev/pleiades/components/util/general';
import ListPage from '$routes/eggs/+page.svelte';
import CreatePage from '$routes/eggs/create/+page.svelte';
import EditPage from '$routes/eggs/[uuid]/+page.svelte';
import { type EggImport, importEgg } from '$routes/eggs/schema.svelte';
import { adminUser, eggInfo, eggList } from '$mocks/e2e/data';
import { Permission } from '$lib/permissions.svelte';

const pageState = vi.hoisted(() => ({ state: {} as Record<string, unknown> }));

vi.mock('$app/state', () => ({
  page: {
    data: {},
    error: null,
    form: null,
    params: {},
    route: { id: null },
    get state() {
      return pageState.state;
    },
    status: 200,
    url: new URL('http://localhost/')
  }
}));

vi.mock('@profidev/pleiades/components/util/general', async (orig) => ({
  ...(await orig<object>()),
  toast: { error: vi.fn(), success: vi.fn() }
}));

vi.mock('$app/forms', async (orig) => ({
  ...(await orig<object>()),
  applyAction: vi.fn(async () => {})
}));

afterEach(() => {
  cleanup();
  pageState.state = {};
});

const readonlyUser = {
  ...adminUser,
  permissions: adminUser.permissions.filter((p) => p !== Permission.EGG_EDIT)
};

const ptdlFile = () => ({
  _comment: 'x',
  author: 'dev@example.com',
  config: { files: '{}', logs: '{}', startup: '{"done":"Done"}', stop: 'stop' },
  description: null,
  docker_images: { 'Java 17': 'ghcr.io/yolks:java_17' },
  features: [],
  file_denylist: [],
  meta: { update_url: null, version: 'PTDL_v2' },
  name: 'Legacy',
  scripts: {
    installation: { container: 'alpine', entrypoint: 'ash', script: 'echo' }
  },
  startup: 'java -jar server.jar',
  variables: []
});

const upload = async (raw: unknown, name = 'egg.json') =>
  fireEvent.change(screen.getByLabelText(/Import egg JSON/), {
    target: {
      files: [
        new File([typeof raw === 'string' ? raw : JSON.stringify(raw)], name)
      ]
    }
  });

const listData = (eggs: unknown[], user = adminUser) => ({
  eggs: Promise.resolve(eggs),
  error: null,
  user: Promise.resolve(user)
});

describe('eggs list page', () => {
  it('shows the header, subtitle and egg count', async () => {
    render(ListPage, {
      props: { data: listData(eggList.default.eggs) } as never
    });
    expect(screen.getByRole('heading', { name: 'Eggs' })).toBeInTheDocument();
    expect(
      screen.getByText(
        'Server templates that define how game servers are installed and run.'
      )
    ).toBeInTheDocument();
    expect(await screen.findByText('2 eggs')).toBeInTheDocument();
    expect(await screen.findByRole('link', { name: 'Paper' })).toHaveAttribute(
      'href',
      '/eggs/egg-paper'
    );
  });

  it('uses the singular for one egg', async () => {
    render(ListPage, {
      props: { data: listData(eggList.default.eggs.slice(0, 1)) } as never
    });
    expect(await screen.findByText('1 egg')).toBeInTheDocument();
  });

  it('shows the empty state with create and import, and no table', async () => {
    render(ListPage, { props: { data: listData([]) } as never });
    expect(await screen.findByText('No eggs yet')).toBeInTheDocument();
    expect(screen.getAllByRole('link', { name: 'Create' })).toHaveLength(1);
    expect(screen.getAllByRole('button', { name: 'Import JSON' })).toHaveLength(
      1
    );
    expect(screen.queryByText(/\d+ eggs?$/)).not.toBeInTheDocument();
    expect(screen.queryByRole('table')).not.toBeInTheDocument();
  });

  it('opens the create page with the imported egg in page state', async () => {
    render(ListPage, {
      props: { data: listData(eggList.default.eggs) } as never
    });
    await upload(ptdlFile());
    await vi.waitFor(() =>
      expect(goto).toHaveBeenCalledWith('/eggs/create', {
        state: { eggImport: importEgg(ptdlFile()) }
      })
    );
  });

  it.each([
    ['invalid JSON', '{nope', 'Invalid egg file'],
    [
      'unknown version',
      { meta: { version: 'X' } },
      'The file format is not recognized.'
    ]
  ])('toasts and stays on %s', async (_, raw, message) => {
    render(ListPage, {
      props: { data: listData(eggList.default.eggs) } as never
    });
    await upload(raw);
    await vi.waitFor(() => expect(toast.error).toHaveBeenCalledWith(message));
    expect(goto).not.toHaveBeenCalled();
  });

  it('disables import and create without egg:edit', async () => {
    render(ListPage, {
      props: { data: listData([], readonlyUser) } as never
    });
    await screen.findByText('No eggs yet');
    await vi.waitFor(() =>
      expect(screen.getByRole('button', { name: 'Import JSON' })).toBeDisabled()
    );
    expect(screen.getByLabelText(/Import egg JSON/)).toBeDisabled();
  });
});

describe('egg create page', () => {
  it('prefills the wizard and shows the import alert from page state', async () => {
    const eggImport: EggImport = importEgg(ptdlFile());
    pageState.state = { eggImport };
    render(CreatePage);
    expect(screen.getByText('Imported from PTDL_v2')).toBeInTheDocument();
    expect(
      screen.getByText(
        'Upgraded to PLCN_v3. Review every step before creating the egg.'
      )
    ).toBeInTheDocument();
    expect(screen.getByLabelText('Egg Name')).toHaveValue('Legacy');
  });

  it('shows no alert without an import', () => {
    render(CreatePage);
    expect(screen.queryByText(/Imported from/)).not.toBeInTheDocument();
    expect(screen.getByLabelText('Egg Name')).toHaveValue('');
    expect(
      screen.getByText('Basic details, icon and feature flags.')
    ).toBeInTheDocument();
  });

  it('imports on the create page and reports the source version', async () => {
    render(CreatePage);
    await upload(
      { ...ptdlFile(), meta: { update_url: null, version: 'PLCN_v1' } },
      'old.json'
    );
    expect(
      await screen.findByText('Imported from PLCN_v1')
    ).toBeInTheDocument();
    expect(toast.success).toHaveBeenCalledWith('Imported old.json');
    await vi.waitFor(() =>
      expect(screen.getByLabelText('Egg Name')).toHaveValue('Legacy')
    );
  });
});

describe('egg edit page save bar', () => {
  const editData = (user = adminUser) => ({
    eggRes: Promise.resolve({
      data: structuredClone(eggInfo),
      response: new Response()
    }),
    user: Promise.resolve(user),
    uuid: 'egg-paper'
  });

  it('shows "Unsaved changes" only once the loaded form is edited', async () => {
    render(EditPage, { props: { data: editData() } as never });
    const name = screen.getByLabelText('Egg Name');
    await vi.waitFor(() => expect(name).toHaveValue('Paper'));
    expect(screen.queryByText('Unsaved changes')).not.toBeInTheDocument();
    await fireEvent.input(name, { target: { value: 'Paper 2' } });
    expect(await screen.findByText('Unsaved changes')).toBeInTheDocument();
    await fireEvent.input(name, { target: { value: 'Paper' } });
    await vi.waitFor(() =>
      expect(screen.queryByText('Unsaved changes')).not.toBeInTheDocument()
    );
  });

  it('keeps save disabled and hides the hint for read-only users', async () => {
    render(EditPage, { props: { data: editData(readonlyUser) } as never });
    await vi.waitFor(() =>
      expect(
        screen.getByRole('button', { name: 'Save Changes' })
      ).toBeDisabled()
    );
    expect(screen.queryByText('Unsaved changes')).not.toBeInTheDocument();
  });

  it('renders the section descriptions', async () => {
    render(EditPage, { props: { data: editData() } as never });
    for (const text of [
      'Basic details, icon and feature flags.',
      'Environment variables users can configure on a server.',
      'The container and script that install the server files.',
      'Docker images, startup commands and how the panel detects a running server.',
      'Files the panel edits before each start.'
    ]) {
      expect(screen.getByText(text)).toBeInTheDocument();
    }
  });
});
