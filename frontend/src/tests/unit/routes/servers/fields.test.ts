import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  cleanup,
  fireEvent,
  render,
  screen,
  within
} from '@testing-library/svelte';
import Harness from '$test_helpers/ServerFieldsHarness.svelte';
import GeneralFields from '$routes/servers/GeneralFields.svelte';
import StartupFields from '$routes/servers/StartupFields.svelte';
import VariablesFields from '$routes/servers/VariablesFields.svelte';
import ResourcesFields from '$routes/servers/ResourcesFields.svelte';
import PermissionsFields from '$routes/servers/PermissionsFields.svelte';
import {
  type ServerForm,
  eggVariables,
  fromServerInfo
} from '$routes/servers/schema.svelte';
import { options, serverInfo } from './fixtures';

// SvelteKit's real applyAction needs a mounted app root; superforms calls it on invalid submits.
vi.mock('$app/forms', async (orig) => ({
  ...(await orig<object>()),
  applyAction: vi.fn(async () => {})
}));

afterEach(cleanup);

const tick = async () => new Promise((r) => setTimeout(r, 0));
const base = (): ServerForm => fromServerInfo(structuredClone(serverInfo));
const [paper] = options.eggs;

const mount = (
  fields: unknown,
  {
    disabled = false,
    errors,
    extra = {},
    initialValue = base(),
    onsubmit = vi.fn()
  }: {
    disabled?: boolean;
    errors?: Record<string, string>;
    extra?: Record<string, unknown>;
    initialValue?: ServerForm;
    onsubmit?: (v: ServerForm) => void;
  } = {}
) => {
  const r = render(Harness, {
    props: {
      disabled,
      errors,
      extra,
      fields: fields as never,
      initialValue,
      onsubmit
    }
  });
  return { ...r, value: (): ServerForm => r.component.getValue()! };
};

// Error rendering after submit goes through SvelteKit's page store (not
// Available here; e2e covers it), so invalid data is asserted as not submitted.
const expectRejected = async (
  fields: unknown,
  initialValue: ServerForm,
  extra: Record<string, unknown> = {}
) => {
  const onsubmit = vi.fn();
  mount(fields, { extra, initialValue, onsubmit });
  await fireEvent.submit(
    screen.getByRole('button', { name: 'submit' }).closest('form')!
  );
  await new Promise((r) => setTimeout(r, 200));
  expect(onsubmit).not.toHaveBeenCalled();
};

describe('valid form', () => {
  it('submits the converted server info unchanged', async () => {
    const onsubmit = vi.fn();
    const init = base();
    mount(VariablesFields, { initialValue: init, onsubmit });
    await fireEvent.submit(
      screen.getByRole('button', { name: 'submit' }).closest('form')!
    );
    await vi.waitFor(() => expect(onsubmit).toHaveBeenCalledWith(init));
  });
});

/** Opens a FormSelect (shows `current` as its text) and picks `option`. */
const pick = async (current: string, option: string) => {
  const trigger = screen
    .getAllByRole('combobox')
    .find((b) => b.textContent?.trim() === current)!;
  await fireEvent.click(trigger);
  await fireEvent.click(await screen.findByRole('option', { name: option }));
  await tick();
};

