import { createRouter } from '@tanstack/react-router'
import { routeTree } from './routeTree.gen'

export function getRouter() {
  const router = createRouter({ routeTree, scrollRestoration: true })
  // On Back or Forward the router restores the saved window position instead of scrolling to the
  // section, and leaving through the header, which is not sticky, saves the top of the page.
  router.subscribe('onRendered', ({ pathChanged, toLocation }) => {
    if (pathChanged && toLocation.hash) document.getElementById(toLocation.hash)?.scrollIntoView()
  })
  return router
}

declare module '@tanstack/react-router' {
  interface Register {
    router: ReturnType<typeof getRouter>
  }
}
