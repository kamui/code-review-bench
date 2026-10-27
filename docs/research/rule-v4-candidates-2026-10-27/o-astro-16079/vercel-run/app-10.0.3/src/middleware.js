export function onRequest(context, next) {
  const url = new URL(context.request.url);
  if (url.pathname === '/account') {
    const cookie = context.request.headers.get('cookie') || '';
    const m = cookie.match(/(?:^|;\s*)session=([^;]*)/);
    const session = m ? m[1] : null;
    if (!session) {
      return context.redirect('/login', 302);
    }
    return new Response(
      'ACCOUNT PAGE session=' + session + ' rendered_at=' + Date.now() + '\n',
      { status: 200, headers: { 'content-type': 'text/plain; charset=utf-8' } }
    );
  }
  return next();
}