describe('GeneralFields', () => {
  const general = (extra: Record<string, unknown> = {}, init = base()) =>
    mount(GeneralFields, {
      extra: { mode: 'create', options, ...extra },
      initialValue: init
    });

  it('renders the values and select labels', () => {
    general();
    expect(screen.getByLabelText('Name')).toHaveValue('survival');
    expect(
      screen.getAllByRole('combobox').map((b) => b.textContent?.trim())
    ).toEqual(['Ada Admin', 'eu-1', 'Paper']);
    expect(screen.getAllByLabelText('IP address')[0]).toHaveValue('10.0.0.5');
    expect(screen.getAllByLabelText('Port')[1]).toHaveValue(25_566);
    expect(screen.getByText('Primary')).toBeInTheDocument();
    expect(screen.getByText('Extra')).toBeInTheDocument();
    expect(
      screen.getByText(
        'Where the server runs. The IPs you type must belong to this node.'
      )
    ).toBeInTheDocument();
  });

  it('binds name and allocation inputs', async () => {
    const { value } = general();
    await fireEvent.input(screen.getByLabelText('Name'), {
      target: { value: 'new' }
    });
    await fireEvent.input(screen.getAllByLabelText('IP address')[1], {
      target: { value: '::1' }
    });
    await fireEvent.input(screen.getAllByLabelText('Port')[1], {
      target: { value: '30000' }
    });
    expect(value().name).toBe('new');
    expect(value().allocations[1]).toEqual({ ip: '::1', port: 30_000 });
  });

  it('adds a row after the last one and removes the chosen row', async () => {
    const { value } = general();
    await fireEvent.click(
      screen.getByRole('button', { name: 'Add allocation' })
    );
    expect(value().allocations[2]).toEqual({ ip: '10.0.0.5', port: 25_567 });
    expect(screen.getAllByText('Extra')).toHaveLength(2);
    await fireEvent.click(
      screen.getAllByRole('button', { name: 'Remove allocation' })[0]
    );
    expect(value().allocations.map((a) => a.port)).toEqual([25_566, 25_567]);
  });

  it('adds a default row when the previous port is empty', async () => {
    const init = base();
    init.allocations = [{ ip: '', port: null as never }];
    const { value } = general({}, init);
    await fireEvent.click(
      screen.getByRole('button', { name: 'Add allocation' })
    );
    expect(value().allocations[1]).toEqual({ ip: '', port: 25_565 });
  });

  it('disables remove on the last row', async () => {
    const init = base();
    init.allocations = init.allocations.slice(0, 1);
    general({}, init);
    expect(
      screen.getByRole('button', { name: 'Remove allocation' })
    ).toBeDisabled();
  });

  it('picks owner and node', async () => {
    const { value } = general();
    await pick('Ada Admin', 'Bob User');
    await pick('eu-1', 'us-1');
    expect(value().owner).toEqual(['user-1']);
    expect(value().node).toEqual(['node-2']);
  });

  it('changes the egg at once when variables are untouched', async () => {
    const oneggchange = vi.fn();
    const { value } = general({ oneggchange, variablesEdited: false });
    await pick('Paper', 'Vanilla');
    expect(value().egg).toEqual(['egg-vanilla']);
    expect(oneggchange).toHaveBeenCalledWith(options.eggs[1]);
    expect(screen.queryByText('Change egg?')).not.toBeInTheDocument();
  });

  it('changes the egg at once when none was picked yet', async () => {
    const oneggchange = vi.fn();
    const init = base();
    init.egg = [];
    general({ oneggchange, variablesEdited: true }, init);
    await pick('No Egg', 'Paper');
    expect(oneggchange).toHaveBeenCalledWith(paper);
  });

  it('ignores picking the current egg again', async () => {
    const oneggchange = vi.fn();
    general({ oneggchange, variablesEdited: true });
    await pick('Paper', 'Paper');
    expect(oneggchange).not.toHaveBeenCalled();
    expect(screen.queryByText('Change egg?')).not.toBeInTheDocument();
  });

  it('asks before discarding edited variables and reverts on cancel', async () => {
    const oneggchange = vi.fn();
    const { value } = general({ oneggchange, variablesEdited: true });
    await pick('Paper', 'Vanilla');
    expect(await screen.findByText('Change egg?')).toBeInTheDocument();
    expect(value().egg).toEqual(['egg-paper']);
    await fireEvent.click(screen.getByRole('button', { name: 'Keep egg' }));
    await tick();
    expect(value().egg).toEqual(['egg-paper']);
    expect(oneggchange).not.toHaveBeenCalled();
  });

  it('switches the egg once confirmed', async () => {
    const oneggchange = vi.fn();
    const { value } = general({ oneggchange, variablesEdited: true });
    await pick('Paper', 'Vanilla');
    await fireEvent.click(
      await screen.findByRole('button', { name: 'Change egg' })
    );
    await tick();
    expect(value().egg).toEqual(['egg-vanilla']);
    expect(oneggchange).toHaveBeenCalledWith(options.eggs[1]);
  });

  it('locks the node and explains egg changes in edit mode', () => {
    general({ mode: 'edit' });
    const node = screen
      .getAllByRole('combobox')
      .find((b) => b.textContent?.trim() === 'eu-1');
    expect(node).toBeDisabled();
    expect(
      screen.getByText('Use Transfer on the Actions tab.')
    ).toBeInTheDocument();
    expect(screen.getByText(/A reinstall is recommended/)).toBeInTheDocument();
  });

  it('shows list-level and row server errors', async () => {
    mount(GeneralFields, {
      errors: {
        'allocations.1.ip': 'Allocation is already in use.',
        'allocations._errors': 'Too many allocations.'
      },
      extra: { mode: 'create', options }
    });
    expect(
      await screen.findByText('Too many allocations.')
    ).toBeInTheDocument();
    expect(
      await screen.findByText('Allocation is already in use.')
    ).toBeInTheDocument();
  });

  it('does not submit an empty name or duplicate allocations', async () => {
    const init = base();
    init.allocations[1] = { ...init.allocations[0] };
    await expectRejected(GeneralFields, init, { mode: 'create', options });
    cleanup();
    await expectRejected(
      GeneralFields,
      { ...base(), name: '' },
      {
        mode: 'create',
        options
      }
    );
  });

  it('disables every control when disabled', () => {
    mount(GeneralFields, {
      disabled: true,
      extra: { mode: 'create', options }
    });
    expect(screen.getByLabelText('Name')).toBeDisabled();
    expect(
      screen.getByRole('button', { name: 'Add allocation' })
    ).toBeDisabled();
    for (const b of screen.getAllByRole('combobox')) {
      expect(b).toBeDisabled();
    }
  });

  it('locks only the admin fields when locked', () => {
    general({ locked: true, mode: 'edit' });
    expect(screen.getByLabelText('Name')).not.toBeDisabled();
    for (const b of screen.getAllByRole('combobox')) {
      expect(b).toBeDisabled();
    }
    for (const i of [
      ...screen.getAllByLabelText('IP address'),
      ...screen.getAllByLabelText('Port')
    ]) {
      expect(i).toBeDisabled();
    }
    expect(
      screen.getByRole('button', { name: 'Add allocation' })
    ).toBeDisabled();
    for (const b of screen.getAllByRole('button', {
      name: 'Remove allocation'
    })) {
      expect(b).toBeDisabled();
    }
  });
});

