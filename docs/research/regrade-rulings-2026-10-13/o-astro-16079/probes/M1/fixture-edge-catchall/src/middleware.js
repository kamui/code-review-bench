// A pass-through middleware. It records the address it was called with, so the probe can show
// that the middleware itself still sees the query string.
export const onRequest = async (context, next) => {
	const response = await next();
	response.headers.set('x-probe-middleware-saw', context.url.pathname + context.url.search);
	return response;
};
