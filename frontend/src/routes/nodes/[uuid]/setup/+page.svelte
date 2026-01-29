<script lang="ts">
  import { Label } from 'positron-components/components/ui/label';
  import { CopyButton } from 'positron-components/components/ui-extra/copy-button';
  import * as Select from 'positron-components/components/ui/select';
  import * as Code from '$lib/components/code';
  import { dockerCompose, dockerRun } from './code.svelte';

  let { data } = $props();

  let setupMethod = $state('Docker Compose');
</script>

<h4 class="mb-2">Node Setup</h4>
<div class="flex flex-col gap-2">
  <Label class="mr-4 text-nowrap">Node Auth Token:</Label>
  <CopyButton text={data.node.token} variant="outline" class="max-w-155">
    <span class="truncate">{data.node.token}</span>
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
      ? dockerCompose(data.node.token)
      : dockerRun(data.node.token)}
    lang={setupMethod === 'Docker Compose' ? 'yaml' : 'bash'}
    class="mt-4"
  >
    <Code.CopyButton />
  </Code.Root>
</div>