describe('StartupFields', () => {
  const startup = (
    over: Partial<ServerForm> = {},
    extra: Record<string, unknown> = {}
  ) =>
    mount(StartupFields, {
      extra: { egg: paper, mode: 'create', ...extra },
      initialValue: { ...base(), ...over }
    });

  const commandSelect = () => screen.getByLabelText('Startup command');
  const imageSelect = () => screen.getByLabelText('Image name');
  const shown = (el: HTMLElement) => el.textContent?.trim();
  // The bits-ui Select opens on pointerdown, which jsdom can't drive; use the keyboard.
  const open = async (select: HTMLElement) =>
    fireEvent.keyDown(select, { key: 'ArrowDown' });
  const choose = async (select: HTMLElement, name: string) => {
    await open(select);
    // Items select on pointerup.
    await fireEvent.pointerUp(await screen.findByRole('option', { name }));
    await tick();
  };
  const listed = async (select: HTMLElement) => {
    await open(select);
    return (await screen.findAllByRole('option')).map((o) =>
      o.textContent?.trim()
    );
  };

  it('shows the install switches only in create mode', () => {
    startup();
    expect(
      screen.getByRole('switch', { name: 'Run install script' })
    ).toBeChecked();
    cleanup();
    startup({}, { mode: 'edit' });
    expect(
      screen.queryByRole('switch', { name: 'Run install script' })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole('switch', { name: 'Start after install' })
    ).not.toBeInTheDocument();
  });

  it('binds the install switches', async () => {
    const { value } = startup({ runInstall: true, startOnCompletion: true });
    await fireEvent.click(
      screen.getByRole('switch', { name: 'Run install script' })
    );
    await fireEvent.click(
      screen.getByRole('switch', { name: 'Start after install' })
    );
    expect(value().runInstall).toBe(false);
    expect(value().startOnCompletion).toBe(false);
  });

  it('selects the egg command matching the text and hides the badge', () => {
    startup({
      startup: 'java -jar {{SERVER_JARFILE}}',
      startupOverridden: false
    });
    expect(shown(commandSelect())).toBe('Default');
    expect(screen.queryByText('Overridden')).not.toBeInTheDocument();
    expect(
      screen.getByText('Follows the egg. Egg updates change it.')
    ).toBeInTheDocument();
  });

  it('lists the egg commands plus Custom', async () => {
    startup();
    expect(await listed(commandSelect())).toEqual([
      'Default',
      'Aikar',
      'Custom'
    ]);
  });

  it('switches to Custom with the Overridden badge when the text is edited', async () => {
    const { value } = startup({
      startup: 'java -jar {{SERVER_JARFILE}}',
      startupOverridden: false
    });
    await fireEvent.input(screen.getByLabelText('Command'), {
      target: { value: 'java -jar mine.jar' }
    });
    expect(value().startupOverridden).toBe(true);
    expect(shown(commandSelect())).toBe('Custom');
    expect(screen.getByText('Overridden')).toBeInTheDocument();
    expect(
      screen.getByText(
        'This server uses its own command. Egg updates will not change it.'
      )
    ).toBeInTheDocument();
  });

  it('resets to the egg default', async () => {
    const { value } = startup();
    await fireEvent.click(
      screen.getByRole('button', { name: 'Reset to default' })
    );
    expect(value().startup).toBe('java -jar {{SERVER_JARFILE}}');
    expect(value().startupOverridden).toBe(false);
    expect(screen.queryByText('Overridden')).not.toBeInTheDocument();
  });

  it('marks a non-default egg command as overridden and the default as not', async () => {
    const { value } = startup({
      startup: 'java -jar {{SERVER_JARFILE}}',
      startupOverridden: false
    });
    await choose(commandSelect(), 'Aikar');
    expect(value().startup).toBe('java -XX:+UseG1GC -jar {{SERVER_JARFILE}}');
    expect(value().startupOverridden).toBe(true);
    await choose(commandSelect(), 'Default');
    expect(value().startupOverridden).toBe(false);
  });

  it('keeps the text and marks it overridden when Custom is picked', async () => {
    const { value } = startup({
      startup: 'java -jar {{SERVER_JARFILE}}',
      startupOverridden: false
    });
    await choose(commandSelect(), 'Custom');
    expect(value().startup).toBe('java -jar {{SERVER_JARFILE}}');
    expect(value().startupOverridden).toBe(true);
    expect(shown(commandSelect())).toBe('Custom');
    expect(screen.getByText('Overridden')).toBeInTheDocument();
    expect(screen.getByLabelText('Command')).not.toBeDisabled();
    await fireEvent.input(screen.getByLabelText('Command'), {
      target: { value: 'java -jar mine.jar' }
    });
    expect(value().startup).toBe('java -jar mine.jar');
    expect(shown(commandSelect())).toBe('Custom');
    await fireEvent.click(
      screen.getByRole('button', { name: 'Reset to default' })
    );
    expect(shown(commandSelect())).toBe('Default');
    expect(screen.queryByText('Overridden')).not.toBeInTheDocument();
  });

  it('switches to Custom image with the hint when Custom image is picked', async () => {
    const { value } = startup();
    await choose(imageSelect(), 'Custom image');
    expect(shown(imageSelect())).toBe('Custom image');
    expect(value().image).toBe('ghcr.io/yolks:java_21');
    expect(
      screen.getByText('Custom image. Must be pullable by the node.')
    ).toBeInTheDocument();
    await choose(imageSelect(), 'Java 21');
    expect(shown(imageSelect())).toBe('Java 21');
    expect(screen.queryByText(/Must be pullable/)).not.toBeInTheDocument();
  });

  it('picks an egg image and switches to Custom image when typed', async () => {
    const { value } = startup();
    expect(shown(imageSelect())).toBe('Java 21');
    expect(screen.queryByText(/Must be pullable/)).not.toBeInTheDocument();
    await choose(imageSelect(), 'Java 17');
    expect(value().image).toBe('ghcr.io/yolks:java_17');
    await fireEvent.input(screen.getByLabelText('Image'), {
      target: { value: 'my/image:1' }
    });
    expect(shown(imageSelect())).toBe('Custom image');
    expect(
      screen.getByText('Custom image. Must be pullable by the node.')
    ).toBeInTheDocument();
    await choose(imageSelect(), 'Custom image');
    expect(value().image).toBe('my/image:1');
  });

  it('offers only Custom without an egg and cannot reset', async () => {
    startup({}, { egg: undefined });
    expect(await listed(commandSelect())).toEqual(['Custom']);
    expect(
      screen.getByRole('button', { name: 'Reset to default' })
    ).toBeDisabled();
  });

  it('does not submit without a command or image', async () => {
    const extra = { egg: paper, mode: 'create' };
    await expectRejected(StartupFields, { ...base(), image: '' }, extra);
    cleanup();
    await expectRejected(StartupFields, { ...base(), startup: '' }, extra);
  });

  it('disables every control when disabled', () => {
    mount(StartupFields, {
      disabled: true,
      extra: { egg: paper, mode: 'create' }
    });
    expect(commandSelect()).toBeDisabled();
    expect(imageSelect()).toBeDisabled();
    expect(screen.getByLabelText('Command')).toBeDisabled();
    expect(
      screen.getByRole('switch', { name: 'Run install script' })
    ).toBeDisabled();
    expect(
      screen.getByRole('button', { name: 'Reset to default' })
    ).toBeDisabled();
  });

  it('locks only the image when imageDisabled', () => {
    startup({}, { imageDisabled: true, mode: 'edit' });
    expect(imageSelect()).toBeDisabled();
    expect(screen.getByLabelText('Image')).toBeDisabled();
    expect(commandSelect()).not.toBeDisabled();
    expect(screen.getByLabelText('Command')).not.toBeDisabled();
  });
});

