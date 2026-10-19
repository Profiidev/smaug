import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/svelte';
import Harness from '$test_helpers/EggFieldsHarness.svelte';
import VariablesFields from '$routes/eggs/VariablesFields.svelte';
import RuntimeFields from '$routes/eggs/RuntimeFields.svelte';
import ConfigFields from '$routes/eggs/ConfigFields.svelte';
import GeneralFields from '$routes/eggs/GeneralFields.svelte';
import InstallScriptFields from '$routes/eggs/InstallScriptFields.svelte';
import General from '$routes/eggs/General.svelte';
import Variables from '$routes/eggs/Variables.svelte';
import InstallScript from '$routes/eggs/InstallScript.svelte';
import Runtime from '$routes/eggs/Runtime.svelte';
import Config from '$routes/eggs/Config.svelte';
import {
  type EggForm,
  KNOWN_FEATURES,
  STAGE_KEYS,
  formatData
} from '$routes/eggs/schema.svelte';
import { eggInfo } from '$mocks/e2e/data';

const base = (): EggForm => formatData(structuredClone(eggInfo));

const mount = (fields: unknown, initialValue = base(), disabled = false) => {
  const r = render(Harness, {
    props: { disabled, fields: fields as never, initialValue }
  });
  return { ...r, value: (): EggForm => r.component.getValue()! };
};

// SvelteKit's real applyAction needs a mounted app root; superforms calls it on invalid submits.
vi.mock('$app/forms', async (orig) => ({
  ...(await orig<object>()),
  applyAction: vi.fn(async () => {})
}));

afterEach(cleanup);

const tick = async () => new Promise((r) => setTimeout(r, 0));

describe('VariablesFields (nested fieldProxy binding)', () => {
  it('renders each variable from the form value', () => {
    mount(VariablesFields);
    expect(screen.getByText('Startup variables (1)')).toBeInTheDocument();
    expect(screen.getByLabelText('Env Variable')).toHaveValue('SERVER_JARFILE');
    expect(screen.getByLabelText('Description')).toHaveValue('Jar to run');
  });

  it('adds a variable with default rules and binds its nested inputs', async () => {
    const { value } = mount(VariablesFields);
    await fireEvent.click(screen.getByRole('button', { name: 'Add variable' }));
    expect(screen.getByText('Startup variables (2)')).toBeInTheDocument();

    await fireEvent.input(screen.getAllByLabelText('Name')[1], {
      target: { value: 'Second' }
    });
    await fireEvent.input(screen.getAllByLabelText('Env Variable')[1], {
      target: { value: 'SECOND' }
    });
    await fireEvent.input(screen.getAllByLabelText('Default Value')[1], {
      target: { value: 'two' }
    });
    await fireEvent.input(screen.getAllByLabelText('Description')[1], {
      target: { value: 'desc 2' }
    });
    await fireEvent.click(
      screen.getAllByRole('switch', { name: 'Visible to users' })[1]
    );
    await tick();

    expect(value().variables[1]).toEqual({
      defaultValue: 'two',
      description: 'desc 2',
      envVariable: 'SECOND',
      name: 'Second',
      rules: ['required', 'string'],
      userEditable: false,
      userViewable: true
    });
    // The first variable is untouched.
    expect(value().variables[0]).toEqual(base().variables[0]);
  });

  it('removes the chosen variable', async () => {
    const init = base();
    init.variables.push({
      ...init.variables[0],
      envVariable: 'OTHER',
      name: 'Other'
    });
    const { value } = mount(VariablesFields, init);
    await fireEvent.click(
      screen.getAllByRole('button', { name: 'Remove variable' })[0]
    );
    expect(value().variables.map((v) => v.envVariable)).toEqual(['OTHER']);
  });

  it('disables every control when disabled', () => {
    mount(VariablesFields, base(), true);
    expect(screen.getByRole('button', { name: 'Add variable' })).toBeDisabled();
    expect(
      screen.getByRole('button', { name: 'Remove variable' })
    ).toBeDisabled();
    expect(screen.getByLabelText('Name')).toBeDisabled();
  });
});

