import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/svelte';
import { toast as sonner } from 'svelte-sonner';
import { invalidate } from '$app/navigation';
import { toast } from '@profidev/pleiades/components/util/general';
import { transferServer } from '$lib/client';
import TransferDialog from '$routes/servers/[uuid]/TransferDialog.svelte';
import { options, serverInfo } from './fixtures';

vi.mock('@profidev/pleiades/components/util/general', async (orig) => ({
  ...(await orig<object>()),
  toast: { error: vi.fn(), success: vi.fn() }
}));

vi.mock('$app/forms', async (orig) => ({
  ...(await orig<object>()),
  applyAction: vi.fn(async () => {})
}));

vi.mock('$lib/client', async (orig) => ({
  ...(await orig<object>()),
  transferServer: vi.fn()
}));

afterEach(cleanup);

beforeEach(() => {
  vi.spyOn(sonner, 'error').mockImplementation(() => 0);
});

const tick = async (ms = 0) => new Promise((r) => setTimeout(r, ms));

const result = (status: number, body: unknown) =>
  ({
    data: status === 200 ? body : undefined,
    error: status === 200 ? undefined : body,
    request: new Request('http://x'),
    response: new Response(null, { status })
  }) as never;

const nodes = [...options.nodes, { name: 'ap-1', uuid: 'node-3' }];

const open = () =>
  render(TransferDialog, {
    props: { nodes, open: true, server: serverInfo }
  });

const pickNode = async (name: string) => {
  await fireEvent.click(screen.getByRole('combobox'));
  await fireEvent.click(await screen.findByRole('option', { name }));
  await tick();
};

const fill = async (ip = '10.0.1.5', port = '25570') => {
  await pickNode('us-1');
  await fireEvent.input(screen.getByLabelText('IP address'), {
    target: { value: ip }
  });
  await fireEvent.input(screen.getByLabelText('Port'), {
    target: { value: port }
  });
};

const submit = async () => {
  await fireEvent.click(screen.getByRole('button', { name: 'Start transfer' }));
  await tick(50);
};

describe('TransferDialog', () => {
  it('shows the title, fields and hint', () => {
    open();
    expect(screen.getByText('Transfer survival')).toBeInTheDocument();
    expect(
      screen.getByText('Choose where to move this server.')
    ).toBeInTheDocument();
    expect(screen.getByText('Target node')).toBeInTheDocument();
    expect(
      screen.getByText('Primary allocation on target')
    ).toBeInTheDocument();
    expect(screen.getByLabelText('Port')).toHaveValue(25_565);
  });

  it('leaves the current node out of the targets', async () => {
    open();
    await fireEvent.click(screen.getByRole('combobox'));
    const names = (await screen.findAllByRole('option')).map((o) =>
      o.textContent?.trim()
    );
    expect(names).toEqual(['us-1', 'ap-1']);
  });

  it('does not submit without a node or with a bad IP / port', async () => {
    open();
    await submit();
    await fill('nope', '80');
    await submit();
    await tick(150);
    expect(transferServer).not.toHaveBeenCalled();
  });

  it('transfers, toasts, invalidates and closes', async () => {
    vi.mocked(transferServer).mockResolvedValue(result(200, undefined));
    open();
    await fill(' 10.0.1.5 ');
    await submit();
    await vi.waitFor(() =>
      expect(toast.success).toHaveBeenCalledWith(
        'Server survival transferred to us-1'
      )
    );
    expect(transferServer).toHaveBeenCalledWith({
      body: { ip: '10.0.1.5', node: 'node-2', port: 25_570 },
      path: { uuid: 'srv-1' }
    });
    expect(vi.mocked(invalidate).mock.calls).toEqual([['/api/servers/srv-1']]);
    await vi.waitFor(() =>
      expect(screen.queryByText('Transfer survival')).not.toBeInTheDocument()
    );
  });

  it('shows 400 field errors on node, ip and port', async () => {
    vi.mocked(transferServer).mockResolvedValue(
      result(400, {
        errors: [
          { field: 'node', message: 'Node not found.' },
          { field: 'ip', message: 'Bad IP.' },
          { field: 'port', message: 'Bad port.' }
        ]
      })
    );
    open();
    await fill();
    await submit();
    await vi.waitFor(() =>
      expect(sonner.error).toHaveBeenCalledWith(
        "Couldn't transfer server. Fix the highlighted fields."
      )
    );
    expect(await screen.findByText('Node not found.')).toBeInTheDocument();
    expect(screen.getByText('Bad IP.')).toBeInTheDocument();
    expect(screen.getByText('Bad port.')).toBeInTheDocument();
    expect(screen.getByText('Transfer survival')).toBeInTheDocument();
    expect(toast.success).not.toHaveBeenCalled();
  });

  it('shows a 409 port conflict', async () => {
    vi.mocked(transferServer).mockResolvedValue(
      result(409, { errors: [{ field: 'port', message: 'Port in use.' }] })
    );
    open();
    await fill();
    await submit();
    await vi.waitFor(() =>
      expect(sonner.error).toHaveBeenCalledWith(
        "Couldn't transfer server: the allocation is already in use."
      )
    );
    expect(await screen.findByText('Port in use.')).toBeInTheDocument();
  });

  it('toasts field errors it cannot place', async () => {
    vi.mocked(transferServer).mockResolvedValue(
      result(400, { errors: [{ field: 'mystery', message: 'Odd.' }] })
    );
    open();
    await fill();
    await submit();
    await vi.waitFor(() => expect(sonner.error).toHaveBeenCalledWith('Odd.'));
  });

  it.each([409, 403, 500])('toasts a generic error on %i', async (status) => {
    vi.mocked(transferServer).mockResolvedValue(result(status, 'nope'));
    open();
    await fill();
    await submit();
    await vi.waitFor(() =>
      expect(sonner.error).toHaveBeenCalledWith("Couldn't transfer server.")
    );
    expect(invalidate).not.toHaveBeenCalled();
    expect(screen.getByRole('button', { name: 'Retry' })).toBeInTheDocument();
  });
});