describe('VariablesFields', () => {
  it('renders a card per variable with env badge, description and rules', () => {
    mount(VariablesFields);
    expect(screen.getByText('SERVER_JARFILE')).toBeInTheDocument();
    expect(screen.getByText('Jar to run')).toBeInTheDocument();
    expect(screen.getByText('required|string|max:20')).toBeInTheDocument();
    expect(screen.getByLabelText('Server Jar File')).toHaveValue('paper.jar');
  });

  it('binds values and locks read-only variables', async () => {
    const { value } = mount(VariablesFields);
    await fireEvent.input(screen.getByLabelText('Server Jar File'), {
      target: { value: 'other.jar' }
    });
    expect(value().variables[0].value).toBe('other.jar');
    expect(screen.getByLabelText('Max players')).toBeDisabled();
  });

  it('does not submit a value breaking the egg rules', async () => {
    const init = base();
    init.variables = eggVariables(paper);
    init.variables[1].value = 'abc';
    await expectRejected(VariablesFields, init);
  });

  it('shows a server error for a variable', async () => {
    mount(VariablesFields, {
      errors: { 'variables.0.value': 'Bad jar.' }
    });
    expect(await screen.findByText('Bad jar.')).toBeInTheDocument();
  });

  it('shows an empty state without variables', () => {
    mount(VariablesFields, { initialValue: { ...base(), variables: [] } });
    expect(screen.getByText('No variables')).toBeInTheDocument();
  });

  it('disables every input when disabled', () => {
    mount(VariablesFields, { disabled: true });
    expect(screen.getByLabelText('Server Jar File')).toBeDisabled();
  });
});