describe('RuntimeFields', () => {
  it('binds docker image rows and adds / removes them', async () => {
    const { value } = mount(RuntimeFields);
    await fireEvent.click(screen.getByRole('button', { name: 'Add image' }));
    const labels = screen.getAllByLabelText('Label');
    // [image 0, image 1, startup command 0]
    await fireEvent.input(labels[1], { target: { value: 'Java 17' } });
    await fireEvent.input(screen.getAllByLabelText('Image')[1], {
      target: { value: 'img:17' }
    });
    expect(value().dockerImages).toEqual([
      { image: 'ghcr.io/pelican-eggs/yolks:java_21', label: 'Java 21' },
      { image: 'img:17', label: 'Java 17' }
    ]);
    await fireEvent.click(
      screen.getAllByRole('button', { name: 'Remove image' })[0]
    );
    expect(value().dockerImages).toEqual([
      { image: 'img:17', label: 'Java 17' }
    ]);
  });

  it('binds startup commands and the stop command', async () => {
    const { value } = mount(RuntimeFields);
    await fireEvent.click(
      screen.getByRole('button', { name: 'Add startup command' })
    );
    await fireEvent.input(screen.getAllByLabelText('Command')[1], {
      target: { value: './run' }
    });
    await fireEvent.input(screen.getByLabelText('Stop Command'), {
      target: { value: '^C' }
    });
    expect(value().startupCommands[1]).toEqual({ command: './run', label: '' });
    expect(value().configStop).toBe('^C');
  });
});

describe('ConfigFields', () => {
  it('binds nested find entries', async () => {
    const { value } = mount(ConfigFields);
    expect(screen.getByLabelText('File Path')).toHaveValue('server.properties');
    await fireEvent.click(
      screen.getByRole('button', { name: 'Add find entry' })
    );
    await fireEvent.input(screen.getAllByLabelText('Key')[1], {
      target: { value: 'motd' }
    });
    await fireEvent.input(screen.getAllByLabelText('Value')[1], {
      target: { value: 'hi' }
    });
    expect(value().configFiles[0].find[1]).toEqual({
      key: 'motd',
      map: [],
      mode: ['string'],
      value: 'hi'
    });
  });

  it('adds and removes config files', async () => {
    const { value } = mount(ConfigFields);
    await fireEvent.click(
      screen.getByRole('button', { name: 'Add config file' })
    );
    await fireEvent.input(screen.getAllByLabelText('File Path')[1], {
      target: { value: 'b.yml' }
    });
    expect(value().configFiles.map((f) => f.path)).toEqual([
      'server.properties',
      'b.yml'
    ]);
    await fireEvent.click(
      screen.getAllByRole('button', { name: 'Remove config file' })[0]
    );
    expect(value().configFiles.map((f) => f.path)).toEqual(['b.yml']);
  });

  it('edits map-mode replacements', async () => {
    const init = base();
    init.configFiles[0].find = [
      {
        key: 'ip',
        map: [{ match: 'a', replace: 'b' }],
        mode: ['map'],
        value: ''
      }
    ];
    const { value } = mount(ConfigFields, init);
    expect(screen.queryByLabelText('Value')).not.toBeInTheDocument();
    await fireEvent.click(screen.getByRole('button', { name: 'Add pair' }));
    await fireEvent.input(screen.getAllByLabelText('Match')[1], {
      target: { value: 'x' }
    });
    await fireEvent.input(screen.getAllByLabelText('Replace with')[1], {
      target: { value: 'y' }
    });
    expect(value().configFiles[0].find[0].map).toEqual([
      { match: 'a', replace: 'b' },
      { match: 'x', replace: 'y' }
    ]);
    await fireEvent.click(
      screen.getAllByRole('button', { name: 'Remove pair' })[0]
    );
    expect(value().configFiles[0].find[0].map).toEqual([
      { match: 'x', replace: 'y' }
    ]);
  });

  it('binds the top-level switch and input', async () => {
    const { value } = mount(ConfigFields);
    await fireEvent.click(screen.getByRole('switch', { name: 'Custom logs' }));
    await fireEvent.input(screen.getByLabelText('Log File Location'), {
      target: { value: 'l.log' }
    });
    await tick();
    expect(value().configLogsCustom).toBe(true);
    expect(value().configLogsLocation).toBe('l.log');
  });
});

