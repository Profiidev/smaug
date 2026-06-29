<script lang="ts">
  import { Label } from '@profidev/pleiades/components/ui/label';
  import { CopyButton } from '@profidev/pleiades/components/ui-extra/copy-button';
  import * as Select from '@profidev/pleiades/components/ui/select';
  import * as Code from '$lib/components/code';
  import { dockerCompose, dockerRun } from './code.svelte';
  import type { NodeInfo } from '$lib/client';

  let { data } = $props();

  let node: NodeInfo | undefined = $state();
  let setupMethod = $state('Docker Compose');

  $effect(() => {
    data.nodeRes.then((res) => {
      if (!res.data) return;
      node = res.data;
    });
  });
</script>

<h4 class="mb-2">Node Setup</h4>
<div class="flex flex-col gap-2 w-full">
  <Label class="mr-4 text-nowrap">Node Auth Token:</Label>
  <CopyButton
    text={node?.token ?? 'loading...'}
    variant="outline"
    class="max-w-141"
  >
    <span class="truncate">{node?.token ?? 'loading...'}</span>
  </CopyButton>
  <Label class="mr-4 text-nowrap">Setup Method:</Label>
  <Select.Root bind:value={setupMethod} type="single" allowDeselect={false}>
    <Select.Trigger class="w-48">
      {setupMethod}
    </Select.Trigger>
    <Select.Content>
      <Select.Group>
        <Select.Item value="Docker Compose">Docker Compose</Select.Item>
        <Select.Item value="Docker Run">Docker Run</Select.Item>
      </Select.Group>
    </Select.Content>
  </Select.Root>
  <Code.Root
    code={setupMethod === 'Docker Compose'
      ? dockerCompose(node?.token ?? 'loading...')
      : dockerRun(node?.token ?? 'loading...')}
    lang={setupMethod === 'Docker Compose' ? 'yaml' : 'bash'}
    class="mt-4 min-w-0 grow"
  >
    <Code.CopyButton />
  </Code.Root>
</div>
