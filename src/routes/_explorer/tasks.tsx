import { createFileRoute } from '@tanstack/react-router'

export const Route = createFileRoute('/_explorer/tasks')({
  head: () => ({ meta: [{ title: 'Tasks | codereviewbench' }] }),
})