describe('FindEntry type controls', () => {
  const withEntry = (
    mode: 'string' | 'number' | 'boolean' | 'map',
    value = ''
  ) => {
    const init = base();
    init.configFiles[0].find = [{ key: 'k', map: [], mode: [mode], value }];
    return mount(ConfigFields, init);
  };
  const toggle = (name: string) => screen.getByRole('radio', { name });

  it('marks the current type in the toggle group', () => {
    withEntry('number', '1');
    expect(screen.getByRole('radio', { name: 'Number' })).toHaveAttribute(
      'aria-checked',
      'true'
    );
    expect(screen.getByRole('radio', { name: 'String' })).toHaveAttribute(
      'aria-checked',
      'false'
    );
  });

  it('string shows a text input', () => {
    withEntry('string', 'hi');
    expect(screen.getByLabelText('Value')).not.toHaveAttribute(
      'type',
      'number'
    );
    expect(screen.getByLabelText('Value')).toHaveValue('hi');
  });

  it('number shows a number input that writes a string value', async () => {
    const { value } = withEntry('string');
    await fireEvent.click(toggle('Number'));
    const input = screen.getByLabelText('Value');
    expect(input).toHaveAttribute('type', 'number');
    await fireEvent.input(input, { target: { value: '25565' } });
    expect(value().configFiles[0].find[0]).toMatchObject({
      mode: ['number'],
      value: '25565'
    });
  });

  it('boolean shows a true/false select and defaults to false', async () => {
    const { value } = withEntry('string', 'x');
    await fireEvent.click(toggle('Boolean'));
    expect(value().configFiles[0].find[0]).toMatchObject({
      mode: ['boolean'],
      value: 'false'
    });
    expect(screen.getByLabelText('Value')).toHaveTextContent('false');
  });

  it('boolean keeps an existing true value', async () => {
    const { value } = withEntry('string', 'true');
    await fireEvent.click(toggle('Boolean'));
    expect(value().configFiles[0].find[0].value).toBe('true');
  });

  it('map shows match/replace pairs instead of a value', async () => {
    const { value } = withEntry('string');
    await fireEvent.click(toggle('Map'));
    expect(screen.queryByLabelText('Value')).not.toBeInTheDocument();
    await fireEvent.click(screen.getByRole('button', { name: 'Add pair' }));
    expect(value().configFiles[0].find[0]).toMatchObject({
      map: [{ match: '', replace: '' }],
      mode: ['map']
    });
  });

  it('clicking the active type keeps it', async () => {
    const { value } = withEntry('number', '1');
    await fireEvent.click(toggle('Number'));
    expect(value().configFiles[0].find[0].mode).toEqual(['number']);
  });

  it('removes the entry', async () => {
    const { value } = withEntry('string');
    await fireEvent.click(
      screen.getByRole('button', { name: 'Remove find entry' })
    );
    expect(value().configFiles[0].find).toEqual([]);
  });

  it('disables the type toggle when disabled', () => {
    const init = base();
    mount(ConfigFields, init, true);
    for (const radio of screen.getAllByRole('radio')) {
      expect(radio).toBeDisabled();
    }
  });
});