describe('ResourcesFields', () => {
  it('shows inputs only for limited resources', () => {
    mount(ResourcesFields);
    expect(screen.getByLabelText('CPU limit (millicores)')).toHaveValue(2000);
    expect(screen.getByLabelText('Memory limit (MiB)')).toHaveValue(4096);
    expect(
      screen.queryByLabelText('Storage limit (MiB)')
    ).not.toBeInTheDocument();
    expect(
      screen.getByRole('switch', { name: 'Unlimited Storage' })
    ).toBeChecked();
    expect(screen.getByText('No storage limit.')).toBeInTheDocument();
  });

  it.each([
    ['CPU limit (millicores)', 'Unlimited CPU'],
    ['Memory limit (MiB)', 'Unlimited Memory']
  ])('renders %s inside its switch row', (input, toggle) => {
    mount(ResourcesFields);
    expect(
      screen.getByLabelText(input).closest('[data-slot="item"]')
    ).toContainElement(screen.getByRole('switch', { name: toggle }));
  });

  it('reveals the default when a switch is turned off and hides it again', async () => {
    const { value } = mount(ResourcesFields);
    await fireEvent.click(
      screen.getByRole('switch', { name: 'Unlimited Storage' })
    );
    expect(screen.getByLabelText('Storage limit (MiB)')).toHaveValue(10_240);
    expect(value().diskUnlimited).toBe(false);
    await fireEvent.click(
      screen.getByRole('switch', { name: 'Unlimited CPU' })
    );
    expect(
      screen.queryByLabelText('CPU limit (millicores)')
    ).not.toBeInTheDocument();
    expect(value().cpuUnlimited).toBe(true);
  });

  it('binds limits and the backup limit', async () => {
    const { value } = mount(ResourcesFields);
    await fireEvent.input(screen.getByLabelText('Memory limit (MiB)'), {
      target: { value: '2048' }
    });
    await fireEvent.input(screen.getByLabelText('Backup limit'), {
      target: { value: '5' }
    });
    expect(value().memory).toBe(2048);
    expect(value().backupLimit).toBe(5);
  });

  it('shows a server error on a hidden limit under its switch', async () => {
    mount(ResourcesFields, {
      errors: { disk: 'Storage too large.', memory: 'Memory too large.' }
    });
    expect(
      screen.queryByLabelText('Storage limit (MiB)')
    ).not.toBeInTheDocument();
    expect(await screen.findByText('Storage too large.')).toBeInTheDocument();
    expect(screen.getByText('Memory too large.')).toBeInTheDocument();
  });

  it('does not submit a negative revealed limit', async () => {
    await expectRejected(ResourcesFields, { ...base(), memory: -1 });
  });

  it('disables the switches when disabled', () => {
    mount(ResourcesFields, { disabled: true });
    for (const s of screen.getAllByRole('switch')) {
      expect(s).toBeDisabled();
    }
    expect(screen.getByLabelText('Backup limit')).toBeDisabled();
  });
});

