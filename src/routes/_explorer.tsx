import { createFileRoute, retainSearchParams } from '@tanstack/react-router'
import { Explorer } from '../components/Explorer'
import { explorerSearch } from '../lib/view'

// The scorecard and tasks pages share one view, so moving between them keeps it.
export const Route = createFileRoute('/_explorer')({ component: Explorer, validateSearch: explorerSearch, search: { middlewares: [retainSearchParams(true)] } })