describe('ConfigFields collapsible card header', () => {
  it('shows the path, parser badge and entry count', async () => {
    const init = base();
    const n = init.configFiles[0].find.length;
    mount(ConfigFields, init);
    const header = screen.getByRole('button', {
      name: 'Toggle config file'
    }).parentElement!;
    expect(header).toHaveTextContent('server.properties');
    expect(header).toHaveTextContent('properties');
    expect(header).toHaveTextContent(
      `${n} find ${n === 1 ? 'entry' : 'entries'}`
    );
    await fireEvent.click(
      screen.getByRole('button', { name: 'Add find entry' })
    );
    expect(header).toHaveTextContent(`${n + 1} find entries`);
  });

  it('labels a new file and uses the singular for one entry', async () => {
    const init = base();
    init.configFiles = [];
    mount(ConfigFields, init);
    await fireEvent.click(
      screen.getByRole('button', { name: 'Add config file' })
    );
    const header = screen.getByRole('button', {
      name: 'Toggle config file'
    }).parentElement!;
    expect(header).toHaveTextContent('New config file');
    expect(header).toHaveTextContent('file');
    expect(header).toHaveTextContent('0 find entries');
    await fireEvent.click(
      screen.getByRole('button', { name: 'Add find entry' })
    );
    expect(header).toHaveTextContent('1 find entry');
  });

  it('collapses and expands the card body', async () => {
    mount(ConfigFields);
    const trigger = screen.getByRole('button', { name: 'Toggle config file' });
    expect(trigger).toHaveAttribute('aria-expanded', 'true');
    expect(screen.getByLabelText('File Path')).toBeInTheDocument();
    await fireEvent.click(trigger);
    expect(trigger).toHaveAttribute('aria-expanded', 'false');
    await vi.waitFor(() =>
      expect(screen.queryByLabelText('File Path')).not.toBeVisible()
    );
    await fireEvent.click(trigger);
    expect(screen.getByLabelText('File Path')).toBeVisible();
  });
});

describe('VariablesFields cards', () => {
  it('shows the #n badge and the variable name as the card title', async () => {
    mount(VariablesFields);
    expect(screen.getByText('#1')).toBeInTheDocument();
    expect(
      screen.getAllByText(base().variables[0].name).length
    ).toBeGreaterThan(0);
    await fireEvent.click(screen.getByRole('button', { name: 'Add variable' }));
    expect(screen.getByText('#2')).toBeInTheDocument();
    expect(screen.getByText('New variable')).toBeInTheDocument();
  });

  it('shows the empty state without variables', async () => {
    mount(VariablesFields, { ...base(), variables: [] });
    expect(screen.getByText('No variables yet')).toBeInTheDocument();
    expect(screen.getByText('Startup variables (0)')).toBeInTheDocument();
    await fireEvent.click(screen.getByRole('button', { name: 'Add variable' }));
    expect(screen.queryByText('No variables yet')).not.toBeInTheDocument();
  });
});

describe('item switches', () => {
  it.each(KNOWN_FEATURES.map((f) => [f.label, f.formKey] as const))(
    'feature %s binds its flag',
    async (label, formKey) => {
      const { value } = mount(GeneralFields, { ...base(), [formKey]: false });
      await fireEvent.click(screen.getByRole('switch', { name: label }));
      await tick();
      expect(value()[formKey]).toBe(true);
    }
  );

  it('shows each feature key in the item title', () => {
    mount(GeneralFields);
    for (const f of KNOWN_FEATURES) {
      expect(screen.getByText(f.key)).toBeInTheDocument();
    }
  });

  it('strip ANSI binds configStripAnsi', async () => {
    const { value } = mount(RuntimeFields, {
      ...base(),
      configStripAnsi: false
    });
    await fireEvent.click(
      screen.getByRole('switch', { name: 'Strip ANSI codes' })
    );
    await tick();
    expect(value().configStripAnsi).toBe(true);
  });

  it('disables the item switches when disabled', () => {
    mount(GeneralFields, base(), true);
    for (const f of KNOWN_FEATURES) {
      expect(screen.getByRole('switch', { name: f.label })).toBeDisabled();
    }
  });
});