describe('PermissionsFields', () => {
  const perms = (init = base(), disabled = false, opts = options) =>
    mount(PermissionsFields, {
      disabled,
      extra: { options: opts },
      initialValue: init
    });
  const section = (name: string) =>
    screen.getByRole('region', { name: `${name} grants` });
  const select = (group: string) =>
    within(screen.getByRole('group', { name: group })).getByRole('combobox');
  const optionNames = async (group: string) => {
    await fireEvent.click(select(group));
    const names = (await screen.findAllByRole('option')).map((o) =>
      o.textContent?.trim()
    );
    await fireEvent.keyDown(document.activeElement!, { key: 'Escape' });
    await tick();
    return names;
  };
  const empty = (): ServerForm => ({
    ...base(),
    groupGrants: [],
    userGrants: []
  });

  it('groups grants into a users and a groups section', () => {
    perms();
    expect(
      within(section('User'))
        .getAllByRole('combobox')
        .map((b) => b.textContent?.trim())
    ).toEqual(['Bob User', 'Read']);
    expect(
      within(section('Group'))
        .getAllByRole('combobox')
        .map((b) => b.textContent?.trim())
    ).toEqual(['Admins', 'Write']);
    expect(select('User 1')).toHaveTextContent('Bob User');
    expect(select('Access for user 1')).toHaveTextContent('Read');
    expect(select('Group 1')).toHaveTextContent('Admins');
    expect(select('Access for group 1')).toHaveTextContent('Write');
  });

  it('shows the column labels once per section above the rows', () => {
    const init = base();
    init.userGrants.push({ access: ['read'], uuid: [] });
    perms(init);
    expect(
      within(section('User')).getAllByText('User', { exact: true })
    ).toHaveLength(1);
    expect(
      within(section('User')).getAllByText('Access', { exact: true })
    ).toHaveLength(1);
    expect(
      within(section('Group')).getAllByText('Group', { exact: true })
    ).toHaveLength(1);
    expect(
      within(section('Group')).getAllByText('Access', { exact: true })
    ).toHaveLength(1);
  });

  it('shows an empty state per section without column labels', () => {
    perms({ ...base(), userGrants: [] });
    expect(within(section('User')).getByText('No users')).toBeInTheDocument();
    expect(
      within(section('User')).queryByText('Access', { exact: true })
    ).toBeNull();
    expect(within(section('Group')).queryByText('No groups')).toBeNull();
    cleanup();
    perms({ ...base(), groupGrants: [] });
    expect(within(section('Group')).getByText('No groups')).toBeInTheDocument();
    expect(within(section('User')).queryByText('No users')).toBeNull();
  });

  it('adds a user row and picks a user and access for it', async () => {
    const { value } = perms(empty());
    await fireEvent.click(screen.getByRole('button', { name: 'Add user' }));
    expect(value().userGrants).toEqual([{ access: ['read'], uuid: [] }]);
    expect(value().groupGrants).toEqual([]);
    expect(select('User 1')).toHaveTextContent('No user');
    await pick('No user', 'Ada Admin');
    await pick('Read', 'Write');
    expect(value().userGrants).toEqual([
      { access: ['write'], uuid: ['user-admin'] }
    ]);
  });

  it('adds a group row', async () => {
    const { value } = perms(empty());
    await fireEvent.click(screen.getByRole('button', { name: 'Add group' }));
    expect(value().groupGrants).toEqual([{ access: ['read'], uuid: [] }]);
    expect(value().userGrants).toEqual([]);
    await pick('No group', 'Admins');
    expect(value().groupGrants).toEqual([
      { access: ['read'], uuid: ['group-admins'] }
    ]);
  });

  it('lists only users in user rows and only groups in group rows', async () => {
    perms(empty());
    await fireEvent.click(screen.getByRole('button', { name: 'Add user' }));
    await fireEvent.click(screen.getByRole('button', { name: 'Add group' }));
    expect(await optionNames('User 1')).toEqual(['Ada Admin', 'Bob User']);
    expect(await optionNames('Group 1')).toEqual(['Admins']);
  });

  it('hides entries picked in other rows but keeps the own value', async () => {
    const opts = {
      ...options,
      groups: [
        { name: 'Admins', uuid: 'group-admins' },
        { name: 'Staff', uuid: 'group-staff' },
        { name: 'Mods', uuid: 'group-mods' }
      ]
    };
    const init = base();
    init.groupGrants.push({ access: ['read'], uuid: ['group-staff'] });
    init.groupGrants.push({ access: ['read'], uuid: [] });
    perms(init, false, opts);
    expect(await optionNames('Group 1')).toEqual(['Admins', 'Mods']);
    expect(await optionNames('Group 2')).toEqual(['Staff', 'Mods']);
    expect(await optionNames('Group 3')).toEqual(['Mods']);
    // Users picked elsewhere do not affect groups, and vice versa.
    expect(await optionNames('User 1')).toEqual(['Ada Admin', 'Bob User']);
  });

  it('removes the chosen row of its own section', async () => {
    const init = base();
    init.userGrants.push({ access: ['write'], uuid: ['user-admin'] });
    const { value } = perms(init);
    await fireEvent.click(
      screen.getByRole('button', { name: 'Remove user 1' })
    );
    expect(value().userGrants).toEqual([
      { access: ['write'], uuid: ['user-admin'] }
    ]);
    expect(value().groupGrants).toEqual(base().groupGrants);
    await fireEvent.click(
      screen.getByRole('button', { name: 'Remove group 1' })
    );
    expect(value().groupGrants).toEqual([]);
    expect(screen.getByText('No groups')).toBeInTheDocument();
  });

  it.each(['userGrants', 'groupGrants'] as const)(
    'does not submit duplicate %s',
    async (key) => {
      const init = base();
      init[key].push({ access: ['write'], uuid: init[key][0].uuid });
      await expectRejected(PermissionsFields, init, { options });
    }
  );

  it('does not submit a row without a target', async () => {
    const init = base();
    init.userGrants.push({ access: ['read'], uuid: [] });
    await expectRejected(PermissionsFields, init, { options });
  });

  it('disables every control when disabled', () => {
    perms(base(), true);
    for (const name of [
      'Add user',
      'Add group',
      'Remove user 1',
      'Remove group 1'
    ]) {
      expect(screen.getByRole('button', { name })).toBeDisabled();
    }
  });
});