describe('GeneralFields / InstallScriptFields', () => {
  it('bind top-level inputs and textarea', async () => {
    const g = mount(GeneralFields);
    await fireEvent.input(screen.getByLabelText('Egg Name'), {
      target: { value: 'Purpur' }
    });
    await fireEvent.input(screen.getByLabelText('Description'), {
      target: { value: 'd' }
    });
    expect(g.value().name).toBe('Purpur');
    expect(g.value().description).toBe('d');
    g.unmount();

    const i = mount(InstallScriptFields);
    await fireEvent.input(screen.getByLabelText('Install Script'), {
      target: { value: 'echo hi' }
    });
    await fireEvent.input(screen.getByLabelText('Entrypoint Shell'), {
      target: { value: 'ash' }
    });
    expect(i.value().scriptsScript).toBe('echo hi');
    expect(i.value().scriptsEntrypoint).toBe('ash');
  });

  it('shows and clears the icon', async () => {
    const { value } = mount(GeneralFields, {
      ...base(),
      icon: 'data:image/png;base64,AAA'
    });
    expect(screen.getByAltText('Egg icon')).toHaveAttribute(
      'src',
      'data:image/png;base64,AAA'
    );
    await fireEvent.click(screen.getByRole('button', { name: 'Remove icon' }));
    expect(value().icon).toBe('');
    expect(screen.queryByAltText('Egg icon')).not.toBeInTheDocument();
  });
});

describe('FormTags / FormSelect (nested)', () => {
  it('adds a validation rule to a nested variable', async () => {
    const { value } = mount(VariablesFields);
    const input = screen.getByPlaceholderText(/required, string, max:20/);
    await fireEvent.input(input, { target: { value: 'max:20' } });
    await fireEvent.keyDown(input, { key: 'Enter' });
    await tick();
    expect(value().variables[0].rules).toEqual([
      'required',
      'string',
      'max:20'
    ]);
  });

  it('renders the selected nested parser', () => {
    mount(ConfigFields);
    expect(screen.getAllByText('properties').length).toBeGreaterThan(0);
  });
});

describe('FormBase', () => {
  const submitForm = async () =>
    fireEvent.submit(
      screen.getByRole('button', { name: 'submit' }).closest('form')!
    );

  it('submits valid data', async () => {
    const onsubmit = vi.fn();
    const init = base();
    render(Harness, {
      props: {
        fields: InstallScriptFields as never,
        initialValue: init,
        onsubmit
      }
    });
    await submitForm();
    await vi.waitFor(() => expect(onsubmit).toHaveBeenCalledWith(init));
  });

  it('does not submit invalid data', async () => {
    const onsubmit = vi.fn();
    render(Harness, {
      props: {
        fields: InstallScriptFields as never,
        initialValue: { ...base(), scriptsContainer: '' },
        onsubmit
      }
    });
    await submitForm();
    // Error rendering goes through SvelteKit's page store (not available here); e2e covers it.
    await new Promise((r) => setTimeout(r, 200));
    expect(onsubmit).not.toHaveBeenCalled();
  });
});

describe('wizard stage components', () => {
  it.each([
    ['General', General, 0],
    ['Variables', Variables, 1],
    ['InstallScript', InstallScript, 2],
    ['Runtime', Runtime, 3],
    ['Config', Config, 4]
  ] as const)('%s exposes its stage values via getValue', (_, Stage, i) => {
    const init = base();
    const stageValue = Object.fromEntries(
      STAGE_KEYS[i].map((k) => [k, init[k as keyof EggForm]])
    );
    const { component } = render(Stage as never, {
      props: {
        initialValue: stageValue,
        isLoading: false,
        onsubmit: vi.fn()
      } as never
    });
    expect((component as { getValue: () => unknown }).getValue()).toEqual(
      stageValue
    );
  });
});

describe('remove buttons', () => {
  const withMapEntry = (): EggForm => {
    const v = base();
    v.configFiles[0].find.push({
      key: 'motd',
      map: [{ match: 'a', replace: 'b' }],
      mode: ['map'],
      value: ''
    });
    v.icon = 'data:image/png;base64,AAAA';
    return v;
  };

  it.each([
    ['GeneralFields', GeneralFields],
    ['VariablesFields', VariablesFields],
    ['RuntimeFields', RuntimeFields],
    ['ConfigFields', ConfigFields]
  ])('%s uses the destructive variant for every remove button', (_, fields) => {
    mount(fields, withMapEntry());
    const remove = screen.getAllByRole('button', {
      name: /^(?:Remove|Delete)/
    });
    expect(remove.length).toBeGreaterThan(0);
    for (const button of remove) {
      expect(button.className).toContain('text-destructive');
    }
  });
});
